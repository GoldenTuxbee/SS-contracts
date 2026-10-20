//! Opt-in issuer collateral accounts, denominated in the invoice payment token.
use soroban_sdk::{contractevent, contractimpl, contracttype, token, Address, BytesN, Env};

use crate::{
    math, storage, types::StorageKey, Error, InvoiceEscrow, InvoiceEscrowArgs, InvoiceEscrowClient,
};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CollateralStatus {
    pub token: Address,
    pub total_collateral: i128,
    pub total_outstanding: i128,
    /// Basis points. i128::MAX denotes no outstanding obligations.
    pub ratio_bps: i128,
    pub minimum_bps: u32,
    pub issuance_blocked: bool,
    /// Rounded up to ensure depositing this amount restores the threshold.
    pub required_topup: i128,
}

#[contractevent(topics = ["CollateralTopUpRequired"])]
pub struct CollateralTopUpRequired {
    #[topic]
    pub issuer: Address,
    pub amount: i128,
    pub total_collateral: i128,
    pub total_outstanding: i128,
    pub ratio_bps: i128,
    pub minimum_bps: u32,
}

#[contractevent(topics = ["CollateralToppedUp"])]
pub struct CollateralToppedUp {
    #[topic]
    pub issuer: Address,
    #[topic]
    pub invoice_id: BytesN<32>,
    pub amount: i128,
    pub total_collateral: i128,
    pub ratio_bps: i128,
    pub issuance_blocked: bool,
}

pub fn get(env: &Env, issuer: &Address) -> Option<CollateralStatus> {
    let key = StorageKey::Collateral(issuer.clone());
    let result: Option<CollateralStatus> = env.storage().persistent().get(&key);
    if result.is_some() {
        storage::bump_persistent(env, &key);
    }
    if let Some(ref status) = result {
        let reserve_key = StorageKey::CollateralReserve(status.token.clone());
        if env.storage().persistent().has(&reserve_key) {
            storage::bump_persistent(env, &reserve_key);
        }
    }
    result
}

fn save(env: &Env, issuer: &Address, status: &CollateralStatus) {
    let key = StorageKey::Collateral(issuer.clone());
    env.storage().persistent().set(&key, status);
    storage::bump_persistent(env, &key);
}

/// All outgoing escrow payments preserve collateral pooled in the same token.
/// Checking before transfer also prevents a token callback from observing a
/// balance below the reserve.
pub fn transfer(
    env: &Env,
    client: &token::Client,
    recipient: &Address,
    amount: &i128,
) -> Result<(), Error> {
    let key = StorageKey::CollateralReserve(client.address.clone());
    let reserved: i128 = env.storage().persistent().get(&key).unwrap_or(0);
    let repayment_key = StorageKey::RepaymentReserve(client.address.clone());
    let repayment: i128 = env.storage().persistent().get(&repayment_key).unwrap_or(0);
    if reserved > 0 {
        storage::bump_persistent(env, &key);
    }
    if repayment > 0 {
        storage::bump_persistent(env, &repayment_key);
    }
    if reserved > 0 || repayment > 0 {
        let remaining = client
            .balance(&env.current_contract_address())
            .checked_sub(*amount)
            .ok_or(Error::Overflow)?;
        if remaining < reserved {
            return Err(Error::CollateralReserved);
        }
        if remaining < reserved.checked_add(repayment).ok_or(Error::Overflow)? {
            return Err(Error::RepaymentReserved);
        }
    }
    client.transfer(&env.current_contract_address(), recipient, amount);
    Ok(())
}

fn recalculate(env: &Env, status: &mut CollateralStatus) -> Result<(), Error> {
    if status.total_outstanding == 0 {
        status.ratio_bps = i128::MAX;
        status.required_topup = 0;
    } else {
        status.ratio_bps = math::mul_div(
            env,
            status.total_collateral,
            10_000,
            status.total_outstanding,
        )?;
        let minimum = i128::from(status.minimum_bps);
        let truncated = math::mul_div(env, status.total_outstanding, minimum, 10_000)?;
        let fractional = (status.total_outstanding % 10_000) * minimum % 10_000;
        let required = truncated
            .checked_add(i128::from(fractional != 0))
            .ok_or(Error::Overflow)?;
        status.required_topup = required.saturating_sub(status.total_collateral).max(0);
    }
    if status.required_topup == 0 {
        status.issuance_blocked = false;
    }
    Ok(())
}

