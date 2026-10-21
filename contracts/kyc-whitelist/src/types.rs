use soroban_sdk::{contracttype, Address};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StorageKey {
    Admin,
    /// Whether `Address` is currently whitelisted, and when it was added.
    Whitelisted(Address),
    /// Every address ever added, in insertion order, for `get_whitelist`.
    /// Entries are never removed from this index on `remove_from_whitelist`
    /// (only their `Whitelisted` status flips to false); callers filter on
    /// `is_active` from `get_whitelist`'s returned entries.
    AllAddresses,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WhitelistEntry {
    pub wallet: Address,
    pub added_at: u64,
    pub is_active: bool,
}
