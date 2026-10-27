use soroban_sdk::{BytesN, Env, Symbol};

pub fn milestone_verified(env: &Env, invoice_id: &BytesN<32>, milestone_index: u32) {
    let topics = (Symbol::new(env, "MilestoneVerified"), invoice_id.clone());
    env.events().publish(topics, milestone_index);
}

pub fn tranche_disbursed(env: &Env, invoice_id: &BytesN<32>, milestone_index: u32, amount: i128) {
    let topics = (Symbol::new(env, "TrancheDisbursed"), invoice_id.clone());
    env.events().publish(topics, (milestone_index, amount));
}
