//! Data types for the invoice co-investment pool contract.

use soroban_sdk::{contracttype, Address, Symbol};

/// Storage key enum for instance and persistent storage.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StorageKey {
    /// Instance: admin allowed to manage the contract.
    Admin,
    /// Instance: payment token the pools are funded in.
    Token,
    /// Instance: invoice escrow the pools fund.
    Escrow,
    /// Instance: identifier handed to the next pool.
    NextPoolId,
    /// Persistent: a co-investment pool.
    Pool(u32),
    /// Persistent: the contribution of an investor to a pool.
    Position(u32, Address),
}

/// Lifecycle state of a co-investment pool.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PoolStatus {
    /// The pool still accepts contributions.
    Open,
    /// The pool funded the invoice as a single position.
    Executed,
}

/// A co-investment pool for one invoice.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Pool {
    /// Identifier of the pool.
    pub pool_id: u32,
    /// Invoice the pool funds.
    pub invoice_id: Symbol,
    /// Amount the pool has to raise to fund the invoice.
    pub target_amount: i128,
    /// Smallest contribution an investor may make.
    pub min_contribution: i128,
    /// Amount raised so far.
    pub total_raised: i128,
    /// Number of investors in the pool.
    pub investor_count: u32,
    /// Lifecycle state of the pool.
    pub status: PoolStatus,
}
