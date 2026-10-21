#![allow(deprecated)]
//! Events emitted by the platform pause contract.

use soroban_sdk::{Address, BytesN, Env, Symbol};

/// Emitted when the contract is initialized.
pub fn initialized(env: &Env, admin: &Address) {
    let topics = (Symbol::new(env, "initialized"),);
    env.events().publish(topics, admin.clone());
}

/// Emitted when the platform is paused.
/// Topics: (PlatformPaused,); Data: (timestamp, actor).
pub fn platform_paused(env: &Env, actor: &Address) {
    let topics = (Symbol::new(env, "PlatformPaused"),);
    env.events()
        .publish(topics, (env.ledger().timestamp(), actor.clone()));
}

/// Emitted when a resume is queued (timelock started).
/// Topics: (ResumeQueued,); Data: (eta, actor).
pub fn resume_queued(env: &Env, eta: u64, actor: &Address) {
    let topics = (Symbol::new(env, "ResumeQueued"),);
    env.events().publish(topics, (eta, actor.clone()));
}

/// Emitted when the platform resumes after its timelock elapses.
/// Topics: (PlatformResumed,); Data: (timestamp, actor).
pub fn platform_resumed(env: &Env, actor: &Address) {
    let topics = (Symbol::new(env, "PlatformResumed"),);
    env.events()
        .publish(topics, (env.ledger().timestamp(), actor.clone()));
}

/// Emitted when a per-invoice pause override is set or cleared.
/// Topics: (InvoicePauseSet, invoice_id); Data: paused.
pub fn invoice_pause_set(env: &Env, invoice_id: &BytesN<32>, paused: bool) {
    let topics = (Symbol::new(env, "InvoicePauseSet"), invoice_id.clone());
    env.events().publish(topics, paused);
}
