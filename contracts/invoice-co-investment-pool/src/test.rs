//! Tests for the invoice co-investment pool contract (issue #637).

use super::*;
use soroban_sdk::testutils::{Address as _, Events as _, Ledger as _, MockAuth, MockAuthInvoke};
use soroban_sdk::token::{Client as TokenClient, StellarAssetClient};
use soroban_sdk::xdr::{ContractEvent, ContractEventBody, ScVal};
use soroban_sdk::{
    contract, contractimpl, contracttype, Address, Env, IntoVal, Symbol, TryFromVal, TryIntoVal,
    Val, Vec,
};

/// Storage key of the mock escrow.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MockKey {
    Position(Symbol, Address),
    Calls(Symbol),
}

/// Minimal invoice escrow recording the funding positions it receives.
#[contract]
pub struct MockEscrow;

#[contractimpl]
impl MockEscrow {
    /// Record a funding position for the buyer and pull the funds.
    pub fn fund_escrow(
        env: Env,
        invoice_id: Symbol,
        buyer: Address,
        amount: i128,
        funding_token: Address,
    ) {
        buyer.require_auth();
        let contract = env.current_contract_address();
        TokenClient::new(&env, &funding_token).transfer(&buyer, &contract, &amount);

        let key = MockKey::Position(invoice_id.clone(), buyer.clone());
        let previous: i128 = env.storage().persistent().get(&key).unwrap_or(0);
        env.storage().persistent().set(&key, &(previous + amount));

        let calls_key = MockKey::Calls(invoice_id.clone());
        let calls: u32 = env.storage().persistent().get(&calls_key).unwrap_or(0);
        env.storage().persistent().set(&calls_key, &(calls + 1));
    }

    /// The position a buyer holds on the invoice.
    pub fn position_of(env: Env, invoice_id: Symbol, buyer: Address) -> i128 {
        let key = MockKey::Position(invoice_id, buyer);
        env.storage().persistent().get(&key).unwrap_or(0)
    }

    /// The number of funding calls the invoice received.
    pub fn funding_calls(env: Env, invoice_id: Symbol) -> u32 {
        let key = MockKey::Calls(invoice_id);
        env.storage().persistent().get(&key).unwrap_or(0)
    }
}

/// Deployed contract, its actors and the tracked invoice.
struct Ctx<'a> {
    env: Env,
    contract_id: Address,
    investor: Address,
    other_investor: Address,
    token_id: Address,
    escrow_id: Address,
    invoice_id: Symbol,
    client: InvoiceCoInvestmentPoolClient<'a>,
}

impl<'a> Ctx<'a> {
    /// Open a pool for the tracked invoice.
    fn create_pool(&self, target: i128, min: i128) -> u32 {
        self.client.create_pool(&self.invoice_id, &target, &min)
    }

    /// Contribute to a pool.
    fn join(&self, pool_id: u32, investor: &Address, amount: i128) {
        self.client.join_pool(&pool_id, investor, &amount);
    }

    /// The balance of an address in the payment token.
    fn balance(&self, address: &Address) -> i128 {
        StellarAssetClient::new(&self.env, &self.token_id).balance(address)
    }
}

