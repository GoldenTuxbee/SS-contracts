use soroban_sdk::{Address, BytesN, Env, Symbol};

pub fn cooldown_active(env: &Env, buyer: &Address, invoice_id: &BytesN<32>, seconds_remaining: u64) {
    let topics = (Symbol::new(env, "CooldownActive"), buyer.clone());
    env.events().publish(topics, (invoice_id.clone(), seconds_remaining));
}

pub fn buy_recorded(env: &Env, buyer: &Address, invoice_id: &BytesN<32>, timestamp: u64) {
    let topics = (Symbol::new(env, "BuyRecorded"), buyer.clone());
    env.events().publish(topics, (invoice_id.clone(), timestamp));
}
