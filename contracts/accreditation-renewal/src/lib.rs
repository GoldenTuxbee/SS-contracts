#![no_std]

mod errors;
mod events;
mod types;

pub use errors::Error;
pub use types::{DataKey, ReminderRecord};

use soroban_sdk::{contract, contractimpl, Address, Env, Vec};

const SECONDS_PER_DAY: u64 = 86400;

#[contract]
pub struct AccreditationRenewalContract;

#[contractimpl]
impl AccreditationRenewalContract {
    pub fn initialize(env: Env, admin: Address, operator: Address) -> Result<(), Error> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(Error::AlreadyInitialized);
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Operator, &operator);
        Ok(())
    }

    pub fn set_accreditation_expiry(
        env: Env,
        wallet: Address,
        expiry_timestamp: u64,
    ) -> Result<(), Error> {
        let is_admin = env
            .storage()
            .instance()
            .get::<DataKey, Address>(&DataKey::Admin)
            .map(|a| a == wallet)
            .unwrap_or(false);
        let admin_addr: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)?;
        let operator_addr: Address = env
            .storage()
            .instance()
            .get(&DataKey::Operator)
            .ok_or(Error::NotInitialized)?;

        if !is_admin {
            // Require auth from either admin or operator
            if admin_addr.require_auth().is_err() && operator_addr.require_auth().is_err() {
                return Err(Error::Unauthorized);
            }
        }

        if expiry_timestamp <= env.ledger().timestamp() {
            return Err(Error::InvalidExpiry);
        }

        let key = DataKey::Expiry(wallet.clone());
        env.storage().persistent().set(&key, &expiry_timestamp);

        let list_key = DataKey::WalletList;
        let mut list: Vec<Address> = env
            .storage()
            .persistent()
            .get(&list_key)
            .unwrap_or(Vec::new(&env));
        let mut exists = false;
        for w in list.iter() {
            if w == wallet {
                exists = true;
                break;
            }
        }
        if !exists {
            list.push_back(wallet);
            env.storage().persistent().set(&list_key, &list);
        }
        Ok(())
    }

    pub fn check_accreditation_reminders(env: Env) -> Result<u32, Error> {
        let operator: Address = env
            .storage()
            .instance()
            .get(&DataKey::Operator)
            .ok_or(Error::NotInitialized)?;
        operator.require_auth();

        let now = env.ledger().timestamp();
        let list_key = DataKey::WalletList;
        let list: Vec<Address> = env
            .storage()
            .persistent()
            .get(&list_key)
            .unwrap_or(Vec::new(&env));

        let intervals: [u32; 3] = [60, 30, 7];
        let mut emitted_count = 0u32;

        for wallet in list.iter() {
            let expiry_key = DataKey::Expiry(wallet.clone());
            if let Some(expiry_timestamp) = env.storage().persistent().get::<DataKey, u64>(&expiry_key) {
                if expiry_timestamp > now {
                    let days_remaining = (expiry_timestamp - now) / SECONDS_PER_DAY;
                    let log_key = DataKey::ReminderLog(wallet.clone());
                    let mut log: Vec<ReminderRecord> = env
                        .storage()
                        .persistent()
                        .get(&log_key)
                        .unwrap_or(Vec::new(&env));

                    for &interval in intervals.iter() {
                        if days_remaining <= interval as u64 {
                            let mut already_emitted = false;
                            for record in log.iter() {
                                if record.interval_days == interval {
                                    already_emitted = true;
                                    break;
                                }
                            }
                            if !already_emitted {
                                events::accreditation_renewal_reminder(
                                    &env,
                                    &wallet,
                                    days_remaining,
                                    expiry_timestamp,
                                );
                                log.push_back(ReminderRecord {
                                    interval_days: interval,
                                    timestamp: now,
                                });
                                emitted_count += 1;
                            }
                        }
                    }
                    env.storage().persistent().set(&log_key, &log);
                }
            }
        }

        Ok(emitted_count)
    }

    pub fn get_reminder_log(env: Env, wallet: Address) -> Vec<ReminderRecord> {
        let log_key = DataKey::ReminderLog(wallet);
        env.storage()
            .persistent()
            .get(&log_key)
            .unwrap_or(Vec::new(&env))
    }

    pub fn get_accreditation_expiry(env: Env, wallet: Address) -> Result<u64, Error> {
        let key = DataKey::Expiry(wallet);
        env.storage()
            .persistent()
            .get(&key)
            .ok_or(Error::WalletNotFound)
    }
}