fn setup<'a>(env: &'a Env) -> Ctx<'a> {
    env.mock_all_auths();

    let admin = Address::generate(env);
    let token_admin = Address::generate(env);
    let token_id = env.register_stellar_asset_contract_v2(token_admin).address();
    let escrow_id = env.register(MockEscrow, ());
    let contract_id = env.register(InvoiceCoInvestmentPool, ());
    let client = InvoiceCoInvestmentPoolClient::new(env, &contract_id);
    client.initialize(&admin, &token_id, &escrow_id);

    Ctx {
        env: env.clone(),
        contract_id,
        investor: Address::generate(env),
        other_investor: Address::generate(env),
        token_id,
        escrow_id,
        invoice_id: Symbol::new(env, "INVPOOL1"),
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

// ── Pool creation ──────────────────────────────────────────────────────────────

#[test]
fn create_pool_opens_a_pool() {
    let env = Env::default();
    let ctx = setup(&env);

    let pool_id = ctx.create_pool(1_000, 100);
    assert_eq!(pool_id, 1);

    let pool = ctx.client.get_pool(&pool_id);
    assert_eq!(pool.pool_id, 1);
    assert_eq!(pool.invoice_id, ctx.invoice_id);
    assert_eq!(pool.target_amount, 1_000);
    assert_eq!(pool.min_contribution, 100);
    assert_eq!(pool.total_raised, 0);
    assert_eq!(pool.investor_count, 0);
    assert_eq!(pool.status, PoolStatus::Open);

    assert_eq!(event_count(&env, "PoolCreated"), 1);
    let data = last_event_data(&env, "PoolCreated").unwrap();
    let (invoice_id, target, min): (Symbol, i128, i128) = data.try_into_val(&env).unwrap();
    assert_eq!(invoice_id, ctx.invoice_id);
    assert_eq!(target, 1_000);
    assert_eq!(min, 100);
}

#[test]
fn create_pool_is_restricted_to_the_admin() {
    let env = Env::default();
    let ctx = setup(&env);

    let impostor = Address::generate(&env);
    let invoke = MockAuthInvoke {
        contract: &ctx.contract_id,
        fn_name: "create_pool",
        args: (ctx.invoice_id.clone(), 1_000i128, 100i128).into_val(&env),
        sub_invokes: &[],
    };
    let auth = MockAuth {
        address: &impostor,
        invoke: &invoke,
    };
    env.mock_auths(&[auth]);

    let result = ctx.client.try_create_pool(&ctx.invoice_id, &1_000, &100);
    assert!(result.is_err());
}

#[test]
fn create_pool_rejects_invalid_amounts() {
    let env = Env::default();
    let ctx = setup(&env);

    let result = ctx.client.try_create_pool(&ctx.invoice_id, &0, &100);
    assert_eq!(result, Err(Ok(Error::InvalidAmount)));

    let result = ctx.client.try_create_pool(&ctx.invoice_id, &1_000, &0);
    assert_eq!(result, Err(Ok(Error::InvalidAmount)));

    let result = ctx.client.try_create_pool(&ctx.invoice_id, &100, &1_000);
    assert_eq!(result, Err(Ok(Error::InvalidAmount)));
}

// ── Contributions ──────────────────────────────────────────────────────────────

#[test]
fn join_pool_below_the_minimum_is_rejected() {
    let env = Env::default();
    let ctx = setup(&env);
    let pool_id = ctx.create_pool(1_000, 100);
    StellarAssetClient::new(&env, &ctx.token_id).mint(&ctx.investor, &10_000);

    let result = ctx.client.try_join_pool(&pool_id, &ctx.investor, &99);
    assert_eq!(result, Err(Ok(Error::BelowMinimum)));

    // The rejected contribution did not move any funds.
    assert_eq!(ctx.balance(&ctx.investor), 10_000);
    let pool = ctx.client.get_pool(&pool_id);
    assert_eq!(pool.total_raised, 0);
}

#[test]
fn join_pool_moves_the_funds_into_the_pool() {
    let env = Env::default();
    let ctx = setup(&env);
    let pool_id = ctx.create_pool(1_000, 100);
    StellarAssetClient::new(&env, &ctx.token_id).mint(&ctx.investor, &10_000);

    ctx.join(&pool_id, &ctx.investor, 600);

    assert_eq!(ctx.balance(&ctx.investor), 9_400);
    assert_eq!(ctx.balance(&ctx.contract_id), 600);
    assert_eq!(ctx.client.get_contribution(&pool_id, &ctx.investor), 600);

    let pool = ctx.client.get_pool(&pool_id);
    assert_eq!(pool.total_raised, 600);
    assert_eq!(pool.investor_count, 1);

    assert_eq!(event_count(&env, "PoolJoined"), 1);
    let data = last_event_data(&env, "PoolJoined").unwrap();
    let (investor, amount, total): (Address, i128, i128) = data.try_into_val(&env).unwrap();
    assert_eq!(investor, ctx.investor);
    assert_eq!(amount, 600);
    assert_eq!(total, 600);
}

#[test]
fn join_pool_tops_up_an_existing_contribution() {
    let env = Env::default();
    let ctx = setup(&env);
    let pool_id = ctx.create_pool(1_000, 100);
    StellarAssetClient::new(&env, &ctx.token_id).mint(&ctx.investor, &10_000);

    ctx.join(&pool_id, &ctx.investor, 600);
    ctx.join(&pool_id, &ctx.investor, 400);

    assert_eq!(ctx.client.get_contribution(&pool_id, &ctx.investor), 1_000);
    let pool = ctx.client.get_pool(&pool_id);
    assert_eq!(pool.total_raised, 1_000);
    assert_eq!(pool.investor_count, 1);
}

// ── Execution ──────────────────────────────────────────────────────────────────

#[test]
fn execute_pool_funds_the_invoice_as_one_position() {
    let env = Env::default();
    let ctx = setup(&env);
    let pool_id = ctx.create_pool(1_000, 100);
    StellarAssetClient::new(&env, &ctx.token_id).mint(&ctx.investor, &10_000);
    StellarAssetClient::new(&env, &ctx.token_id).mint(&ctx.other_investor, &10_000);

    ctx.join(&pool_id, &ctx.investor, 600);
    ctx.join(&pool_id, &ctx.other_investor, 400);

    let amount = ctx.client.execute_pool(&pool_id);
    assert_eq!(amount, 1_000);

    // The invoice received a single position held by the pool contract.
    let escrow = MockEscrowClient::new(&env, &ctx.escrow_id);
    assert_eq!(escrow.funding_calls(&ctx.invoice_id), 1);
    assert_eq!(escrow.position_of(&ctx.invoice_id, &ctx.contract_id), 1_000);
    assert_eq!(escrow.position_of(&ctx.invoice_id, &ctx.investor), 0);
    assert_eq!(escrow.position_of(&ctx.invoice_id, &ctx.other_investor), 0);

    // The funds moved from the investors to the invoice.
    assert_eq!(ctx.balance(&ctx.investor), 9_400);
    assert_eq!(ctx.balance(&ctx.other_investor), 9_600);
    assert_eq!(ctx.balance(&ctx.escrow_id), 1_000);
    assert_eq!(ctx.balance(&ctx.contract_id), 0);

    let pool = ctx.client.get_pool(&pool_id);
    assert_eq!(pool.status, PoolStatus::Executed);

    assert_eq!(event_count(&env, "PoolExecuted"), 1);
    let data = last_event_data(&env, "PoolExecuted").unwrap();
    let (invoice_id, funded): (Symbol, i128) = data.try_into_val(&env).unwrap();
    assert_eq!(invoice_id, ctx.invoice_id);
    assert_eq!(funded, 1_000);
}

#[test]
fn execute_pool_is_restricted_to_the_admin() {
    let env = Env::default();
    let ctx = setup(&env);
    let pool_id = ctx.create_pool(1_000, 100);
    StellarAssetClient::new(&env, &ctx.token_id).mint(&ctx.investor, &10_000);
    ctx.join(&pool_id, &ctx.investor, 1_000);

    let impostor = Address::generate(&env);
    let invoke = MockAuthInvoke {
        contract: &ctx.contract_id,
        fn_name: "execute_pool",
        args: (pool_id,).into_val(&env),
        sub_invokes: &[],
    };
    let auth = MockAuth {
        address: &impostor,
        invoke: &invoke,
    };
    env.mock_auths(&[auth]);

    let result = ctx.client.try_execute_pool(&pool_id);
    assert!(result.is_err());
}

#[test]
fn execute_pool_before_the_target_is_rejected() {
    let env = Env::default();
    let ctx = setup(&env);
    let pool_id = ctx.create_pool(1_000, 100);
    StellarAssetClient::new(&env, &ctx.token_id).mint(&ctx.investor, &10_000);
    ctx.join(&pool_id, &ctx.investor, 600);

    let result = ctx.client.try_execute_pool(&pool_id);
    assert_eq!(result, Err(Ok(Error::TargetNotMet)));
}

#[test]
fn an_executed_pool_does_not_accept_contributions_anymore() {
    let env = Env::default();
    let ctx = setup(&env);
    let pool_id = ctx.create_pool(1_000, 100);
    StellarAssetClient::new(&env, &ctx.token_id).mint(&ctx.investor, &10_000);
    ctx.join(&pool_id, &ctx.investor, 1_000);
    ctx.client.execute_pool(&pool_id);

    let result = ctx.client.try_join_pool(&pool_id, &ctx.investor, &1_000);
    assert_eq!(result, Err(Ok(Error::PoolClosed)));
}

// ── Exits ──────────────────────────────────────────────────────────────────────

#[test]
fn exit_pool_returns_the_contribution() {
    let env = Env::default();
    let ctx = setup(&env);
    let pool_id = ctx.create_pool(1_000, 100);
    StellarAssetClient::new(&env, &ctx.token_id).mint(&ctx.investor, &10_000);

    ctx.join(&pool_id, &ctx.investor, 600);
    assert_eq!(ctx.balance(&ctx.investor), 9_400);

    let returned = ctx.client.exit_pool(&pool_id, &ctx.investor);
    assert_eq!(returned, 600);
    assert_eq!(ctx.balance(&ctx.investor), 10_000);
    assert_eq!(ctx.balance(&ctx.contract_id), 0);
    assert_eq!(ctx.client.get_contribution(&pool_id, &ctx.investor), 0);

    let pool = ctx.client.get_pool(&pool_id);
    assert_eq!(pool.total_raised, 0);
    assert_eq!(pool.investor_count, 0);
    assert_eq!(pool.status, PoolStatus::Open);

    assert_eq!(event_count(&env, "PoolExited"), 1);
    let data = last_event_data(&env, "PoolExited").unwrap();
    let (investor, amount): (Address, i128) = data.try_into_val(&env).unwrap();
    assert_eq!(investor, ctx.investor);
    assert_eq!(amount, 600);
}

#[test]
fn exit_pool_after_the_execution_is_rejected() {
    let env = Env::default();
    let ctx = setup(&env);
    let pool_id = ctx.create_pool(1_000, 100);
    StellarAssetClient::new(&env, &ctx.token_id).mint(&ctx.investor, &10_000);
    ctx.join(&pool_id, &ctx.investor, 1_000);
    ctx.client.execute_pool(&pool_id);

    let result = ctx.client.try_exit_pool(&pool_id, &ctx.investor);
    assert_eq!(result, Err(Ok(Error::PoolClosed)));
}

#[test]
fn exit_pool_without_a_contribution_is_rejected() {
    let env = Env::default();
    let ctx = setup(&env);
    let pool_id = ctx.create_pool(1_000, 100);

    let result = ctx.client.try_exit_pool(&pool_id, &ctx.investor);
    assert_eq!(result, Err(Ok(Error::NoContribution)));
}

#[test]
fn operations_on_an_unknown_pool_are_rejected() {
    let env = Env::default();
    let ctx = setup(&env);

    let result = ctx.client.try_join_pool(&7, &ctx.investor, &1_000);
    assert_eq!(result, Err(Ok(Error::PoolNotFound)));

    let result = ctx.client.try_execute_pool(&7);
    assert_eq!(result, Err(Ok(Error::PoolNotFound)));

    let result = ctx.client.try_exit_pool(&7, &ctx.investor);
    assert_eq!(result, Err(Ok(Error::PoolNotFound)));
}
