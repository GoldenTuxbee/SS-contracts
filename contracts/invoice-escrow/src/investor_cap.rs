use soroban_sdk::{contractimpl, contracttype, log, Address, BytesN, Env, Vec};

use crate::{
    storage,
    types::{EscrowStatus, InvoiceStatus, StorageKey},
    Error, InvoiceEscrow, InvoiceEscrowArgs, InvoiceEscrowClient, MAX_INVESTORS_HARD_LIMIT,
};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InvestorCount {
    pub current: u32,
    pub max: u32,
}

pub fn investors(env: &Env, id: &BytesN<32>) -> Vec<Address> {
    if let Some(record) = storage::get_invoice_record(env, id) {
        return record.investors;
    }
    let key = StorageKey::Investors(id.clone());
    let result = env
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or(Vec::new(env));
    if env.storage().persistent().has(&key) {
        storage::bump_persistent(env, &key);
    }
    result
}

pub fn count(env: &Env, id: &BytesN<32>) -> InvestorCount {
    let key = StorageKey::InvoiceCap(id.clone());
    let max = env
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or(storage::get_max_investors(env));
    if env.storage().persistent().has(&key) {
        storage::bump_persistent(env, &key);
    }
    InvestorCount {
        current: storage::get_investor_count(env, id),
        max,
    }
}

pub fn check_admission(env: &Env, id: &BytesN<32>, investor: &Address) -> Result<(), Error> {
    if storage::get_investor_position(env, id, investor) > 0 {
        return Ok(());
    }
    let counts = count(env, id);
    if counts.current >= counts.max {
        log!(
            env,
            "InvestorCapReached: current/max",
            counts.current,
            counts.max
        );
        // Preserve the pre-existing global-cap error for legacy registered invoices.
        if storage::has_invoice_record(env, id)
            && !env
                .storage()
                .persistent()
                .has(&StorageKey::InvoiceCap(id.clone()))
        {
            return Err(Error::MaxInvestorsReached);
        }
        return Err(Error::InvestorCapReached);
    }
    Ok(())
}

/// Synchronize active holders after a successful position change. Registered
/// invoice lists are updated here too so withdrawals cannot leave stale counts.
pub fn sync_holder(env: &Env, id: &BytesN<32>, investor: &Address) {
    let mut holders = investors(env, id);
    let found = holders.first_index_of(investor.clone());
    let active = storage::get_investor_position(env, id, investor) > 0;
    match (found, active) {
        (None, true) => holders.push_back(investor.clone()),
        (Some(index), false) => {
            holders.remove(index);
        }
        _ => {}
    }
    storage::set_investor_count(env, id, holders.len());
    if let Some(mut record) = storage::get_invoice_record(env, id) {
        record.investors = holders;
        storage::set_invoice_record(env, id, &record);
    } else {
        let key = StorageKey::Investors(id.clone());
        env.storage().persistent().set(&key, &holders);
        storage::bump_persistent(env, &key);
    }
    if active {
        let key = StorageKey::InvestmentStarted(id.clone());
        env.storage().persistent().set(&key, &true);
        storage::bump_persistent(env, &key);
    }
}

#[contractimpl]
impl InvoiceEscrow {
    /// The issuer may choose a cap only before the first investment. For the
    /// admin-issued metadata API, the initialized admin is the issuer.
    pub fn set_investor_cap(
        env: Env,
        invoice_id: BytesN<32>,
        max_investors: u32,
    ) -> Result<(), Error> {
        if max_investors == 0 || max_investors > MAX_INVESTORS_HARD_LIMIT {
            return Err(Error::InvalidMaxInvestors);
        }
        let (issuer, open, raised) =
            if let Some(invoice) = storage::get_invoice(&env, invoice_id.clone()) {
                (
                    invoice.seller,
                    invoice.status == InvoiceStatus::Open,
                    invoice.total_raised,
                )
            } else {
                let record =
                    storage::get_invoice_record(&env, &invoice_id).ok_or(Error::EscrowNotFound)?;
                let config = storage::get_config(&env).ok_or(Error::NotInit)?;
                (
                    config.admin,
                    record.status == EscrowStatus::Created,
                    record.total_raised,
                )
            };
        issuer.require_auth();
        if !open
            || raised != 0
            || storage::get_investor_count(&env, &invoice_id) != 0
            || env
                .storage()
                .persistent()
                .has(&StorageKey::InvestmentStarted(invoice_id.clone()))
        {
            return Err(Error::InvalidInvoiceStatus);
        }
        let key = StorageKey::InvoiceCap(invoice_id);
        env.storage().persistent().set(&key, &max_investors);
        storage::bump_persistent(&env, &key);
        Ok(())
    }

    pub fn get_investor_count(env: Env, invoice_id: BytesN<32>) -> Result<InvestorCount, Error> {
        if !storage::has_invoice(&env, invoice_id.clone())
            && !storage::has_invoice_record(&env, &invoice_id)
        {
            return Err(Error::EscrowNotFound);
        }
        Ok(count(&env, &invoice_id))
    }
}
