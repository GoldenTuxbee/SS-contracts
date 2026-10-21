//! Error types for the platform pause contract.

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
    /// Caller is not the admin.
    Unauthorized = 3,
    /// Address is the zero address (all-zero 32-byte key).
    InvalidAddress = 4,
    /// The platform is not currently paused.
    NotPaused = 5,
    /// A resume has not been queued; nothing to execute or cancel.
    NoResumeQueued = 6,
    /// The queued resume's 24h timelock has not yet elapsed.
    TimelockNotElapsed = 7,
}
