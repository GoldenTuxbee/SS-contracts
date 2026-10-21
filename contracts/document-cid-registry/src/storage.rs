//! Storage helpers for the document CID registry contract.

use soroban_sdk::{Address, BytesN, Env, Vec};

use crate::types::{DocumentRecord, StorageKey};

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

pub fn set_issuer(env: &Env, invoice_id: &BytesN<32>, issuer: &Address) {
    let key = StorageKey::Issuer(invoice_id.clone());
    env.storage().persistent().set(&key, issuer);
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_THRESHOLD, MIN_TTL_EXTEND);
}

pub fn get_issuer(env: &Env, invoice_id: &BytesN<32>) -> Option<Address> {
    let key = StorageKey::Issuer(invoice_id.clone());
    let data = env.storage().persistent().get(&key);
    if data.is_some() {
        env.storage()
            .persistent()
            .extend_ttl(&key, TTL_THRESHOLD, MIN_TTL_EXTEND);
    }
    data
}

pub fn get_documents(env: &Env, invoice_id: &BytesN<32>) -> Vec<DocumentRecord> {
    let key = StorageKey::Documents(invoice_id.clone());
    let data = env
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or_else(|| Vec::new(env));
    if env.storage().persistent().has(&key) {
        env.storage()
            .persistent()
            .extend_ttl(&key, TTL_THRESHOLD, MIN_TTL_EXTEND);
    }
    data
}

pub fn set_documents(env: &Env, invoice_id: &BytesN<32>, documents: &Vec<DocumentRecord>) {
    let key = StorageKey::Documents(invoice_id.clone());
    env.storage().persistent().set(&key, documents);
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_THRESHOLD, MIN_TTL_EXTEND);
}
