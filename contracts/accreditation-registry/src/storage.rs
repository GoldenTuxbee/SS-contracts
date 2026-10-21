//! Storage helpers for the accreditation registry contract.

use soroban_sdk::{Address, Env};

use crate::types::{AccreditationTier, StorageKey};

/// Ledgers below which a persistent entry's TTL is extended (~7 days at 5s/ledger).
pub const TTL_THRESHOLD: u32 = 120_960;
/// Minimum TTL extension in ledger units (~60 days at 5s/ledger).
pub const MIN_TTL_EXTEND: u32 = 1_036_800;

pub fn set_admin(env: &Env, admin: &Address) {
    env.storage().instance().set(&StorageKey::Admin, admin);
    env.storage()
        .instance()
        .extend_ttl(TTL_THRESHOLD, MIN_TTL_EXTEND);
}

pub fn get_admin(env: &Env) -> Option<Address> {
    env.storage()
        .instance()
        .extend_ttl(TTL_THRESHOLD, MIN_TTL_EXTEND);
    env.storage().instance().get(&StorageKey::Admin)
}

/// Load the tier explicitly set for `wallet`, or `None` if it was never set
/// (the caller is responsible for applying the `Unaccredited` default).
pub fn get_tier_raw(env: &Env, wallet: &Address) -> Option<AccreditationTier> {
    let key = StorageKey::Tier(wallet.clone());
    let data = env.storage().persistent().get(&key);
    if data.is_some() {
        env.storage()
            .persistent()
            .extend_ttl(&key, TTL_THRESHOLD, MIN_TTL_EXTEND);
    }
    data
}

/// Save the tier assigned to `wallet`.
pub fn set_tier(env: &Env, wallet: &Address, tier: AccreditationTier) {
    let key = StorageKey::Tier(wallet.clone());
    env.storage().persistent().set(&key, &tier);
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_THRESHOLD, MIN_TTL_EXTEND);
}
