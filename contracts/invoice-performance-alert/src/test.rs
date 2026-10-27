//! Tests for the invoice performance alert contract (issue #625).

use super::*;
use soroban_sdk::testutils::{Address as _, Events as _, Ledger as _, MockAuth, MockAuthInvoke};
use soroban_sdk::xdr::{ContractEvent, ContractEventBody, ScVal};
use soroban_sdk::{Address, Env, IntoVal, Symbol, TryFromVal, Val, Vec};

const START: u64 = 1_000;
const ONE_DAY: u64 = 86_400;

/// Deployed contract, its actors and the tracked invoice.
struct Ctx<'a> {
    contract_id: Address,
    admin: Address,
    operator: Address,
    client: InvoicePerformanceAlertClient<'a>,
    invoice_id: Symbol,
}

impl<'a> Ctx<'a> {
    /// Record the performance metrics of the tracked invoice.
    fn set_metrics(
        &self,
        target: i128,
        funded: i128,
        maturity: u64,
        due: u64,
        status: PaymentStatus,
    ) {
        let metrics = InvoiceMetrics {
            invoice_id: self.invoice_id.clone(),
            target_amount: target,
            funded_amount: funded,
            maturity_timestamp: maturity,
            due_date: due,
            payment_status: status,
        };
        self.client.set_invoice_metrics(&self.admin, &metrics);
    }
}

fn setup<'a>(env: &'a Env) -> Ctx<'a> {
    env.mock_all_auths();
    env.ledger().set_timestamp(START);

    let admin = Address::generate(env);
    let operator = Address::generate(env);
    let contract_id = env.register(InvoicePerformanceAlert, ());
    let client = InvoicePerformanceAlertClient::new(env, &contract_id);
    client.initialize(&admin, &operator);

    Ctx {
        contract_id,
        admin,
        operator,
        client,
        invoice_id: Symbol::new(env, "INVPA001"),
    }
}

/// Split an event into its topics and its data.
fn parse_event(env: &Env, event: &ContractEvent) -> (Vec<Val>, Val) {
    let ContractEventBody::V0(v0) = &event.body;
    let raw_topics = ScVal::Vec(Some(v0.topics.clone().into()));
    let topics = Vec::<Val>::try_from_val(env, &raw_topics).unwrap();
    let data = Val::try_from_val(env, &v0.data).unwrap();
    (topics, data)
}

/// The `InvoiceAtRisk` events emitted by the last invocation.
fn risk_alerts(env: &Env) -> Vec<RiskAlert> {
    let mut alerts = Vec::new(env);
    for event in env.events().all().events().iter() {
        let (topics, data) = parse_event(env, event);
        let topic = Symbol::try_from_val(env, &topics.get(0).unwrap()).unwrap();
        if topic == Symbol::new(env, "InvoiceAtRisk") {
            alerts.push_back(RiskAlert::try_from_val(env, &data).unwrap());
        }
    }
    alerts
}

// ── Initialisation ─────────────────────────────────────────────────────────────

#[test]
fn double_initialisation_is_rejected() {
    let env = Env::default();
    let ctx = setup(&env);
    let result = ctx.client.try_initialize(&ctx.admin, &ctx.operator);
    assert_eq!(result, Err(Ok(Error::AlreadyInitialized)));
}

#[test]
fn only_the_admin_can_rotate_the_operator() {
    let env = Env::default();
    let ctx = setup(&env);
    let impostor = Address::generate(&env);
    let new_operator = Address::generate(&env);

    let result = ctx.client.try_set_operator(&impostor, &new_operator);
    assert_eq!(result, Err(Ok(Error::Unauthorized)));

    ctx.client.set_operator(&ctx.admin, &new_operator);
}

// ── Metrics ────────────────────────────────────────────────────────────────────

#[test]
fn metrics_can_only_be_recorded_by_the_admin_or_the_operator() {
    let env = Env::default();
    let ctx = setup(&env);
    let impostor = Address::generate(&env);
    let metrics = InvoiceMetrics {
        invoice_id: ctx.invoice_id.clone(),
        target_amount: 1_000,
        funded_amount: 700,
        maturity_timestamp: START + 5 * ONE_DAY,
        due_date: START + 30 * ONE_DAY,
        payment_status: PaymentStatus::Pending,
    };

    let result = ctx.client.try_set_invoice_metrics(&impostor, &metrics);
    assert_eq!(result, Err(Ok(Error::Unauthorized)));
}

