use crate::types::CreditTier;
use soroban_sdk::{Address, Env, Symbol};

pub fn credit_score_updated(env: &Env, issuer: &Address, old_tier: CreditTier, new_tier: CreditTier) {
    let topics = (Symbol::new(env, "CreditScoreUpdated"), issuer.clone());
    env.events().publish(topics, (old_tier as u32, new_tier as u32));
}
