use soroban_sdk::{contracterror, contracttype, Address, BytesN, Env, Symbol};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StorageKey {
    Admin,
    NextAmendmentId,
    InvoiceMetadata(BytesN<32>),
    InvoiceIssuer(BytesN<32>),
    Amendment(u32),
}

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AmendmentField {
    Amount,
    MaturityDate,
    YieldRate,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AmendmentStatus {
    Pending,
    Executed,
    Revoked,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Amendment {
    pub amendment_id: u32,
    pub invoice_id: BytesN<32>,
    pub issuer: Address,
    pub field: AmendmentField,
    pub new_value: i128,
    pub proposed_at: u64,
    pub execute_after: u64,
    pub status: AmendmentStatus,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InvoiceMetadata {
    pub invoice_id: BytesN<32>,
    pub amount: i128,
    pub maturity_date: u64,
    pub yield_rate: i128,
}

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Error {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    Unauthorized = 3,
    InvalidAmount = 4,
    InvalidField = 5,
    InvoiceNotFound = 6,
    AmendmentNotFound = 7,
    NotPending = 8,
    TimeLockNotExpired = 9,
    AlreadyExecuted = 10,
    AlreadyRevoked = 11,
    Overflow = 12,
}

impl AmendmentField {
    pub fn as_symbol(&self, env: &Env) -> Symbol {
        match self {
            AmendmentField::Amount => Symbol::new(env, "amount"),
            AmendmentField::MaturityDate => Symbol::new(env, "maturity_date"),
            AmendmentField::YieldRate => Symbol::new(env, "yield_rate"),
        }
    }
}
