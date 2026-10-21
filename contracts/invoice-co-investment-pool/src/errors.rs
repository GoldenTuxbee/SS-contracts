use soroban_sdk::contracterror;

/// Errors that can occur while operating a co-investment pool.
#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Error {
    /// Contract has already been initialized.
    AlreadyInitialized = 1,
    /// Contract has not been initialized.
    NotInitialized = 2,
    /// Caller is not allowed to perform the operation.
    Unauthorized = 3,
    /// An amount is not a valid positive amount.
    InvalidAmount = 4,
    /// The contribution is below the minimum of the pool.
    BelowMinimum = 5,
    /// The pool does not exist.
    PoolNotFound = 6,
    /// The pool was already executed and no longer accepts operations.
    PoolClosed = 7,
    /// The pool did not reach its target amount yet.
    TargetNotMet = 8,
    /// The investor has no contribution in the pool.
    NoContribution = 9,
    /// An amount overflowed while it was accumulated.
    Overflow = 10,
}
