//! Beneficial ownership of token-backed investor positions. Custody reserves
//! part or all of an existing position; it never creates an unbacked claim.
use soroban_sdk::{contractevent, contractimpl, contracttype, token, Address, BytesN, Env};

use crate::{
    collateral, ensure_not_paused, investor_cap, math, storage,
    types::{InvoiceStatus, StorageKey},
    Error, InvoiceEscrow, InvoiceEscrowArgs, InvoiceEscrowClient,
};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CustodyPosition {
    pub beneficiary: Address,
    pub amount: i128,
}

#[contractevent(topics = ["CustodyRegistered"])]
pub struct CustodyRegistered {
    #[topic]
    pub invoice_id: BytesN<32>,
    #[topic]
    pub custodian: Address,
    pub beneficiary: Address,
    pub amount: i128,
}

#[contractevent(topics = ["CustodyTransferred"])]
pub struct CustodyTransferred {
    #[topic]
    pub invoice_id: BytesN<32>,
    pub old_custodian: Address,
    pub new_custodian: Address,
    pub beneficiary: Address,
    pub amount: i128,
}

pub fn get(env: &Env, id: &BytesN<32>, custodian: &Address) -> Option<CustodyPosition> {
    let key = StorageKey::Custody(id.clone(), custodian.clone());
    let result = env.storage().persistent().get(&key);
    if result.is_some() {
        storage::bump_persistent(env, &key);
    }
    result
}

fn save(env: &Env, id: &BytesN<32>, custodian: &Address, position: &CustodyPosition) {
    let key = StorageKey::Custody(id.clone(), custodian.clone());
    env.storage().persistent().set(&key, position);
    storage::bump_persistent(env, &key);
}

fn reserve_repayment(env: &Env, token: &Address, delta: i128) -> Result<(), Error> {
    let key = StorageKey::RepaymentReserve(token.clone());
    let old: i128 = env.storage().persistent().get(&key).unwrap_or(0);
    let total = old.checked_add(delta).ok_or(Error::Overflow)?;
    if total < 0 {
        return Err(Error::InsufficientRepayment);
    }
    env.storage().persistent().set(&key, &total);
    storage::bump_persistent(env, &key);
    Ok(())
}

pub fn ensure_available(
    env: &Env,
    id: &BytesN<32>,
    holder: &Address,
    remaining: i128,
) -> Result<(), Error> {
    if get(env, id, holder).is_some_and(|position| remaining < position.amount) {
        return Err(Error::PositionInCustody);
    }
    Ok(())
}

/// Execute the existing two-admin settlement using separately deposited
/// repayment funds. The investor list is bounded by the invoice's cap.
pub fn settle(env: &Env, id: &BytesN<32>, repayment: i128) -> Result<(), Error> {
    ensure_not_paused(&storage::get_config(env).ok_or(Error::NotInit)?)?;
    let mut invoice = storage::get_invoice(env, id.clone()).ok_or(Error::EscrowNotFound)?;
    if invoice.status != InvoiceStatus::Funded {
        return Err(Error::InvalidInvoiceStatus);
    }
    let repayment_key = StorageKey::Repayment(id.clone());
    let deposited: i128 = env.storage().persistent().get(&repayment_key).unwrap_or(0);
    if repayment < invoice.total_raised || deposited != repayment || invoice.total_raised <= 0 {
        return Err(Error::InsufficientRepayment);
    }
    let holders = investor_cap::investors(env, id);
    if holders.is_empty() {
        return Err(Error::NoPositionFound);
    }
    // Mark terminal state before invoking token contracts. Any failed transfer
    // rolls back the entire invocation, including these writes.
    invoice.status = InvoiceStatus::Settled;
    storage::set_invoice(env, id.clone(), &invoice);
    env.storage().persistent().remove(&repayment_key);
    reserve_repayment(env, &invoice.token, -repayment)?;
    let token = token::Client::new(env, &invoice.token);
    let mut paid = 0i128;
    for (index, holder) in holders.iter().enumerate() {
        let committed = storage::get_investor_position(env, id, &holder);
        let payout = if index as u32 == holders.len() - 1 {
            repayment.checked_sub(paid).ok_or(Error::Overflow)?
        } else {
            math::mul_div(env, committed, repayment, invoice.total_raised)?
        };
        paid = paid.checked_add(payout).ok_or(Error::Overflow)?;
        let custody = get(env, id, &holder);
        let direct_principal = committed
            .checked_sub(
                custody
                    .as_ref()
                    .map(|position| position.amount)
                    .unwrap_or(0),
            )
            .ok_or(Error::Overflow)?;
        let beneficiary_payout = if let Some(ref position) = custody {
            math::mul_div(env, payout, position.amount, committed)?
        } else {
            0
        };
        storage::set_investor_position(env, id, &holder, 0);
        investor_cap::sync_holder(env, id, &holder);
        if let Some(position) = custody {
            env.storage()
                .persistent()
                .remove(&StorageKey::Custody(id.clone(), holder.clone()));
            if beneficiary_payout > 0 {
                collateral::transfer(env, &token, &position.beneficiary, &beneficiary_payout)?;
                crate::events::settlement_paid(
                    env,
                    &position.beneficiary,
                    id,
                    beneficiary_payout,
                    beneficiary_payout.saturating_sub(position.amount),
                );
            }
        }
        let direct_payout = payout
            .checked_sub(beneficiary_payout)
            .ok_or(Error::Overflow)?;
        if direct_payout > 0 {
            collateral::transfer(env, &token, &holder, &direct_payout)?;
            crate::events::settlement_paid(
                env,
                &holder,
                id,
                direct_payout,
                direct_payout.saturating_sub(direct_principal),
            );
        }
    }
    collateral::set_outstanding(env, id, &invoice.seller, 0)?;
    Ok(())
}

