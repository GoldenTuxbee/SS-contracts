//! Investor cool-down contract between secondary market purchases (issue #656).
//!
//! Enforces a per-wallet, per-invoice cooldown period between secondary market
//! purchases to reduce wash trading and artificial price inflation.

#![no_std]

mod errors;
mod events;
mod types;

#[cfg(test)]
mod test;

pub use errors::Error;
pub use types::StorageKey;

use soroban_sdk::{contract, contractimpl, Address, BytesN, Env};

fn require_admin(env: &Env) -> Result<Address, Error> {
    let admin: Address = env
        .storage()
        .instance()
        .get(&StorageKey::Admin)
        .ok_or(Error::NotInitialized)?;
    admin.require_auth();
    Ok(admin)
}

#[contract]
pub struct SecondaryCooldown;

#[contractimpl]
impl SecondaryCooldown {
    pub fn initialize(env: Env, admin: Address) -> Result<(), Error> {
        if env.storage().instance().has(&StorageKey::Admin) {
            return Err(Error::AlreadyInitialized);
        }
        admin.require_auth();
        env.storage().instance().set(&StorageKey::Admin, &admin);
        Ok(())
    }

    /// Admin-only: set the cooldown period in seconds for an invoice.
    pub fn set_secondary_cooldown(
        env: Env,
        invoice_id: BytesN<32>,
        period_seconds: u64,
    ) -> Result<(), Error> {
        require_admin(&env)?;
        env.storage()
            .persistent()
            .set(&StorageKey::CooldownPeriod(invoice_id), &period_seconds);
        Ok(())
    }

    /// Record a successful secondary market buy. Returns CooldownActive if
    /// within the cooldown window.
    pub fn record_buy(
        env: Env,
        buyer: Address,
        invoice_id: BytesN<32>,
    ) -> Result<(), Error> {
        buyer.require_auth();

        let now = env.ledger().timestamp();
        let period: u64 = env
            .storage()
            .persistent()
            .get(&StorageKey::CooldownPeriod(invoice_id.clone()))
            .unwrap_or(0);

        if period > 0 {
            let last_buy: u64 = env
                .storage()
                .persistent()
                .get(&StorageKey::LastBuy(buyer.clone(), invoice_id.clone()))
                .unwrap_or(0);

            if last_buy > 0 {
                let elapsed = now.saturating_sub(last_buy);
                if elapsed < period {
                    let seconds_remaining = period - elapsed;
                    events::cooldown_active(&env, &buyer, &invoice_id, seconds_remaining);
                    return Err(Error::CooldownActive);
                }
            }
        }

        // Record timestamp for this wallet/invoice pair.
        env.storage()
            .persistent()
            .set(&StorageKey::LastBuy(buyer.clone(), invoice_id.clone()), &now);

        events::buy_recorded(&env, &buyer, &invoice_id, now);
        Ok(())
    }

    /// Returns remaining cooldown seconds for a wallet/invoice pair. 0 if none.
    pub fn get_secondary_cooldown_remaining(
        env: Env,
        wallet: Address,
        invoice_id: BytesN<32>,
    ) -> u64 {
        let now = env.ledger().timestamp();
        let period: u64 = env
            .storage()
            .persistent()
            .get(&StorageKey::CooldownPeriod(invoice_id.clone()))
            .unwrap_or(0);

        if period == 0 {
            return 0;
        }

        let last_buy: u64 = env
            .storage()
            .persistent()
            .get(&StorageKey::LastBuy(wallet, invoice_id))
            .unwrap_or(0);

        if last_buy == 0 {
            return 0;
        }

        let elapsed = now.saturating_sub(last_buy);
        if elapsed >= period {
            0
        } else {
            period - elapsed
        }
    }
}
