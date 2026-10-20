#![allow(dead_code)]
//! Core dynamic-yield adjustment math for issue #649. Not wired into
//! contractimpl yet - per-issuer storage, history count, automatic use on
//! new invoices and the DynamicYieldUpdated event are left for follow-up.

pub const ON_TIME_ADJUST_BPS: i32 = -10;
pub const LATE_ADJUST_BPS: i32 = 15;

pub fn adjust_yield(current_bps: u32, on_time: bool) -> u32 {
    let delta = if on_time { ON_TIME_ADJUST_BPS } else { LATE_ADJUST_BPS };
    (current_bps as i32 + delta).max(0) as u32
}
