#![no_std]

mod errors;
mod events;
mod types;

pub use errors::Error;
pub use types::{DataKey, PriceObservation, StabilisationStatus};

use soroban_sdk::{contract, contractimpl, Address, BytesN, Env, Vec};

const BPS_DENOMINATOR: i128 = 10_000;
const DROP_THRESHOLD_BPS: u32 = 2_000; // 20%
const ONE_HOUR_SECONDS: u64 = 3600;

#[contract]
pub struct PriceStabilisationContract;

#[contractimpl]
impl PriceStabilisationContract {
    pub fn initialize(env: Env, admin: Address, operator: Address) -> Result<(), Error> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(Error::AlreadyInitialized);
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Operator, &operator);
        Ok(())
    }

    pub fn set_floor_price(
        env: Env,
        invoice_id: BytesN<32>,
        floor_price: i128,
    ) -> Result<(), Error> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)?;
        admin.require_auth();
        if floor_price <= 0 {
            return Err(Error::InvalidPrice);
        }
        env.storage()
            .persistent()
            .set(&DataKey::FloorPrice(invoice_id), &floor_price);
        Ok(())
    }

    pub fn fund_stabilisation_reserve(
        env: Env,
        invoice_id: BytesN<32>,
        amount: i128,
    ) -> Result<(), Error> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)?;
        admin.require_auth();
        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }
        let current_reserve: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::Reserve(invoice_id.clone()))
            .unwrap_or(0);
        let new_reserve = current_reserve
            .checked_add(amount)
            .ok_or(Error::InvalidAmount)?;
        env.storage()
            .persistent()
            .set(&DataKey::Reserve(invoice_id), &new_reserve);
        Ok(())
    }

    pub fn record_price(
        env: Env,
        invoice_id: BytesN<32>,
        price: i128,
    ) -> Result<(), Error> {
        let operator: Address = env
            .storage()
            .instance()
            .get(&DataKey::Operator)
            .ok_or(Error::NotInitialized)?;
        operator.require_auth();
        if price <= 0 {
            return Err(Error::InvalidPrice);
        }
        let now = env.ledger().timestamp();
        let key = DataKey::PriceObservation(invoice_id);
        let mut history: Vec<PriceObservation> = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or(Vec::new(&env));
        history.push_back(PriceObservation { price, timestamp: now });
        if history.len() > 20 {
            history.remove(0);
        }
        env.storage().persistent().set(&key, &history);
        Ok(())
    }

    pub fn trigger_stabilisation(
        env: Env,
        invoice_id: BytesN<32>,
    ) -> Result<u32, Error> {
        let operator: Address = env
            .storage()
            .instance()
            .get(&DataKey::Operator)
            .ok_or(Error::NotInitialized)?;
        operator.require_auth();

        let key = DataKey::PriceObservation(invoice_id.clone());
        let history: Vec<PriceObservation> = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(Error::InvoiceNotFound)?;

        if history.len() < 2 {
            return Err(Error::ThresholdNotMet);
        }

        let now = env.ledger().timestamp();
        let current_obs = history.get(history.len() - 1).unwrap();
        let mut baseline_price = 0i128;

        for i in 0..history.len() - 1 {
            let obs = history.get(i).unwrap();
            if now.saturating_sub(obs.timestamp) <= ONE_HOUR_SECONDS && obs.price > current_obs.price {
                if obs.price > baseline_price {
                    baseline_price = obs.price;
                }
            }
        }

        if baseline_price <= 0 {
            return Err(Error::ThresholdNotMet);
        }

        let price_drop = baseline_price - current_obs.price;
        let drop_bps = ((price_drop as i128 * BPS_DENOMINATOR) / baseline_price) as u32;

        if drop_bps <= DROP_THRESHOLD_BPS {
            return Err(Error::ThresholdNotMet);
        }

        let floor_price: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::FloorPrice(invoice_id.clone()))
            .ok_or(Error::InvalidPrice)?;

        let reserve_key = DataKey::Reserve(invoice_id.clone());
        let reserve: i128 = env
            .storage()
            .persistent()
            .get(&reserve_key)
            .unwrap_or(0);

        if reserve < floor_price {
            return Err(Error::InsufficientReserve);
        }

        let quantity = reserve / floor_price;
        if quantity <= 0 {
            return Err(Error::InsufficientReserve);
        }

        let order_cost = quantity * floor_price;
        let remaining_reserve = reserve - order_cost;
        env.storage().persistent().set(&reserve_key, &remaining_reserve);
        env.storage()
            .persistent()
            .set(&DataKey::IsActive(invoice_id.clone()), &true);
        env.storage()
            .persistent()
            .set(&DataKey::LastTriggered(invoice_id.clone()), &now);

        let order_key = DataKey::NextOrderId(invoice_id.clone());
        let order_id: u32 = env.storage().persistent().get(&order_key).unwrap_or(1);
        env.storage().persistent().set(&order_key, &(order_id + 1));

        events::stabilisation_triggered(&env, &invoice_id, drop_bps, now);
        events::stabilisation_order_placed(&env, &invoice_id, order_id, floor_price, quantity);

        Ok(order_id)
    }

    pub fn get_stabilisation_status(
        env: Env,
        invoice_id: BytesN<32>,
    ) -> StabilisationStatus {
        let reserve_balance: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::Reserve(invoice_id.clone()))
            .unwrap_or(0);
        let is_active: bool = env
            .storage()
            .persistent()
            .get(&DataKey::IsActive(invoice_id.clone()))
            .unwrap_or(false);
        let floor_price: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::FloorPrice(invoice_id.clone()))
            .unwrap_or(0);
        let last_triggered_at: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::LastTriggered(invoice_id.clone()))
            .unwrap_or(0);

        let key = DataKey::PriceObservation(invoice_id);
        let last_price = if let Some(history) = env.storage().persistent().get::<DataKey, Vec<PriceObservation>>(&key) {
            if !history.is_empty() {
                history.get(history.len() - 1).unwrap().price
            } else {
                0
            }
        } else {
            0
        };

        StabilisationStatus {
            reserve_balance,
            is_active,
            floor_price,
            last_price,
            last_triggered_at,
        }
    }
}
