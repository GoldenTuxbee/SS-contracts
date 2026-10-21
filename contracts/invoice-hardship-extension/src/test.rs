//! Tests for the invoice hardship extension contract (issue #638).

use super::*;
use soroban_sdk::testutils::{Address as _, Events as _, Ledger as _, MockAuth, MockAuthInvoke};
use soroban_sdk::xdr::{ContractEvent, ContractEventBody, ScVal};
use soroban_sdk::{Address, Env, IntoVal, String, Symbol, TryFromVal, TryIntoVal, Val, Vec};

const START: u64 = 1_000;
const ONE_DAY: u64 = 86_400;
const GRACE_PERIOD_DAYS: u32 = 5;
const DUE_IN_DAYS: u64 = 10;

/// Deployed contract, its admin and the tracked invoice.
struct Ctx<'a> {
    env: Env,
    contract_id: Address,
    admin: Address,
    invoice_id: Symbol,
    client: InvoiceHardshipExtensionClient<'a>,
}

impl<'a> Ctx<'a> {
    /// Register the tracked invoice with a due date.
    fn register(&self, due_date: u64) {
        self.client.register_invoice(&self.invoice_id, &due_date);
    }

    /// Grant a hardship extension to the tracked invoice.
    fn grant(&self, days: u32, text: &str) -> u32 {
        let reason = String::from_str(&self.env(), text);
        self.client.grant_hardship_extension(&self.invoice_id, &days, &reason)
    }

    /// The environment the contract runs in.
    fn env(&self) -> &Env {
        &self.env
    }
}

fn setup<'a>(env: &'a Env) -> Ctx<'a> {
    env.mock_all_auths();
    env.ledger().set_timestamp(START);

    let admin = Address::generate(env);
    let contract_id = env.register(InvoiceHardshipExtension, ());
    let client = InvoiceHardshipExtensionClient::new(env, &contract_id);
    client.initialize(&admin, &GRACE_PERIOD_DAYS);

    Ctx {
        env: env.clone(),
        contract_id,
        admin,
        invoice_id: Symbol::new(env, "INVGRACE1"),
        client,
    }
}

/// The reason recorded with a hardship extension.
fn reason(env: &Env) -> String {
    String::from_str(env, "verified hardship")
}

/// Split an event into its topics and its data.
fn parse_event(env: &Env, event: &ContractEvent) -> (Vec<Val>, Val) {
    let ContractEventBody::V0(v0) = &event.body;
    let raw_topics = ScVal::Vec(Some(v0.topics.clone().into()));
    let topics = Vec::<Val>::try_from_val(env, &raw_topics).unwrap();
    let data = Val::try_from_val(env, &v0.data).unwrap();
    (topics, data)
}

/// The data of the last event with the given name.
fn last_event_data(env: &Env, name: &str) -> Option<Val> {
    let mut result = None;
    for event in env.events().all().events().iter() {
        let (topics, data) = parse_event(env, event);
        let topic = Symbol::try_from_val(env, &topics.get(0).unwrap()).unwrap();
        if topic == Symbol::new(env, name) {
            result = Some(data);
        }
    }
    result
}

/// The number of events with the given name.
fn event_count(env: &Env, name: &str) -> u32 {
    let mut count = 0;
    for event in env.events().all().events().iter() {
        let (topics, _data) = parse_event(env, event);
        let topic = Symbol::try_from_val(env, &topics.get(0).unwrap()).unwrap();
        if topic == Symbol::new(env, name) {
            count += 1;
        }
    }
    count
}

// ── Initialisation ─────────────────────────────────────────────────────────────

#[test]
fn double_initialisation_is_rejected() {
    let env = Env::default();
    let ctx = setup(&env);
    let result = ctx.client.try_initialize(&ctx.admin, &GRACE_PERIOD_DAYS);
    assert_eq!(result, Err(Ok(Error::AlreadyInitialized)));
}

#[test]
fn registering_an_invoice_records_the_standard_grace_end() {
    let env = Env::default();
    let ctx = setup(&env);
    let due_date = START + DUE_IN_DAYS * ONE_DAY;
    ctx.register(due_date);

    let status = ctx.client.get_grace_status(&ctx.invoice_id);
    assert_eq!(status.due_date, due_date);
    assert_eq!(
        status.standard_grace_end,
        due_date + (GRACE_PERIOD_DAYS as u64) * ONE_DAY
    );
    assert_eq!(status.extension_end, 0);
    assert_eq!(status.extension_days, 0);
    assert!(!status.penalty_paused);
}

#[test]
fn registering_an_invoice_is_restricted_to_the_admin() {
    let env = Env::default();
    let ctx = setup(&env);

    let impostor = Address::generate(&env);
    let invoke = MockAuthInvoke {
        contract: &ctx.contract_id,
        fn_name: "register_invoice",
        args: (ctx.invoice_id.clone(), START + DUE_IN_DAYS * ONE_DAY).into_val(&env),
        sub_invokes: &[],
    };
    let auth = MockAuth {
        address: &impostor,
        invoke: &invoke,
    };
    env.mock_auths(&[auth]);

    let due_date = START + DUE_IN_DAYS * ONE_DAY;
    let result = ctx.client.try_register_invoice(&ctx.invoice_id, &due_date);
    assert!(result.is_err());
}

// ── Extensions ─────────────────────────────────────────────────────────────────

