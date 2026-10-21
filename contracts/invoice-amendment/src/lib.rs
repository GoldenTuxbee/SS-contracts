#![no_std]

mod errors;
mod events;
mod types;

pub use types::{AmendmentField, AmendmentStatus, Error};
use types::{Amendment, InvoiceMetadata, StorageKey};

use soroban_sdk::{contract, contractimpl, Address, BytesN, Env, Symbol};

const TIME_LOCK_SECS: u64 = 86_400;

fn metadata(env: &Env, invoice_id: &BytesN<32>) -> Result<InvoiceMetadata, Error> {
    env.storage()
        .persistent()
        .get(&StorageKey::InvoiceMetadata(invoice_id.clone()))
        .ok_or(Error::InvoiceNotFound)
}

fn save_metadata(env: &Env, data: &InvoiceMetadata) {
    env.storage()
        .persistent()
        .set(&StorageKey::InvoiceMetadata(data.invoice_id.clone()), data);
}

fn amendment(env: &Env, id: u32) -> Result<Amendment, Error> {
    env.storage()
        .persistent()
        .get(&StorageKey::Amendment(id))
        .ok_or(Error::AmendmentNotFound)
}

fn save_amendment(env: &Env, data: &Amendment) {
    env.storage()
        .persistent()
        .set(&StorageKey::Amendment(data.amendment_id), data);
}

#[contract]
pub struct InvoiceAmendment;

#[contractimpl]
impl InvoiceAmendment {
    pub fn initialize(env: Env, admin: Address) -> Result<(), Error> {
        if env.storage().instance().has(&StorageKey::Admin) {
            return Err(Error::AlreadyInitialized);
        }
        admin.require_auth();
        env.storage().instance().set(&StorageKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&StorageKey::NextAmendmentId, &1u32);
        Ok(())
    }

    pub fn register_invoice(
        env: Env,
        invoice_id: BytesN<32>,
        issuer: Address,
        amount: i128,
        maturity_date: u64,
        yield_rate: i128,
    ) -> Result<(), Error> {
        if amount <= 0 || maturity_date == 0 || yield_rate < 0 {
            return Err(Error::InvalidAmount);
        }
        issuer.require_auth();
        if env
            .storage()
            .persistent()
            .has(&StorageKey::InvoiceMetadata(invoice_id.clone()))
        {
            return Err(Error::InvoiceNotFound);
        }
        env.storage()
            .instance()
            .set(&StorageKey::InvoiceIssuer(invoice_id.clone()), &issuer);
        save_metadata(
            &env,
            &InvoiceMetadata {
                invoice_id,
                amount,
                maturity_date,
                yield_rate,
            },
        );
        Ok(())
    }

    pub fn propose_amendment(
        env: Env,
        invoice_id: BytesN<32>,
        field: AmendmentField,
        new_value: i128,
    ) -> Result<u32, Error> {
        if new_value <= 0 {
            return Err(Error::InvalidAmount);
        }
        let meta = metadata(&env, &invoice_id)?;
        let _ = meta;
        let issuer: Address = env
            .storage()
            .instance()
            .get(&StorageKey::InvoiceIssuer(invoice_id.clone()))
            .ok_or(Error::InvoiceNotFound)?;
        issuer.require_auth();

        let id: u32 = env
            .storage()
            .instance()
            .get(&StorageKey::NextAmendmentId)
            .unwrap_or(1);
        let now = env.ledger().timestamp();
        let field_sym = field.as_symbol(&env);
        let amendment = Amendment {
            amendment_id: id,
            invoice_id: invoice_id.clone(),
            issuer: issuer.clone(),
            field,
            new_value,
            proposed_at: now,
            execute_after: now.checked_add(TIME_LOCK_SECS).ok_or(Error::Overflow)?,
            status: AmendmentStatus::Pending,
        };
        save_amendment(&env, &amendment);
        env.storage()
            .instance()
            .set(&StorageKey::NextAmendmentId, &(id + 1));

        let field_sym = field.as_symbol(&env);
        events::amendment_proposed(&env, id, &invoice_id, &issuer, &field_sym, new_value);
        Ok(id)
    }

    pub fn execute_amendment(env: Env, amendment_id: u32) -> Result<InvoiceMetadata, Error> {
        let mut data = amendment(&env, amendment_id)?;
        if data.status != AmendmentStatus::Pending {
            return Err(Error::NotPending);
        }
        if env.ledger().timestamp() < data.execute_after {
            return Err(Error::TimeLockNotExpired);
        }

        let mut meta = metadata(&env, &data.invoice_id)?;
        let field_sym = data.field.as_symbol(&env);
        match data.field {
            AmendmentField::Amount => meta.amount = data.new_value,
            AmendmentField::MaturityDate => {
                meta.maturity_date = data
                    .new_value
                    .try_into()
                    .map_err(|_| Error::InvalidAmount)?;
            }
            AmendmentField::YieldRate => meta.yield_rate = data.new_value,
        }
        save_metadata(&env, &meta);

        data.status = AmendmentStatus::Executed;
        save_amendment(&env, &data);

        let field_sym = data.field.as_symbol(&env);
        events::amendment_executed(&env, amendment_id, &data.invoice_id, &field_sym, data.new_value);
        Ok(meta)
    }

    pub fn revoke_amendment(env: Env, amendment_id: u32) -> Result<(), Error> {
        let mut data = amendment(&env, amendment_id)?;
        if data.status != AmendmentStatus::Pending {
            return Err(Error::NotPending);
        }
        data.issuer.require_auth();
        data.status = AmendmentStatus::Revoked;
        save_amendment(&env, &data);
        events::amendment_revoked(&env, amendment_id, &data.invoice_id, &data.issuer);
        Ok(())
    }

    pub fn get_amendment(env: Env, amendment_id: u32) -> Result<Amendment, Error> {
        amendment(&env, amendment_id)
    }

    pub fn get_invoice_metadata(env: Env, invoice_id: BytesN<32>) -> Result<InvoiceMetadata, Error> {
        metadata(&env, &invoice_id)
    }
}
