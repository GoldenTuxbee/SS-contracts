use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    InvalidAmount = 3,
    InvalidPrice = 4,
    InsufficientBalance = 5,
    ListingNotFound = 6,
    ListingNotActive = 7,
    ListingExpired = 8,
    Unauthorized = 9,
    ExceedsListedQuantity = 10,
    InvalidExpiry = 11,
    /// Ask is below the current minimum; get_floor_price supplies the breakdown.
    FloorPriceViolation = 12,
    /// Multiplier must be between 1 and 10,000 basis points.
    InvalidFloorMultiplier = 13,
    /// Yield must be nonnegative and total fractions strictly positive.
    InvalidFloorInputs = 14,
    FloorNotConfigured = 15,
    /// Listing assets do not match the admin-configured invoice assets.
    FloorAssetMismatch = 16,
    Overflow = 17,
}
