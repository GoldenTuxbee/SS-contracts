use soroban_sdk::{Env, U256};

use crate::Error;

/// Full-width multiplication avoids overflowing valid i128 token amounts.
pub fn mul_div(env: &Env, a: i128, b: i128, divisor: i128) -> Result<i128, Error> {
    if a < 0 || b < 0 || divisor <= 0 {
        return Err(Error::InvalidAmount);
    }
    let result = U256::from_u128(env, a as u128)
        .mul(&U256::from_u128(env, b as u128))
        .div(&U256::from_u128(env, divisor as u128))
        .to_u128()
        .ok_or(Error::Overflow)?;
    i128::try_from(result).map_err(|_| Error::Overflow)
}
