#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, Env};

fn setup() -> (Env, CreditScoringClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, CreditScoring);
    let client = CreditScoringClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin).unwrap();
    (env, client, admin)
}

fn high_quality_factors(env: &Env) -> IssuerFactors {
    IssuerFactors {
        kyc_tier: 3,
        settlement_history: 10,
        default_count: 0,
        platform_tenure_days: 300,
    }
}

fn low_quality_factors(env: &Env) -> IssuerFactors {
    IssuerFactors {
        kyc_tier: 0,
        settlement_history: 0,
        default_count: 3,
        platform_tenure_days: 0,
    }
}

#[test]
fn test_aaa_tier_for_high_quality_issuer() {
    let (env, client, admin) = setup();
    let issuer = Address::generate(&env);
    let score = client
        .compute_credit_score(&admin, &issuer, &high_quality_factors(&env))
        .unwrap();
    assert_eq!(score.tier, CreditTier::AAA);
    assert!(score.score >= 90);
}

#[test]
fn test_c_tier_for_defaulted_issuer() {
    let (env, client, admin) = setup();
    let issuer = Address::generate(&env);
    let score = client
        .compute_credit_score(&admin, &issuer, &low_quality_factors(&env))
        .unwrap();
    assert_eq!(score.tier, CreditTier::C);
}

#[test]
fn test_get_credit_score_returns_stored_value() {
    let (env, client, admin) = setup();
    let issuer = Address::generate(&env);
    let factors = high_quality_factors(&env);
    let computed = client
        .compute_credit_score(&admin, &issuer, &factors)
        .unwrap();
    let fetched = client.get_credit_score(&issuer).unwrap();
    assert_eq!(computed.score, fetched.score);
    assert_eq!(computed.tier, fetched.tier);
}

#[test]
fn test_unauthorised_caller_rejected() {
    let (env, client, _admin) = setup();
    let issuer = Address::generate(&env);
    let stranger = Address::generate(&env);
    assert_eq!(
        client.compute_credit_score(&stranger, &issuer, &high_quality_factors(&env)),
        Err(Ok(Error::Unauthorized))
    );
}

#[test]
fn test_authorised_contract_can_compute() {
    let (env, client, admin) = setup();
    let authorised = Address::generate(&env);
    let issuer = Address::generate(&env);
    client.set_authorised_caller(&authorised, &true).unwrap();
    let score = client
        .compute_credit_score(&authorised, &issuer, &high_quality_factors(&env))
        .unwrap();
    assert!(score.score > 0);
}

#[test]
fn test_score_factors_breakdown_accurate() {
    let (env, client, admin) = setup();
    let issuer = Address::generate(&env);
    let factors = IssuerFactors {
        kyc_tier: 2,
        settlement_history: 5,
        default_count: 1,
        platform_tenure_days: 100,
    };
    // kyc=40, settlement=25, tenure=10, default_penalty=25 → 50
    let score = client
        .compute_credit_score(&admin, &issuer, &factors)
        .unwrap();
    assert_eq!(score.score, 50);
    assert_eq!(score.tier, CreditTier::B);
}
