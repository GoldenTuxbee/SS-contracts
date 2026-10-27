//! Invoice over-collateralisation reward contract (issue #658).
//!
//! Grants issuers a 10% fee discount on their next invoice when they maintain
//! collateral above the required minimum for 30 consecutive days.

#![no_std]

mod errors;
mod events;
mod types;

#[cfg(test)]
mod test;

pub use errors::Error;
pub use types::{CollateralRecord, RewardStatus, StorageKey};

use soroban_sdk::{contract, contractimpl, Address, Env};

const SECONDS_PER_DAY: u64 = 86_400;
const REQUIRED_STREAK_DAYS: u64 = 30;
/// 10% fee discount expressed as discount_bps off the platform fee.
const DISCOUNT_BPS: u32 = 1_000;

fn require_admin(env: &Env) -> Result<(), Error> {
    let admin: Address = env
        .storage()
        .instance()
        .get(&StorageKey::Admin)
        .ok_or(Error::NotInitialized)?;
    admin.require_auth();
    Ok(())
}

#[contract]
pub struct OvercollateralReward;

#[contractimpl]
impl OvercollateralReward {
    pub fn initialize(env: Env, admin: Address) -> Result<(), Error> {
        if env.storage().instance().has(&StorageKey::Admin) {
            return Err(Error::AlreadyInitialized);
        }
        admin.require_auth();
        env.storage().instance().set(&StorageKey::Admin, &admin);
        Ok(())
    }

    /// Admin-only: record the current collateral state for an issuer.
    /// `collateral_bps` is the issuer's current collateral expressed in bps
    /// of the invoice value; `required_bps` is the minimum required threshold.
    pub fn record_collateral(
        env: Env,
        issuer: Address,
        collateral_bps: u32,
        required_bps: u32,
    ) -> Result<(), Error> {
        require_admin(&env)?;

        let now = env.ledger().timestamp();
        let is_over = collateral_bps > required_bps;
        let excess_bps = if is_over {
            collateral_bps - required_bps
        } else {
            0
        };

        let mut record: CollateralRecord = env
            .storage()
            .persistent()
            .get(&StorageKey::Record(issuer.clone()))
            .unwrap_or(CollateralRecord {
                streak_start: 0,
                is_overcollateral: false,
                excess_bps: 0,
                discount_pending: false,
            });

        if is_over {
            if !record.is_overcollateral {
                // Start a new streak.
                record.streak_start = now;
            }
            record.is_overcollateral = true;
            record.excess_bps = excess_bps;
        } else {
            // Streak broken.
            record.is_overcollateral = false;
            record.excess_bps = 0;
            record.streak_start = 0;
        }

        env.storage()
            .persistent()
            .set(&StorageKey::Record(issuer), &record);
        Ok(())
    }

    /// Returns whether the issuer is currently over-collateralised, their
    /// excess bps, and how many days the current streak has lasted.
    pub fn check_overcollateral(
        env: Env,
        issuer: Address,
    ) -> Result<(bool, u32, u32), Error> {
        let record: CollateralRecord = env
            .storage()
            .persistent()
            .get(&StorageKey::Record(issuer))
            .unwrap_or(CollateralRecord {
                streak_start: 0,
                is_overcollateral: false,
                excess_bps: 0,
                discount_pending: false,
            });

        let duration_days = if record.is_overcollateral && record.streak_start > 0 {
            let now = env.ledger().timestamp();
            ((now.saturating_sub(record.streak_start)) / SECONDS_PER_DAY) as u32
        } else {
            0
        };

        Ok((record.is_overcollateral, record.excess_bps, duration_days))
    }

    /// Admin-only: grant the fee discount after 30+ days of over-collateralisation.
    pub fn grant_fee_discount(env: Env, issuer: Address) -> Result<(), Error> {
        require_admin(&env)?;

        let mut record: CollateralRecord = env
            .storage()
            .persistent()
            .get(&StorageKey::Record(issuer.clone()))
            .ok_or(Error::IssuerNotFound)?;

        if !record.is_overcollateral {
            return Err(Error::NotOvercollateral);
        }

        let now = env.ledger().timestamp();
        let days_elapsed =
            (now.saturating_sub(record.streak_start)) / SECONDS_PER_DAY;

        if days_elapsed < REQUIRED_STREAK_DAYS {
            return Err(Error::StreakTooShort);
        }
        if record.discount_pending {
            return Err(Error::DiscountAlreadyPending);
        }

        record.discount_pending = true;
        env.storage()
            .persistent()
            .set(&StorageKey::Record(issuer.clone()), &record);

        events::overcollateral_reward_granted(&env, &issuer, DISCOUNT_BPS);
        Ok(())
    }

    /// Returns the issuer's current streak duration and whether a discount is pending.
    pub fn get_collateral_reward_status(env: Env, issuer: Address) -> Result<RewardStatus, Error> {
        let record: CollateralRecord = env
            .storage()
            .persistent()
            .get(&StorageKey::Record(issuer))
            .unwrap_or(CollateralRecord {
                streak_start: 0,
                is_overcollateral: false,
                excess_bps: 0,
                discount_pending: false,
            });

        let streak_days = if record.is_overcollateral && record.streak_start > 0 {
            let now = env.ledger().timestamp();
            ((now.saturating_sub(record.streak_start)) / SECONDS_PER_DAY) as u32
        } else {
            0
        };

        Ok(RewardStatus {
            streak_days,
            discount_pending: record.discount_pending,
            discount_bps: if record.discount_pending { DISCOUNT_BPS } else { 0 },
        })
    }

    /// Consume a pending discount (called when an invoice is created).
    pub fn consume_discount(env: Env, issuer: Address) -> Result<u32, Error> {
        issuer.require_auth();

        let mut record: CollateralRecord = env
            .storage()
            .persistent()
            .get(&StorageKey::Record(issuer.clone()))
            .ok_or(Error::IssuerNotFound)?;

        if !record.discount_pending {
            return Ok(0);
        }

        record.discount_pending = false;
        env.storage()
            .persistent()
            .set(&StorageKey::Record(issuer), &record);

        Ok(DISCOUNT_BPS)
    }
}
