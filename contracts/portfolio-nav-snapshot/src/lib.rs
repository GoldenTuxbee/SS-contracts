#![no_std]

mod errors;
mod events;
mod types;

pub use errors::Error;
pub use types::{DataKey, NavRecord};

use soroban_sdk::{contract, contractimpl, Address, BytesN, Env, Vec};

const SECONDS_PER_DAY: u64 = 86400;

#[contract]
pub struct PortfolioNavSnapshotContract;

#[contractimpl]
impl PortfolioNavSnapshotContract {
    pub fn initialize(env: Env, admin: Address, operator: Address) -> Result<(), Error> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(Error::AlreadyInitialized);
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Operator, &operator);
        Ok(())
    }

    pub fn set_invoice_price(
        env: Env,
        invoice_id: BytesN<32>,
        price: i128,
    ) -> Result<(), Error> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)?;
        let operator: Address = env
            .storage()
            .instance()
            .get(&DataKey::Operator)
            .ok_or(Error::NotInitialized)?;

        if admin.require_auth().is_err() && operator.require_auth().is_err() {
            return Err(Error::Unauthorized);
        }

        if price < 0 {
            return Err(Error::InvalidAmount);
        }

        env.storage()
            .persistent()
            .set(&DataKey::InvoicePrice(invoice_id), &price);
        Ok(())
    }

    pub fn set_investor_balance(
        env: Env,
        wallet: Address,
        invoice_id: BytesN<32>,
        balance: i128,
    ) -> Result<(), Error> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)?;
        let operator: Address = env
            .storage()
            .instance()
            .get(&DataKey::Operator)
            .ok_or(Error::NotInitialized)?;

        if admin.require_auth().is_err() && operator.require_auth().is_err() {
            return Err(Error::Unauthorized);
        }

        if balance < 0 {
            return Err(Error::InvalidAmount);
        }

        let held_key = DataKey::HeldInvoices(wallet.clone());
        let mut held: Vec<BytesN<32>> = env
            .storage()
            .persistent()
            .get(&held_key)
            .unwrap_or(Vec::new(&env));

        let mut exists = false;
        for id in held.iter() {
            if id == invoice_id {
                exists = true;
                break;
            }
        }
        if !exists {
            held.push_back(invoice_id.clone());
            env.storage().persistent().set(&held_key, &held);
        }

        env.storage()
            .persistent()
            .set(&DataKey::FractionBalance(wallet, invoice_id), &balance);
        Ok(())
    }

    pub fn take_portfolio_snapshot(env: Env, wallet: Address) -> Result<i128, Error> {
        let operator: Address = env
            .storage()
            .instance()
            .get(&DataKey::Operator)
            .ok_or(Error::NotInitialized)?;
        operator.require_auth();

        let now = env.ledger().timestamp();
        let date = (now / SECONDS_PER_DAY) * SECONDS_PER_DAY;

        let snapshot_key = DataKey::Snapshot(wallet.clone(), date);
        if env.storage().persistent().has(&snapshot_key) {
            return Err(Error::DuplicateSnapshot);
        }

        let held_key = DataKey::HeldInvoices(wallet.clone());
        let held: Vec<BytesN<32>> = env
            .storage()
            .persistent()
            .get(&held_key)
            .unwrap_or(Vec::new(&env));

        let mut total_nav: i128 = 0;
        for invoice_id in held.iter() {
            let balance: i128 = env
                .storage()
                .persistent()
                .get(&DataKey::FractionBalance(wallet.clone(), invoice_id.clone()))
                .unwrap_or(0);
            let price: i128 = env
                .storage()
                .persistent()
                .get(&DataKey::InvoicePrice(invoice_id))
                .unwrap_or(0);

            let value = balance.checked_mul(price).unwrap_or(0);
            total_nav = total_nav.checked_add(value).unwrap_or(total_nav);
        }

        env.storage().persistent().set(&snapshot_key, &total_nav);

        let dates_key = DataKey::SnapshotDates(wallet.clone());
        let mut dates: Vec<u64> = env
            .storage()
            .persistent()
            .get(&dates_key)
            .unwrap_or(Vec::new(&env));
        dates.push_back(date);
        env.storage().persistent().set(&dates_key, &dates);

        events::portfolio_snapshot_taken(&env, &wallet, date, total_nav);

        Ok(total_nav)
    }

    pub fn get_portfolio_nav(env: Env, wallet: Address, date: u64) -> Result<i128, Error> {
        let snapshot_key = DataKey::Snapshot(wallet, date);
        env.storage()
            .persistent()
            .get(&snapshot_key)
            .ok_or(Error::SnapshotNotFound)
    }

    pub fn get_portfolio_nav_range(
        env: Env,
        wallet: Address,
        from_date: u64,
        to_date: u64,
    ) -> Result<Vec<NavRecord>, Error> {
        if from_date > to_date {
            return Err(Error::InvalidDateRange);
        }

        let dates_key = DataKey::SnapshotDates(wallet.clone());
        let dates: Vec<u64> = env
            .storage()
            .persistent()
            .get(&dates_key)
            .unwrap_or(Vec::new(&env));

        let mut records = Vec::new(&env);
        for d in dates.iter() {
            if d >= from_date && d <= to_date {
                if let Some(nav) = env
                    .storage()
                    .persistent()
                    .get::<DataKey, i128>(&DataKey::Snapshot(wallet.clone(), d))
                {
                    records.push_back(NavRecord {
                        date: d,
                        nav_amount: nav,
                    });
                }
            }
        }

        Ok(records)
    }
}
