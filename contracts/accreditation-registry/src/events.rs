#![allow(deprecated)]
//! Events emitted by the accreditation registry contract.

use soroban_sdk::{Address, Env, Symbol};

use crate::types::AccreditationTier;

/// Emitted when the registry is initialized.
pub fn initialized(env: &Env, admin: &Address) {
    let topics = (Symbol::new(env, "initialized"),);
    env.events().publish(topics, admin.clone());
}

/// Emitted when a wallet's accreditation tier is set or changed.
/// Topics: (AccreditationUpdated, wallet); Data: (old_tier, new_tier).
pub fn accreditation_updated(
    env: &Env,
    wallet: &Address,
    old_tier: AccreditationTier,
    new_tier: AccreditationTier,
) {
    let topics = (Symbol::new(env, "AccreditationUpdated"), wallet.clone());
    env.events()
        .publish(topics, (old_tier as u32, new_tier as u32));
}
