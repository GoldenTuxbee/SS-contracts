use soroban_sdk::contracterror;

/// Errors that can occur while granting a hardship extension.
#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Error {
    /// Contract has already been initialized.
    AlreadyInitialized = 1,
    /// Contract has not been initialized.
    NotInitialized = 2,
    /// The invoice is not registered with the contract.
    InvoiceNotFound = 3,
    /// The number of additional days is not a positive number.
    InvalidDays = 4,
    /// The extension exceeds the maximum number of additional days.
    MaxExtensionExceeded = 5,
    /// The invoice has no due date.
    InvalidDueDate = 6,
}
