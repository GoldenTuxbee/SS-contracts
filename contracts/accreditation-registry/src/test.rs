#![cfg(test)]
#![allow(deprecated)]

use soroban_sdk::testutils::{Address as _, Events};
use soroban_sdk::{Address, Env, String, Symbol, TryFromVal, TryIntoVal, Val, Vec};

use crate::types::{AccreditationTier, InvoiceRiskTier};
use crate::{AccreditationRegistry, AccreditationRegistryClient, Error};

fn zero_address(env: &Env) -> Address {
    Address::from_string(&String::from_str(
        env,
        "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
    ))
}

fn setup<'a>(env: &Env) -> (Address, AccreditationRegistryClient<'a>) {
    env.mock_all_auths();
    let contract_id = env.register(AccreditationRegistry, ());
    let client = AccreditationRegistryClient::new(env, &contract_id);
    let admin = Address::generate(env);
    client.initialize(&admin);
    (admin, client)
}

/// Returns the topics and data of the most recently emitted event on `env`,
/// decoded from its raw XDR form.
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
    let contract_id = env.register(AccreditationRegistry, ());
    let client = AccreditationRegistryClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);
    assert_eq!(client.try_initialize(&admin), Err(Ok(Error::AlreadyInit)));
}

#[test]
fn test_uninitialized_calls_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(AccreditationRegistry, ());
    let client = AccreditationRegistryClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let wallet = Address::generate(&env);
    assert_eq!(
        client.try_set_tier(&admin, &wallet, &AccreditationTier::Accredited),
        Err(Ok(Error::NotInit))
    );
    assert_eq!(client.try_get_admin(), Err(Ok(Error::NotInit)));
}

#[test]
fn test_get_tier_defaults_to_unaccredited_for_unregistered_wallet() {
    let env = Env::default();
    let (_admin, client) = setup(&env);
    let wallet = Address::generate(&env);
    assert_eq!(client.get_tier(&wallet), AccreditationTier::Unaccredited);
}

#[test]
fn test_set_tier_restricted_to_admin() {
    let env = Env::default();
    let (_admin, client) = setup(&env);
    let impostor = Address::generate(&env);
    let wallet = Address::generate(&env);

    assert_eq!(
        client.try_set_tier(&impostor, &wallet, &AccreditationTier::Qualified),
        Err(Ok(Error::Unauthorized))
    );
    // Confirm the rejected call had no effect.
    assert_eq!(client.get_tier(&wallet), AccreditationTier::Unaccredited);
}

#[test]
fn test_set_tier_admin_can_set_and_change_tier() {
    let env = Env::default();
    let (admin, client) = setup(&env);
    let wallet = Address::generate(&env);

    client.set_tier(&admin, &wallet, &AccreditationTier::Accredited);
    assert_eq!(client.get_tier(&wallet), AccreditationTier::Accredited);

    client.set_tier(&admin, &wallet, &AccreditationTier::Qualified);
    assert_eq!(client.get_tier(&wallet), AccreditationTier::Qualified);

    client.set_tier(&admin, &wallet, &AccreditationTier::Unaccredited);
    assert_eq!(client.get_tier(&wallet), AccreditationTier::Unaccredited);
}

#[test]
fn test_check_eligible_all_tier_and_risk_combinations() {
    let env = Env::default();
    let (admin, client) = setup(&env);

    let tiers = [
        AccreditationTier::Unaccredited,
        AccreditationTier::Accredited,
        AccreditationTier::Qualified,
    ];
    let risks = [
        InvoiceRiskTier::Low,
        InvoiceRiskTier::Medium,
        InvoiceRiskTier::High,
    ];

    for tier in tiers.iter() {
        let wallet = Address::generate(&env);
        client.set_tier(&admin, &wallet, tier);
        for risk in risks.iter() {
            let expected = (*tier as u32) >= (*risk as u32);
            assert_eq!(
                client.check_eligible(&wallet, risk),
                expected,
                "tier {:?} vs risk {:?}",
                tier,
                risk
            );
        }
    }
}

