use soroban_sdk::contracterror;

/// Errors that can occur while scoring an investor portfolio.
#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Error {
    /// Contract has already been initialized.
    AlreadyInitialized = 1,
    /// Contract has not been initialized.
    NotInitialized = 2,
    /// Caller is neither the admin nor the authorised operator.
    Unauthorized = 3,
    /// The reported amount is not a positive amount.
    InvalidAmount = 4,
    /// The wallet does not hold a position in the invoice.
    PositionNotFound = 5,
}
