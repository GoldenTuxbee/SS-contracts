//! Storage helpers for the platform pause contract.

use soroban_sdk::{Address, BytesN, Env};

use crate::types::StorageKey;

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

pub fn set_paused(env: &Env, paused: bool) {
    env.storage().instance().set(&StorageKey::Paused, &paused);
    env.storage()
        .instance()
        .extend_ttl(TTL_THRESHOLD, MIN_TTL_EXTEND);
}

pub fn get_paused(env: &Env) -> bool {
    env.storage()
        .instance()
        .get(&StorageKey::Paused)
        .unwrap_or(false)
}

pub fn set_resume_eta(env: &Env, eta: u64) {
    env.storage().instance().set(&StorageKey::ResumeEta, &eta);
    env.storage()
        .instance()
        .extend_ttl(TTL_THRESHOLD, MIN_TTL_EXTEND);
}

pub fn get_resume_eta(env: &Env) -> Option<u64> {
    env.storage().instance().get(&StorageKey::ResumeEta)
}

pub fn clear_resume_eta(env: &Env) {
    env.storage().instance().remove(&StorageKey::ResumeEta);
}

pub fn set_invoice_paused(env: &Env, invoice_id: &BytesN<32>, paused: bool) {
    let key = StorageKey::InvoicePaused(invoice_id.clone());
    if paused {
        env.storage().persistent().set(&key, &true);
        env.storage()
            .persistent()
            .extend_ttl(&key, TTL_THRESHOLD, MIN_TTL_EXTEND);
    } else {
        env.storage().persistent().remove(&key);
    }
}

pub fn get_invoice_paused(env: &Env, invoice_id: &BytesN<32>) -> bool {
    let key = StorageKey::InvoicePaused(invoice_id.clone());
    let paused = env.storage().persistent().get(&key).unwrap_or(false);
    if env.storage().persistent().has(&key) {
        env.storage()
            .persistent()
            .extend_ttl(&key, TTL_THRESHOLD, MIN_TTL_EXTEND);
    }
    paused
}
