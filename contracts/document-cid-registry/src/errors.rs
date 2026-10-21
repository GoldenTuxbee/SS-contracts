//! Error types for the document CID registry contract.

use soroban_sdk::contracterror;

/// Errors that can occur during contract execution.
#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Error {
    /// Contract has already been initialized.
    AlreadyInit = 1,
    /// Contract has not been initialized.
    NotInit = 2,
    /// Caller is neither the admin nor the invoice's registered issuer.
    Unauthorized = 3,
    /// Address is the zero address (all-zero 32-byte key).
    InvalidAddress = 4,
    /// The supplied CID is empty.
    InvalidCid = 5,
    /// This exact CID has already been registered for this invoice.
    DuplicateCid = 6,
}
