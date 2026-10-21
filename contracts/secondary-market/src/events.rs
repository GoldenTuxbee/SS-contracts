#![allow(deprecated)]

use soroban_sdk::{Address, BytesN, Env, Symbol};

pub fn listing_created(
    env: &Env,
    listing_id: u32,
    seller: &Address,
    invoice_id: &BytesN<32>,
    quantity: i128,
    ask_price_per_fraction: i128,
    expiry: u64,
) {
    let topics = (
        Symbol::new(env, "ListingCreated"),
        listing_id,
        invoice_id.clone(),
    );
    env.events().publish(
        topics,
        (seller.clone(), quantity, ask_price_per_fraction, expiry),
    );
}

pub fn listing_sold(
    env: &Env,
    listing_id: u32,
    seller: &Address,
    buyer: &Address,
    quantity: i128,
    total_price: i128,
) {
    let topics = (Symbol::new(env, "ListingSold"), listing_id);
    env.events().publish(
        topics,
        (seller.clone(), buyer.clone(), quantity, total_price),
    );
}

pub fn listing_cancelled(env: &Env, listing_id: u32, seller: &Address) {
    let topics = (Symbol::new(env, "ListingCancelled"), listing_id);
    env.events().publish(topics, seller.clone());
}

pub fn listing_expired(env: &Env, listing_id: u32, seller: &Address, invoice_id: &BytesN<32>) {
    let topics = (Symbol::new(env, "ListingExpired"), listing_id);
    env.events().publish(
        topics,
        (seller.clone(), invoice_id.clone(), env.ledger().timestamp()),
    );
}
