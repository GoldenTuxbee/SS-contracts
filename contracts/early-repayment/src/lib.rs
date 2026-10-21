#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, token, Address, BytesN, Env, Symbol, Vec,
};

const DAY: u64 = 86_400;
const YEAR_DAYS: u64 = 365;

#[derive(Clone)]
#[contracttype]
enum DataKey {
    Admin,
    Invoice(BytesN<32>),
    Holders(BytesN<32>),
    Position(BytesN<32>, Address),
}

#[derive(Clone)]
#[contracttype]
pub struct Terms {
    pub issuer: Address,
    pub token: Address,
    pub principal: i128,
    pub yield_amount: i128,
    pub maturity: u64,
    pub total_shares: i128,
    pub repaid: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[contracttype]
pub struct RepaymentQuote {
    pub principal: i128,
    pub penalty: i128,
    pub investor_payout: i128,
    pub required_amount: i128,
}

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Error {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    InvalidAmount = 3,
    InvalidTerms = 4,
    InvoiceNotFound = 5,
    AlreadyRepaid = 6,
    InsufficientPayment = 7,
    NoHolders = 8,
    Overflow = 9,
}

#[contract]
pub struct EarlyRepayment;

#[contractimpl]
impl EarlyRepayment {
    pub fn initialize(env: Env, admin: Address) -> Result<(), Error> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(Error::AlreadyInitialized);
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        Ok(())
    }
    pub fn register_invoice(env: Env, invoice_id: BytesN<32>, terms: Terms) -> Result<(), Error> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)?;
        admin.require_auth();
        if terms.principal <= 0
            || terms.yield_amount < 0
            || terms.total_shares <= 0
            || terms.maturity <= env.ledger().timestamp()
        {
            return Err(Error::InvalidTerms);
        }
        env.storage()
            .persistent()
            .set(&DataKey::Invoice(invoice_id), &terms);
        Ok(())
    }
    pub fn set_holder(
        env: Env,
        invoice_id: BytesN<32>,
        holder: Address,
        shares: i128,
    ) -> Result<(), Error> {
        let terms: Terms = env
            .storage()
            .persistent()
            .get(&DataKey::Invoice(invoice_id.clone()))
            .ok_or(Error::InvoiceNotFound)?;
        terms.issuer.require_auth();
        if shares <= 0 {
            return Err(Error::InvalidAmount);
        }
        let key = DataKey::Holders(invoice_id.clone());
        let mut holders: Vec<Address> = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or(Vec::new(&env));
        let mut found = false;
        for current in holders.iter() {
            if current == holder {
                found = true;
                break;
            }
        }
        if !found {
            holders.push_back(holder.clone());
        }
        env.storage().persistent().set(&key, &holders);
        env.storage()
            .persistent()
            .set(&DataKey::Position(invoice_id, holder), &shares);
        Ok(())
    }
    fn quote_for(env: &Env, terms: &Terms) -> Result<RepaymentQuote, Error> {
        let now = env.ledger().timestamp();
        let days_early = terms.maturity.saturating_sub(now) / DAY;
        let penalty = terms
            .yield_amount
            .checked_mul(days_early as i128)
            .ok_or(Error::Overflow)?
            .checked_div(YEAR_DAYS as i128)
            .ok_or(Error::Overflow)?;
        let payout = terms
            .principal
            .checked_add(
                terms
                    .yield_amount
                    .checked_sub(penalty)
                    .ok_or(Error::Overflow)?,
            )
            .ok_or(Error::Overflow)?;
        Ok(RepaymentQuote {
            principal: terms.principal,
            penalty,
            investor_payout: payout,
            required_amount: terms
                .principal
                .checked_add(penalty)
                .ok_or(Error::Overflow)?,
        })
    }
    pub fn get_early_repayment_quote(
        env: Env,
        invoice_id: BytesN<32>,
    ) -> Result<RepaymentQuote, Error> {
        let terms: Terms = env
            .storage()
            .persistent()
            .get(&DataKey::Invoice(invoice_id))
            .ok_or(Error::InvoiceNotFound)?;
        Self::quote_for(&env, &terms)
    }
    pub fn early_repay(env: Env, invoice_id: BytesN<32>, amount: i128) -> Result<(), Error> {
        let mut terms: Terms = env
            .storage()
            .persistent()
            .get(&DataKey::Invoice(invoice_id.clone()))
            .ok_or(Error::InvoiceNotFound)?;
        terms.issuer.require_auth();
        if terms.repaid {
            return Err(Error::AlreadyRepaid);
        }
        let quote = Self::quote_for(&env, &terms)?;
        if amount < quote.required_amount {
            return Err(Error::InsufficientPayment);
        }
        let holders: Vec<Address> = env
            .storage()
            .persistent()
            .get(&DataKey::Holders(invoice_id.clone()))
            .unwrap_or(Vec::new(&env));
        if holders.is_empty() {
            return Err(Error::NoHolders);
        }
        let client = token::Client::new(&env, &terms.token);
        let contract = env.current_contract_address();
        client.transfer(&terms.issuer, &contract, &amount);
        for holder in holders.iter() {
            let shares: i128 = env
                .storage()
                .persistent()
                .get(&DataKey::Position(invoice_id.clone(), holder.clone()))
                .unwrap_or(0);
            let payout = quote
                .investor_payout
                .checked_mul(shares)
                .ok_or(Error::Overflow)?
                .checked_div(terms.total_shares)
                .ok_or(Error::Overflow)?;
            if payout > 0 {
                client.transfer(&contract, &holder, &payout);
            }
        }
        terms.repaid = true;
        env.storage()
            .persistent()
            .set(&DataKey::Invoice(invoice_id.clone()), &terms);
        env.events().publish(
            (Symbol::new(&env, "EarlyRepaymentExecuted"), invoice_id),
            (quote.penalty, quote.investor_payout),
        );
        Ok(())
    }
}
