//! Invoice metadata registry contract for StellarSettle.
//!
//! Stores key invoice parameters on-chain (face value, yield, maturity date,
//! issuer, lifecycle status), accessible by other contracts and indexers
//! without reconstructing the invoice from off-chain records.

#![no_std]

mod errors;
mod events;
mod storage;
mod types;

pub use types::{InvoiceMetadata, InvoiceMetadataInput, InvoiceStatus};

use soroban_sdk::{contract, contractimpl, Address, BytesN, Env, String};

use errors::Error;

/// Reject the zero address (all-zero 32-byte Ed25519 key).
fn ensure_non_zero_address(env: &Env, address: &Address) -> Result<(), Error> {
    let zero_str = String::from_str(
        env,
        "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
    );
    let zero = Address::from_string(&zero_str);
    if *address == zero {
        return Err(Error::InvalidAddress);
    }
    Ok(())
}

#[contract]
pub struct InvoiceMetadataRegistry;

#[contractimpl]
impl InvoiceMetadataRegistry {
    /// Initialize the registry with the admin allowed to register invoices
    /// and transition their status.
    pub fn initialize(env: Env, admin: Address) -> Result<(), Error> {
        ensure_non_zero_address(&env, &admin)?;
        if storage::get_admin(&env).is_some() {
            return Err(Error::AlreadyInit);
        }
        storage::set_admin(&env, &admin);
        events::initialized(&env, &admin);
        Ok(())
    }

    /// Admin-only: register a new invoice with its full metadata. Starts in
    /// `Pending` status. Fails if `invoice_id` is already registered.
    pub fn register_invoice(
        env: Env,
        admin: Address,
        invoice_id: BytesN<32>,
        metadata: InvoiceMetadataInput,
    ) -> Result<(), Error> {
        let stored_admin = storage::get_admin(&env).ok_or(Error::NotInit)?;
        if admin != stored_admin {
            return Err(Error::Unauthorized);
        }
        admin.require_auth();
        ensure_non_zero_address(&env, &metadata.issuer)?;

        if metadata.face_value <= 0 || metadata.yield_amount <= 0 {
            return Err(Error::InvalidAmount);
        }
        if storage::get_invoice(&env, &invoice_id).is_some() {
            return Err(Error::AlreadyRegistered);
        }

        let record = InvoiceMetadata {
            invoice_id: invoice_id.clone(),
            face_value: metadata.face_value,
            yield_amount: metadata.yield_amount,
            maturity_date: metadata.maturity_date,
            issuer: metadata.issuer.clone(),
            status: InvoiceStatus::Pending,
        };
        storage::set_invoice(&env, &invoice_id, &record);
        events::invoice_registered(&env, &invoice_id, &metadata.issuer);
        Ok(())
    }

    /// Admin-only: move an invoice to `new_status`. Rejected with
    /// `InvalidTransition` if the move is not a valid edge in the invoice
    /// state machine (see `InvoiceStatus::can_transition_to`).
    pub fn update_status(
        env: Env,
        admin: Address,
        invoice_id: BytesN<32>,
        new_status: InvoiceStatus,
    ) -> Result<(), Error> {
        let stored_admin = storage::get_admin(&env).ok_or(Error::NotInit)?;
        if admin != stored_admin {
            return Err(Error::Unauthorized);
        }
        admin.require_auth();

        let mut record = storage::get_invoice(&env, &invoice_id).ok_or(Error::NotRegistered)?;
        if !record.status.can_transition_to(new_status) {
            return Err(Error::InvalidTransition);
        }

        let old_status = record.status;
        record.status = new_status;
        storage::set_invoice(&env, &invoice_id, &record);
        events::invoice_status_updated(&env, &invoice_id, old_status, new_status);
        Ok(())
    }

    /// View: return an invoice's full metadata at its current state.
    pub fn get_invoice(env: Env, invoice_id: BytesN<32>) -> Result<InvoiceMetadata, Error> {
        storage::get_invoice(&env, &invoice_id).ok_or(Error::NotRegistered)
    }

    /// View: return the current admin.
    pub fn get_admin(env: Env) -> Result<Address, Error> {
        storage::get_admin(&env).ok_or(Error::NotInit)
    }
}

#[cfg(test)]
mod test;
