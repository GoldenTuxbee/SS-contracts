#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    BytesN, Env,
};

fn setup() -> (Env, SecondaryCooldownClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, SecondaryCooldown);
    let client = SecondaryCooldownClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin).unwrap();
    (env, client, admin)
}

fn inv(env: &Env) -> BytesN<32> {
    BytesN::from_array(env, &[1u8; 32])
}

#[test]
fn test_buy_without_cooldown_succeeds() {
    let (env, client, _admin) = setup();
    let buyer = Address::generate(&env);
    let invoice = inv(&env);
    client.record_buy(&buyer, &invoice).unwrap();
}

#[test]
fn test_cooldown_blocks_immediate_rebuy() {
    let (env, client, _admin) = setup();
    let buyer = Address::generate(&env);
    let invoice = inv(&env);

    client.set_secondary_cooldown(&invoice, &3600).unwrap();
    client.record_buy(&buyer, &invoice).unwrap();

    // Immediate second buy should fail.
    assert_eq!(
        client.record_buy(&buyer, &invoice),
        Err(Ok(Error::CooldownActive))
    );
}

#[test]
fn test_cooldown_expires_correctly() {
    let (env, client, _admin) = setup();
    let buyer = Address::generate(&env);
    let invoice = inv(&env);

    client.set_secondary_cooldown(&invoice, &3600).unwrap();
    client.record_buy(&buyer, &invoice).unwrap();

    // Advance past cooldown.
    env.ledger().with_mut(|l| l.timestamp += 3601);
    client.record_buy(&buyer, &invoice).unwrap();
}

#[test]
fn test_cooldown_independent_per_wallet() {
    let (env, client, _admin) = setup();
    let buyer1 = Address::generate(&env);
    let buyer2 = Address::generate(&env);
    let invoice = inv(&env);

    client.set_secondary_cooldown(&invoice, &3600).unwrap();
    client.record_buy(&buyer1, &invoice).unwrap();

    // buyer2 has not bought yet, so they are not blocked.
    client.record_buy(&buyer2, &invoice).unwrap();
}

#[test]
fn test_get_cooldown_remaining() {
    let (env, client, _admin) = setup();
    let buyer = Address::generate(&env);
    let invoice = inv(&env);

    client.set_secondary_cooldown(&invoice, &3600).unwrap();
    assert_eq!(client.get_secondary_cooldown_remaining(&buyer, &invoice), 0);

    client.record_buy(&buyer, &invoice).unwrap();
    let remaining = client.get_secondary_cooldown_remaining(&buyer, &invoice);
    assert!(remaining > 0 && remaining <= 3600);
}
