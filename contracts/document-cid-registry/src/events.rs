#![allow(deprecated)]
//! Events emitted by the document CID registry contract.

use soroban_sdk::{BytesN, Env, String, Symbol};

use crate::types::DocumentType;

/// Emitted when the registry is initialized.
pub fn initialized(env: &Env, admin: &soroban_sdk::Address) {
    let topics = (Symbol::new(env, "initialized"),);
    env.events().publish(topics, admin.clone());
}

/// Emitted when a document is registered for an invoice.
/// Topics: (DocumentRegistered, invoice_id); Data: (cid, doc_type).
pub fn document_registered(
    env: &Env,
    invoice_id: &BytesN<32>,
    cid: &String,
    doc_type: DocumentType,
) {
    let topics = (Symbol::new(env, "DocumentRegistered"), invoice_id.clone());
    env.events().publish(topics, (cid.clone(), doc_type as u32));
}
