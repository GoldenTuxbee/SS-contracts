//! Document CID registry contract for StellarSettle.
//!
//! Stores IPFS content identifiers for invoice supporting documents
//! on-chain. Once registered, a CID is immutable: there is no update
//! function, and re-registering the same CID for the same invoice is
//! rejected. Any party can verify a document's integrity by comparing an
//! off-chain file's recomputed CID against the on-chain record via
//! `verify_document`.

#![no_std]

mod errors;
mod events;
mod storage;
mod types;

pub use types::{DocumentRecord, DocumentType};

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
pub struct DocumentCidRegistry;

#[contractimpl]
impl DocumentCidRegistry {
    /// Initialize the registry with the admin allowed to bind issuers and
    /// register documents on any invoice.
    pub fn initialize(env: Env, admin: Address) -> Result<(), Error> {
        ensure_non_zero_address(&env, &admin)?;
        if storage::get_admin(&env).is_some() {
            return Err(Error::AlreadyInit);
        }
        storage::set_admin(&env, &admin);
        events::initialized(&env, &admin);
        Ok(())
    }

    /// Admin-only: bind the address allowed to register documents for
    /// `invoice_id` as its issuer, alongside the admin.
    pub fn set_issuer(
        env: Env,
        admin: Address,
        invoice_id: BytesN<32>,
        issuer: Address,
    ) -> Result<(), Error> {
        let stored_admin = storage::get_admin(&env).ok_or(Error::NotInit)?;
        if admin != stored_admin {
            return Err(Error::Unauthorized);
        }
        admin.require_auth();
        ensure_non_zero_address(&env, &issuer)?;
        storage::set_issuer(&env, &invoice_id, &issuer);
        Ok(())
    }

    /// Issuer-or-admin-only: register a new document for `invoice_id`.
    /// Rejects an empty CID and rejects re-registering a CID already
    /// recorded for this invoice (CIDs are immutable once registered, so
    /// there is deliberately no update path).
    pub fn register_document(
        env: Env,
        caller: Address,
        invoice_id: BytesN<32>,
        cid: String,
        doc_type: DocumentType,
    ) -> Result<(), Error> {
        let stored_admin = storage::get_admin(&env).ok_or(Error::NotInit)?;
        let bound_issuer = storage::get_issuer(&env, &invoice_id);
        let is_admin = caller == stored_admin;
        let is_issuer = bound_issuer == Some(caller.clone());
        if !is_admin && !is_issuer {
            return Err(Error::Unauthorized);
        }
        caller.require_auth();

        if cid.is_empty() {
            return Err(Error::InvalidCid);
        }

        let mut documents = storage::get_documents(&env, &invoice_id);
        for existing in documents.iter() {
            if existing.cid == cid {
                return Err(Error::DuplicateCid);
            }
        }

        documents.push_back(DocumentRecord {
            cid: cid.clone(),
            doc_type,
            registered_at: env.ledger().timestamp(),
        });
        storage::set_documents(&env, &invoice_id, &documents);
        events::document_registered(&env, &invoice_id, &cid, doc_type);
        Ok(())
    }

    /// View: whether `cid` is registered for `invoice_id`.
    pub fn verify_document(env: Env, invoice_id: BytesN<32>, cid: String) -> bool {
        storage::get_documents(&env, &invoice_id)
            .iter()
            .any(|doc| doc.cid == cid)
    }

    /// View: return every document registered for `invoice_id`, in
    /// registration order.
    pub fn get_documents(env: Env, invoice_id: BytesN<32>) -> soroban_sdk::Vec<DocumentRecord> {
        storage::get_documents(&env, &invoice_id)
    }

    /// View: return the current admin.
    pub fn get_admin(env: Env) -> Result<Address, Error> {
        storage::get_admin(&env).ok_or(Error::NotInit)
    }
}

#[cfg(test)]
mod test;
