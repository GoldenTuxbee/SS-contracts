//! Data types for the invoice hardship extension contract.

use soroban_sdk::{contracttype, String, Symbol};

/// Storage key enum for instance and persistent storage.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StorageKey {
    /// Instance: admin allowed to manage the contract.
    Admin,
    /// Instance: the standard grace period of an invoice, in days.
    GracePeriodDays,
    /// Persistent: the registered invoice and its standard grace end.
    Invoice(Symbol),
    /// Persistent: the hardship extension granted to an invoice.
    Extension(Symbol),
}

/// The standard grace window of an invoice.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InvoiceGrace {
    /// Invoice the record belongs to.
    pub invoice_id: Symbol,
    /// Ledger timestamp the invoice is due.
    pub due_date: u64,
    /// Ledger timestamp the standard grace period ends.
    pub standard_grace_end: u64,
}

/// A hardship extension granted to an issuer.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HardshipExtension {
    /// Invoice the extension belongs to.
    pub invoice_id: Symbol,
    /// Total additional days granted as hardship.
    pub additional_days: u32,
    /// Ledger timestamp the extension ends.
    pub extension_end: u64,
    /// Ledger timestamp the extension was granted at.
    pub granted_at: u64,
    /// Why the extension was granted.
    pub reason: String,
}

/// The grace window of an invoice and the state of its penalty accrual.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraceStatus {
    /// Invoice the status belongs to.
    pub invoice_id: Symbol,
    /// Ledger timestamp the invoice is due.
    pub due_date: u64,
    /// Ledger timestamp the standard grace period ends.
    pub standard_grace_end: u64,
    /// Ledger timestamp the hardship extension ends, zero without one.
    pub extension_end: u64,
    /// Total additional days granted as hardship.
    pub extension_days: u32,
    /// Whether the penalty accrual is paused right now.
    pub penalty_paused: bool,
}
