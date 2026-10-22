//! Investor portfolio health score contract (issue #639).
//!
//! Computes a health score between 0 and 100 for the portfolio of an
//! investor from four risk factors: the concentration of the largest
//! position, the average maturity of the portfolio, the number of issuers and
//! the defaulted exposure of the portfolio.
//!
//! The operator reports the portfolio events of a wallet - an investment, a
//! settlement or a default - and every event recomputes the score, stores it
//! and emits a `HealthScoreUpdated` event with the score before and after the
//! recomputation.
//!
//! The score starts at 100 and every factor subtracts a penalty:
//!
//! - concentration: up to 30 points for a portfolio in a single position,
//! - diversification: 5 points per issuer missing below three issuers,
//! - maturity: up to 10 points for an average maturity of a year or more,
//! - defaults: up to 50 points for a fully defaulted portfolio.
//!
//! The penalties add up to 100 points, so the score is always between 0 and
//! 100. A portfolio without positions has no risk and scores 100.

#![no_std]

mod errors;
mod events;
mod storage;
mod types;

#[cfg(test)]
mod test;

pub use errors::Error;
pub use types::{DataKey, HealthFactors, HealthScore, Position, PositionStatus};

use soroban_sdk::{contract, contractimpl, Address, Env, Symbol, Vec};

/// Seconds in a day.
const SECONDS_PER_DAY: u64 = 86_400;

/// Basis point denominator (10_000 = 100%).
const BPS_DENOMINATOR: u32 = 10_000;

/// Points a fully concentrated portfolio loses.
const CONCENTRATION_WEIGHT: u32 = 30;

/// Points lost per issuer missing below the diversification target.
const DIVERSIFICATION_WEIGHT: u32 = 5;

/// Number of issuers that removes the diversification penalty.
const DIVERSIFICATION_TARGET: u32 = 3;

/// Points lost for a portfolio with an average maturity of a year or more.
const MATURITY_WEIGHT: u32 = 10;

/// Average maturity in days that removes the maturity penalty.
const MATURITY_HORIZON_DAYS: u32 = 365;

/// Points a fully defaulted portfolio loses.
const DEFAULT_WEIGHT: u32 = 50;

/// Score of a portfolio without any risk.
const MAX_SCORE: u32 = 100;

#[contract]
pub struct InvestorPortfolioHealth;

#[contractimpl]
impl InvestorPortfolioHealth {
    /// Initialise the contract with the admin and the operator that reports
    /// the portfolio events.
    pub fn initialize(env: Env, admin: Address, operator: Address) -> Result<(), Error> {
        admin.require_auth();
        if storage::get_admin(&env).is_some() {
            return Err(Error::AlreadyInitialized);
        }
        storage::set_admin(&env, &admin);
        storage::set_operator(&env, &operator);
        Ok(())
    }

    /// Admin-only: rotate the operator that reports the portfolio events.
    pub fn set_operator(env: Env, admin: Address, operator: Address) -> Result<(), Error> {
        admin.require_auth();
        let stored_admin = storage::get_admin(&env).ok_or(Error::NotInitialized)?;
        if stored_admin != admin {
            return Err(Error::Unauthorized);
        }
        storage::set_operator(&env, &operator);
        Ok(())
    }

    /// Report an investment of a wallet and recompute its health score.
    ///
    /// The amount is added to the position of the invoice, a position that
    /// does not exist yet is opened.
    pub fn record_investment(
        env: Env,
        caller: Address,
        wallet: Address,
        invoice_id: Symbol,
        amount: i128,
        issuer: Address,
        maturity_timestamp: u64,
    ) -> Result<u32, Error> {
        Self::require_reporter(&env, &caller)?;
        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }

