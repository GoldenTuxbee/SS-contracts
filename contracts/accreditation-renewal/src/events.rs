use soroban_sdk::{symbol_short, Address, Env};

pub fn accreditation_renewal_reminder(
    env: &Env,
    wallet: &Address,
    days_remaining: u64,
    expiry_timestamp: u64,
) {
    env.events().publish(
        (symbol_short!("renew_rem"), wallet.clone()),
        (days_remaining, expiry_timestamp),
    );
}
