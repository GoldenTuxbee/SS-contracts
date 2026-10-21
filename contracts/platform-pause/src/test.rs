#![cfg(test)]
#![allow(deprecated)]

use soroban_sdk::testutils::{Address as _, Events, Ledger};
use soroban_sdk::{Address, BytesN, Env, String, Symbol, TryFromVal, TryIntoVal, Val, Vec};

use crate::{Error, PlatformPause, PlatformPauseClient, RESUME_TIMELOCK_SECONDS};

fn zero_address(env: &Env) -> Address {
    Address::from_string(&String::from_str(
        env,
        "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
    ))
}

fn invoice_id(env: &Env, seed: u8) -> BytesN<32> {
    BytesN::from_array(env, &[seed; 32])
}

fn setup<'a>(env: &Env) -> (Address, PlatformPauseClient<'a>) {
    env.mock_all_auths();
    env.ledger().set_timestamp(1_000_000);
    let contract_id = env.register(PlatformPause, ());
    let client = PlatformPauseClient::new(env, &contract_id);
    let admin = Address::generate(env);
    client.initialize(&admin);
    (admin, client)
}

fn advance_time(env: &Env, by_seconds: u64) {
    let now = env.ledger().timestamp();
    env.ledger().set_timestamp(now + by_seconds);
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
    let contract_id = env.register(PlatformPause, ());
    let client = PlatformPauseClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);
    assert_eq!(client.try_initialize(&admin), Err(Ok(Error::AlreadyInit)));
}

#[test]
fn test_not_paused_by_default() {
    let env = Env::default();
    let (_admin, client) = setup(&env);
    assert!(!client.is_paused());
}

#[test]
fn test_pause_platform_halts_activity_immediately() {
    let env = Env::default();
    let (admin, client) = setup(&env);
    client.pause_platform(&admin);
    assert!(client.is_paused());
}

#[test]
fn test_pause_platform_restricted_to_admin() {
    let env = Env::default();
    let (_admin, client) = setup(&env);
    let impostor = Address::generate(&env);
    assert_eq!(
        client.try_pause_platform(&impostor),
        Err(Ok(Error::Unauthorized))
    );
    assert!(!client.is_paused());
}

#[test]
fn test_resume_platform_rejected_when_not_paused() {
    let env = Env::default();
    let (admin, client) = setup(&env);
    assert_eq!(
        client.try_resume_platform(&admin),
        Err(Ok(Error::NotPaused))
    );
}

#[test]
fn test_resume_platform_first_call_queues_timelock_without_resuming() {
    let env = Env::default();
    let (admin, client) = setup(&env);
    client.pause_platform(&admin);

    client.resume_platform(&admin);
    // Still paused: the first call only queues the resume.
    assert!(client.is_paused());
    assert!(client.resume_eta().is_some());
}

#[test]
fn test_resume_platform_blocked_until_24h_timelock_elapses() {
    let env = Env::default();
    let (admin, client) = setup(&env);
    client.pause_platform(&admin);
    client.resume_platform(&admin);

    // Immediately after queuing: still blocked.
    assert_eq!(
        client.try_resume_platform(&admin),
        Err(Ok(Error::TimelockNotElapsed))
    );

    // One second before the deadline: still blocked.
    advance_time(&env, RESUME_TIMELOCK_SECONDS - 1);
    assert_eq!(
        client.try_resume_platform(&admin),
        Err(Ok(Error::TimelockNotElapsed))
    );
    assert!(client.is_paused());
}

#[test]
fn test_resume_platform_succeeds_once_24h_timelock_elapses() {
    let env = Env::default();
    let (admin, client) = setup(&env);
    client.pause_platform(&admin);
    client.resume_platform(&admin);

    advance_time(&env, RESUME_TIMELOCK_SECONDS);
    client.resume_platform(&admin);

    assert!(!client.is_paused());
    assert!(client.resume_eta().is_none());
}

#[test]
fn test_resume_platform_succeeds_well_after_timelock_elapses() {
    let env = Env::default();
    let (admin, client) = setup(&env);
    client.pause_platform(&admin);
    client.resume_platform(&admin);

    advance_time(&env, RESUME_TIMELOCK_SECONDS + 10_000);
    client.resume_platform(&admin);
    assert!(!client.is_paused());
}

#[test]
fn test_resume_platform_restricted_to_admin() {
    let env = Env::default();
    let (admin, client) = setup(&env);
    let impostor = Address::generate(&env);
    client.pause_platform(&admin);

    assert_eq!(
        client.try_resume_platform(&impostor),
        Err(Ok(Error::Unauthorized))
    );
}

