use soroban_sdk::{Address, BytesN, Env, Symbol};

pub fn repayment_received(env: &Env, invoice_id: &BytesN<32>, issuer: &Address, amount: i128) {
    let topics = (Symbol::new(env, "RepaymentReceived"), invoice_id.clone());
    env.events()
        .publish(topics, (issuer.clone(), amount, env.ledger().timestamp()));
}

pub fn settlement_complete(
    env: &Env,
    invoice_id: &BytesN<32>,
    distributed: i128,
    fee: i128,
    total: i128,
) {
    let topics = (Symbol::new(env, "SettlementComplete"), invoice_id.clone());
    env.events()
        .publish(topics, (distributed, fee, total, env.ledger().timestamp()));
}
