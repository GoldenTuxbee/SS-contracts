use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    Unauthorized = 3,
    BatchTooLarge = 4,
    BatchLengthMismatch = 5,
    EmptyBatch = 6,
    InvalidPagination = 7,
}
