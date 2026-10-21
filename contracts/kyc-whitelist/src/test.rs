#![allow(deprecated, unused_variables, dead_code, unused_mut, clippy::all)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Events as _, Ledger as _},
    vec, Address, Env, IntoVal,
};

fn setup() -> (Env, KycWhitelistClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(KycWhitelist, ());
    let client = KycWhitelistClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);

    (env, client, admin)
}

#[test]
fn test_add_to_whitelist_restricted_to_admin() {
    let (env, client, admin) = setup();
    let investor = Address::generate(&env);

    // Admin-authorized call succeeds.
    client.add_to_whitelist(&vec![&env, investor.clone()]);
    assert!(client.is_whitelisted(&investor));

    // A non-admin caller fails auth (mock_auths restricts who actually signed).
    let impostor = Address::generate(&env);
    env.mock_auths(&[soroban_sdk::testutils::MockAuth {
        address: &impostor,
        invoke: &soroban_sdk::testutils::MockAuthInvoke {
            contract: &client.address,
            fn_name: "add_to_whitelist",
            args: (vec![&env, Address::generate(&env)],).into_val(&env),
            sub_invokes: &[],
        },
    }]);
    let result = client.try_add_to_whitelist(&vec![&env, Address::generate(&env)]);
    assert!(result.is_err());
}

#[test]
fn test_is_whitelisted_returns_correct_bool_for_all_wallets() {
    let (env, client, _admin) = setup();
    let listed = Address::generate(&env);
    let unlisted = Address::generate(&env);

    client.add_to_whitelist(&vec![&env, listed.clone()]);

    assert!(client.is_whitelisted(&listed));
    assert!(!client.is_whitelisted(&unlisted));
}

#[test]
fn test_is_accredited_matches_is_whitelisted() {
    let (env, client, _admin) = setup();
    let investor = Address::generate(&env);

    assert_eq!(
        client.is_accredited(&investor),
        client.is_whitelisted(&investor)
    );

    client.add_to_whitelist(&vec![&env, investor.clone()]);

    assert_eq!(
        client.is_accredited(&investor),
        client.is_whitelisted(&investor)
    );
    assert!(client.is_accredited(&investor));
}

#[test]
fn test_batch_add_handles_up_to_100_wallets() {
    // Hardcoded (not MAX_BATCH_SIZE) for the same reason as the over-100
    // test above: this must keep proving exactly 100 works, independent of
    // the constant's current value.
    let (env, client, _admin) = setup();

    let mut wallets = Vec::new(&env);
    for _ in 0..100u32 {
        wallets.push_back(Address::generate(&env));
    }

    client.add_to_whitelist(&wallets);

    for i in 0..wallets.len() {
        assert!(client.is_whitelisted(&wallets.get(i).unwrap()));
    }
}

#[test]
fn test_batch_add_over_100_wallets_is_rejected() {
    // #514's own acceptance criterion names 100 explicitly ("Batch add
    // handles up to 100 wallets per call"); hardcoded rather than derived
    // from MAX_BATCH_SIZE so this test still enforces that literal number
    // even if the constant is ever changed.
    let (env, client, _admin) = setup();

    let mut wallets = Vec::new(&env);
    for _ in 0..101u32 {
        wallets.push_back(Address::generate(&env));
    }

    let result = client.try_add_to_whitelist(&wallets);
    assert_eq!(result, Err(Ok(Error::BatchTooLarge)));
}

#[test]
fn test_empty_batch_is_rejected() {
    let (env, client, _admin) = setup();
    let result = client.try_add_to_whitelist(&Vec::new(&env));
    assert_eq!(result, Err(Ok(Error::EmptyBatch)));
}

#[test]
fn test_remove_from_whitelist_correctly_removes_wallet_from_active_list() {
    let (env, client, _admin) = setup();
    let investor = Address::generate(&env);

    client.add_to_whitelist(&vec![&env, investor.clone()]);
    assert!(client.is_whitelisted(&investor));

    client.remove_from_whitelist(&vec![&env, investor.clone()]);
    assert!(!client.is_whitelisted(&investor));
}

#[test]
fn test_remove_restricted_to_admin() {
    let (env, client, _admin) = setup();
    let investor = Address::generate(&env);
    client.add_to_whitelist(&vec![&env, investor.clone()]);

    let impostor = Address::generate(&env);
    env.mock_auths(&[soroban_sdk::testutils::MockAuth {
        address: &impostor,
        invoke: &soroban_sdk::testutils::MockAuthInvoke {
            contract: &client.address,
            fn_name: "remove_from_whitelist",
            args: (vec![&env, investor.clone()],).into_val(&env),
            sub_invokes: &[],
        },
    }]);
    let result = client.try_remove_from_whitelist(&vec![&env, investor.clone()]);
    assert!(result.is_err());
    // Unaffected by the failed removal attempt.
    assert!(client.is_whitelisted(&investor));
}

#[test]
fn test_whitelist_updated_event_emitted_on_every_add_and_remove() {
    let (env, client, _admin) = setup();
    let a = Address::generate(&env);
    let b = Address::generate(&env);

    client.add_to_whitelist(&vec![&env, a.clone(), b.clone()]);
    assert_eq!(env.events().all().events().len(), 1);

    client.remove_from_whitelist(&vec![&env, a.clone()]);
    assert_eq!(env.events().all().events().len(), 1);
}

#[test]
fn test_re_adding_a_removed_wallet_keeps_its_original_added_at() {
    let (env, client, _admin) = setup();
    let investor = Address::generate(&env);

    client.add_to_whitelist(&vec![&env, investor.clone()]);
    let first = client.get_whitelist(&0, &10);
    let original_added_at = first.get(0).unwrap().added_at;

    env.ledger().with_mut(|li| li.timestamp += 1_000);

    client.remove_from_whitelist(&vec![&env, investor.clone()]);
    client.add_to_whitelist(&vec![&env, investor.clone()]);

    let second = client.get_whitelist(&0, &10);
    assert_eq!(second.get(0).unwrap().added_at, original_added_at);
}

#[test]
fn test_get_whitelist_paginates_and_includes_removed_entries() {
    let (env, client, _admin) = setup();
    let a = Address::generate(&env);
    let b = Address::generate(&env);
    let c = Address::generate(&env);

    client.add_to_whitelist(&vec![&env, a.clone(), b.clone(), c.clone()]);
    client.remove_from_whitelist(&vec![&env, b.clone()]);

    let page0 = client.get_whitelist(&0, &2);
    assert_eq!(page0.len(), 2);
    assert_eq!(page0.get(0).unwrap().wallet, a);
    assert!(page0.get(0).unwrap().is_active);
    assert_eq!(page0.get(1).unwrap().wallet, b);
    assert!(!page0.get(1).unwrap().is_active);

    let page1 = client.get_whitelist(&1, &2);
    assert_eq!(page1.len(), 1);
    assert_eq!(page1.get(0).unwrap().wallet, c);
}

#[test]
fn test_double_initialize_is_rejected() {
    let (env, client, admin) = setup();
    let result = client.try_initialize(&admin);
    assert_eq!(result, Err(Ok(Error::AlreadyInitialized)));
}
