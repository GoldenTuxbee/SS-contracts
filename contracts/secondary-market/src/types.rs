use soroban_sdk::{contracttype, Address, BytesN};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Listing {
    pub listing_id: u32,
    pub seller: Address,
    pub invoice_id: BytesN<32>,
    /// The invoice-token contract instance for this invoice. Each invoice
    /// has its own deployed fraction-token contract (one balance ledger per
    /// invoice, not a shared multi-invoice token), so the listing carries
    /// the address rather than assuming a single global token.
    pub fraction_token: Address,
    /// Quantity escrowed into this contract when the listing was created.
    /// Decreases as `buy_listing` partially fills it.
    pub quantity: i128,
    pub ask_price_per_fraction: i128,
    /// Payment token the seller is asking to be paid in (e.g. a USDC SAC).
    pub payment_token: Address,
    pub expiry: u64,
    pub is_active: bool,
    pub created_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StorageKey {
    Admin,
    NextListingId,
    Listing(u32),
    InvoiceListings(BytesN<32>),
    FloorMultiplier,
    FloorInputs(BytesN<32>),
    FloorInvoice(Address),
}
