#![cfg(test)]
#![allow(deprecated)]

use soroban_sdk::testutils::{Address as _, Events};
use soroban_sdk::{Address, BytesN, Env, String, Symbol, TryFromVal, TryIntoVal, Val, Vec};

use crate::types::DocumentType;
use crate::{DocumentCidRegistry, DocumentCidRegistryClient, Error};

fn zero_address(env: &Env) -> Address {
    Address::from_string(&String::from_str(
        env,
        "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
    ))
}

fn invoice_id(env: &Env, seed: u8) -> BytesN<32> {
    BytesN::from_array(env, &[seed; 32])
}

fn cid(env: &Env, s: &str) -> String {
    String::from_str(env, s)
}

fn setup<'a>(env: &Env) -> (Address, DocumentCidRegistryClient<'a>) {
    env.mock_all_auths();
    let contract_id = env.register(DocumentCidRegistry, ());
    let client = DocumentCidRegistryClient::new(env, &contract_id);
    let admin = Address::generate(env);
    client.initialize(&admin);
    (admin, client)
}

fn last_event(env: &Env) -> (Vec<Val>, Val) {
    let all = env.events().all();
    let events = all.events();
    let last = events.last().expect("no events emitted");
    let soroban_sdk::xdr::ContractEventBody::V0(v0) = &last.body;
    let topics = Vec::<Val>::try_from_val(
        env,
        &soroban_sdk::xdr::ScVal::Vec(Some(v0.topics.clone().into())),
    )
    .unwrap();
    let data = Val::try_from_val(env, &v0.data).unwrap();
    (topics, data)
}

#[test]
fn test_initialize_sets_admin() {
    let env = Env::default();
    let (admin, client) = setup(&env);
    assert_eq!(client.get_admin(), admin);
}

#[test]
fn test_cannot_initialize_twice() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(DocumentCidRegistry, ());
    let client = DocumentCidRegistryClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);
    assert_eq!(client.try_initialize(&admin), Err(Ok(Error::AlreadyInit)));
}

#[test]
fn test_admin_can_register_document_without_being_bound_issuer() {
    let env = Env::default();
    let (admin, client) = setup(&env);
    let id = invoice_id(&env, 1);
    let hash = cid(&env, "QmTestHash1");

    client.register_document(&admin, &id, &hash, &DocumentType::InvoicePdf);
    assert!(client.verify_document(&id, &hash));
}

#[test]
fn test_bound_issuer_can_register_document() {
    let env = Env::default();
    let (admin, client) = setup(&env);
    let issuer = Address::generate(&env);
    let id = invoice_id(&env, 2);
    client.set_issuer(&admin, &id, &issuer);

    let hash = cid(&env, "QmTestHash2");
    client.register_document(&issuer, &id, &hash, &DocumentType::KycCertificate);
    assert!(client.verify_document(&id, &hash));
}

#[test]
fn test_registration_restricted_to_issuer_or_admin() {
    let env = Env::default();
    let (admin, client) = setup(&env);
    let issuer = Address::generate(&env);
    let impostor = Address::generate(&env);
    let id = invoice_id(&env, 3);
    client.set_issuer(&admin, &id, &issuer);

    let hash = cid(&env, "QmTestHash3");
    assert_eq!(
        client.try_register_document(&impostor, &id, &hash, &DocumentType::Contract),
        Err(Ok(Error::Unauthorized))
    );
    assert!(!client.verify_document(&id, &hash));
}

#[test]
fn test_issuer_bound_to_one_invoice_cannot_register_for_another() {
    let env = Env::default();
    let (admin, client) = setup(&env);
    let issuer_a = Address::generate(&env);
    let id_a = invoice_id(&env, 4);
    let id_b = invoice_id(&env, 5);
    client.set_issuer(&admin, &id_a, &issuer_a);
    // No issuer bound for id_b.

    let hash = cid(&env, "QmTestHash4");
    assert_eq!(
        client.try_register_document(&issuer_a, &id_b, &hash, &DocumentType::AuditReport),
        Err(Ok(Error::Unauthorized))
    );
}

#[test]
fn test_set_issuer_restricted_to_admin() {
    let env = Env::default();
    let (_admin, client) = setup(&env);
    let impostor = Address::generate(&env);
    let issuer = Address::generate(&env);
    let id = invoice_id(&env, 6);

    assert_eq!(
        client.try_set_issuer(&impostor, &id, &issuer),
        Err(Ok(Error::Unauthorized))
    );
}

#[test]
fn test_verify_document_true_for_registered_cid_false_otherwise() {
    let env = Env::default();
    let (admin, client) = setup(&env);
    let id = invoice_id(&env, 7);
    let registered = cid(&env, "QmRegistered");
    let unregistered = cid(&env, "QmNeverRegistered");

    client.register_document(&admin, &id, &registered, &DocumentType::InvoicePdf);

    assert!(client.verify_document(&id, &registered));
    assert!(!client.verify_document(&id, &unregistered));
}

#[test]
fn test_verify_document_false_for_unregistered_invoice() {
    let env = Env::default();
    let (_admin, client) = setup(&env);
    let id = invoice_id(&env, 8);
    assert!(!client.verify_document(&id, &cid(&env, "Qmanything")));
}

