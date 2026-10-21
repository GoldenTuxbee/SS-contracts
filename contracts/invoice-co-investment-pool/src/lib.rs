//! Invoice co-investment pool contract (issue #637).
//!
//! Aggregates the small contributions of retail investors into a single
//! pooled position on an invoice, so investors below the minimum ticket size
//! of the invoice can participate collectively.
//!
//! Flow:
//!   1. The admin calls `create_pool` and opens a pool for an invoice.
//!   2. Retail investors call `join_pool` with at least the minimum
//!      contribution of the pool.
//!   3. Once the target amount is met, the admin calls `execute_pool`, which
//!      funds the invoice as a single position held by the pool contract.
//!   4. An investor can call `exit_pool` before the execution to get its
//!      contribution back.

#![no_std]

mod errors;
mod events;
mod storage;
mod types;

#[cfg(test)]
mod test;

pub use errors::Error;
pub use types::{Pool, PoolStatus, StorageKey};

use soroban_sdk::{
    auth::{ContractContext, InvokerContractAuthEntry, SubContractInvocation},
    contract, contractimpl, symbol_short, token, Address, Env, IntoVal, Symbol,
};

#[contract]
pub struct InvoiceCoInvestmentPool;

#[contractimpl]
impl InvoiceCoInvestmentPool {
    /// Initialise the contract with the admin, the payment token and the
    /// invoice escrow the pools fund.
    pub fn initialize(
        env: Env,
        admin: Address,
        token: Address,
        escrow: Address,
    ) -> Result<(), Error> {
        admin.require_auth();
        if storage::get_admin(&env).is_some() {
            return Err(Error::AlreadyInitialized);
        }
        storage::set_admin(&env, &admin);
        storage::set_token(&env, &token);
        storage::set_escrow(&env, &escrow);
        Ok(())
    }

    /// Admin-only: open a co-investment pool for an invoice.
    ///
    /// The pool raises `target_amount` for the invoice, every contribution is
    /// at least `min_contribution` so that the pool aggregates the investors
    /// that cannot enter the invoice on their own.
    pub fn create_pool(
        env: Env,
        invoice_id: Symbol,
        target_amount: i128,
        min_contribution: i128,
    ) -> Result<u32, Error> {
        let admin = storage::get_admin(&env).ok_or(Error::NotInitialized)?;
        admin.require_auth();

        if target_amount <= 0 || min_contribution <= 0 || min_contribution > target_amount {
            return Err(Error::InvalidAmount);
        }

        let pool_id = storage::next_pool_id(&env);
        let pool = Pool {
            pool_id,
            invoice_id: invoice_id.clone(),
            target_amount,
            min_contribution,
            total_raised: 0,
            investor_count: 0,
            status: PoolStatus::Open,
        };
        storage::set_pool(&env, &pool);

        events::pool_created(&env, pool_id, &invoice_id, target_amount, min_contribution);
        Ok(pool_id)
    }

    /// Contribute to an open pool.
    ///
    /// The contribution is transferred into the pool immediately and is
    /// returned to the investor when it leaves before the pool is executed.
    pub fn join_pool(env: Env, pool_id: u32, investor: Address, amount: i128) -> Result<(), Error> {
        investor.require_auth();
        let mut pool = storage::get_pool(&env, pool_id)?;
        let token = storage::get_token(&env)?;

        if pool.status != PoolStatus::Open {
            return Err(Error::PoolClosed);
        }
        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }
        if amount < pool.min_contribution {
            return Err(Error::BelowMinimum);
        }

        token::Client::new(&env, &token).transfer(
            &investor,
            &env.current_contract_address(),
            &amount,
        );

        let position = storage::get_position(&env, pool_id, &investor);
        if position == 0 {
            let count = pool.investor_count;
            let count = count.checked_add(1).ok_or(Error::Overflow)?;
            pool.investor_count = count;
        }
        let position = position.checked_add(amount).ok_or(Error::Overflow)?;
        storage::set_position(&env, pool_id, &investor, position);
        let raised = pool.total_raised;
        let total = raised.checked_add(amount).ok_or(Error::Overflow)?;
        pool.total_raised = total;
        storage::set_pool(&env, &pool);

