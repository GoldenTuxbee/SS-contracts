#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::{Client as TokenClient, StellarAssetClient},
    BytesN, Env, Vec,
};

fn setup() -> (Env, InvoiceMilestoneClient<'static>, Address, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, InvoiceMilestone);
    let client = InvoiceMilestoneClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let issuer = Address::generate(&env);

    let token_admin = Address::generate(&env);
    let token_id = env.register_stellar_asset_contract_v2(token_admin.clone());
    let token_client = TokenClient::new(&env, &token_id.address());
    let sac = StellarAssetClient::new(&env, &token_id.address());
    sac.mint(&contract_id, &10_000);

    client
        .initialize(&admin, &token_client.address)
        .unwrap();

    (env, client, admin, issuer, token_client.address)
}

fn invoice_id(env: &Env) -> BytesN<32> {
    BytesN::from_array(env, &[1u8; 32])
}

fn make_milestones(env: &Env) -> Vec<Milestone> {
    let mut v = Vec::new(env);
    v.push_back(Milestone {
        amount: 500,
        description_hash: BytesN::from_array(env, &[10u8; 32]),
        verified: false,
    });
    v.push_back(Milestone {
        amount: 300,
        description_hash: BytesN::from_array(env, &[11u8; 32]),
        verified: false,
    });
    v
}

#[test]
fn test_create_and_verify_milestone() {
    let (env, client, _admin, issuer, _token) = setup();
    let inv = invoice_id(&env);
    let milestones = make_milestones(&env);

    client.create_milestones(&inv, &issuer, &milestones).unwrap();

    let statuses = client.get_milestone_status(&inv).unwrap();
    assert_eq!(statuses.len(), 2);
    assert!(!statuses.get(0).unwrap().verified);

    client.verify_milestone(&inv, &0).unwrap();

    let statuses = client.get_milestone_status(&inv).unwrap();
    assert!(statuses.get(0).unwrap().verified);
    assert!(!statuses.get(1).unwrap().verified);
}

#[test]
fn test_double_verify_rejected() {
    let (env, client, _admin, issuer, _token) = setup();
    let inv = invoice_id(&env);
    client
        .create_milestones(&inv, &issuer, &make_milestones(&env))
        .unwrap();
    client.verify_milestone(&inv, &0).unwrap();
    assert_eq!(
        client.verify_milestone(&inv, &0),
        Err(Ok(Error::AlreadyVerified))
    );
}

#[test]
fn test_invalid_index_rejected() {
    let (env, client, _admin, issuer, _token) = setup();
    let inv = invoice_id(&env);
    client
        .create_milestones(&inv, &issuer, &make_milestones(&env))
        .unwrap();
    assert_eq!(
        client.verify_milestone(&inv, &99),
        Err(Ok(Error::InvalidIndex))
    );
}

#[test]
fn test_duplicate_milestones_rejected() {
    let (env, client, _admin, issuer, _token) = setup();
    let inv = invoice_id(&env);
    let milestones = make_milestones(&env);
    client.create_milestones(&inv, &issuer, &milestones).unwrap();
    assert_eq!(
        client.create_milestones(&inv, &issuer, &milestones),
        Err(Ok(Error::MilestonesAlreadySet))
    );
}
