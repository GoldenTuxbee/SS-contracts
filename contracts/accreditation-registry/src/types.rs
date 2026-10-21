//! Data types for the accreditation registry contract.

use soroban_sdk::{contracttype, Address};

/// Storage key enum for instance and persistent storage.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StorageKey {
    /// Instance: registry admin.
    Admin,
    /// Persistent: accreditation tier for a wallet.
    Tier(Address),
}

/// Investor accreditation tier. Ordered from least to most permissive;
/// `as u32` comparisons rely on this ordering to decide eligibility.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum AccreditationTier {
    /// Default tier for any wallet never explicitly set. May only
    /// participate in the lowest-risk invoices.
    Unaccredited = 0,
    /// Verified accredited investor. May participate in low and medium
    /// risk invoices.
    Accredited = 1,
    /// Verified qualified purchaser. May participate in any risk tier.
    Qualified = 2,
}

/// Invoice risk tier, as classified elsewhere in the protocol (see the
/// invoice registry's `RiskTier`). Duplicated here rather than imported so
/// this contract has no compile-time dependency on another contract crate;
/// the two enums must stay in the same Low/Medium/High order and discriminant
/// values for `check_eligible`'s comparison to remain meaningful.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum InvoiceRiskTier {
    /// Lowest risk.
    Low = 0,
    /// Moderate risk.
    Medium = 1,
    /// Highest risk.
    High = 2,
}
