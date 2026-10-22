//! Data types for the investor portfolio health contract.

use soroban_sdk::{contracttype, Address, Symbol};

/// Storage key enum for instance and persistent storage.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    /// Instance: admin allowed to manage the contract.
    Admin,
    /// Instance: operator reporting the portfolio events.
    Operator,
    /// Persistent: the portfolio book of a wallet.
    Book(Address),
    /// Persistent: the last computed health score of a wallet.
    Score(Address),
}

/// State of a single position of a portfolio.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PositionStatus {
    /// The invoice is still outstanding.
    Active,
    /// The invoice defaulted.
    Defaulted,
}

/// A single invoice position held by an investor.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Position {
    /// Identifier of the invoice the position refers to.
    pub invoice_id: Symbol,
    /// Amount invested in the invoice.
    pub amount: i128,
    /// Issuer of the invoice.
    pub issuer: Address,
    /// Ledger timestamp the invoice matures at.
    pub maturity_timestamp: u64,
    /// Whether the invoice is still outstanding or already defaulted.
    pub status: PositionStatus,
}

/// The risk factors a health score is derived from.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HealthFactors {
    /// Largest single position in basis points of the portfolio value.
    pub max_concentration_bps: u32,
    /// Amount weighted average of the days left until maturity.
    pub avg_days_to_maturity: u32,
    /// Number of distinct issuers in the portfolio.
    pub issuer_count: u32,
    /// Defaulted exposure in basis points of the portfolio value.
    pub default_exposure_bps: u32,
    /// Total value of the portfolio.
    pub total_value: i128,
    /// Number of positions in the portfolio.
    pub position_count: u32,
}

/// The health score of an investor portfolio.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HealthScore {
    /// Investor the score belongs to.
    pub wallet: Address,
    /// Score between 0 (worst) and 100 (best).
    pub score: u32,
    /// The factors the score was derived from.
    pub factors: HealthFactors,
    /// Ledger timestamp the score was computed at.
    pub computed_at: u64,
}