#[test]
fn the_operator_can_record_and_update_the_metrics() {
    let env = Env::default();
    let ctx = setup(&env);
    ctx.set_metrics(
        1_000,
        700,
        START + 5 * ONE_DAY,
        START + 30 * ONE_DAY,
        PaymentStatus::Pending,
    );

    let metrics = ctx.client.get_invoice_metrics(&ctx.invoice_id);
    assert_eq!(metrics.funded_amount, 700);
    assert_eq!(metrics.target_amount, 1_000);
    assert_eq!(metrics.payment_status, PaymentStatus::Pending);

    let updated = InvoiceMetrics {
        invoice_id: ctx.invoice_id.clone(),
        target_amount: 1_000,
        funded_amount: 900,
        maturity_timestamp: START + 5 * ONE_DAY,
        due_date: START + 30 * ONE_DAY,
        payment_status: PaymentStatus::Pending,
    };
    ctx.client.set_invoice_metrics(&ctx.operator, &updated);

    let metrics = ctx.client.get_invoice_metrics(&ctx.invoice_id);
    assert_eq!(metrics.funded_amount, 900);
}

#[test]
fn invalid_metrics_are_rejected() {
    let env = Env::default();
    let ctx = setup(&env);

    let no_target = InvoiceMetrics {
        invoice_id: ctx.invoice_id.clone(),
        target_amount: 0,
        funded_amount: 0,
        maturity_timestamp: START + 5 * ONE_DAY,
        due_date: START + 30 * ONE_DAY,
        payment_status: PaymentStatus::Pending,
    };
    let result = ctx.client.try_set_invoice_metrics(&ctx.admin, &no_target);
    assert_eq!(result, Err(Ok(Error::InvalidAmount)));

    let no_maturity = InvoiceMetrics {
        invoice_id: ctx.invoice_id.clone(),
        target_amount: 1_000,
        funded_amount: 0,
        maturity_timestamp: 0,
        due_date: START + 30 * ONE_DAY,
        payment_status: PaymentStatus::Pending,
    };
    let result = ctx.client.try_set_invoice_metrics(&ctx.admin, &no_maturity);
    assert_eq!(result, Err(Ok(Error::InvalidTimestamp)));
}

// ── Underfunded risk ───────────────────────────────────────────────────────────

#[test]
fn underfunded_alert_is_emitted_with_low_severity() {
    let env = Env::default();
    let ctx = setup(&env);
    // 70% funded and closing in 5 days.
    ctx.set_metrics(
        1_000,
        700,
        START + 5 * ONE_DAY,
        START + 30 * ONE_DAY,
        PaymentStatus::Pending,
    );

    let emitted = ctx.client.check_invoice_health(&ctx.invoice_id);
    assert_eq!(emitted, 1);

    let alerts = risk_alerts(&env);
    assert_eq!(alerts.len(), 1);
    let alert = alerts.get(0).unwrap();
    assert_eq!(alert.invoice_id, ctx.invoice_id);
    assert_eq!(alert.risk_type, RiskType::Underfunded);
    assert_eq!(alert.severity, Severity::Low);
    assert_eq!(alert.timestamp, START);
}

#[test]
fn underfunded_alert_is_medium_severity() {
    let env = Env::default();
    let ctx = setup(&env);
    // 50% funded.
    ctx.set_metrics(
        1_000,
        500,
        START + 5 * ONE_DAY,
        START + 30 * ONE_DAY,
        PaymentStatus::Pending,
    );

    assert_eq!(ctx.client.check_invoice_health(&ctx.invoice_id), 1);
    let alerts = risk_alerts(&env);
    let alert = alerts.get(0).unwrap();
    assert_eq!(alert.severity, Severity::Medium);
}

#[test]
fn underfunded_alert_is_high_severity() {
    let env = Env::default();
    let ctx = setup(&env);
    // 20% funded.
    ctx.set_metrics(
        1_000,
        200,
        START + 5 * ONE_DAY,
        START + 30 * ONE_DAY,
        PaymentStatus::Pending,
    );

    assert_eq!(ctx.client.check_invoice_health(&ctx.invoice_id), 1);
    let alerts = risk_alerts(&env);
    let alert = alerts.get(0).unwrap();
    assert_eq!(alert.severity, Severity::High);
}

#[test]
fn funding_at_the_threshold_is_not_a_risk() {
    let env = Env::default();
    let ctx = setup(&env);
    // Exactly 80% funded.
    ctx.set_metrics(
        1_000,
        800,
        START + 5 * ONE_DAY,
        START + 30 * ONE_DAY,
        PaymentStatus::Pending,
    );

    assert_eq!(ctx.client.check_invoice_health(&ctx.invoice_id), 0);
    assert_eq!(risk_alerts(&env).len(), 0);
}