#[contractimpl]
impl InvoiceEscrow {
    /// Admin registers beneficial ownership of an already funded position.
    pub fn register_custody(
        env: Env,
        custodian: Address,
        beneficiary: Address,
        invoice_id: BytesN<32>,
        amount: i128,
    ) -> Result<(), Error> {
        let config = storage::get_config(&env).ok_or(Error::NotInit)?;
        config.admin.require_auth();
        ensure_not_paused(&config)?;
        let invoice =
            storage::get_invoice(&env, invoice_id.clone()).ok_or(Error::EscrowNotFound)?;
        if invoice.status == InvoiceStatus::Settled {
            return Err(Error::InvalidInvoiceStatus);
        }
        if amount <= 0 || amount > storage::get_investor_position(&env, &invoice_id, &custodian) {
            return Err(Error::InvalidAmount);
        }
        crate::ensure_non_zero_address(&env, &beneficiary)?;
        if get(&env, &invoice_id, &custodian).is_some() {
            return Err(Error::CustodyExists);
        }
        save(
            &env,
            &invoice_id,
            &custodian,
            &CustodyPosition {
                beneficiary: beneficiary.clone(),
                amount,
            },
        );
        CustodyRegistered {
            invoice_id,
            custodian,
            beneficiary,
            amount,
        }
        .publish(&env);
        Ok(())
    }

    pub fn get_beneficial_owner(
        env: Env,
        custodian: Address,
        invoice_id: BytesN<32>,
    ) -> Result<Address, Error> {
        Ok(get(&env, &invoice_id, &custodian)
            .ok_or(Error::CustodyNotFound)?
            .beneficiary)
    }

    pub fn get_custody(
        env: Env,
        custodian: Address,
        invoice_id: BytesN<32>,
    ) -> Result<CustodyPosition, Error> {
        get(&env, &invoice_id, &custodian).ok_or(Error::CustodyNotFound)
    }

    /// The old custodian and beneficial owner must both sign. The receiving
    /// custodian signs to accept the holding, and cannot have another custody
    /// record for this invoice. Beneficial ownership is preserved.
    pub fn transfer_custody(
        env: Env,
        custodian: Address,
        new_custodian: Address,
        invoice_id: BytesN<32>,
    ) -> Result<(), Error> {
        let config = storage::get_config(&env).ok_or(Error::NotInit)?;
        ensure_not_paused(&config)?;
        let invoice =
            storage::get_invoice(&env, invoice_id.clone()).ok_or(Error::EscrowNotFound)?;
        if invoice.status == InvoiceStatus::Settled || custodian == new_custodian {
            return Err(Error::InvalidInvoiceStatus);
        }
        let position = get(&env, &invoice_id, &custodian).ok_or(Error::CustodyNotFound)?;
        custodian.require_auth();
        position.beneficiary.require_auth();
        new_custodian.require_auth();
        crate::ensure_non_zero_address(&env, &new_custodian)?;
        if get(&env, &invoice_id, &new_custodian).is_some() {
            return Err(Error::CustodyExists);
        }
        let old_amount = storage::get_investor_position(&env, &invoice_id, &custodian);
        let remaining = old_amount
            .checked_sub(position.amount)
            .ok_or(Error::Overflow)?;
        let new_amount = storage::get_investor_position(&env, &invoice_id, &new_custodian)
            .checked_add(position.amount)
            .ok_or(Error::Overflow)?;
        if let Some(cap) = invoice.per_investor_cap {
            if new_amount > cap {
                return Err(Error::InvalidAmount);
            }
        }
        // Free a fully transferred holder's slot before admitting the new one.
        storage::set_investor_position(&env, &invoice_id, &custodian, remaining);
        investor_cap::sync_holder(&env, &invoice_id, &custodian);
        investor_cap::check_admission(&env, &invoice_id, &new_custodian)?;
        storage::set_investor_position(&env, &invoice_id, &new_custodian, new_amount);
        investor_cap::sync_holder(&env, &invoice_id, &new_custodian);
        env.storage()
            .persistent()
            .remove(&StorageKey::Custody(invoice_id.clone(), custodian.clone()));
        save(&env, &invoice_id, &new_custodian, &position);
        CustodyTransferred {
            invoice_id,
            old_custodian: custodian,
            new_custodian,
            beneficiary: position.beneficiary,
            amount: position.amount,
        }
        .publish(&env);
        Ok(())
    }

    /// Segregate repayment from pooled investor principal before the existing
    /// two-admin proposal/approval flow. Deposits can be made in installments.
    pub fn deposit_repayment(
        env: Env,
        invoice_id: BytesN<32>,
        payer: Address,
        amount: i128,
    ) -> Result<(), Error> {
        payer.require_auth();
        let config = storage::get_config(&env).ok_or(Error::NotInit)?;
        ensure_not_paused(&config)?;
        let invoice =
            storage::get_invoice(&env, invoice_id.clone()).ok_or(Error::EscrowNotFound)?;
        if invoice.status != InvoiceStatus::Funded
            || storage::get_settlement_proposal(&env, &invoice_id).is_some()
        {
            return Err(Error::InvalidInvoiceStatus);
        }
        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }
        let key = StorageKey::Repayment(invoice_id);
        let deposited: i128 = env.storage().persistent().get(&key).unwrap_or(0);
        let total = deposited.checked_add(amount).ok_or(Error::Overflow)?;
        token::Client::new(&env, &invoice.token).transfer(
            &payer,
            env.current_contract_address(),
            &amount,
        );
        env.storage().persistent().set(&key, &total);
        storage::bump_persistent(&env, &key);
        reserve_repayment(&env, &invoice.token, amount)?;
        Ok(())
    }
}
