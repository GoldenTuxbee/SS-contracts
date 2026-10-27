//! Event definitions for the invoice hardship extension contract.

use soroban_sdk::{Env, String, Symbol};

/// Emitted when the admin grants a hardship extension to an invoice.
pub fn hardship_extension_granted(
    env: &Env,
    invoice_id: &Symbol,
    additional_days: u32,
    reason: &String,
) {
    let topics = (Symbol::new(env, "HardshipExtensionGranted"), invoice_id.clone());
    env.events()
        .publish(topics, (additional_days, reason.clone()));
}
