//! Invoice credit scoring contract for issuer risk tiering (issue #660).
//!
//! Assigns issuers a credit tier (C/B/A/AA/AAA) based on KYC tier,
//! settlement history, default count, and platform tenure.
//! Tier determines maximum invoice size and yield rate floor.

#![no_std]

mod errors;
mod events;
mod types;

#[cfg(test)]
mod test;

pub use errors::Error;
pub use types::{CreditScore, CreditTier, IssuerFactors, StorageKey};

use soroban_sdk::{contract, contractimpl, Address, Env};

/// Scoring weights (points per unit):
///  kyc_tier (0-3)            × 20 = up to  60 pts
///  settlement_history (0-N)  × 5  = up to  50 pts (capped at 10)
///  default_count             × -25 = penalty
///  platform_tenure_days      × 0.1 (1pt per 10 days, capped at 30 pts)
fn tier_from_score(score: u32) -> CreditTier {
    match score {
        0..=39 => CreditTier::C,
        40..=59 => CreditTier::B,
        60..=74 => CreditTier::A,
        75..=89 => CreditTier::AA,
        _ => CreditTier::AAA,
    }
}

fn compute_score_internal(factors: &IssuerFactors) -> u32 {
    let kyc_pts = (factors.kyc_tier.min(3) as u32) * 20;

    let settlement_pts = (factors.settlement_history.min(10) as u32) * 5;

    let default_penalty = (factors.default_count as u32) * 25;

    let tenure_pts = (factors.platform_tenure_days / 10).min(30);

    let raw = kyc_pts + settlement_pts + tenure_pts;
    raw.saturating_sub(default_penalty)
}

fn require_authorised(env: &Env, caller: &Address) -> Result<(), Error> {
    // The caller must be either the admin or a registered authorised contract.
    let admin: Address = env
        .storage()
        .instance()
        .get(&StorageKey::Admin)
        .ok_or(Error::NotInitialized)?;

    let is_admin = *caller == admin;
    let is_authorised: bool = env
        .storage()
        .persistent()
        .get(&StorageKey::AuthorisedCaller(caller.clone()))
        .unwrap_or(false);

    if !is_admin && !is_authorised {
        return Err(Error::Unauthorized);
    }
    caller.require_auth();
    Ok(())
}

#[contract]
pub struct CreditScoring;

#[contractimpl]
impl CreditScoring {
    pub fn initialize(env: Env, admin: Address) -> Result<(), Error> {
        if env.storage().instance().has(&StorageKey::Admin) {
            return Err(Error::AlreadyInitialized);
        }
        admin.require_auth();
        env.storage().instance().set(&StorageKey::Admin, &admin);
        Ok(())
    }

    /// Admin-only: register a contract address as authorised to call
    /// `compute_credit_score`.
    pub fn set_authorised_caller(
        env: Env,
        caller: Address,
        authorised: bool,
    ) -> Result<(), Error> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&StorageKey::Admin)
            .ok_or(Error::NotInitialized)?;
        admin.require_auth();
        env.storage()
            .persistent()
            .set(&StorageKey::AuthorisedCaller(caller), &authorised);
        Ok(())
    }

    /// Authorised-caller only: compute and persist an issuer's credit score.
    pub fn compute_credit_score(
        env: Env,
        caller: Address,
        issuer: Address,
        factors: IssuerFactors,
    ) -> Result<CreditScore, Error> {
        require_authorised(&env, &caller)?;

        let score = compute_score_internal(&factors);
        let tier = tier_from_score(score);

        let old_tier: CreditTier = env
            .storage()
            .persistent()
            .get(&StorageKey::Score(issuer.clone()))
            .map(|s: CreditScore| s.tier)
            .unwrap_or(CreditTier::C);

        let credit = CreditScore {
            score,
            tier,
            factors: factors.clone(),
        };

        env.storage()
            .persistent()
            .set(&StorageKey::Score(issuer.clone()), &credit);

        events::credit_score_updated(&env, &issuer, old_tier, tier);

        Ok(credit)
    }

    /// Returns the current credit score and factor breakdown for an issuer.
    pub fn get_credit_score(env: Env, issuer: Address) -> Result<CreditScore, Error> {
        env.storage()
            .persistent()
            .get(&StorageKey::Score(issuer))
            .ok_or(Error::IssuerNotFound)
    }
}