pub fn ensure_issuance_allowed(env: &Env, issuer: &Address) -> Result<(), Error> {
    if get(env, issuer).is_some_and(|status| status.issuance_blocked) {
        return Err(Error::CollateralTopUpRequired);
    }
    Ok(())
}

fn update_total(env: &Env, issuer: &Address, key: StorageKey, amount: i128) -> Result<(), Error> {
    let Some(mut status) = get(env, issuer) else {
        return Ok(());
    };
    let old: i128 = env.storage().persistent().get(&key).unwrap_or(0);
    status.total_outstanding = status
        .total_outstanding
        .checked_sub(old)
        .and_then(|total| total.checked_add(amount))
        .ok_or(Error::Overflow)?;
    recalculate(env, &mut status)?;
    env.storage().persistent().set(&key, &amount);
    storage::bump_persistent(env, &key);
    save(env, issuer, &status);
    Ok(())
}

pub fn set_outstanding(
    env: &Env,
    id: &BytesN<32>,
    issuer: &Address,
    amount: i128,
) -> Result<(), Error> {
    update_total(env, issuer, StorageKey::Outstanding(id.clone()), amount)
}

pub fn on_create(
    env: &Env,
    id: &BytesN<32>,
    issuer: &Address,
    payment_token: &Address,
    outstanding: i128,
) -> Result<(), Error> {
    ensure_issuance_allowed(env, issuer)?;
    if let Some(status) = get(env, issuer) {
        if status.token != *payment_token {
            return Err(Error::InvalidCollateralConfig);
        }
        set_outstanding(env, id, issuer, outstanding)?;
    }
    Ok(())
}

/// Symbol-based escrows participate in the same issuer totals. Every state
/// save derives outstanding debt from face value less repayments, and removes
/// obligations when an escrow is cancelled, refunded or settled.
pub fn sync_escrow(env: &Env, escrow: &crate::types::EscrowData) -> Result<(), Error> {
    let Some(status) = get(env, &escrow.seller) else {
        return Ok(());
    };
    if status.token != escrow.token {
        return Err(Error::InvalidCollateralConfig);
    }
    let outstanding = match escrow.status {
        crate::EscrowStatus::Settled
        | crate::EscrowStatus::Cancelled
        | crate::EscrowStatus::Refunded => 0,
        _ => escrow
            .face_value
            .checked_sub(escrow.paid_amt)
            .ok_or(Error::Overflow)?
            .max(0),
    };
    update_total(
        env,
        &escrow.seller,
        StorageKey::EscrowOutstanding(escrow.inv_id.clone()),
        outstanding,
    )
}

#[contractimpl]
impl InvoiceEscrow {
    /// Configure before issuer invoice creation. A single payment token keeps
    /// amounts comparable; later adoption can import existing funding invoices
    /// with set_invoice_outstanding and existing Symbol escrows on their next save.
    pub fn configure_collateral(
        env: Env,
        issuer: Address,
        token: Address,
        minimum_bps: u32,
    ) -> Result<(), Error> {
        let config = storage::get_config(&env).ok_or(Error::NotInit)?;
        config.admin.require_auth();
        if minimum_bps == 0 || minimum_bps > 1_000_000 {
            return Err(Error::InvalidCollateralConfig);
        }
        if get(&env, &issuer).is_some() {
            return Err(Error::AlreadyInit);
        }
        save(
            &env,
            &issuer,
            &CollateralStatus {
                token,
                total_collateral: 0,
                total_outstanding: 0,
                ratio_bps: i128::MAX,
                minimum_bps,
                issuance_blocked: false,
                required_topup: 0,
            },
        );
        Ok(())
    }