#[test]
fn test_check_eligible_unaccredited_only_eligible_for_low_risk() {
    let env = Env::default();
    let (_admin, client) = setup(&env);
    // Never explicitly set: defaults to Unaccredited.
    let wallet = Address::generate(&env);

    assert!(client.check_eligible(&wallet, &InvoiceRiskTier::Low));
    assert!(!client.check_eligible(&wallet, &InvoiceRiskTier::Medium));
    assert!(!client.check_eligible(&wallet, &InvoiceRiskTier::High));
}

#[test]
fn test_check_eligible_accredited_covers_low_and_medium_not_high() {
    let env = Env::default();
    let (admin, client) = setup(&env);
    let wallet = Address::generate(&env);
    client.set_tier(&admin, &wallet, &AccreditationTier::Accredited);

    assert!(client.check_eligible(&wallet, &InvoiceRiskTier::Low));
    assert!(client.check_eligible(&wallet, &InvoiceRiskTier::Medium));
    assert!(!client.check_eligible(&wallet, &InvoiceRiskTier::High));
}

#[test]
fn test_check_eligible_qualified_covers_every_risk_tier() {
    let env = Env::default();
    let (admin, client) = setup(&env);
    let wallet = Address::generate(&env);
    client.set_tier(&admin, &wallet, &AccreditationTier::Qualified);

    assert!(client.check_eligible(&wallet, &InvoiceRiskTier::Low));
    assert!(client.check_eligible(&wallet, &InvoiceRiskTier::Medium));
    assert!(client.check_eligible(&wallet, &InvoiceRiskTier::High));
}

#[test]
fn test_accreditation_updated_event_emitted_with_wallet_old_and_new_tier() {
    let env = Env::default();
    let (admin, client) = setup(&env);
    let wallet = Address::generate(&env);

    client.set_tier(&admin, &wallet, &AccreditationTier::Accredited);
    let (topics, data) = last_event(&env);
    assert_eq!(topics.len(), 2);

    let event_name: Symbol = topics.get(0).unwrap().try_into_val(&env).unwrap();
    assert_eq!(event_name, Symbol::new(&env, "AccreditationUpdated"));

    let event_wallet: Address = topics.get(1).unwrap().try_into_val(&env).unwrap();
    assert_eq!(event_wallet, wallet);

    let (old_tier, new_tier): (u32, u32) = data.try_into_val(&env).unwrap();
    assert_eq!(old_tier, AccreditationTier::Unaccredited as u32);
    assert_eq!(new_tier, AccreditationTier::Accredited as u32);
}

#[test]
fn test_accreditation_updated_event_reflects_previous_tier_on_second_change() {
    let env = Env::default();
    let (admin, client) = setup(&env);
    let wallet = Address::generate(&env);

    client.set_tier(&admin, &wallet, &AccreditationTier::Accredited);
    client.set_tier(&admin, &wallet, &AccreditationTier::Qualified);

    let (topics, data) = last_event(&env);
    let event_wallet: Address = topics.get(1).unwrap().try_into_val(&env).unwrap();
    assert_eq!(event_wallet, wallet);

    let (old_tier, new_tier): (u32, u32) = data.try_into_val(&env).unwrap();
    assert_eq!(old_tier, AccreditationTier::Accredited as u32);
    assert_eq!(new_tier, AccreditationTier::Qualified as u32);
}

#[test]
fn test_initialize_rejects_zero_address() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(AccreditationRegistry, ());
    let client = AccreditationRegistryClient::new(&env, &contract_id);

    assert_eq!(
        client.try_initialize(&zero_address(&env)),
        Err(Ok(Error::InvalidAddress))
    );
}

#[test]
fn test_set_tier_rejects_zero_address_wallet() {
    let env = Env::default();
    let (admin, client) = setup(&env);
    assert_eq!(
        client.try_set_tier(&admin, &zero_address(&env), &AccreditationTier::Accredited),
        Err(Ok(Error::InvalidAddress))
    );
}
