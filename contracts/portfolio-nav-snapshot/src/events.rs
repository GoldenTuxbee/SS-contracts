use soroban_sdk::{symbol_short, Address, Env};

pub fn portfolio_snapshot_taken(env: &Env, wallet: &Address, date: u64, nav_amount: i128) {
    env.events().publish(
        (symbol_short!("nav_snap"), wallet.clone()),
        (date, nav_amount),
    );
}
