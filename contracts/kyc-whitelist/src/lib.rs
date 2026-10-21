#![no_std]

//! KYC whitelist contract (#514): stores approved investor wallet addresses
//! and exposes an access check queried by other contracts before accepting
//! an investment.
//!
//! `is_accredited` is deliberately also exposed under that name, matching
//! the signature `invoice-escrow`'s existing, already-shipped
//! `accreditation_callback` hook invokes (`is_accredited(wallet) -> bool`,
//! see `invoice-escrow/src/lib.rs`'s `fund_escrow_core`). Registering this
//! contract's address via `invoice-escrow::set_accreditation_callback` wires
//! it in with no change to the escrow contract: `is_whitelisted` and
//! `is_accredited` return the same answer, under the two names each caller
//! expects.

mod errors;
mod events;
mod types;

#[cfg(test)]
mod test;

pub use errors::Error;
use soroban_sdk::{contract, contractimpl, Address, Env, Vec};
pub use types::{StorageKey, WhitelistEntry};

/// Largest batch `add_to_whitelist`/`remove_from_whitelist` accepts per call (#514).
pub const MAX_BATCH_SIZE: u32 = 100;

#[contract]
pub struct KycWhitelist;

#[contractimpl]
impl KycWhitelist {
    pub fn initialize(env: Env, admin: Address) -> Result<(), Error> {
        if env.storage().instance().has(&StorageKey::Admin) {
            return Err(Error::AlreadyInitialized);
        }
        admin.require_auth();
        env.storage().instance().set(&StorageKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&StorageKey::AllAddresses, &Vec::<Address>::new(&env));
        Ok(())
    }

    /// Admin-only, batch support up to `MAX_BATCH_SIZE` wallets (#514).
    /// Idempotent: a wallet already whitelisted is left unchanged (its
    /// `added_at` is not reset) and is not duplicated in the event payload.
    pub fn add_to_whitelist(env: Env, wallets: Vec<Address>) -> Result<(), Error> {
        Self::require_admin(&env)?;
        if wallets.is_empty() {
            return Err(Error::EmptyBatch);
        }
        if wallets.len() > MAX_BATCH_SIZE {
            return Err(Error::BatchTooLarge);
        }

        let mut all_addresses: Vec<Address> = env
            .storage()
            .instance()
            .get(&StorageKey::AllAddresses)
            .unwrap_or(Vec::new(&env));
        let mut newly_added: Vec<Address> = Vec::new(&env);
        let now = env.ledger().timestamp();

        for i in 0..wallets.len() {
            let wallet = wallets.get(i).unwrap();
            let key = StorageKey::Whitelisted(wallet.clone());
            let already_active = env
                .storage()
                .persistent()
                .get::<_, WhitelistEntry>(&key)
                .map(|entry| entry.is_active)
                .unwrap_or(false);
            if already_active {
                continue;
            }

            let is_new_address = !env.storage().persistent().has(&key);
            let added_at = if is_new_address {
                now
            } else {
                // Re-adding a previously removed wallet keeps its original
                // added_at, so get_whitelist's date reflects first approval,
                // not the most recent re-approval.
                env.storage()
                    .persistent()
                    .get::<_, WhitelistEntry>(&key)
                    .map(|entry| entry.added_at)
                    .unwrap_or(now)
            };

            env.storage().persistent().set(
                &key,
                &WhitelistEntry {
                    wallet: wallet.clone(),
                    added_at,
                    is_active: true,
                },
            );

            if is_new_address {
                all_addresses.push_back(wallet.clone());
            }
            newly_added.push_back(wallet);
        }

        env.storage()
            .instance()
            .set(&StorageKey::AllAddresses, &all_addresses);

        if !newly_added.is_empty() {
            events::whitelist_updated(&env, &newly_added, true);
        }
        Ok(())
    }

    /// Admin-only (#514). Marks each wallet inactive; `get_whitelist` still
    /// lists it (with `is_active: false`) rather than erasing its history.
    pub fn remove_from_whitelist(env: Env, wallets: Vec<Address>) -> Result<(), Error> {
        Self::require_admin(&env)?;
        if wallets.is_empty() {
            return Err(Error::EmptyBatch);
        }
        if wallets.len() > MAX_BATCH_SIZE {
            return Err(Error::BatchTooLarge);
        }

        let mut removed: Vec<Address> = Vec::new(&env);
        for i in 0..wallets.len() {
            let wallet = wallets.get(i).unwrap();
            let key = StorageKey::Whitelisted(wallet.clone());
            if let Some(mut entry) = env.storage().persistent().get::<_, WhitelistEntry>(&key) {
                if entry.is_active {
                    entry.is_active = false;
                    env.storage().persistent().set(&key, &entry);
                    removed.push_back(wallet);
                }
            }
        }

        if !removed.is_empty() {
            events::whitelist_updated(&env, &removed, false);
        }
        Ok(())
    }

    /// View check called by escrow (or any integrator) on invest (#514).
    pub fn is_whitelisted(env: Env, wallet: Address) -> bool {
        env.storage()
            .persistent()
            .get::<_, WhitelistEntry>(&StorageKey::Whitelisted(wallet))
            .map(|entry| entry.is_active)
            .unwrap_or(false)
    }

    /// Alias of `is_whitelisted`, under the exact name and signature
    /// `invoice-escrow`'s `accreditation_callback` hook invokes
    /// (`is_accredited(buyer: Address) -> bool`), so this contract's address
    /// can be registered directly via
    /// `invoice-escrow::set_accreditation_callback` with no adapter.
    pub fn is_accredited(env: Env, wallet: Address) -> bool {
        Self::is_whitelisted(env, wallet)
    }

    /// Returns the full whitelist, active and previously-removed entries
    /// alike, each with the date it was added (#514). Paginated like
    /// `watchlist::get_watchlist`, since the list has no bound otherwise.
    pub fn get_whitelist(
        env: Env,
        page: u32,
        page_size: u32,
    ) -> Result<Vec<WhitelistEntry>, Error> {
        if page_size == 0 {
            return Err(Error::InvalidPagination);
        }

        let all_addresses: Vec<Address> = env
            .storage()
            .instance()
            .get(&StorageKey::AllAddresses)
            .unwrap_or(Vec::new(&env));

        let mut result = Vec::new(&env);
        let start_index = page.saturating_mul(page_size);
        if start_index >= all_addresses.len() {
            return Ok(result);
        }
        let end_index = (start_index + page_size).min(all_addresses.len());

        for i in start_index..end_index {
            let wallet = all_addresses.get(i).unwrap();
            if let Some(entry) = env
                .storage()
                .persistent()
                .get::<_, WhitelistEntry>(&StorageKey::Whitelisted(wallet))
            {
                result.push_back(entry);
            }
        }
        Ok(result)
    }

    pub fn admin(env: Env) -> Result<Address, Error> {
        env.storage()
            .instance()
            .get(&StorageKey::Admin)
            .ok_or(Error::NotInitialized)
    }

    fn require_admin(env: &Env) -> Result<(), Error> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&StorageKey::Admin)
            .ok_or(Error::NotInitialized)?;
        admin.require_auth();
        Ok(())
    }
}