        let mut book = storage::get_book(&env, &wallet);
        let mut opened = true;
        for i in 0..book.len() {
            let mut position = book.get(i).unwrap();
            if position.invoice_id == invoice_id {
                position.amount = position.amount.saturating_add(amount);
                position.issuer = issuer.clone();
                position.maturity_timestamp = maturity_timestamp;
                book.set(i, position);
                opened = false;
                break;
            }
        }
        if opened {
            book.push_back(Position {
                invoice_id: invoice_id.clone(),
                amount,
                issuer: issuer.clone(),
                maturity_timestamp,
                status: PositionStatus::Active,
            });
        }
        storage::set_book(&env, &wallet, &book);

        Ok(Self::recompute(&env, &wallet)?.score)
    }

    /// Report the settlement of an invoice of a wallet and recompute its
    /// health score.
    ///
    /// The settled position leaves the portfolio, the capital was returned.
    pub fn record_settlement(
        env: Env,
        caller: Address,
        wallet: Address,
        invoice_id: Symbol,
    ) -> Result<u32, Error> {
        Self::require_reporter(&env, &caller)?;

        let mut book = storage::get_book(&env, &wallet);
        let mut settled = None;
        for i in 0..book.len() {
            if book.get(i).unwrap().invoice_id == invoice_id {
                settled = Some(i);
                break;
            }
        }
        let index = settled.ok_or(Error::PositionNotFound)?;
        let _ = book.remove(index);
        storage::set_book(&env, &wallet, &book);

        Ok(Self::recompute(&env, &wallet)?.score)
    }

    /// Report the default of an invoice of a wallet and recompute its health
    /// score.
    ///
    /// The position stays in the portfolio and counts as default exposure.
    pub fn record_default(
        env: Env,
        caller: Address,
        wallet: Address,
        invoice_id: Symbol,
    ) -> Result<u32, Error> {
        Self::require_reporter(&env, &caller)?;

        let mut book = storage::get_book(&env, &wallet);
        let mut defaulted = false;
        for i in 0..book.len() {
            let mut position = book.get(i).unwrap();
            if position.invoice_id == invoice_id {
                position.status = PositionStatus::Defaulted;
                book.set(i, position);
                defaulted = true;
                break;
            }
        }
        if !defaulted {
            return Err(Error::PositionNotFound);
        }
        storage::set_book(&env, &wallet, &book);

        Ok(Self::recompute(&env, &wallet)?.score)
    }

    /// Recompute the health score of a wallet and store it.
    ///
    /// The score is recomputed on every portfolio event, this entry point
    /// recomputes it on demand and returns the per factor breakdown.
    pub fn compute_health_score(env: Env, wallet: Address) -> Result<HealthScore, Error> {
        Self::recompute(&env, &wallet)
    }

    /// View: the latest computed health score of a wallet.
    ///
    /// A wallet without a computed score reports the score of its current
    /// portfolio with a computation timestamp of zero.
    pub fn get_health_score(env: Env, wallet: Address) -> Result<HealthScore, Error> {
        match storage::get_score(&env, &wallet) {
            Some(stored) => Ok(stored),
            None => {
                let factors = compute_factors(&env, &wallet, env.ledger().timestamp());
                let score = score_from_factors(&factors);
                Ok(HealthScore {
                    wallet,
                    score,
                    factors,
                    computed_at: 0,
                })
            }
        }
    }

    /// View: the positions a wallet holds.
    pub fn get_positions(env: Env, wallet: Address) -> Result<Vec<Position>, Error> {
        Ok(storage::get_book(&env, &wallet))
    }

    /// Check that the caller is allowed to report portfolio events.
    fn require_reporter(env: &Env, caller: &Address) -> Result<(), Error> {
        let admin = storage::get_admin(env).ok_or(Error::NotInitialized)?;
        let operator = storage::get_operator(env).ok_or(Error::NotInitialized)?;
        if caller != &admin && caller != &operator {
            return Err(Error::Unauthorized);
        }
        caller.require_auth();
        Ok(())
    }

    /// Recompute the health score of a wallet, store it and emit the update
    /// event with the score before and after the recomputation.
    fn recompute(env: &Env, wallet: &Address) -> Result<HealthScore, Error> {
        let now = env.ledger().timestamp();
        let factors = compute_factors(env, wallet, now);
        let score = score_from_factors(&factors);

        let old_score = storage::get_score(env, wallet).map_or(MAX_SCORE, |s| s.score);

        let health = HealthScore {
            wallet: wallet.clone(),
            score,
            factors,
            computed_at: now,
        };
        storage::set_score(env, wallet, &health);

        events::health_score_updated(env, wallet, old_score, score, now);

        Ok(health)
    }
}

