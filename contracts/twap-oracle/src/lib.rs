#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, Address, BytesN, Env, Symbol, Vec,
};

pub const HOUR: u64 = 3_600;
pub const FOUR_HOURS: u64 = 14_400;
pub const DAY: u64 = 86_400;

#[derive(Clone)]
#[contracttype]
enum DataKey {
    Admin,
    Observations(BytesN<32>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[contracttype]
pub struct Observation {
    pub price: i128,
    pub recorded_at: u64,
}

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Error {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    InvalidPrice = 3,
    InvalidWindow = 4,
    InsufficientObservations = 5,
    Overflow = 6,
}

#[contract]
pub struct TwapOracle;

#[contractimpl]
impl TwapOracle {
    pub fn initialize(env: Env, admin: Address) -> Result<(), Error> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(Error::AlreadyInitialized);
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        Ok(())
    }
    pub fn record_price(env: Env, invoice_id: BytesN<32>, price: i128) -> Result<(), Error> {
        if price <= 0 {
            return Err(Error::InvalidPrice);
        }
        let now = env.ledger().timestamp();
        let key = DataKey::Observations(invoice_id.clone());
        let old: Vec<Observation> = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or(Vec::new(&env));
        let mut kept = Vec::new(&env);
        for item in old.iter() {
            if now.saturating_sub(item.recorded_at) <= DAY {
                kept.push_back(item);
            }
        }
        kept.push_back(Observation {
            price,
            recorded_at: now,
        });
        env.storage().persistent().set(&key, &kept);
        env.events()
            .publish((Symbol::new(&env, "TWAPUpdated"), invoice_id), (price, now));
        Ok(())
    }
    pub fn get_twap(env: Env, invoice_id: BytesN<32>, window_seconds: u64) -> Result<i128, Error> {
        if window_seconds != HOUR && window_seconds != FOUR_HOURS && window_seconds != DAY {
            return Err(Error::InvalidWindow);
        }
        let now = env.ledger().timestamp();
        let start = now.saturating_sub(window_seconds);
        let items: Vec<Observation> = env
            .storage()
            .persistent()
            .get(&DataKey::Observations(invoice_id))
            .unwrap_or(Vec::new(&env));
        if items.len() < 2 {
            return Err(Error::InsufficientObservations);
        }
        let mut weighted = 0i128;
        let mut duration = 0u64;
        let mut previous: Option<Observation> = None;
        for item in items.iter() {
            if item.recorded_at > now {
                break;
            }
            if let Some(prev) = previous {
                let from = prev.recorded_at.max(start);
                let to = item.recorded_at.min(now);
                if to > from {
                    weighted = weighted
                        .checked_add(
                            prev.price
                                .checked_mul((to - from) as i128)
                                .ok_or(Error::Overflow)?,
                        )
                        .ok_or(Error::Overflow)?;
                    duration += to - from;
                }
            }
            previous = Some(item);
        }
        if let Some(prev) = previous {
            let from = prev.recorded_at.max(start);
            if now > from {
                weighted = weighted
                    .checked_add(
                        prev.price
                            .checked_mul((now - from) as i128)
                            .ok_or(Error::Overflow)?,
                    )
                    .ok_or(Error::Overflow)?;
                duration += now - from;
            }
        }
        if duration == 0 {
            return Err(Error::InsufficientObservations);
        }
        Ok(weighted / duration as i128)
    }
    pub fn get_observations(env: Env, invoice_id: BytesN<32>) -> Vec<Observation> {
        env.storage()
            .persistent()
            .get(&DataKey::Observations(invoice_id))
            .unwrap_or(Vec::new(&env))
    }
}
