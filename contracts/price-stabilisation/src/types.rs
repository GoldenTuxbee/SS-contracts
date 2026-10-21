use soroban_sdk::{contracttype, BytesN};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StabilisationStatus {
    pub reserve_balance: i128,
    pub is_active: bool,
    pub floor_price: i128,
    pub last_price: i128,
    pub last_triggered_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PriceObservation {
    pub price: i128,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    Admin,
    Operator,
    FloorPrice(BytesN<32>),
    Reserve(BytesN<32>),
    IsActive(BytesN<32>),
    LastTriggered(BytesN<32>),
    PriceObservation(BytesN<32>),
    NextOrderId(BytesN<32>),
}
