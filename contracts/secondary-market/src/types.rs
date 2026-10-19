use soroban_sdk::{contracterror, contracttype, Address, BytesN};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StorageKey {
    Admin,
    FractionToken,
    PaymentToken,
    NextListing,
    Listing(u32),
    InvoiceListings(BytesN<32>),
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ListingStatus {
    Active,
    Sold,
    Expired,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Listing {
    pub listing_id: u32,
    pub seller: Address,
    pub invoice_id: BytesN<32>,
    pub fraction_amount: i128,
    pub price_per_fraction: i128,
    pub expiry_timestamp: u64,
    pub status: ListingStatus,
}

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Error {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    Unauthorized = 3,
    InvalidAmount = 4,
    InvalidExpiry = 5,
    InsufficientBalance = 6,
    ListingNotFound = 7,
    ListingNotActive = 8,
    ListingAlreadySold = 9,
    ListingExpired = 10,
    Overflow = 11,
}