#[test]
fn a_fully_funded_invoice_is_not_underfunded() {
    let env = Env::default();
    let ctx = setup(&env);
    ctx.set_metrics(
        1_000,
        1_000,
        START + 5 * ONE_DAY,
        START + 30 * ONE_DAY,
        PaymentStatus::Pending,
    );

    assert_eq!(ctx.client.check_invoice_health(&ctx.invoice_id), 0);
    assert_eq!(risk_alerts(&env).len(), 0);
}

#[test]
fn the_underfunded_risk_is_only_raised_close_to_the_maturity() {
    let env = Env::default();
    let ctx = setup(&env);
    // Closing in 8 days, outside of the 7 day window.
    ctx.set_metrics(
        1_000,
        700,
        START + 8 * ONE_DAY,
        START + 30 * ONE_DAY,
        PaymentStatus::Pending,
    );

    assert_eq!(ctx.client.check_invoice_health(&ctx.invoice_id), 0);
    assert_eq!(risk_alerts(&env).len(), 0);
}

#[test]
fn the_underfunded_risk_is_raised_exactly_seven_days_before_the_maturity() {
    let env = Env::default();
    let ctx = setup(&env);
    ctx.set_metrics(
        1_000,
        700,
        START + 7 * ONE_DAY,
        START + 30 * ONE_DAY,
        PaymentStatus::Pending,
    );

    assert_eq!(ctx.client.check_invoice_health(&ctx.invoice_id), 1);
    let alerts = risk_alerts(&env);
    let alert = alerts.get(0).unwrap();
    assert_eq!(alert.risk_type, RiskType::Underfunded);
}

// ── Overdue risk ───────────────────────────────────────────────────────────────

#[test]
fn overdue_alert_is_emitted_with_high_severity() {
    let env = Env::default();
    let ctx = setup(&env);
    // Fully funded but the payment deadline passed yesterday.
    ctx.set_metrics(
        1_000,
        1_000,
        START + 30 * ONE_DAY,
        START - ONE_DAY,
        PaymentStatus::Pending,
    );

    assert_eq!(ctx.client.check_invoice_health(&ctx.invoice_id), 1);

    let alerts = risk_alerts(&env);
    let alert = alerts.get(0).unwrap();
    assert_eq!(alert.invoice_id, ctx.invoice_id);
    assert_eq!(alert.risk_type, RiskType::Overdue);
    assert_eq!(alert.severity, Severity::High);
    assert_eq!(alert.timestamp, START);
}

#[test]
fn an_invoice_that_is_not_due_yet_is_not_overdue() {
    let env = Env::default();
    let ctx = setup(&env);
    ctx.set_metrics(
        1_000,
        1_000,
        START + 30 * ONE_DAY,
        START + 30 * ONE_DAY,
        PaymentStatus::Pending,
    );

    assert_eq!(ctx.client.check_invoice_health(&ctx.invoice_id), 0);
    assert_eq!(risk_alerts(&env).len(), 0);

    let status = ctx.client.get_risk_status(&ctx.invoice_id);
    assert!(!status.overdue);
}

// ── Default proximity risk ─────────────────────────────────────────────────────

#[test]
fn default_proximity_alert_severity_depends_on_the_time_left() {
    let env = Env::default();
    let ctx = setup(&env);
    // 5 days left: the default is approaching.
    ctx.set_metrics(
        1_000,
        1_000,
        START + 30 * ONE_DAY,
        START + 5 * ONE_DAY,
        PaymentStatus::Pending,
    );

    assert_eq!(ctx.client.check_invoice_health(&ctx.invoice_id), 1);
    let alerts = risk_alerts(&env);
    let alert = alerts.get(0).unwrap();
    assert_eq!(alert.risk_type, RiskType::DefaultProximity);
    assert_eq!(alert.severity, Severity::Low);
}

#[test]
fn default_proximity_alert_is_medium_severity_within_three_days() {
    let env = Env::default();
    let ctx = setup(&env);
    ctx.set_metrics(
        1_000,
        1_000,
        START + 30 * ONE_DAY,
        START + 2 * ONE_DAY,
        PaymentStatus::Pending,
    );

    assert_eq!(ctx.client.check_invoice_health(&ctx.invoice_id), 1);
    let alerts = risk_alerts(&env);
    let alert = alerts.get(0).unwrap();
    assert_eq!(alert.severity, Severity::Medium);
}

