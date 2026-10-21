//! Emergency pause contract for StellarSettle.
//!
//! Lets the admin halt all escrow investment, fraction transfer, and
//! secondary market activity across the platform in a single transaction.
//! Resuming is deliberately slower than pausing: `resume_platform` queues a
//! 24-hour timelock rather than resuming immediately, giving the community
//! time to observe and react to a resumption decision after an incident.
//! Individual invoices can also be paused independently of the platform-wide
//! switch, for incidents scoped to a single invoice.

#![no_std]

mod errors;
mod events;
mod storage;
mod types;

use soroban_sdk::{contract, contractimpl, Address, BytesN, Env, String};

use errors::Error;

/// Minimum delay between queuing a resume and it taking effect.
pub const RESUME_TIMELOCK_SECONDS: u64 = 24 * 3_600;

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
pub struct PlatformPause;

#[contractimpl]
impl PlatformPause {
    /// Initialize the contract with the admin allowed to pause, queue and
    /// execute resumes, and set per-invoice overrides.
    pub fn initialize(env: Env, admin: Address) -> Result<(), Error> {
        ensure_non_zero_address(&env, &admin)?;
        if storage::get_admin(&env).is_some() {
            return Err(Error::AlreadyInit);
        }
        storage::set_admin(&env, &admin);
        events::initialized(&env, &admin);
        Ok(())
    }

    /// Admin-only: immediately halt all platform investment and transfer
    /// activity. Clears any resume that was already queued, since a fresh
    /// pause supersedes it.
    pub fn pause_platform(env: Env, admin: Address) -> Result<(), Error> {
        let stored_admin = storage::get_admin(&env).ok_or(Error::NotInit)?;
        if admin != stored_admin {
            return Err(Error::Unauthorized);
        }
        admin.require_auth();
        storage::set_paused(&env, true);
        storage::clear_resume_eta(&env);
        events::platform_paused(&env, &admin);
        Ok(())
    }

    /// Admin-only: queue a resume, or execute one already queued once its
    /// 24-hour timelock has elapsed.
    ///
    /// The first call while paused with no resume queued starts the
    /// timelock and returns without resuming. A later call (by anyone with
    /// admin rights, not necessarily the same caller) after
    /// `RESUME_TIMELOCK_SECONDS` have elapsed since that call actually
    /// clears the pause.
    pub fn resume_platform(env: Env, admin: Address) -> Result<(), Error> {
        let stored_admin = storage::get_admin(&env).ok_or(Error::NotInit)?;
        if admin != stored_admin {
            return Err(Error::Unauthorized);
        }
        admin.require_auth();

        if !storage::get_paused(&env) {
            return Err(Error::NotPaused);
        }

        match storage::get_resume_eta(&env) {
            None => {
                let eta = env.ledger().timestamp() + RESUME_TIMELOCK_SECONDS;
                storage::set_resume_eta(&env, eta);
                events::resume_queued(&env, eta, &admin);
                Ok(())
            }
            Some(eta) => {
                if env.ledger().timestamp() < eta {
                    return Err(Error::TimelockNotElapsed);
                }
                storage::set_paused(&env, false);
                storage::clear_resume_eta(&env);
                events::platform_resumed(&env, &admin);
                Ok(())
            }
        }
    }

    /// Admin-only: cancel a queued resume without resuming. Useful if new
    /// information surfaces during the 24h window.
    pub fn cancel_resume(env: Env, admin: Address) -> Result<(), Error> {
        let stored_admin = storage::get_admin(&env).ok_or(Error::NotInit)?;
        if admin != stored_admin {
            return Err(Error::Unauthorized);
        }
        admin.require_auth();

        if storage::get_resume_eta(&env).is_none() {
            return Err(Error::NoResumeQueued);
        }
        storage::clear_resume_eta(&env);
        Ok(())
    }

    /// View: whether the platform is currently paused.
    pub fn is_paused(env: Env) -> bool {
        storage::get_paused(&env)
    }

    /// View: the unix timestamp a queued resume becomes executable at, or
    /// `None` if no resume is queued.
    pub fn resume_eta(env: Env) -> Option<u64> {
        storage::get_resume_eta(&env)
    }

    /// Admin-only: pause or unpause a single invoice, independent of the
    /// platform-wide pause.
    pub fn set_invoice_paused(
        env: Env,
        admin: Address,
        invoice_id: BytesN<32>,
        paused: bool,
    ) -> Result<(), Error> {
        let stored_admin = storage::get_admin(&env).ok_or(Error::NotInit)?;
        if admin != stored_admin {
            return Err(Error::Unauthorized);
        }
        admin.require_auth();
        storage::set_invoice_paused(&env, &invoice_id, paused);
        events::invoice_pause_set(&env, &invoice_id, paused);
        Ok(())
    }

    /// View: whether `invoice_id` is paused, independent of the
    /// platform-wide pause. Dependent contracts should treat an invoice as
    /// blocked if either `is_paused()` or this returns true.
    pub fn is_invoice_paused(env: Env, invoice_id: BytesN<32>) -> bool {
        storage::get_invoice_paused(&env, &invoice_id)
    }

    /// View: return the current admin.
    pub fn get_admin(env: Env) -> Result<Address, Error> {
        storage::get_admin(&env).ok_or(Error::NotInit)
    }
}

#[cfg(test)]
mod test;
