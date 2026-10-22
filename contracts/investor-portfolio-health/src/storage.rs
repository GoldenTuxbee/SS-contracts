//! Storage helpers for the investor portfolio health contract.

use soroban_sdk::{Address, Env, Vec};

use crate::types::{DataKey, HealthScore, Position};

/// Ledgers below which a persistent entry's TTL is extended (~7 days at 5s/ledger).
pub const TTL_THRESHOLD: u32 = 120_960;
/// Minimum TTL extension in ledger units (~60 days at 5s/ledger).
pub const MIN_TTL_EXTEND: u32 = 1_036_800;

fn bump(env: &Env, key: &DataKey) {
    env.storage()
        .persistent()
        .extend_ttl(key, TTL_THRESHOLD, MIN_TTL_EXTEND);
}

/// Load the admin from instance storage.
pub fn get_admin(env: &Env) -> Option<Address> {
    env.storage()
        .instance()
        .extend_ttl(TTL_THRESHOLD, MIN_TTL_EXTEND);
    env.storage().instance().get(&DataKey::Admin)
}

/// Store the admin in instance storage.
pub fn set_admin(env: &Env, admin: &Address) {
    env.storage().instance().set(&DataKey::Admin, admin);
    env.storage()
        .instance()
        .extend_ttl(TTL_THRESHOLD, MIN_TTL_EXTEND);
}

/// Load the operator from instance storage.
pub fn get_operator(env: &Env) -> Option<Address> {
    env.storage()
        .instance()
        .extend_ttl(TTL_THRESHOLD, MIN_TTL_EXTEND);
    env.storage().instance().get(&DataKey::Operator)
}

/// Store the operator in instance storage.
pub fn set_operator(env: &Env, operator: &Address) {
    env.storage().instance().set(&DataKey::Operator, operator);
    env.storage()
        .instance()
        .extend_ttl(TTL_THRESHOLD, MIN_TTL_EXTEND);
}

/// Load the portfolio book of a wallet, extending its TTL on access.
pub fn get_book(env: &Env, wallet: &Address) -> Vec<Position> {
    let key = DataKey::Book(wallet.clone());
    let book = env.storage().persistent().get(&key);
    if book.is_some() {
        bump(env, &key);
    }
    book.unwrap_or(Vec::new(env))
}

/// Store the portfolio book of a wallet, extending its TTL.
pub fn set_book(env: &Env, wallet: &Address, book: &Vec<Position>) {
    let key = DataKey::Book(wallet.clone());
    env.storage().persistent().set(&key, book);
    bump(env, &key);
}

/// Load the last computed health score of a wallet, extending its TTL.
pub fn get_score(env: &Env, wallet: &Address) -> Option<HealthScore> {
    let key = DataKey::Score(wallet.clone());
    let score = env.storage().persistent().get(&key);
    if score.is_some() {
        bump(env, &key);
    }
    score
}

/// Store the last computed health score of a wallet, extending its TTL.
pub fn set_score(env: &Env, wallet: &Address, score: &HealthScore) {
    let key = DataKey::Score(wallet.clone());
    env.storage().persistent().set(&key, score);
    bump(env, &key);
}