#[test]
fn default_proximity_alert_is_high_severity_within_a_day() {
    let env = Env::default();
    let ctx = setup(&env);
    ctx.set_metrics(
        1_000,
        1_000,
        START + 30 * ONE_DAY,
        START + 12 * 60 * 60,
        PaymentStatus::Pending,
    );

    assert_eq!(ctx.client.check_invoice_health(&ctx.invoice_id), 1);
    let alerts = risk_alerts(&env);
    let alert = alerts.get(0).unwrap();
    assert_eq!(alert.severity, Severity::High);
}

#[test]
fn a_paid_invoice_is_not_at_risk() {
    let env = Env::default();
    let ctx = setup(&env);
    ctx.set_metrics(
        1_000,
        700,
        START + 5 * ONE_DAY,
        START - ONE_DAY,
        PaymentStatus::Paid,
    );

    assert_eq!(ctx.client.check_invoice_health(&ctx.invoice_id), 0);
    assert_eq!(risk_alerts(&env).len(), 0);

    let status = ctx.client.get_risk_status(&ctx.invoice_id);
    assert!(!status.underfunded);
    assert!(!status.overdue);
    assert!(!status.default_proximity);
}

// ── Alert suppression ──────────────────────────────────────────────────────────

#[test]
fn duplicate_alerts_are_suppressed_within_24_hours() {
    let env = Env::default();
    let ctx = setup(&env);
    ctx.set_metrics(
        1_000,
        700,
        START + 5 * ONE_DAY,
        START + 30 * ONE_DAY,
        PaymentStatus::Pending,
    );

    assert_eq!(ctx.client.check_invoice_health(&ctx.invoice_id), 1);
    assert_eq!(risk_alerts(&env).len(), 1);

    // A second check on the same day is suppressed.
    env.ledger().set_timestamp(START + 23 * 60 * 60);
    assert_eq!(ctx.client.check_invoice_health(&ctx.invoice_id), 0);
    assert_eq!(risk_alerts(&env).len(), 0);

    // Once the suppression window elapsed the alert is emitted again.
    env.ledger().set_timestamp(START + 24 * 60 * 60);
    assert_eq!(ctx.client.check_invoice_health(&ctx.invoice_id), 1);
    assert_eq!(risk_alerts(&env).len(), 1);
}

#[test]
fn the_suppression_is_tracked_per_risk_type() {
    let env = Env::default();
    let ctx = setup(&env);
    // Underfunded now, the default threshold comes into reach later today.
    ctx.set_metrics(
        1_000,
        700,
        START + 5 * ONE_DAY,
        START + 7 * ONE_DAY + 12 * 60 * 60,
        PaymentStatus::Pending,
    );

    assert_eq!(ctx.client.check_invoice_health(&ctx.invoice_id), 1);
    let alerts = risk_alerts(&env);
    let alert = alerts.get(0).unwrap();
    assert_eq!(alert.risk_type, RiskType::Underfunded);

    // The underfunded alert is still suppressed but the new risk type is not.
    env.ledger().set_timestamp(START + 12 * 60 * 60);
    assert_eq!(ctx.client.check_invoice_health(&ctx.invoice_id), 1);
    let alerts = risk_alerts(&env);
    let alert = alerts.get(0).unwrap();
    assert_eq!(alert.risk_type, RiskType::DefaultProximity);
}

#[test]
fn the_suppression_is_tracked_per_invoice() {
    let env = Env::default();
    let ctx = setup(&env);
    ctx.set_metrics(
        1_000,
        700,
        START + 5 * ONE_DAY,
        START + 30 * ONE_DAY,
        PaymentStatus::Pending,
    );

    let other_invoice = Symbol::new(&env, "INVPA002");
    let other = InvoiceMetrics {
        invoice_id: other_invoice.clone(),
        target_amount: 1_000,
        funded_amount: 700,
        maturity_timestamp: START + 5 * ONE_DAY,
        due_date: START + 30 * ONE_DAY,
        payment_status: PaymentStatus::Pending,
    };
    ctx.client.set_invoice_metrics(&ctx.admin, &other);

    assert_eq!(ctx.client.check_invoice_health(&ctx.invoice_id), 1);
    assert_eq!(ctx.client.check_invoice_health(&other_invoice), 1);
}

// ── Risk status ────────────────────────────────────────────────────────────────

