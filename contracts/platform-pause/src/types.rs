//! Data types for the platform pause contract.

use soroban_sdk::{contracttype, BytesN};

/// Storage key enum for instance and persistent storage.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StorageKey {
    /// Instance: contract admin.
    Admin,
    /// Instance: whether the platform is currently paused.
    Paused,
    /// Instance: unix timestamp a queued resume becomes executable at, if
    /// any resume is currently queued.
    ResumeEta,
    /// Persistent: per-invoice pause override, independent of the
    /// platform-wide pause.
    InvoicePaused(BytesN<32>),
}
