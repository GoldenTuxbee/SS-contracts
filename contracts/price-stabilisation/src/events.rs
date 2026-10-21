use soroban_sdk::{symbol_short, BytesN, Env};

pub fn stabilisation_triggered(env: &Env, invoice_id: &BytesN<32>, drop_bps: u32, timestamp: u64) {
    env.events().publish(
        (symbol_short!("stab_trg"), invoice_id.clone()),
        (drop_bps, timestamp),
    );
}

pub fn stabilisation_order_placed(
    env: &Env,
    invoice_id: &BytesN<32>,
    order_id: u32,
    floor_price: i128,
    quantity: i128,
) {
    env.events().publish(
        (symbol_short!("ord_plac"), invoice_id.clone()),
        (order_id, floor_price, quantity),
    );
}
