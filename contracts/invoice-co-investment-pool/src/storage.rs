//! Storage helpers for the invoice co-investment pool contract.

use soroban_sdk::{Address, Env};

use crate::errors::Error;
use crate::types::{Pool, StorageKey};

/// Ledgers below which a persistent entry's TTL is extended (~7 days at 5s/ledger).
pub const TTL_THRESHOLD: u32 = 120_960;
/// Minimum TTL extension in ledger units (~60 days at 5s/ledger).
pub const MIN_TTL_EXTEND: u32 = 1_036_800;

fn bump(env: &Env, key: &StorageKey) {
    env.storage()
        .persistent()
        .extend_ttl(key, TTL_THRESHOLD, MIN_TTL_EXTEND);
}

/// Load the admin from instance storage.
pub fn get_admin(env: &Env) -> Option<Address> {
    env.storage()
        .instance()
        .extend_ttl(TTL_THRESHOLD, MIN_TTL_EXTEND);
    env.storage().instance().get(&StorageKey::Admin)
}

/// Store the admin in instance storage.
pub fn set_admin(env: &Env, admin: &Address) {
    env.storage().instance().set(&StorageKey::Admin, admin);
    env.storage()
        .instance()
        .extend_ttl(TTL_THRESHOLD, MIN_TTL_EXTEND);
}

/// Load the payment token from instance storage.
pub fn get_token(env: &Env) -> Result<Address, Error> {
    env.storage()
        .instance()
        .extend_ttl(TTL_THRESHOLD, MIN_TTL_EXTEND);
    env.storage()
        .instance()
        .get(&StorageKey::Token)
        .ok_or(Error::NotInitialized)
}

/// Store the payment token in instance storage.
pub fn set_token(env: &Env, token: &Address) {
    env.storage().instance().set(&StorageKey::Token, token);
    env.storage()
        .instance()
        .extend_ttl(TTL_THRESHOLD, MIN_TTL_EXTEND);
}

/// Load the invoice escrow from instance storage.
pub fn get_escrow(env: &Env) -> Result<Address, Error> {
    env.storage()
        .instance()
        .extend_ttl(TTL_THRESHOLD, MIN_TTL_EXTEND);
    env.storage()
        .instance()
        .get(&StorageKey::Escrow)
        .ok_or(Error::NotInitialized)
}

/// Store the invoice escrow in instance storage.
pub fn set_escrow(env: &Env, escrow: &Address) {
    env.storage().instance().set(&StorageKey::Escrow, escrow);
    env.storage()
        .instance()
        .extend_ttl(TTL_THRESHOLD, MIN_TTL_EXTEND);
}

/// Hand out the identifier of the next pool.
pub fn next_pool_id(env: &Env) -> u32 {
    let pool_id: u32 = env
        .storage()
        .instance()
        .get(&StorageKey::NextPoolId)
        .unwrap_or(1);
    env.storage().instance().set(&StorageKey::NextPoolId, &(pool_id + 1));
    pool_id
}

/// Load a co-investment pool, extending its TTL on access.
pub fn get_pool(env: &Env, pool_id: u32) -> Result<Pool, Error> {
    let key = StorageKey::Pool(pool_id);
    let pool = env.storage().persistent().get(&key);
    if pool.is_some() {
        bump(env, &key);
    }
    pool.ok_or(Error::PoolNotFound)
}

/// Store a co-investment pool, extending its TTL.
pub fn set_pool(env: &Env, pool: &Pool) {
    let key = StorageKey::Pool(pool.pool_id);
    env.storage().persistent().set(&key, pool);
    bump(env, &key);
}

/// Load the contribution of an investor to a pool.
pub fn get_position(env: &Env, pool_id: u32, investor: &Address) -> i128 {
    let key = StorageKey::Position(pool_id, investor.clone());
    let amount: i128 = env.storage().persistent().get(&key).unwrap_or(0);
    bump(env, &key);
    amount
}

/// Store the contribution of an investor to a pool.
pub fn set_position(env: &Env, pool_id: u32, investor: &Address, amount: i128) {
    let key = StorageKey::Position(pool_id, investor.clone());
    env.storage().persistent().set(&key, &amount);
    bump(env, &key);
}
