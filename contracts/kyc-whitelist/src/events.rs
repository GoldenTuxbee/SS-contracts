#![allow(deprecated)]

use soroban_sdk::{Address, Env, Symbol, Vec};

/// Published on every add and every remove (#514), batch or single. `added`
/// carries the wallets that changed state in this call (not the whole
/// whitelist), so an indexer can apply a diff rather than re-reading
/// everything on each event.
pub fn whitelist_updated(env: &Env, wallets: &Vec<Address>, added: bool) {
    let topics = (Symbol::new(env, "WhitelistUpdated"), added);
    let timestamp = env.ledger().timestamp();
    env.events().publish(topics, (wallets.clone(), timestamp));
}