#[test]
fn test_pausing_again_clears_a_pending_resume() {
    let env = Env::default();
    let (admin, client) = setup(&env);
    client.pause_platform(&admin);
    client.resume_platform(&admin);
    let first_eta = client.resume_eta().unwrap();

    // A fresh pause supersedes the queued resume.
    client.pause_platform(&admin);
    assert!(client.resume_eta().is_none());

    advance_time(&env, RESUME_TIMELOCK_SECONDS);
    // The old timelock no longer applies: calling resume_platform now
    // queues a brand new one (relative to the current, later time) rather
    // than honoring the original, already-elapsed deadline.
    client.resume_platform(&admin);
    let second_eta = client.resume_eta().unwrap();
    assert!(second_eta > first_eta);
    assert!(client.is_paused());
}

#[test]
fn test_cancel_resume_clears_queued_timelock() {
    let env = Env::default();
    let (admin, client) = setup(&env);
    client.pause_platform(&admin);
    client.resume_platform(&admin);
    let first_eta = client.resume_eta().unwrap();

    client.cancel_resume(&admin);
    assert!(client.resume_eta().is_none());

    advance_time(&env, RESUME_TIMELOCK_SECONDS);
    // Cancelling reset the queue entirely: the next call starts a fresh
    // timelock from the current (later) time rather than resuming
    // immediately just because the original deadline has passed.
    client.resume_platform(&admin);
    let second_eta = client.resume_eta().unwrap();
    assert!(second_eta > first_eta);
    assert!(client.is_paused());
}

#[test]
fn test_cancel_resume_rejected_when_nothing_queued() {
    let env = Env::default();
    let (admin, client) = setup(&env);
    assert_eq!(
        client.try_cancel_resume(&admin),
        Err(Ok(Error::NoResumeQueued))
    );
}

#[test]
fn test_platform_paused_event_emitted_with_timestamp_and_actor() {
    let env = Env::default();
    let (admin, client) = setup(&env);
    client.pause_platform(&admin);

    let (topics, data) = last_event(&env);
    let name: Symbol = topics.get(0).unwrap().try_into_val(&env).unwrap();
    assert_eq!(name, Symbol::new(&env, "PlatformPaused"));

    let (timestamp, actor): (u64, Address) = data.try_into_val(&env).unwrap();
    assert_eq!(timestamp, env.ledger().timestamp());
    assert_eq!(actor, admin);
}

#[test]
fn test_platform_resumed_event_emitted_on_successful_resume() {
    let env = Env::default();
    let (admin, client) = setup(&env);
    client.pause_platform(&admin);
    client.resume_platform(&admin);
    advance_time(&env, RESUME_TIMELOCK_SECONDS);
    client.resume_platform(&admin);

    let (topics, _data) = last_event(&env);
    let name: Symbol = topics.get(0).unwrap().try_into_val(&env).unwrap();
    assert_eq!(name, Symbol::new(&env, "PlatformResumed"));
}

#[test]
fn test_invoice_pause_works_independently_of_platform_pause() {
    let env = Env::default();
    let (admin, client) = setup(&env);
    let id = invoice_id(&env, 1);

    // Platform not paused; invoice can still be paused on its own.
    assert!(!client.is_paused());
    client.set_invoice_paused(&admin, &id, &true);
    assert!(client.is_invoice_paused(&id));
    assert!(!client.is_paused());

    client.set_invoice_paused(&admin, &id, &false);
    assert!(!client.is_invoice_paused(&id));
}

#[test]
fn test_invoice_pause_independent_of_platform_pause_when_platform_also_paused() {
    let env = Env::default();
    let (admin, client) = setup(&env);
    let id_a = invoice_id(&env, 2);
    let id_b = invoice_id(&env, 3);

    client.set_invoice_paused(&admin, &id_a, &true);
    client.pause_platform(&admin);

    // Both flags are independently true/false; clearing one doesn't affect
    // the other.
    assert!(client.is_invoice_paused(&id_a));
    assert!(!client.is_invoice_paused(&id_b));
    assert!(client.is_paused());
}

#[test]
fn test_set_invoice_paused_restricted_to_admin() {
    let env = Env::default();
    let (_admin, client) = setup(&env);
    let impostor = Address::generate(&env);
    let id = invoice_id(&env, 4);
    assert_eq!(
        client.try_set_invoice_paused(&impostor, &id, &true),
        Err(Ok(Error::Unauthorized))
    );
    assert!(!client.is_invoice_paused(&id));
}

#[test]
fn test_initialize_rejects_zero_address() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(PlatformPause, ());
    let client = PlatformPauseClient::new(&env, &contract_id);
    assert_eq!(
        client.try_initialize(&zero_address(&env)),
        Err(Ok(Error::InvalidAddress))
    );
}
