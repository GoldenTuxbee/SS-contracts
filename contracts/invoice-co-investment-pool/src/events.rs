//! Event definitions for the invoice co-investment pool contract.

use soroban_sdk::{Address, Env, Symbol};

/// Emitted when the admin opens a co-investment pool for an invoice.
pub fn pool_created(
    env: &Env,
    pool_id: u32,
    invoice_id: &Symbol,
    target_amount: i128,
    min_contribution: i128,
) {
    let topics = (Symbol::new(env, "PoolCreated"), pool_id);
    env.events()
        .publish(topics, (invoice_id.clone(), target_amount, min_contribution));
}

/// Emitted when an investor contributes to a pool.
pub fn pool_joined(
    env: &Env,
    pool_id: u32,
    investor: &Address,
    amount: i128,
    total_raised: i128,
) {
    let topics = (Symbol::new(env, "PoolJoined"), pool_id);
    env.events()
        .publish(topics, (investor.clone(), amount, total_raised));
}

/// Emitted when the admin executes a pool and funds the invoice.
pub fn pool_executed(env: &Env, pool_id: u32, invoice_id: &Symbol, amount: i128) {
    let topics = (Symbol::new(env, "PoolExecuted"), pool_id);
    env.events().publish(topics, (invoice_id.clone(), amount));
}

/// Emitted when an investor leaves a pool and gets its funds back.
pub fn pool_exited(env: &Env, pool_id: u32, investor: &Address, amount: i128) {
    let topics = (Symbol::new(env, "PoolExited"), pool_id);
    env.events().publish(topics, (investor.clone(), amount));
}
