use soroban_sdk::{Address, BytesN, Env, Symbol};

pub fn listing_created(env: &Env, listing_id: u32, seller: &Address, invoice_id: &BytesN<32>) {
    let topics = (Symbol::new(env, "ListingCreated"), listing_id);
    env.events()
        .publish(topics, (seller.clone(), invoice_id.clone(), env.ledger().timestamp()));
}

pub fn listing_bought(
    env: &Env,
    listing_id: u32,
    buyer: &Address,
    seller: &Address,
    invoice_id: &BytesN<32>,
    amount: i128,
) {
    let topics = (Symbol::new(env, "ListingBought"), listing_id);
    env.events()
        .publish(
            topics,
            (buyer.clone(), seller.clone(), invoice_id.clone(), amount, env.ledger().timestamp()),
        );
}

pub fn listing_expired(env: &Env, listing_id: u32, seller: &Address, invoice_id: &BytesN<32>) {
    let topics = (Symbol::new(env, "ListingExpired"), listing_id);
    env.events()
        .publish(
            topics,
            (seller.clone(), invoice_id.clone(), env.ledger().timestamp()),
        );
}
