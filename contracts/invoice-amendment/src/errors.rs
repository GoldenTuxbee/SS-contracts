use soroban_sdk::contracterror;

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
