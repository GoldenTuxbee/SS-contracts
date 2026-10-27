use soroban_sdk::{Address, Env, Symbol};

pub fn overcollateral_reward_granted(env: &Env, issuer: &Address, discount_bps: u32) {
    let topics = (Symbol::new(env, "OvercollateralRewardGranted"), issuer.clone());
    env.events().publish(topics, discount_bps);
}
