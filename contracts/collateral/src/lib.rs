#![no_std]

mod errors;
mod events;
mod types;

pub use types::{CollateralState, CollateralStatus, Error};
use types::{CollateralRecord, StorageKey};

use soroban_sdk::{contract, contractimpl, token, Address, BytesN, Env};

fn admin(env: &Env) -> Result<Address, Error> {
    env.storage()
        .instance()
        .get(&StorageKey::Admin)
        .ok_or(Error::NotInitialized)
}

fn collateral(env: &Env, invoice_id: &BytesN<32>) -> Result<CollateralRecord, Error> {
    env.storage()
        .persistent()
        .get(&StorageKey::Collateral(invoice_id.clone()))
        .ok_or(Error::InvoiceNotFound)
}

fn save_collateral(env: &Env, data: &CollateralRecord) {
    env.storage()
        .persistent()
        .set(&StorageKey::Collateral(data.invoice_id.clone()), data);
}

#[contract]
pub struct Collateral;

#[contractimpl]
impl Collateral {
    pub fn initialize(
        env: Env,
        admin: Address,
        token: Address,
        compensation_pool: Address,
    ) -> Result<(), Error> {
        if env.storage().instance().has(&StorageKey::Admin) {
            return Err(Error::AlreadyInitialized);
        }
        admin.require_auth();
        env.storage().instance().set(&StorageKey::Admin, &admin);
        env.storage().instance().set(&StorageKey::Token, &token);
        env.storage()
            .instance()
            .set(&StorageKey::CompensationPool, &compensation_pool);
        Ok(())
    }

    pub fn deposit_collateral(
        env: Env,
        issuer: Address,
        invoice_id: BytesN<32>,
        amount: i128,
    ) -> Result<(), Error> {
        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }
        if env
            .storage()
            .persistent()
            .has(&StorageKey::Collateral(invoice_id.clone()))
        {
            return Err(Error::AlreadyLocked);
        }
        issuer.require_auth();

        let token: Address = env.storage().instance().get(&StorageKey::Token).ok_or(Error::NotInitialized)?;
        let client = token::Client::new(&env, &token);
        if client.balance(&issuer) < amount {
            return Err(Error::InsufficientBalance);
        }
        client.transfer(&issuer, &env.current_contract_address(), &amount);

        let record = CollateralRecord {
            invoice_id: invoice_id.clone(),
            issuer: issuer.clone(),
            amount,
            state: CollateralState::Locked,
            deposited_at: env.ledger().timestamp(),
        };
        save_collateral(&env, &record);
        events::collateral_deposited(&env, &invoice_id, &issuer, amount);
        Ok(())
    }

    pub fn release_collateral(env: Env, invoice_id: BytesN<32>) -> Result<(), Error> {
        let admin_addr = admin(&env)?;
        admin_addr.require_auth();

        let mut data = collateral(&env, &invoice_id)?;
        if data.state != CollateralState::Locked {
            return Err(Error::NotLocked);
        }

        let token: Address = env.storage().instance().get(&StorageKey::Token).ok_or(Error::NotInitialized)?;
        let client = token::Client::new(&env, &token);
        client
            .transfer(&env.current_contract_address(), &data.issuer, &data.amount)
            ;


        data.state = CollateralState::Released;
        save_collateral(&env, &data);
        events::collateral_released(&env, &invoice_id, &data.issuer, data.amount);
        Ok(())
    }

    pub fn forfeit_collateral(env: Env, invoice_id: BytesN<32>) -> Result<(), Error> {
        let admin_addr = admin(&env)?;
        admin_addr.require_auth();

        let mut data = collateral(&env, &invoice_id)?;
        if data.state != CollateralState::Locked {
            return Err(Error::NotLocked);
        }

        let pool: Address = env
            .storage()
            .instance()
            .get(&StorageKey::CompensationPool)
            .ok_or(Error::NotInitialized)?;
        let token: Address = env.storage().instance().get(&StorageKey::Token).ok_or(Error::NotInitialized)?;
        let client = token::Client::new(&env, &token);
        client
            .transfer(&env.current_contract_address(), &pool, &data.amount)
            ;


        data.state = CollateralState::Forfeited;
        save_collateral(&env, &data);
        events::collateral_forfeited(&env, &invoice_id, &data.issuer, data.amount);
        Ok(())
    }

    pub fn get_collateral_status(
        env: Env,
        invoice_id: BytesN<32>,
    ) -> Result<CollateralStatus, Error> {
        let data = collateral(&env, &invoice_id)?;
        Ok(CollateralStatus {
            invoice_id: data.invoice_id,
            issuer: data.issuer,
            amount: data.amount,
            state: data.state,
        })
    }
}