#[test]
fn test_get_documents_returns_all_registered_documents_with_correct_types() {
    let env = Env::default();
    let (admin, client) = setup(&env);
    let id = invoice_id(&env, 9);

    client.register_document(&admin, &id, &cid(&env, "QmPdf"), &DocumentType::InvoicePdf);
    client.register_document(
        &admin,
        &id,
        &cid(&env, "QmKyc"),
        &DocumentType::KycCertificate,
    );
    client.register_document(
        &admin,
        &id,
        &cid(&env, "QmAudit"),
        &DocumentType::AuditReport,
    );

    let docs = client.get_documents(&id);
    assert_eq!(docs.len(), 3);
    assert_eq!(docs.get(0).unwrap().cid, cid(&env, "QmPdf"));
    assert_eq!(docs.get(0).unwrap().doc_type, DocumentType::InvoicePdf);
    assert_eq!(docs.get(1).unwrap().cid, cid(&env, "QmKyc"));
    assert_eq!(docs.get(1).unwrap().doc_type, DocumentType::KycCertificate);
    assert_eq!(docs.get(2).unwrap().cid, cid(&env, "QmAudit"));
    assert_eq!(docs.get(2).unwrap().doc_type, DocumentType::AuditReport);
}

#[test]
fn test_get_documents_empty_for_invoice_with_none_registered() {
    let env = Env::default();
    let (_admin, client) = setup(&env);
    let id = invoice_id(&env, 10);
    assert_eq!(client.get_documents(&id).len(), 0);
}

#[test]
fn test_reregistration_of_same_cid_for_same_invoice_rejected() {
    let env = Env::default();
    let (admin, client) = setup(&env);
    let id = invoice_id(&env, 11);
    let hash = cid(&env, "QmDuplicate");

    client.register_document(&admin, &id, &hash, &DocumentType::InvoicePdf);
    assert_eq!(
        client.try_register_document(&admin, &id, &hash, &DocumentType::InvoicePdf),
        Err(Ok(Error::DuplicateCid))
    );
    // Still only one entry, not two.
    assert_eq!(client.get_documents(&id).len(), 1);
}

#[test]
fn test_same_cid_allowed_across_different_invoices() {
    let env = Env::default();
    let (admin, client) = setup(&env);
    let id_a = invoice_id(&env, 12);
    let id_b = invoice_id(&env, 13);
    let hash = cid(&env, "QmSharedAcrossInvoices");

    client.register_document(&admin, &id_a, &hash, &DocumentType::InvoicePdf);
    // Same CID, different invoice: not a duplicate for this contract's purposes.
    client.register_document(&admin, &id_b, &hash, &DocumentType::InvoicePdf);

    assert!(client.verify_document(&id_a, &hash));
    assert!(client.verify_document(&id_b, &hash));
}

#[test]
fn test_register_document_rejects_empty_cid() {
    let env = Env::default();
    let (admin, client) = setup(&env);
    let id = invoice_id(&env, 14);
    assert_eq!(
        client.try_register_document(&admin, &id, &cid(&env, ""), &DocumentType::Other),
        Err(Ok(Error::InvalidCid))
    );
}

#[test]
fn test_set_issuer_rejects_zero_address() {
    let env = Env::default();
    let (admin, client) = setup(&env);
    let id = invoice_id(&env, 15);
    assert_eq!(
        client.try_set_issuer(&admin, &id, &zero_address(&env)),
        Err(Ok(Error::InvalidAddress))
    );
}

#[test]
fn test_document_registered_event_emitted_with_invoice_id_cid_and_type() {
    let env = Env::default();
    let (admin, client) = setup(&env);
    let id = invoice_id(&env, 16);
    let hash = cid(&env, "QmEventTest");

    client.register_document(&admin, &id, &hash, &DocumentType::Contract);

    let (topics, data) = last_event(&env);
    assert_eq!(topics.len(), 2);
    let name: Symbol = topics.get(0).unwrap().try_into_val(&env).unwrap();
    assert_eq!(name, Symbol::new(&env, "DocumentRegistered"));
    let event_id: BytesN<32> = topics.get(1).unwrap().try_into_val(&env).unwrap();
    assert_eq!(event_id, id);

    let (event_cid, event_type): (String, u32) = data.try_into_val(&env).unwrap();
    assert_eq!(event_cid, hash);
    assert_eq!(event_type, DocumentType::Contract as u32);
}

#[test]
fn test_no_update_function_exists_cids_are_immutable() {
    // There is no update_document method on the generated client at all;
    // this test exists to document that fact directly rather than leaving
    // it implicit. Attempting to call a nonexistent
    // `client.update_document(...)` here would be a compile error, which is
    // the enforcement mechanism (issue #521: "CIDs immutable after
    // registration -- no updates allowed").
    let env = Env::default();
    let (admin, client) = setup(&env);
    let id = invoice_id(&env, 17);
    let hash = cid(&env, "QmImmutable");
    client.register_document(&admin, &id, &hash, &DocumentType::InvoicePdf);

    let docs = client.get_documents(&id);
    assert_eq!(docs.len(), 1);
    assert_eq!(docs.get(0).unwrap().cid, hash);
}