#[test]
fn granting_an_extension_extends_the_grace_end() {
    let env = Env::default();
    let ctx = setup(&env);
    ctx.register(START + DUE_IN_DAYS * ONE_DAY);

    let total = ctx.grant(10, "verified hardship");
    assert_eq!(total, 10);

    let status = ctx.client.get_grace_status(&ctx.invoice_id);
    let standard_grace_end = status.standard_grace_end;
    assert_eq!(status.extension_days, 10);
    assert_eq!(status.extension_end, standard_grace_end + 10 * ONE_DAY);
    assert!(status.penalty_paused);

    let extension = ctx.client.get_extension(&ctx.invoice_id);
    assert_eq!(extension.additional_days, 10);
    assert_eq!(extension.extension_end, standard_grace_end + 10 * ONE_DAY);
    assert_eq!(extension.granted_at, START);
}

#[test]
fn granting_an_extension_is_restricted_to_the_admin() {
    let env = Env::default();
    let ctx = setup(&env);
    ctx.register(START + DUE_IN_DAYS * ONE_DAY);

    let impostor = Address::generate(&env);
    let reason = String::from_str(&env, "verified hardship");
    let invoke = MockAuthInvoke {
        contract: &ctx.contract_id,
        fn_name: "grant_hardship_extension",
        args: (ctx.invoice_id.clone(), 10u32, reason).into_val(&env),
        sub_invokes: &[],
    };
    let auth = MockAuth {
        address: &impostor,
        invoke: &invoke,
    };
    env.mock_auths(&[auth]);

    let result = ctx
        .client
        .try_grant_hardship_extension(&ctx.invoice_id, &10, &reason);
    assert!(result.is_err());
}

#[test]
fn an_extension_beyond_thirty_days_is_rejected() {
    let env = Env::default();
    let ctx = setup(&env);
    ctx.register(START + DUE_IN_DAYS * ONE_DAY);

    let text = reason(&env);
    let result = ctx
        .client
        .try_grant_hardship_extension(&ctx.invoice_id, &31, &text);
    assert_eq!(result, Err(Ok(Error::MaxExtensionExceeded)));
}

#[test]
fn the_granted_days_count_towards_the_maximum() {
    let env = Env::default();
    let ctx = setup(&env);
    ctx.register(START + DUE_IN_DAYS * ONE_DAY);

    assert_eq!(ctx.grant(20, "first hardship"), 20);

    // Ten more days would exceed the maximum of thirty days.
    let text = reason(&env);
    let result = ctx
        .client
        .try_grant_hardship_extension(&ctx.invoice_id, &10, &text);
    assert_eq!(result, Err(Ok(Error::MaxExtensionExceeded)));

    // The extension that was granted before is untouched.
    let status = ctx.client.get_grace_status(&ctx.invoice_id);
    assert_eq!(status.extension_days, 20);
}

#[test]
fn an_extension_without_days_is_rejected() {
    let env = Env::default();
    let ctx = setup(&env);
    ctx.register(START + DUE_IN_DAYS * ONE_DAY);

    let text = reason(&env);
    let result = ctx
        .client
        .try_grant_hardship_extension(&ctx.invoice_id, &0, &text);
    assert_eq!(result, Err(Ok(Error::InvalidDays)));
}

#[test]
fn an_extension_for_an_unknown_invoice_is_rejected() {
    let env = Env::default();
    let ctx = setup(&env);

    let text = reason(&env);
    let result = ctx
        .client
        .try_grant_hardship_extension(&ctx.invoice_id, &10, &text);
    assert_eq!(result, Err(Ok(Error::InvoiceNotFound)));
}

// ── Penalty accrual ────────────────────────────────────────────────────────────

#[test]
fn the_penalty_is_paused_during_the_extension_only() {
    let env = Env::default();
    let ctx = setup(&env);
    ctx.register(START + DUE_IN_DAYS * ONE_DAY);
    ctx.grant(10, "verified hardship");

    let extension_end = ctx.client.get_grace_status(&ctx.invoice_id).extension_end;

    // The penalty does not accrue while the extension is active.
    env.ledger().set_timestamp(START);
    assert!(ctx.client.is_penalty_paused(&ctx.invoice_id));

    env.ledger().set_timestamp(extension_end - 1);
    assert!(ctx.client.is_penalty_paused(&ctx.invoice_id));

    // Once the extension ended the penalty accrues again.
    env.ledger().set_timestamp(extension_end + 1);
    assert!(!ctx.client.is_penalty_paused(&ctx.invoice_id));
}

#[test]
fn the_penalty_is_not_paused_without_an_extension() {
    let env = Env::default();
    let ctx = setup(&env);
    ctx.register(START + DUE_IN_DAYS * ONE_DAY);

    assert!(!ctx.client.is_penalty_paused(&ctx.invoice_id));

    let status = ctx.client.get_grace_status(&ctx.invoice_id);
    assert!(!status.penalty_paused);
}

// ── Events ─────────────────────────────────────────────────────────────────────

#[test]
fn the_event_carries_the_invoice_days_and_reason() {
    let env = Env::default();
    let ctx = setup(&env);
    ctx.register(START + DUE_IN_DAYS * ONE_DAY);
    ctx.grant(10, "verified hardship");

    assert_eq!(event_count(&env, "HardshipExtensionGranted"), 1);

    let data = last_event_data(&env, "HardshipExtensionGranted").unwrap();
    let (days, reason): (u32, String) = data.try_into_val(&env).unwrap();
    assert_eq!(days, 10);
    assert_eq!(reason, String::from_str(&env, "verified hardship"));
}
