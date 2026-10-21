use soroban_sdk::contracterror;

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
