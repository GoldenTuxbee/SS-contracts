#![allow(dead_code)]
//! Core symmetric-spread quote math for issue #650. Not wired into
//! contractimpl yet - admin registration, listing cancel/replace, and
//! QuoteUpdated events are left for follow-up.

pub struct Quote {
    pub bid: i128,
    pub ask: i128,
}

pub fn quotes_at_spread(last_price: i128, spread_bps: u32) -> Quote {
    let delta = last_price * spread_bps as i128 / 10_000;
    Quote { bid: last_price - delta, ask: last_price + delta }
}

pub fn capped_position(position: i128, max_position: i128) -> i128 {
    position.clamp(-max_position, max_position)
}
