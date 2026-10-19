use soroban_sdk::{Address, BytesN, Env, Symbol};

pub fn amendment_proposed(
    env: &Env,
    amendment_id: u32,
    invoice_id: &BytesN<32>,
    issuer: &Address,
    field: &Symbol,
    new_value: i128,
) {
    let topics = (Symbol::new(env, "AmendmentProposed"), invoice_id.clone());
    env.events()
        .publish(
            topics,
            (
                amendment_id,
                issuer.clone(),
                field.clone(),
                new_value,
                env.ledger().timestamp(),
            ),
        );
}

pub fn amendment_executed(
    env: &Env,
    amendment_id: u32,
    invoice_id: &BytesN<32>,
    field: &Symbol,
    new_value: i128,
) {
    let topics = (Symbol::new(env, "AmendmentExecuted"), invoice_id.clone());
    env.events()
        .publish(
            topics,
            (
                amendment_id,
                field.clone(),
                new_value,
                env.ledger().timestamp(),
            ),
        );
}

pub fn amendment_revoked(
    env: &Env,
    amendment_id: u32,
    invoice_id: &BytesN<32>,
    issuer: &Address,
) {
    let topics = (Symbol::new(env, "AmendmentRevoked"), invoice_id.clone());
    env.events()
        .publish(
            topics,
            (amendment_id, issuer.clone(), env.ledger().timestamp()),
        );
}
