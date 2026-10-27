use soroban_sdk::{contracterror, contracttype, Address, BytesN};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StorageKey {
    Admin,
    Token,
    CompensationPool,
    Collateral(BytesN<32>),
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CollateralState {
    Locked,
    Released,
    Forfeited,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CollateralRecord {
    pub invoice_id: BytesN<32>,
    pub issuer: Address,
    pub amount: i128,
    pub state: CollateralState,
    pub deposited_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CollateralStatus {
    pub invoice_id: BytesN<32>,
    pub issuer: Address,
    pub amount: i128,
    pub state: CollateralState,
}

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Error {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    Unauthorized = 3,
    InvalidAmount = 4,
    AlreadyLocked = 5,
    NotLocked = 6,
    InvoiceNotFound = 7,
    InsufficientBalance = 8,
    Overflow = 9,
}
