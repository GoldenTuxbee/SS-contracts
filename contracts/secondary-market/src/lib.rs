#![no_std]

mod events;
mod types;

pub use types::Error;
pub use types::{Listing, ListingStatus};
use types::StorageKey;

use soroban_sdk::{contract, contractimpl, token, Address, BytesN, Env, Vec};

fn admin(env: &Env) -> Result<Address, Error> {
    env.storage()
        .instance()
        .get(&StorageKey::Admin)
        .ok_or(Error::NotInitialized)
}

fn listing(env: &Env, id: u32) -> Result<Listing, Error> {
    env.storage()
        .persistent()
        .get(&StorageKey::Listing(id))
        .ok_or(Error::ListingNotFound)
}

fn save_listing(env: &Env, data: &Listing) {
    env.storage()
        .persistent()
        .set(&StorageKey::Listing(data.listing_id), data);
}

fn append_invoice_listing(env: &Env, invoice_id: &BytesN<32>, id: u32) {
    let key = StorageKey::InvoiceListings(invoice_id.clone());
    let mut ids: Vec<u32> = env
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or(Vec::new(&env));
    ids.push_back(id);
    env.storage().persistent().set(&key, &ids);
}

#[contract]
pub struct SecondaryMarket;

#[contractimpl]
impl SecondaryMarket {
    pub fn initialize(
        env: Env,
        admin: Address,
        fraction_token: Address,
        payment_token: Address,
    ) -> Result<(), Error> {
        if env.storage().instance().has(&StorageKey::Admin) {
            return Err(Error::AlreadyInitialized);
        }
        admin.require_auth();
        env.storage().instance().set(&StorageKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&StorageKey::FractionToken, &fraction_token);
        env.storage()
            .instance()
            .set(&StorageKey::PaymentToken, &payment_token);
        env.storage()
            .instance()
            .set(&StorageKey::NextListing, &1u32);
        Ok(())
    }

    pub fn create_listing(
        env: Env,
        seller: Address,
        invoice_id: BytesN<32>,
        fraction_amount: i128,
        price_per_fraction: i128,
        expiry_timestamp: u64,
    ) -> Result<u32, Error> {
        if fraction_amount <= 0 || price_per_fraction <= 0 {
            return Err(Error::InvalidAmount);
        }
        if expiry_timestamp <= env.ledger().timestamp() {
            return Err(Error::InvalidExpiry);
        }
        seller.require_auth();

        let id: u32 = env
            .storage()
            .instance()
            .get(&StorageKey::NextListing)
            .unwrap_or(1);
        let fraction_token: Address = env
            .storage()
            .instance()
            .get(&StorageKey::FractionToken)
            .ok_or(Error::NotInitialized)?;
        let client = token::Client::new(&env, &fraction_token);
        if client.balance(&seller) < fraction_amount {
            return Err(Error::InsufficientBalance);
        }
        client.transfer(&seller, &env.current_contract_address(), &fraction_amount);

        let listing = Listing {
            listing_id: id,
            seller: seller.clone(),
            invoice_id: invoice_id.clone(),
            fraction_amount,
            price_per_fraction,
            expiry_timestamp,
            status: ListingStatus::Active,
        };
        save_listing(&env, &listing);
        append_invoice_listing(&env, &invoice_id, id);
        env.storage()
            .instance()
            .set(&StorageKey::NextListing, &(id + 1));

        events::listing_created(&env, id, &seller, &invoice_id);
        Ok(id)
    }

    pub fn buy_listing(env: Env, buyer: Address, listing_id: u32) -> Result<i128, Error> {
        buyer.require_auth();
        let mut data = listing(&env, listing_id)?;
        if data.status != ListingStatus::Active {
            return Err(Error::ListingNotActive);
        }
        if env.ledger().timestamp() >= data.expiry_timestamp {
            return Err(Error::ListingExpired);
        }

        let cost = data
            .fraction_amount
            .checked_mul(data.price_per_fraction)
            .ok_or(Error::Overflow)?;
        if cost <= 0 {
            return Err(Error::InvalidAmount);
        }

        let payment_token: Address = env
            .storage()
            .instance()
            .get(&StorageKey::PaymentToken)
            .ok_or(Error::NotInitialized)?;
        let payment_client = token::Client::new(&env, &payment_token);
        if payment_client.balance(&buyer) < cost {
            return Err(Error::InsufficientBalance);
        }

        let fraction_token: Address = env
            .storage()
            .instance()
            .get(&StorageKey::FractionToken)
            .ok_or(Error::NotInitialized)?;
        let fraction_client = token::Client::new(&env, &fraction_token);

        payment_client
            .transfer(&buyer, &data.seller, &cost)
            ;

        fraction_client
            .transfer(
                &env.current_contract_address(),
                &buyer,
                &data.fraction_amount,
            )
            ;


        data.status = ListingStatus::Sold;
        save_listing(&env, &data);
        events::listing_bought(
            &env,
            listing_id,
            &buyer,
            &data.seller,
            &data.invoice_id,
            cost,
        );
        Ok(cost)
    }

    pub fn expire_listing(env: Env, listing_id: u32) -> Result<(), Error> {
        let mut data = listing(&env, listing_id)?;
        if data.status != ListingStatus::Active {
            return Err(Error::ListingNotActive);
        }
        if env.ledger().timestamp() < data.expiry_timestamp {
            return Err(Error::ListingNotActive);
        }

        let fraction_token: Address = env
            .storage()
            .instance()
            .get(&StorageKey::FractionToken)
            .ok_or(Error::NotInitialized)?;
        let client = token::Client::new(&env, &fraction_token);
        client
            .transfer(
                &env.current_contract_address(),
                &data.seller,
                &data.fraction_amount,
            )
            ;


        data.status = ListingStatus::Expired;
        save_listing(&env, &data);
        events::listing_expired(&env, listing_id, &data.seller, &data.invoice_id);
        Ok(())
    }

    pub fn get_active_listings(env: Env, invoice_id: BytesN<32>) -> Result<Vec<Listing>, Error> {
        admin(&env)?;
        let ids: Vec<u32> = env
            .storage()
            .persistent()
            .get(&StorageKey::InvoiceListings(invoice_id.clone()))
            .unwrap_or(Vec::new(&env));
        let now = env.ledger().timestamp();
        let mut active = Vec::new(&env);
        for i in 0..ids.len() {
            let id = ids.get(i).unwrap();
            if let Some(listing) = env.storage().persistent().get::<_, Listing>(&StorageKey::Listing(id)) {
                if listing.status == ListingStatus::Active && now < listing.expiry_timestamp {
                    active.push_back(listing);
                }
            }
        }
        Ok(active)
    }

    pub fn get_listing(env: Env, listing_id: u32) -> Result<Listing, Error> {
        listing(&env, listing_id)
    }
}
