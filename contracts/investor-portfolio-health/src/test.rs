//! Tests for the investor portfolio health contract (issue #639).

use super::*;
use soroban_sdk::testutils::{Address as _, Events as _, Ledger as _};
use soroban_sdk::xdr::{ContractEvent, ContractEventBody, ScVal};
use soroban_sdk::{Address, Env, Symbol, TryFromVal, TryIntoVal, Val, Vec};

const START: u64 = 1_000;
const ONE_DAY: u64 = 86_400;

/// Deployed contract, its actors and the tracked wallet.
struct Ctx<'a> {
    admin: Address,
    operator: Address,
    wallet: Address,
    issuer: Address,
    other_issuer: Address,
    client: InvestorPortfolioHealthClient<'a>,
}

impl<'a> Ctx<'a> {
    /// Report an investment of the tracked wallet in an invoice of the issuer
    /// that matures in the given number of days.
    fn invest(&self, invoice: &Symbol, amount: i128, issuer: &Address, days: u64) -> u32 {
        let maturity = START + days * ONE_DAY;
        self.client.record_investment(
            &self.operator,
            &self.wallet,
            invoice,
            &amount,
            issuer,
            &maturity,
        )
    }
}

fn setup<'a>(env: &'a Env) -> Ctx<'a> {
    env.mock_all_auths();
    env.ledger().set_timestamp(START);

    let admin = Address::generate(env);
    let operator = Address::generate(env);
    let contract_id = env.register(InvestorPortfolioHealth, ());
    let client = InvestorPortfolioHealthClient::new(env, &contract_id);
    client.initialize(&admin, &operator);

    Ctx {
        admin,
        operator,
        wallet: Address::generate(env),
        issuer: Address::generate(env),
        other_issuer: Address::generate(env),
        client,
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

/// The score before and after the last recomputation of a wallet.
fn last_score_event(env: &Env) -> Option<(u32, u32, u64)> {
    let mut result = None;
    for event in env.events().all().events().iter() {
        let (topics, data) = parse_event(env, event);
        let topic = Symbol::try_from_val(env, &topics.get(0).unwrap()).unwrap();
        if topic == Symbol::new(env, "HealthScoreUpdated") {
            let update: (u32, u32, u64) = data.try_into_val(env).unwrap();
            result = Some(update);
        }
    }
    result
}

/// The number of score updates emitted by the last invocation.
fn score_event_count(env: &Env) -> u32 {
    let mut count = 0;
    for event in env.events().all().events().iter() {
        let (topics, _data) = parse_event(env, event);
        let topic = Symbol::try_from_val(env, &topics.get(0).unwrap()).unwrap();
        if topic == Symbol::new(env, "HealthScoreUpdated") {
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

// ── Authorisation ──────────────────────────────────────────────────────────────

#[test]
fn portfolio_events_require_an_authorised_reporter() {
    let env = Env::default();
    let ctx = setup(&env);
    let impostor = Address::generate(&env);
    let invoice = Symbol::new(&env, "INV1");

    let result = ctx.client.try_record_investment(
        &impostor,
        &ctx.wallet,
        &invoice,
        &1_000i128,
        &ctx.issuer,
        &(START + 30 * ONE_DAY),
    );
    assert_eq!(result, Err(Ok(Error::Unauthorized)));

    let result = ctx
        .client
        .try_record_settlement(&impostor, &ctx.wallet, &invoice);
    assert_eq!(result, Err(Ok(Error::Unauthorized)));

    let result = ctx
        .client
        .try_record_default(&impostor, &ctx.wallet, &invoice);
    assert_eq!(result, Err(Ok(Error::Unauthorized)));
}

#[test]
fn an_invalid_amount_is_rejected() {
    let env = Env::default();
    let ctx = setup(&env);
    let invoice = Symbol::new(&env, "INV1");

    let result = ctx.client.try_record_investment(
        &ctx.operator,
        &ctx.wallet,
        &invoice,
        &0i128,
        &ctx.issuer,
        &(START + 30 * ONE_DAY),
    );
    assert_eq!(result, Err(Ok(Error::InvalidAmount)));
}

// ── Portfolio book ─────────────────────────────────────────────────────────────

#[test]
fn an_investment_opens_a_position() {
    let env = Env::default();
    let ctx = setup(&env);
    let invoice = Symbol::new(&env, "INV1");

    let score = ctx.invest(&invoice, 1_000, &ctx.issuer, 30);
    assert_eq!(score, 60);

    let positions = ctx.client.get_positions(&ctx.wallet);
    assert_eq!(positions.len(), 1);
    let position = positions.get(0).unwrap();
    assert_eq!(position.invoice_id, invoice);
    assert_eq!(position.amount, 1_000);
    assert_eq!(position.issuer, ctx.issuer);
    assert_eq!(position.maturity_timestamp, START + 30 * ONE_DAY);
    assert_eq!(position.status, PositionStatus::Active);
}

#[test]
fn a_second_investment_tops_up_the_position() {
    let env = Env::default();
    let ctx = setup(&env);
    let invoice = Symbol::new(&env, "INV1");

    ctx.invest(&invoice, 1_000, &ctx.issuer, 30);
    ctx.invest(&invoice, 500, &ctx.issuer, 30);

    let positions = ctx.client.get_positions(&ctx.wallet);
    assert_eq!(positions.len(), 1);
    assert_eq!(positions.get(0).unwrap().amount, 1_500);
}

#[test]
fn events_for_an_unknown_position_are_rejected() {
    let env = Env::default();
    let ctx = setup(&env);
    let unknown = Symbol::new(&env, "GHOST");

    let result = ctx
        .client
        .try_record_settlement(&ctx.operator, &ctx.wallet, &unknown);
    assert_eq!(result, Err(Ok(Error::PositionNotFound)));

    let result = ctx
        .client
        .try_record_default(&ctx.operator, &ctx.wallet, &unknown);
    assert_eq!(result, Err(Ok(Error::PositionNotFound)));
}

#[test]
fn a_settlement_removes_the_position() {
    let env = Env::default();
    let ctx = setup(&env);
    let invoice = Symbol::new(&env, "INV1");
    let other = Symbol::new(&env, "INV2");

    ctx.invest(&invoice, 1_000, &ctx.issuer, 30);
    ctx.invest(&other, 3_000, &ctx.other_issuer, 60);
    assert_eq!(ctx.client.get_positions(&ctx.wallet).len(), 2);

    let score = ctx
        .client
        .record_settlement(&ctx.operator, &ctx.wallet, &invoice);
    assert_eq!(score, 59);

    let positions = ctx.client.get_positions(&ctx.wallet);
    assert_eq!(positions.len(), 1);
    assert_eq!(positions.get(0).unwrap().invoice_id, other);
}

#[test]
fn a_default_keeps_the_position_in_the_portfolio() {
    let env = Env::default();
    let ctx = setup(&env);
    let invoice = Symbol::new(&env, "INV1");

    ctx.invest(&invoice, 1_000, &ctx.issuer, 30);
    ctx.client
        .record_default(&ctx.operator, &ctx.wallet, &invoice);

    let positions = ctx.client.get_positions(&ctx.wallet);
    assert_eq!(positions.len(), 1);
    assert_eq!(positions.get(0).unwrap().status, PositionStatus::Defaulted);
}

// ── Health score ───────────────────────────────────────────────────────────────

#[test]
fn the_health_score_uses_all_four_factors() {
    let env = Env::default();
    let ctx = setup(&env);
    let invoice = Symbol::new(&env, "INV1");
    let other = Symbol::new(&env, "INV2");

    ctx.invest(&invoice, 1_000, &ctx.issuer, 30);
    ctx.invest(&other, 3_000, &ctx.other_issuer, 60);

    let health = ctx.client.compute_health_score(&ctx.wallet);
    let factors = health.factors;
    assert_eq!(factors.total_value, 4_000);
    assert_eq!(factors.position_count, 2);
    assert_eq!(factors.max_concentration_bps, 7_500);
    assert_eq!(factors.issuer_count, 2);
    assert_eq!(factors.avg_days_to_maturity, 52);
    assert_eq!(factors.default_exposure_bps, 0);
    assert_eq!(health.score, 72);
}

#[test]
fn a_default_is_reported_as_default_exposure() {
    let env = Env::default();
    let ctx = setup(&env);
    let invoice = Symbol::new(&env, "INV1");
    let other = Symbol::new(&env, "INV2");

    ctx.invest(&invoice, 1_000, &ctx.issuer, 30);
    ctx.invest(&other, 3_000, &ctx.other_issuer, 60);
    ctx.client
        .record_default(&ctx.operator, &ctx.wallet, &invoice);

    let health = ctx.client.compute_health_score(&ctx.wallet);
    assert_eq!(health.factors.default_exposure_bps, 2_500);
    assert_eq!(health.factors.max_concentration_bps, 7_500);
    assert_eq!(health.score, 60);
}

#[test]
fn the_score_is_bounded_between_zero_and_one_hundred() {
    let env = Env::default();
    let ctx = setup(&env);
    let invoice = Symbol::new(&env, "INV1");

    // A portfolio without positions carries no risk.
    let health = ctx.client.compute_health_score(&ctx.wallet);
    assert_eq!(health.score, 100);
    assert_eq!(health.factors.position_count, 0);

    // The worst portfolio: one position, one issuer, a year of maturity and
    // fully defaulted.
    ctx.invest(&invoice, 1_000, &ctx.issuer, 365);
    ctx.client
        .record_default(&ctx.operator, &ctx.wallet, &invoice);

    let health = ctx.client.compute_health_score(&ctx.wallet);
    assert_eq!(health.factors.max_concentration_bps, 10_000);
    assert_eq!(health.factors.issuer_count, 1);
    assert_eq!(health.factors.avg_days_to_maturity, 365);
    assert_eq!(health.factors.default_exposure_bps, 10_000);
    assert_eq!(health.score, 0);
}

#[test]
fn a_recomputation_keeps_the_score_of_an_unchanged_portfolio() {
    let env = Env::default();
    let ctx = setup(&env);
    let invoice = Symbol::new(&env, "INV1");
    ctx.invest(&invoice, 1_000, &ctx.issuer, 30);

    let health = ctx.client.compute_health_score(&ctx.wallet);
    assert_eq!(health.score, 60);
    assert_eq!(score_event_count(&env), 1);

    let (old, new, _at) = last_score_event(&env).unwrap();
    assert_eq!(old, 60);
    assert_eq!(new, 60);
}

// ── Score history ──────────────────────────────────────────────────────────────

#[test]
fn get_health_score_returns_the_latest_score_and_timestamp() {
    let env = Env::default();
    let ctx = setup(&env);
    let invoice = Symbol::new(&env, "INV1");

    ctx.invest(&invoice, 1_000, &ctx.issuer, 30);

    let stored = ctx.client.get_health_score(&ctx.wallet);
    assert_eq!(stored.score, 60);
    assert_eq!(stored.computed_at, START);
    assert_eq!(stored.factors.position_count, 1);

    // A later event refreshes the stored score and its timestamp.
    env.ledger().set_timestamp(START + 100);
    let other = Symbol::new(&env, "INV2");
    ctx.invest(&other, 500, &ctx.other_issuer, 40);

    let stored = ctx.client.get_health_score(&ctx.wallet);
    assert_eq!(stored.score, 76);
    assert_eq!(stored.computed_at, START + 100);
    assert_eq!(stored.factors.position_count, 2);
}

#[test]
fn a_wallet_without_a_computed_score_reports_its_current_portfolio() {
    let env = Env::default();
    let ctx = setup(&env);
    let invoice = Symbol::new(&env, "INV1");
    ctx.invest(&invoice, 1_000, &ctx.issuer, 30);

    // The score of another wallet was never computed.
    let other_wallet = Address::generate(&env);
    let stored = ctx.client.get_health_score(&other_wallet);
    assert_eq!(stored.score, 100);
    assert_eq!(stored.computed_at, 0);
    assert_eq!(stored.factors.position_count, 0);
}

// ── Events ─────────────────────────────────────────────────────────────────────

#[test]
fn health_score_updated_is_emitted_with_the_old_and_the_new_score() {
    let env = Env::default();
    let ctx = setup(&env);
    let invoice = Symbol::new(&env, "INV1");

    ctx.invest(&invoice, 1_000, &ctx.issuer, 30);
    assert_eq!(score_event_count(&env), 1);
    let (old, new, at) = last_score_event(&env).unwrap();
    assert_eq!(old, 100);
    assert_eq!(new, 60);
    assert_eq!(at, START);

    let other = Symbol::new(&env, "INV2");
    ctx.invest(&other, 500, &ctx.other_issuer, 40);
    assert_eq!(score_event_count(&env), 1);
    let (old, new, _at) = last_score_event(&env).unwrap();
    assert_eq!(old, 60);
    assert_eq!(new, 76);
}

#[test]
fn every_portfolio_event_recomputes_the_score() {
    let env = Env::default();
    let ctx = setup(&env);
    let invoice = Symbol::new(&env, "INV1");
    let other = Symbol::new(&env, "INV2");

    ctx.invest(&invoice, 1_000, &ctx.issuer, 30);
    assert_eq!(score_event_count(&env), 1);

    ctx.client
        .record_default(&ctx.operator, &ctx.wallet, &invoice);
    assert_eq!(score_event_count(&env), 1);

    ctx.client
        .record_settlement(&ctx.operator, &ctx.wallet, &invoice);
    assert_eq!(score_event_count(&env), 1);

    // The remaining position is worth 59 points on its own.
    let stored = ctx.client.get_health_score(&ctx.wallet);
    assert_eq!(stored.score, 59);
    assert_eq!(stored.factors.position_count, 1);
}
