//! Data types for the document CID registry contract.

use soroban_sdk::{contracttype, BytesN, String};

/// Storage key enum for instance and persistent storage.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StorageKey {
    /// Instance: registry admin.
    Admin,
    /// Persistent: issuer address bound to an invoice, authorized to
    /// register documents for it alongside the admin.
    Issuer(BytesN<32>),
    /// Persistent: ordered list of documents registered for an invoice.
    Documents(BytesN<32>),
}

/// Supporting-document category.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum DocumentType {
    InvoicePdf,
    KycCertificate,
    AuditReport,
    Contract,
    Other,
}

/// A single registered document.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DocumentRecord {
    /// IPFS content identifier.
    pub cid: String,
    /// Category of the document.
    pub doc_type: DocumentType,
    /// Unix timestamp of registration.
    pub registered_at: u64,
}