        events::pool_joined(&env, pool_id, &investor, amount, total);
        Ok(())
    }

    /// Leave an open pool and get the contribution back.
    ///
    /// Only possible before the pool was executed, an executed pool already
    /// funded the invoice.
    pub fn exit_pool(env: Env, pool_id: u32, investor: Address) -> Result<i128, Error> {
        investor.require_auth();
        let mut pool = storage::get_pool(&env, pool_id)?;
        let token = storage::get_token(&env)?;

        if pool.status != PoolStatus::Open {
            return Err(Error::PoolClosed);
        }
        let amount = storage::get_position(&env, pool_id, &investor);
        if amount <= 0 {
            return Err(Error::NoContribution);
        }

        token::Client::new(&env, &token).transfer(
            &env.current_contract_address(),
            &investor,
            &amount,
        );

        storage::set_position(&env, pool_id, &investor, 0);
        let raised = pool.total_raised;
        let total = raised.checked_sub(amount).ok_or(Error::Overflow)?;
        pool.total_raised = total;
        let count = pool.investor_count;
        let count = count.checked_sub(1).ok_or(Error::Overflow)?;
        pool.investor_count = count;
        storage::set_pool(&env, &pool);

        events::pool_exited(&env, pool_id, &investor, amount);
        Ok(amount)
    }

    /// Admin-only: fund the invoice with the pooled amount as one position.
    ///
    /// The pool contract is the single funder of the invoice, the investors
    /// that contributed do not appear individually on the invoice.
    pub fn execute_pool(env: Env, pool_id: u32) -> Result<i128, Error> {
        let admin = storage::get_admin(&env).ok_or(Error::NotInitialized)?;
        admin.require_auth();

        let mut pool = storage::get_pool(&env, pool_id)?;
        let token = storage::get_token(&env)?;
        let escrow = storage::get_escrow(&env)?;

        if pool.status != PoolStatus::Open {
            return Err(Error::PoolClosed);
        }
        if pool.total_raised < pool.target_amount {
            return Err(Error::TargetNotMet);
        }

        let amount = pool.total_raised;
        let pool_address = env.current_contract_address();

        // The escrow pulls the pooled funds from the pool contract, so the
        // pool authorises that transfer before it calls the escrow.
        env.authorize_as_current_contract(soroban_sdk::vec![
            &env,
            InvokerContractAuthEntry::Contract(SubContractInvocation {
                context: ContractContext {
                    contract: token.clone(),
                    fn_name: symbol_short!("transfer"),
                    args: soroban_sdk::vec![
                        &env,
                        pool_address.clone().into_val(&env),
                        escrow.clone().into_val(&env),
                        amount.into_val(&env),
                    ],
                },
                sub_invocations: soroban_sdk::vec![&env],
            }),
        ]);

        env.invoke_contract::<()>(
            &escrow,
            &Symbol::new(&env, "fund_escrow"),
            soroban_sdk::vec![
                &env,
                pool.invoice_id.clone().into_val(&env),
                pool_address.clone().into_val(&env),
                amount.into_val(&env),
                token.clone().into_val(&env),
            ],
        );

        pool.status = PoolStatus::Executed;
        storage::set_pool(&env, &pool);

        events::pool_executed(&env, pool_id, &pool.invoice_id, amount);
        Ok(amount)
    }

    /// View: a co-investment pool.
    pub fn get_pool(env: Env, pool_id: u32) -> Result<Pool, Error> {
        storage::get_pool(&env, pool_id)
    }

    /// View: the contribution of an investor to a pool.
    pub fn get_contribution(env: Env, pool_id: u32, investor: Address) -> i128 {
        storage::get_position(&env, pool_id, &investor)
    }
}