    /// Admin supplies authoritative outstanding debt, including accrued yield
    /// or partial repayments. Totals are adjusted by delta, never double counted.
    pub fn set_invoice_outstanding(
        env: Env,
        invoice_id: BytesN<32>,
        outstanding: i128,
    ) -> Result<(), Error> {
        let config = storage::get_config(&env).ok_or(Error::NotInit)?;
        config.admin.require_auth();
        if outstanding < 0 {
            return Err(Error::InvalidAmount);
        }
        let invoice =
            storage::get_invoice(&env, invoice_id.clone()).ok_or(Error::EscrowNotFound)?;
        if invoice.status == crate::types::InvoiceStatus::Settled {
            return Err(Error::InvalidInvoiceStatus);
        }
        let status = get(&env, &invoice.seller).ok_or(Error::CollateralNotConfigured)?;
        if status.token != invoice.token {
            return Err(Error::InvalidCollateralConfig);
        }
        set_outstanding(&env, &invoice_id, &invoice.seller, outstanding)
    }

    pub fn check_collateral_ratio(env: Env, issuer_wallet: Address) -> Result<(i128, u32), Error> {
        let status = get(&env, &issuer_wallet).ok_or(Error::CollateralNotConfigured)?;
        Ok((status.ratio_bps, status.minimum_bps))
    }

    pub fn get_collateral_status(
        env: Env,
        issuer_wallet: Address,
    ) -> Result<CollateralStatus, Error> {
        get(&env, &issuer_wallet).ok_or(Error::CollateralNotConfigured)
    }

    pub fn request_topup(env: Env, issuer_wallet: Address) -> Result<(), Error> {
        let config = storage::get_config(&env).ok_or(Error::NotInit)?;
        config.admin.require_auth();
        let mut status = get(&env, &issuer_wallet).ok_or(Error::CollateralNotConfigured)?;
        recalculate(&env, &mut status)?;
        if status.required_topup == 0 {
            return Err(Error::CollateralRatioHealthy);
        }
        status.issuance_blocked = true;
        save(&env, &issuer_wallet, &status);
        CollateralTopUpRequired {
            issuer: issuer_wallet,
            amount: status.required_topup,
            total_collateral: status.total_collateral,
            total_outstanding: status.total_outstanding,
            ratio_bps: status.ratio_bps,
            minimum_bps: status.minimum_bps,
        }
        .publish(&env);
        Ok(())
    }

    pub fn top_up_collateral(env: Env, invoice_id: BytesN<32>, amount: i128) -> Result<(), Error> {
        let config = storage::get_config(&env).ok_or(Error::NotInit)?;
        crate::ensure_not_paused(&config)?;
        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }
        let invoice =
            storage::get_invoice(&env, invoice_id.clone()).ok_or(Error::EscrowNotFound)?;
        if invoice.status == crate::types::InvoiceStatus::Settled {
            return Err(Error::InvalidInvoiceStatus);
        }
        invoice.seller.require_auth();
        let mut status = get(&env, &invoice.seller).ok_or(Error::CollateralNotConfigured)?;
        if status.token != invoice.token {
            return Err(Error::InvalidCollateralConfig);
        }
        status.total_collateral = status
            .total_collateral
            .checked_add(amount)
            .ok_or(Error::Overflow)?;
        recalculate(&env, &mut status)?;
        token::Client::new(&env, &status.token).transfer(
            &invoice.seller,
            env.current_contract_address(),
            &amount,
        );
        let reserve_key = StorageKey::CollateralReserve(status.token.clone());
        let reserved: i128 = env.storage().persistent().get(&reserve_key).unwrap_or(0);
        env.storage().persistent().set(
            &reserve_key,
            &reserved.checked_add(amount).ok_or(Error::Overflow)?,
        );
        storage::bump_persistent(&env, &reserve_key);
        save(&env, &invoice.seller, &status);
        CollateralToppedUp {
            issuer: invoice.seller,
            invoice_id,
            amount,
            total_collateral: status.total_collateral,
            ratio_bps: status.ratio_bps,
            issuance_blocked: status.issuance_blocked,
        }
        .publish(&env);
        Ok(())
    }
}