/// The risk factors of the portfolio of a wallet.
fn compute_factors(env: &Env, wallet: &Address, now: u64) -> HealthFactors {
    let book = storage::get_book(env, wallet);

    let mut total_value: i128 = 0;
    let mut max_amount: i128 = 0;
    let mut defaulted_value: i128 = 0;
    let mut weighted_days: i128 = 0;
    let mut issuers: Vec<Address> = Vec::new(env);

    for position in book.iter() {
        let amount = position.amount;
        total_value = total_value.saturating_add(amount);
        if amount > max_amount {
            max_amount = amount;
        }
        if position.status == PositionStatus::Defaulted {
            defaulted_value = defaulted_value.saturating_add(amount);
        }
        let days = days_to_maturity(position.maturity_timestamp, now) as i128;
        weighted_days = weighted_days.saturating_add(amount.saturating_mul(days));
        if !issuers.contains(&position.issuer) {
            issuers.push_back(position.issuer.clone());
        }
    }

    let max_concentration_bps = ratio_bps(max_amount, total_value);
    let default_exposure_bps = ratio_bps(defaulted_value, total_value);
    let avg_days_to_maturity = if total_value > 0 {
        (weighted_days / total_value) as u32
    } else {
        0
    };

    HealthFactors {
        max_concentration_bps,
        avg_days_to_maturity,
        issuer_count: issuers.len(),
        default_exposure_bps,
        total_value,
        position_count: book.len(),
    }
}

/// The health score of a portfolio, between 0 and 100.
fn score_from_factors(factors: &HealthFactors) -> u32 {
    let concentration = penalty_bps(factors.max_concentration_bps, CONCENTRATION_WEIGHT);

    let issuers = factors.issuer_count.min(DIVERSIFICATION_TARGET);
    let diversification = if factors.position_count == 0 {
        0
    } else {
        (DIVERSIFICATION_TARGET - issuers) * DIVERSIFICATION_WEIGHT
    };

    let days = factors.avg_days_to_maturity.min(MATURITY_HORIZON_DAYS);
    let maturity = days * MATURITY_WEIGHT / MATURITY_HORIZON_DAYS;

    let defaulted = penalty_bps(factors.default_exposure_bps, DEFAULT_WEIGHT);

    let penalties = concentration + diversification + maturity + defaulted;
    MAX_SCORE.saturating_sub(penalties)
}

/// The points lost by a share of the portfolio expressed in basis points.
fn penalty_bps(bps: u32, weight: u32) -> u32 {
    (bps * weight) / BPS_DENOMINATOR
}

/// A share of the portfolio value in basis points.
fn ratio_bps(part: i128, total: i128) -> u32 {
    if total <= 0 || part <= 0 {
        return 0;
    }
    let bps = part.saturating_mul(BPS_DENOMINATOR as i128) / total;
    if bps > BPS_DENOMINATOR as i128 {
        BPS_DENOMINATOR
    } else {
        bps as u32
    }
}

/// Whole days left until an invoice matures.
fn days_to_maturity(maturity_timestamp: u64, now: u64) -> u64 {
    if maturity_timestamp <= now {
        0
    } else {
        (maturity_timestamp - now) / SECONDS_PER_DAY
    }
}
