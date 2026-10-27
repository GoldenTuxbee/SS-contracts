use soroban_sdk::{contractimpl, contracttype, log, Address, BytesN, Env, U256};

use crate::{Error, SecondaryMarket, SecondaryMarketArgs, SecondaryMarketClient, StorageKey};

const TTL_THRESHOLD: u32 = 120_960;
const TTL_EXTEND: u32 = 1_036_800;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FloorInputs {
    pub fraction_token: Address,
    pub payment_token: Address,
    pub remaining_yield: i128,
    pub total_fractions: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FloorPrice {
    pub minimum_price: i128,
    pub remaining_yield: i128,
    pub total_fractions: i128,
    pub multiplier_bps: u32,
    pub fraction_token: Address,
    pub payment_token: Address,
}

fn admin(env: &Env) -> Result<Address, Error> {
    env.storage()
        .instance()
        .extend_ttl(TTL_THRESHOLD, TTL_EXTEND);
    env.storage()
        .instance()
        .get(&StorageKey::Admin)
        .ok_or(Error::NotInitialized)
}

fn inputs(env: &Env, id: &BytesN<32>) -> Result<FloorInputs, Error> {
    let key = StorageKey::FloorInputs(id.clone());
    let inputs: FloorInputs = env
        .storage()
        .persistent()
        .get(&key)
        .ok_or(Error::FloorNotConfigured)?;
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_THRESHOLD, TTL_EXTEND);
    let binding_key = StorageKey::FloorInvoice(inputs.fraction_token.clone());
    if env.storage().persistent().has(&binding_key) {
        env.storage()
            .persistent()
            .extend_ttl(&binding_key, TTL_THRESHOLD, TTL_EXTEND);
    }
    Ok(inputs)
}

fn save(env: &Env, id: &BytesN<32>, inputs: &FloorInputs) {
    let key = StorageKey::FloorInputs(id.clone());
    env.storage().persistent().set(&key, inputs);
    env.storage()
        .persistent()
        .extend_ttl(&key, TTL_THRESHOLD, TTL_EXTEND);
}

fn price(env: &Env, id: &BytesN<32>) -> Result<FloorPrice, Error> {
    let inputs = inputs(env, id)?;
    let multiplier: u32 = env
        .storage()
        .instance()
        .get(&StorageKey::FloorMultiplier)
        .unwrap_or(10_000);
    let numerator =
        U256::from_u128(env, inputs.remaining_yield as u128).mul(&U256::from_u32(env, multiplier));
    let denominator =
        U256::from_u128(env, inputs.total_fractions as u128).mul(&U256::from_u32(env, 10_000));
    // One division preserves fractional precision. Round up so a positive
    // reserve cannot be bypassed by choosing a price below one token unit.
    let rounded = numerator
        .add(&denominator)
        .sub(&U256::from_u32(env, 1))
        .div(&denominator);
    let minimum_price =
        i128::try_from(rounded.to_u128().ok_or(Error::Overflow)?).map_err(|_| Error::Overflow)?;
    Ok(FloorPrice {
        minimum_price,
        remaining_yield: inputs.remaining_yield,
        total_fractions: inputs.total_fractions,
        multiplier_bps: multiplier,
        fraction_token: inputs.fraction_token,
        payment_token: inputs.payment_token,
    })
}

pub fn validate(
    env: &Env,
    id: &BytesN<32>,
    fraction: &Address,
    payment: &Address,
    attempted: i128,
) -> Result<(), Error> {
    admin(env)?;
    let binding_key = StorageKey::FloorInvoice(fraction.clone());
    if let Some(bound_id) = env
        .storage()
        .persistent()
        .get::<_, BytesN<32>>(&binding_key)
    {
        env.storage()
            .persistent()
            .extend_ttl(&binding_key, TTL_THRESHOLD, TTL_EXTEND);
        if bound_id != *id {
            return Err(Error::FloorAssetMismatch);
        }
    }
    if !env
        .storage()
        .persistent()
        .has(&StorageKey::FloorInputs(id.clone()))
    {
        return Ok(()); // Existing invoices retain their behavior until configured.
    }
    let floor = price(env, id)?;
    if floor.fraction_token != *fraction || floor.payment_token != *payment {
        return Err(Error::FloorAssetMismatch);
    }
    if attempted < floor.minimum_price {
        log!(
            env,
            "FloorPriceViolation: attempted/minimum",
            attempted,
            floor.minimum_price
        );
        return Err(Error::FloorPriceViolation);
    }
    Ok(())
}

#[contractimpl]
impl SecondaryMarket {
    /// Admin supplies authoritative remaining yield (after accrual/repayment)
    /// and the invoice's fraction supply, in the payment token's smallest unit.
    pub fn set_floor_inputs(
        env: Env,
        invoice_id: BytesN<32>,
        fraction_token: Address,
        payment_token: Address,
        remaining_yield: i128,
        total_fractions: i128,
    ) -> Result<(), Error> {
        admin(&env)?.require_auth();
        if remaining_yield < 0 || total_fractions <= 0 {
            return Err(Error::InvalidFloorInputs);
        }
        let binding_key = StorageKey::FloorInvoice(fraction_token.clone());
        if let Some(bound_id) = env
            .storage()
            .persistent()
            .get::<_, BytesN<32>>(&binding_key)
        {
            if bound_id != invoice_id {
                return Err(Error::FloorAssetMismatch);
            }
        }
        if let Ok(previous) = inputs(&env, &invoice_id) {
            // Keep invoice assets stable so existing listings cannot be
            // orphaned or the same fraction token rebound to bypass its floor.
            if previous.fraction_token != fraction_token || previous.payment_token != payment_token
            {
                return Err(Error::FloorAssetMismatch);
            }
        }
        env.storage().persistent().set(&binding_key, &invoice_id);
        env.storage()
            .persistent()
            .extend_ttl(&binding_key, TTL_THRESHOLD, TTL_EXTEND);
        save(
            &env,
            &invoice_id,
            &FloorInputs {
                fraction_token,
                payment_token,
                remaining_yield,
                total_fractions,
            },
        );
        Ok(())
    }

    pub fn set_remaining_yield(
        env: Env,
        invoice_id: BytesN<32>,
        remaining_yield: i128,
    ) -> Result<(), Error> {
        admin(&env)?.require_auth();
        if remaining_yield < 0 {
            return Err(Error::InvalidFloorInputs);
        }
        let mut inputs = inputs(&env, &invoice_id)?;
        inputs.remaining_yield = remaining_yield;
        save(&env, &invoice_id, &inputs);
        Ok(())
    }

    pub fn set_floor_multiplier(env: Env, multiplier_bps: u32) -> Result<(), Error> {
        admin(&env)?.require_auth();
        if multiplier_bps == 0 || multiplier_bps > 10_000 {
            return Err(Error::InvalidFloorMultiplier);
        }
        env.storage()
            .instance()
            .set(&StorageKey::FloorMultiplier, &multiplier_bps);
        Ok(())
    }

    pub fn compute_floor_price(env: Env, invoice_id: BytesN<32>) -> Result<i128, Error> {
        admin(&env)?;
        Ok(price(&env, &invoice_id)?.minimum_price)
    }

    pub fn get_floor_price(env: Env, invoice_id: BytesN<32>) -> Result<FloorPrice, Error> {
        admin(&env)?;
        price(&env, &invoice_id)
    }
}
