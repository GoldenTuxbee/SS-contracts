use soroban_sdk::{Address, BytesN, Env, Symbol};

pub fn collateral_deposited(
    env: &Env,
    invoice_id: &BytesN<32>,
    issuer: &Address,
    amount: i128,
) {
    let topics = (Symbol::new(env, "CollateralDeposited"), invoice_id.clone());
    env.events()
        .publish(topics, (issuer.clone(), amount, env.ledger().timestamp()));
}

pub fn collateral_released(
    env: &Env,
    invoice_id: &BytesN<32>,
    issuer: &Address,
    amount: i128,
) {
    let topics = (Symbol::new(env, "CollateralReleased"), invoice_id.clone());
    env.events()
        .publish(topics, (issuer.clone(), amount, env.ledger().timestamp()));
}

pub fn collateral_forfeited(
    env: &Env,
    invoice_id: &BytesN<32>,
    issuer: &Address,
    amount: i128,
) {
    let topics = (Symbol::new(env, "CollateralForfeited"), invoice_id.clone());
    env.events()
        .publish(topics, (issuer.clone(), amount, env.ledger().timestamp()));
}