#[test]
fn risk_status_reports_the_current_flags_and_the_latest_alert() {
    let env = Env::default();
    let ctx = setup(&env);
    ctx.set_metrics(
        1_000,
        700,
        START + 5 * ONE_DAY,
        START + 30 * ONE_DAY,
        PaymentStatus::Pending,
    );

    // The flags are accurate before the first check, but no alert exists yet.
    let status = ctx.client.get_risk_status(&ctx.invoice_id);
    assert!(status.underfunded);
    assert!(!status.overdue);
    assert!(!status.default_proximity);
    assert_eq!(status.funding_bps, 7_000);
    assert_eq!(status.days_to_close, 5);
    assert_eq!(status.days_to_due, 30);
    assert_eq!(status.last_alert_at, 0);
    assert_eq!(status.last_alert_type, None);
    assert_eq!(status.last_severity, None);

    assert_eq!(ctx.client.check_invoice_health(&ctx.invoice_id), 1);

    let status = ctx.client.get_risk_status(&ctx.invoice_id);
    assert!(status.underfunded);
    assert_eq!(status.last_alert_at, START);
    assert_eq!(status.last_alert_type, Some(RiskType::Underfunded));
    assert_eq!(status.last_severity, Some(Severity::Low));

    let risk = RiskType::Underfunded;
    let record = ctx.client.get_last_alert(&ctx.invoice_id, &risk);
    let timestamp = record.map(|alert| alert.timestamp);
    let severity = record.map(|alert| alert.severity);
    assert_eq!(timestamp, Some(START));
    assert_eq!(severity, Some(Severity::Low));
}

#[test]
fn risk_status_of_an_unknown_invoice_is_rejected() {
    let env = Env::default();
    let ctx = setup(&env);
    let unknown = Symbol::new(&env, "GHOST");
    let result = ctx.client.try_get_risk_status(&unknown);
    assert_eq!(result, Err(Ok(Error::InvoiceNotFound)));
}

#[test]
fn health_check_of_an_unknown_invoice_is_rejected() {
    let env = Env::default();
    let ctx = setup(&env);
    let unknown = Symbol::new(&env, "GHOST");
    let result = ctx.client.try_check_invoice_health(&unknown);
    assert_eq!(result, Err(Ok(Error::InvoiceNotFound)));
}

// ── Authorisation ──────────────────────────────────────────────────────────────

#[test]
fn the_health_check_requires_the_authorised_operator() {
    let env = Env::default();
    let ctx = setup(&env);
    ctx.set_metrics(
        1_000,
        700,
        START + 5 * ONE_DAY,
        START + 30 * ONE_DAY,
        PaymentStatus::Pending,
    );

    ctx.client.check_invoice_health(&ctx.invoice_id);

    // The authorisation of the operator is the one that was required.
    let operator = ctx.operator.clone();
    let auths = env.auths();
    assert!(auths.iter().any(|(address, _)| *address == operator));
}

#[test]
fn the_health_check_rejects_callers_other_than_the_operator() {
    let env = Env::default();
    let ctx = setup(&env);
    ctx.set_metrics(
        1_000,
        700,
        START + 5 * ONE_DAY,
        START + 30 * ONE_DAY,
        PaymentStatus::Pending,
    );

    let intruder = Address::generate(&env);
    let invoke = MockAuthInvoke {
        contract: &ctx.contract_id,
        fn_name: "check_invoice_health",
        args: (ctx.invoice_id.clone(),).into_val(&env),
        sub_invokes: &[],
    };
    let auth = MockAuth {
        address: &intruder,
        invoke: &invoke,
    };
    env.mock_auths(&[auth]);

    let result = ctx.client.try_check_invoice_health(&ctx.invoice_id);
    assert!(result.is_err());
}

#[test]
fn rotating_the_operator_moves_the_authorisation() {
    let env = Env::default();
    let ctx = setup(&env);
    let new_operator = Address::generate(&env);
    ctx.client.set_operator(&ctx.admin, &new_operator);
    ctx.set_metrics(
        1_000,
        700,
        START + 5 * ONE_DAY,
        START + 30 * ONE_DAY,
        PaymentStatus::Pending,
    );

    let invoke = MockAuthInvoke {
        contract: &ctx.contract_id,
        fn_name: "check_invoice_health",
        args: (ctx.invoice_id.clone(),).into_val(&env),
        sub_invokes: &[],
    };
    let auth = MockAuth {
        address: &new_operator,
        invoke: &invoke,
    };
    env.mock_auths(&[auth]);

    ctx.client.check_invoice_health(&ctx.invoice_id);

    let auths = env.auths();
    assert!(auths.iter().any(|(address, _)| *address == new_operator));
}
