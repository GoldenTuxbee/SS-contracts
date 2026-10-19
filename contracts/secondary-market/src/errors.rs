use soroban_sdk::contracterror;

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Error {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    Unauthorized = 3,
    InvalidAmount = 4,
    InvalidFeeBps = 5,
    InvoiceExists = 6,
    InvoiceNotFound = 7,
    AlreadyRepaid = 8,
    NothingToDistribute = 9,
    AlreadyDistributed = 10,
    HolderNotFound = 11,
    Overflow = 12,
    InvalidHolders = 13,
}
