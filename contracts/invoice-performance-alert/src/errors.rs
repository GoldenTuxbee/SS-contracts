use soroban_sdk::contracterror;

/// Errors that can occur while monitoring invoice performance.
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
    /// No metrics have been recorded for the invoice.
    InvoiceNotFound = 4,
    /// Target or funded amount is not a valid pair of amounts.
    InvalidAmount = 5,
    /// Maturity or due date is missing.
    InvalidTimestamp = 6,
}
