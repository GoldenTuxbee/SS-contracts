#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, token, Address, BytesN, Env, Symbol,
};
const YEAR: u64 = 365 * 24 * 60 * 60;

#[derive(Clone)]
#[contracttype]
enum DataKey {
    Admin,
    Invoice(BytesN<32>),
    Pool(BytesN<32>),
    Stake(u64),
    NextStake,
}
#[derive(Clone)]
#[contracttype]
pub struct Invoice {
    pub issuer: Address,
    pub token: Address,
    pub bonus_bps: u32,
}
#[derive(Clone)]
#[contracttype]
pub struct Stake {
    pub wallet: Address,
    pub invoice_id: BytesN<32>,
    pub amount: i128,
    pub started_at: u64,
    pub unlock_at: u64,
}

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Error {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    InvalidAmount = 3,
    InvalidLock = 4,
    InvoiceNotFound = 5,
    StakeNotFound = 6,
    StillLocked = 7,
    InsufficientPool = 8,
    Overflow = 9,
}

#[contract]
pub struct StakingRewards;

#[contractimpl]
impl StakingRewards {
    pub fn initialize(env: Env, admin: Address) -> Result<(), Error> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(Error::AlreadyInitialized);
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::NextStake, &0u64);
        Ok(())
    }
    pub fn register_invoice(
        env: Env,
        invoice_id: BytesN<32>,
        invoice: Invoice,
    ) -> Result<(), Error> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)?;
        admin.require_auth();
        if invoice.bonus_bps > 10_000 {
            return Err(Error::InvalidAmount);
        }
        env.storage()
            .persistent()
            .set(&DataKey::Invoice(invoice_id), &invoice);
        Ok(())
    }
    pub fn fund_staking_pool(env: Env, invoice_id: BytesN<32>, amount: i128) -> Result<(), Error> {
        let invoice: Invoice = env
            .storage()
            .persistent()
            .get(&DataKey::Invoice(invoice_id.clone()))
            .ok_or(Error::InvoiceNotFound)?;
        invoice.issuer.require_auth();
        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }
        token::Client::new(&env, &invoice.token).transfer(
            &invoice.issuer,
            &env.current_contract_address(),
            &amount,
        );
        let pool: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::Pool(invoice_id.clone()))
            .unwrap_or(0);
        env.storage().persistent().set(
            &DataKey::Pool(invoice_id),
            &pool.checked_add(amount).ok_or(Error::Overflow)?,
        );
        Ok(())
    }
    pub fn stake_fractions(
        env: Env,
        wallet: Address,
        invoice_id: BytesN<32>,
        amount: i128,
        lock_period: u64,
    ) -> Result<u64, Error> {
        let invoice: Invoice = env
            .storage()
            .persistent()
            .get(&DataKey::Invoice(invoice_id.clone()))
            .ok_or(Error::InvoiceNotFound)?;
        wallet.require_auth();
        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }
        if lock_period == 0 {
            return Err(Error::InvalidLock);
        }
        token::Client::new(&env, &invoice.token).transfer(
            &wallet,
            &env.current_contract_address(),
            &amount,
        );
        let id: u64 = env
            .storage()
            .instance()
            .get(&DataKey::NextStake)
            .unwrap_or(0);
        env.storage().instance().set(&DataKey::NextStake, &(id + 1));
        let now = env.ledger().timestamp();
        env.storage().persistent().set(
            &DataKey::Stake(id),
            &Stake {
                wallet,
                invoice_id: invoice_id.clone(),
                amount,
                started_at: now,
                unlock_at: now + lock_period,
            },
        );
        env.events().publish(
            (Symbol::new(&env, "StakeCreated"), id),
            (invoice_id, amount, now + lock_period),
        );
        Ok(id)
    }
    pub fn get_stake(env: Env, stake_id: u64) -> Result<Stake, Error> {
        env.storage()
            .persistent()
            .get(&DataKey::Stake(stake_id))
            .ok_or(Error::StakeNotFound)
    }
    pub fn unstake_fractions(env: Env, stake_id: u64) -> Result<i128, Error> {
        let stake: Stake = Self::get_stake(env.clone(), stake_id)?;
        if env.ledger().timestamp() < stake.unlock_at {
            return Err(Error::StillLocked);
        }
        let invoice: Invoice = env
            .storage()
            .persistent()
            .get(&DataKey::Invoice(stake.invoice_id.clone()))
            .ok_or(Error::InvoiceNotFound)?;
        let reward = stake
            .amount
            .checked_mul(invoice.bonus_bps as i128)
            .ok_or(Error::Overflow)?
            .checked_mul(stake.unlock_at - stake.started_at)
            .ok_or(Error::Overflow)?
            .checked_div(10_000 * YEAR as i128)
            .ok_or(Error::Overflow)?;
        let pool: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::Pool(stake.invoice_id.clone()))
            .unwrap_or(0);
        if reward > pool {
            return Err(Error::InsufficientPool);
        }
        let total = stake.amount.checked_add(reward).ok_or(Error::Overflow)?;
        token::Client::new(&env, &invoice.token).transfer(
            &env.current_contract_address(),
            &stake.wallet,
            &total,
        );
        env.storage()
            .persistent()
            .set(&DataKey::Pool(stake.invoice_id), &(pool - reward));
        env.storage().persistent().remove(&DataKey::Stake(stake_id));
        env.events().publish(
            (Symbol::new(&env, "StakeReleased"), stake_id),
            (stake.amount, reward),
        );
        Ok(reward)
    }
}
