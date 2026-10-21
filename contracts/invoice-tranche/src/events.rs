use soroban_sdk::{symbol_short, Address, BytesN, Env};

pub fn tranche_investment_recorded(
    env: &Env,
    invoice_id: &BytesN<32>,
    investor: &Address,
    tranche_type: u32,
    amount: i128,
) {
    env.events().publish(
        (symbol_short!("tr_inv"), invoice_id.clone()),
        (investor.clone(), tranche_type, amount),
    );
}

pub fn tranche_settled(
    env: &Env,
    invoice_id: &BytesN<32>,
    senior_payout: i128,
    junior_payout: i128,
) {
    env.events().publish(
        (symbol_short!("tr_sett"), invoice_id.clone()),
        (senior_payout, junior_payout),
    );
}
