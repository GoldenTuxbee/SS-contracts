use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    Unauthorized = 3,
    InvalidAmount = 4,
    InvalidPrice = 5,
    ThresholdNotMet = 6,
    InsufficientReserve = 7,
    InvoiceNotFound = 8,
    StabilisationAlreadyActive = 9,
}
