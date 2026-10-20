#![allow(dead_code)]
//! Core fraction-balance arithmetic for issue #648. Not wired into the
//! contractimpl yet - mint/burn/transfer entry points, the restriction-check
//! integration and events are left for follow-up.

pub fn can_transfer(balance: i128, amount: i128) -> bool {
    amount > 0 && amount <= balance
}

pub fn apply_mint(balance: i128, amount: i128) -> i128 {
    balance + amount
}

pub fn apply_burn(balance: i128, amount: i128) -> Option<i128> {
    if amount <= 0 || amount > balance {
        None
    } else {
        Some(balance - amount)
    }
}
