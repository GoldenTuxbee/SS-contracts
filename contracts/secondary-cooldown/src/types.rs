use soroban_sdk::{contracttype, Address, BytesN};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StorageKey {
    Admin,
    CooldownPeriod(BytesN<32>),
    LastBuy(Address, BytesN<32>),
}
