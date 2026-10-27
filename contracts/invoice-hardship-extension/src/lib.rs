//! Invoice hardship grace extension contract (issue #638).
//!
//! Lets the admin grant an issuer of an invoice additional time when the
//! issuer is in a verified hardship case. The extension adds at most 30 days
//! to the standard grace period of the invoice and the penalty accrual of the
//! invoice is paused while the extension is active, so the issuer keeps the
//! invoice alive until the extension ends.
//!
//! Flow:
//!   1. The admin calls `initialize` with the standard grace period and
//!      registers the invoices with their due date.
//!   2. The admin calls `grant_hardship_extension` for a verified hardship
//!      case, the reason is recorded with the extension.
//!   3. `get_grace_status` reports the standard grace end, the end of the
//!      extension and whether the penalty accrual is paused right now. The
//!      contract that accrues the penalty consults `is_penalty_paused` before
//!      it accrues.

#![no_std]

mod errors;
mod events;
mod storage;
mod types;

#[cfg(test)]
mod test;

pub use errors::Error;
pub use types::{GraceStatus, HardshipExtension, InvoiceGrace, StorageKey};

use soroban_sdk::{contract, contractimpl, Address, Env, String, Symbol};

/// Maximum additional days a hardship extension may grant.
const MAX_EXTENSION_DAYS: u32 = 30;

/// Seconds in a day.
const SECONDS_PER_DAY: u64 = 86_400;

#[contract]
pub struct InvoiceHardshipExtension;

#[contractimpl]
impl InvoiceHardshipExtension {
    /// Initialise the contract with the admin and the standard grace period.
    ///
    /// The standard grace period is added to the due date of an invoice to
    /// obtain the end of its standard grace window.
    pub fn initialize(env: Env, admin: Address, grace_period_days: u32) -> Result<(), Error> {
        admin.require_auth();
        if storage::get_admin(&env).is_some() {
            return Err(Error::AlreadyInitialized);
        }
        storage::set_admin(&env, &admin);
        storage::set_grace_period_days(&env, grace_period_days);
        Ok(())
    }

    /// Admin-only: register an invoice with its due date.
    ///
    /// The standard grace period of the contract is added to the due date to
    /// obtain the standard grace end of the invoice.
    pub fn register_invoice(env: Env, invoice_id: Symbol, due_date: u64) -> Result<(), Error> {
        Self::require_admin(&env)?;
        if due_date == 0 {
            return Err(Error::InvalidDueDate);
        }
        let days = storage::get_grace_period_days(&env) as u64;
        let standard_grace_end = due_date + days * SECONDS_PER_DAY;

        let invoice = InvoiceGrace {
            invoice_id: invoice_id.clone(),
            due_date,
            standard_grace_end,
        };
        storage::set_invoice(&env, &invoice);
        Ok(())
    }

    /// Admin-only: grant a hardship extension to the issuer of an invoice.
    ///
    /// The extension adds at most 30 days to the standard grace period of the
    /// invoice, the days granted earlier count towards that limit. The
    /// penalty accrual of the invoice is paused from the moment the extension
    /// is granted until the extension ends.
    pub fn grant_hardship_extension(
        env: Env,
        invoice_id: Symbol,
        additional_days: u32,
        reason: String,
    ) -> Result<u32, Error> {
        Self::require_admin(&env)?;
        if additional_days == 0 {
            return Err(Error::InvalidDays);
        }
        let invoice = storage::get_invoice(&env, &invoice_id)?;

        let granted = match storage::get_extension(&env, &invoice_id) {
            Some(extension) => extension.additional_days,
            None => 0,
        };
        let added = granted.checked_add(additional_days);
        let total = added.ok_or(Error::MaxExtensionExceeded)?;
        if total > MAX_EXTENSION_DAYS {
            return Err(Error::MaxExtensionExceeded);
        }

        let seconds = (total as u64) * SECONDS_PER_DAY;
        let extension_end = invoice.standard_grace_end + seconds;

        let extension = HardshipExtension {
            invoice_id: invoice_id.clone(),
            additional_days: total,
            extension_end,
            granted_at: env.ledger().timestamp(),
            reason,
        };
        storage::set_extension(&env, &extension);

        let days = extension.additional_days;
        events::hardship_extension_granted(&env, &invoice_id, days, &extension.reason);
        Ok(total)
    }

    /// View: the grace window of an invoice and the state of its penalty.
    ///
    /// The extension end is zero while the invoice has no hardship extension
    /// and the penalty is paused while the extension is active.
    pub fn get_grace_status(env: Env, invoice_id: Symbol) -> Result<GraceStatus, Error> {
        let invoice = storage::get_invoice(&env, &invoice_id)?;
        let extension = storage::get_extension(&env, &invoice_id);

        let (extension_end, extension_days) = match &extension {
            Some(granted) => (granted.extension_end, granted.additional_days),
            None => (0, 0),
        };
        let now = env.ledger().timestamp();
        let penalty_paused = extension_end > 0 && now <= extension_end;

        Ok(GraceStatus {
            invoice_id,
            due_date: invoice.due_date,
            standard_grace_end: invoice.standard_grace_end,
            extension_end,
            extension_days,
            penalty_paused,
        })
    }

    /// View: whether the penalty accrual of an invoice is paused right now.
    ///
    /// The contract that accrues the penalty of an invoice consults this
    /// before it accrues.
    pub fn is_penalty_paused(env: Env, invoice_id: Symbol) -> bool {
        match storage::get_extension(&env, &invoice_id) {
            Some(extension) => env.ledger().timestamp() <= extension.extension_end,
            None => false,
        }
    }

    /// View: the hardship extension of an invoice.
    pub fn get_extension(env: Env, invoice_id: Symbol) -> Result<HardshipExtension, Error> {
        storage::get_extension(&env, &invoice_id).ok_or(Error::InvoiceNotFound)
    }

    /// Check that the admin authorised the operation.
    fn require_admin(env: &Env) -> Result<(), Error> {
        let admin = storage::get_admin(env).ok_or(Error::NotInitialized)?;
        admin.require_auth();
        Ok(())
    }
}
