//! Event definitions for the investor portfolio health contract.

use soroban_sdk::{Address, Env, Symbol};

/// Emitted when the health score of a wallet was recomputed.
///
/// The wallet is the topic of the event, the data carries the score before
/// and after the recomputation.
pub fn health_score_updated(
    env: &Env,
    wallet: &Address,
    old_score: u32,
    new_score: u32,
    computed_at: u64,
) {
    let topics = (Symbol::new(env, "HealthScoreUpdated"), wallet.clone());
    env.events()
        .publish(topics, (old_score, new_score, computed_at));
}
