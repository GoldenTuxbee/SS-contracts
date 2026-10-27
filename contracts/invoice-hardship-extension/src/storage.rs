//! Storage helpers for the invoice hardship extension contract.

use soroban_sdk::{Address, Env, Symbol};

use crate::errors::Error;
use crate::types::{HardshipExtension, InvoiceGrace, StorageKey};

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

/// Load the standard grace period in days.
pub fn get_grace_period_days(env: &Env) -> u32 {
    env.storage()
        .instance()
        .extend_ttl(TTL_THRESHOLD, MIN_TTL_EXTEND);
    env.storage()
        .instance()
        .get(&StorageKey::GracePeriodDays)
        .unwrap_or(0)
}

/// Store the standard grace period in days.
pub fn set_grace_period_days(env: &Env, days: u32) {
    env.storage().instance().set(&StorageKey::GracePeriodDays, &days);
    env.storage()
        .instance()
        .extend_ttl(TTL_THRESHOLD, MIN_TTL_EXTEND);
}

/// Load the standard grace window of an invoice.
pub fn get_invoice(env: &Env, invoice_id: &Symbol) -> Result<InvoiceGrace, Error> {
    let key = StorageKey::Invoice(invoice_id.clone());
    let invoice = env.storage().persistent().get(&key);
    if invoice.is_some() {
        bump(env, &key);
    }
    invoice.ok_or(Error::InvoiceNotFound)
}

/// Store the standard grace window of an invoice.
pub fn set_invoice(env: &Env, invoice: &InvoiceGrace) {
    let key = StorageKey::Invoice(invoice.invoice_id.clone());
    env.storage().persistent().set(&key, invoice);
    bump(env, &key);
}

/// Load the hardship extension of an invoice, if it has one.
pub fn get_extension(env: &Env, invoice_id: &Symbol) -> Option<HardshipExtension> {
    let key = StorageKey::Extension(invoice_id.clone());
    let extension = env.storage().persistent().get(&key);
    if extension.is_some() {
        bump(env, &key);
    }
    extension
}

/// Store the hardship extension of an invoice.
pub fn set_extension(env: &Env, extension: &HardshipExtension) {
    let key = StorageKey::Extension(extension.invoice_id.clone());
    env.storage().persistent().set(&key, extension);
    bump(env, &key);
}
