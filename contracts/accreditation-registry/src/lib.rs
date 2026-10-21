//! Investor accreditation registry contract for StellarSettle.
//!
//! Stores each wallet's accreditation tier on-chain. The escrow contract (or
//! any other eligibility checker) consults `check_eligible` before accepting
//! an investment into an invoice of a given risk tier.

#![no_std]

mod errors;
mod events;
mod storage;
mod types;

pub use types::{AccreditationTier, InvoiceRiskTier};

use soroban_sdk::{contract, contractimpl, Address, Env, String};

use errors::Error;

/// Reject the zero address (all-zero 32-byte Ed25519 key).
fn ensure_non_zero_address(env: &Env, address: &Address) -> Result<(), Error> {
    let zero_str = String::from_str(
        env,
        "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
    );
    let zero = Address::from_string(&zero_str);
    if *address == zero {
        return Err(Error::InvalidAddress);
    }
    Ok(())
}

#[contract]
pub struct AccreditationRegistry;

#[contractimpl]
impl AccreditationRegistry {
    /// Initialize the registry with the admin allowed to set tiers.
    pub fn initialize(env: Env, admin: Address) -> Result<(), Error> {
        ensure_non_zero_address(&env, &admin)?;
        if storage::get_admin(&env).is_some() {
            return Err(Error::AlreadyInit);
        }
        storage::set_admin(&env, &admin);
        events::initialized(&env, &admin);
        Ok(())
    }

    /// Admin-only: set (or change) a wallet's accreditation tier.
    pub fn set_tier(
        env: Env,
        admin: Address,
        wallet: Address,
        tier: AccreditationTier,
    ) -> Result<(), Error> {
        let stored_admin = storage::get_admin(&env).ok_or(Error::NotInit)?;
        if admin != stored_admin {
            return Err(Error::Unauthorized);
        }
        admin.require_auth();
        ensure_non_zero_address(&env, &wallet)?;

        let old_tier =
            storage::get_tier_raw(&env, &wallet).unwrap_or(AccreditationTier::Unaccredited);
        storage::set_tier(&env, &wallet, tier);
        events::accreditation_updated(&env, &wallet, old_tier, tier);
        Ok(())
    }

    /// View: return `wallet`'s current accreditation tier, defaulting to
    /// `Unaccredited` for any wallet whose tier was never explicitly set.
    pub fn get_tier(env: Env, wallet: Address) -> AccreditationTier {
        storage::get_tier_raw(&env, &wallet).unwrap_or(AccreditationTier::Unaccredited)
    }

    /// View: whether `wallet` is eligible to invest in an invoice of
    /// `invoice_risk_tier`. A wallet's tier must be at least as permissive
    /// as the invoice's risk tier (Unaccredited <= Low only, Accredited <=
    /// Low/Medium, Qualified <= any), comparing the two enums' shared
    /// Low/Medium/High-equivalent ordering.
    pub fn check_eligible(env: Env, wallet: Address, invoice_risk_tier: InvoiceRiskTier) -> bool {
        let tier = storage::get_tier_raw(&env, &wallet).unwrap_or(AccreditationTier::Unaccredited);
        (tier as u32) >= (invoice_risk_tier as u32)
    }

    /// View: return the current admin.
    pub fn get_admin(env: Env) -> Result<Address, Error> {
        storage::get_admin(&env).ok_or(Error::NotInit)
    }
}

#[cfg(test)]
mod test;
