use soroban_sdk::{contracttype, Address, BytesN};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NavRecord {
    pub date: u64,
    pub nav_amount: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    Admin,
    Operator,
    Snapshot(Address, u64),
    SnapshotDates(Address),
    HeldInvoices(Address),
    FractionBalance(Address, BytesN<32>),
    InvoicePrice(BytesN<32>),
}
