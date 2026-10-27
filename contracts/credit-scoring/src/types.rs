use soroban_sdk::{contracttype, Address};

#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum CreditTier {
    C = 0,
    B = 1,
    A = 2,
    AA = 3,
    AAA = 4,
}

/// The four factors used to compute the credit score.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IssuerFactors {
    /// KYC level 0–3.
    pub kyc_tier: u32,
    /// Number of fully settled invoices.
    pub settlement_history: u32,
    /// Number of defaults.
    pub default_count: u32,
    /// Days since the issuer joined the platform.
    pub platform_tenure_days: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreditScore {
    pub score: u32,
    pub tier: CreditTier,
    pub factors: IssuerFactors,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StorageKey {
    Admin,
    AuthorisedCaller(Address),
    Score(Address),
}
