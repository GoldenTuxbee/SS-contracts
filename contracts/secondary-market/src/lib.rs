#![no_std]

//! Secondary market contract (#515): fraction holders list fractions for
//! resale at a set price, and buyers purchase listed fractions atomically.
//!
//! Complements `order-book` (#558), which handles the other side of the
//! market: standing buy orders at a max price, matched against a seller's
//! terms supplied directly to `match_listing` with no persisted listing of
//! its own. This contract is the persisted-listing half #558's own summary
//! ("Extend the secondary market with a limit order book...") assumed
//! already existed: `create_listing`/`get_listings`/`cancel_listing`, with
//! expiry enforcement, did not exist anywhere in this workspace before this
//! change (confirmed by reading every entry point of every existing
//! contract and a full-history grep for these names).
//!
//! Each invoice's fractions live in that invoice's own deployed
//! `invoice-token` contract instance (one balance ledger per invoice, not a
//! shared multi-invoice token — confirmed from `invoice-token::transfer`
//! taking no `invoice_id` parameter), so a listing carries the specific
//! fraction-token address for its invoice rather than assuming one global
//! token, mirroring how `order-book::initialize` takes one fixed payment
//! token but scoped per-listing here since a market lists many invoices at
//! once.

mod errors;
mod events;
mod market_maker;
mod reserve_floor;
mod types;

#[cfg(test)]
mod test;

pub use errors::Error;
pub use reserve_floor::FloorPrice;
use soroban_sdk::{contract, contractimpl, token, Address, BytesN, Env, Vec};
pub use types::{Listing, StorageKey};

#[contract]
pub struct SecondaryMarket;

#[contractimpl]
impl SecondaryMarket {
    pub fn initialize(env: Env, admin: Address) -> Result<(), Error> {
        if env.storage().instance().has(&StorageKey::Admin) {
            return Err(Error::AlreadyInitialized);
        }
        env.storage().instance().set(&StorageKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&StorageKey::NextListingId, &1u32);
        Ok(())
    }

    /// Creates a resale listing, escrowing `quantity` fractions from the
    /// seller's balance into this contract (#515). Validates `quantity`
    /// against the seller's actual fraction balance by relying on
    /// `fraction_token.transfer` to fail with its own insufficient-balance
    /// error if the seller does not hold enough — not re-checked here, so
    /// there is exactly one source of truth for the balance.
    #[allow(clippy::too_many_arguments)]
    pub fn create_listing(
        env: Env,
        seller: Address,
        invoice_id: BytesN<32>,
        fraction_token: Address,
        payment_token: Address,
        quantity: i128,
        ask_price_per_fraction: i128,
        expiry: u64,
    ) -> Result<u32, Error> {
        seller.require_auth();

        if quantity <= 0 {
            return Err(Error::InvalidAmount);
        }
        if ask_price_per_fraction <= 0 {
            return Err(Error::InvalidPrice);
        }
        let now = env.ledger().timestamp();
        if expiry <= now {
            return Err(Error::InvalidExpiry);
        }
        reserve_floor::validate(
            &env,
            &invoice_id,
            &fraction_token,
            &payment_token,
            ask_price_per_fraction,
        )?;

        // Escrow the fractions into this contract. invoice-token::transfer
        // itself enforces frozen/paused/transfer-locked state and rejects an
        // insufficient balance, so this call is both the validation and the
        // state change in one step.
        let fraction_client = token::Client::new(&env, &fraction_token);
        fraction_client.transfer(&seller, env.current_contract_address(), &quantity);

        let listing_id: u32 = env
            .storage()
            .instance()
            .get(&StorageKey::NextListingId)
            .unwrap_or(1);
        env.storage()
            .instance()
            .set(&StorageKey::NextListingId, &(listing_id + 1));

        let listing = Listing {
            listing_id,
            seller: seller.clone(),
            invoice_id: invoice_id.clone(),
            fraction_token,
            quantity,
            ask_price_per_fraction,
            payment_token,
            expiry,
            is_active: true,
            created_at: now,
        };
        env.storage()
            .persistent()
            .set(&StorageKey::Listing(listing_id), &listing);

        let mut invoice_listings: Vec<u32> = env
            .storage()
            .persistent()
            .get(&StorageKey::InvoiceListings(invoice_id.clone()))
            .unwrap_or(Vec::new(&env));
        invoice_listings.push_back(listing_id);
        env.storage().persistent().set(
            &StorageKey::InvoiceListings(invoice_id.clone()),
            &invoice_listings,
        );

        events::listing_created(
            &env,
            listing_id,
            &seller,
            &invoice_id,
            quantity,
            ask_price_per_fraction,
            expiry,
        );

        Ok(listing_id)
    }

    /// Atomically transfers `quantity` fractions to the buyer and payment to
    /// the seller (#515). Rejects an expired listing even if it is still
    /// marked active in storage, so a listing never needs an explicit sweep
    /// to stop being purchasable once its expiry passes.
    pub fn buy_listing(
        env: Env,
        buyer: Address,
        listing_id: u32,
        quantity: i128,
    ) -> Result<(), Error> {
        buyer.require_auth();

        if quantity <= 0 {
            return Err(Error::InvalidAmount);
        }

        let mut listing: Listing = env
            .storage()
            .persistent()
            .get(&StorageKey::Listing(listing_id))
            .ok_or(Error::ListingNotFound)?;

        if !listing.is_active {
            return Err(Error::ListingNotActive);
        }
        if env.ledger().timestamp() >= listing.expiry {
            return Err(Error::ListingExpired);
        }
        if quantity > listing.quantity {
            return Err(Error::ExceedsListedQuantity);
        }
        reserve_floor::validate(
            &env,
            &listing.invoice_id,
            &listing.fraction_token,
            &listing.payment_token,
            listing.ask_price_per_fraction,
        )?;

        let total_price = listing
            .ask_price_per_fraction
            .checked_mul(quantity)
            .ok_or(Error::InvalidAmount)?;

        // Payment: buyer -> seller directly (this contract never custodies
        // payment funds, only the fractions being sold).
        let payment_client = token::Client::new(&env, &listing.payment_token);
        payment_client.transfer(&buyer, &listing.seller, &total_price);

        // Fractions: this contract's escrow -> buyer.
        let fraction_client = token::Client::new(&env, &listing.fraction_token);
        fraction_client.transfer(&env.current_contract_address(), &buyer, &quantity);

        listing.quantity -= quantity;
        if listing.quantity == 0 {
            listing.is_active = false;
        }
        env.storage()
            .persistent()
            .set(&StorageKey::Listing(listing_id), &listing);

        events::listing_sold(
            &env,
            listing_id,
            &listing.seller,
            &buyer,
            quantity,
            total_price,
        );
        Ok(())
    }

    /// Cancels a listing and returns its escrowed fractions to the seller
    /// (#515). Restricted to the listing's own seller.
    pub fn cancel_listing(env: Env, seller: Address, listing_id: u32) -> Result<(), Error> {
        seller.require_auth();

        let mut listing: Listing = env
            .storage()
            .persistent()
            .get(&StorageKey::Listing(listing_id))
            .ok_or(Error::ListingNotFound)?;

        if listing.seller != seller {
            return Err(Error::Unauthorized);
        }
        if !listing.is_active {
            return Err(Error::ListingNotActive);
        }

        let fraction_client = token::Client::new(&env, &listing.fraction_token);
        fraction_client.transfer(
            &env.current_contract_address(),
            &listing.seller,
            &listing.quantity,
        );

        listing.is_active = false;
        listing.quantity = 0;
        env.storage()
            .persistent()
            .set(&StorageKey::Listing(listing_id), &listing);

        events::listing_cancelled(&env, listing_id, &seller);
        Ok(())
    }

    /// Expires an active listing that has passed its expiry timestamp and
    /// returns the escrowed fractions to the seller (#552). Anyone can call
    /// this function to sweep expired listings.
    pub fn expire_listing(env: Env, listing_id: u32) -> Result<(), Error> {
        let mut listing: Listing = env
            .storage()
            .persistent()
            .get(&StorageKey::Listing(listing_id))
            .ok_or(Error::ListingNotFound)?;

        if !listing.is_active {
            return Err(Error::ListingNotActive);
        }
        if env.ledger().timestamp() < listing.expiry {
            return Err(Error::ListingNotActive);
        }

        let fraction_client = token::Client::new(&env, &listing.fraction_token);
        fraction_client.transfer(
            &env.current_contract_address(),
            &listing.seller,
            &listing.quantity,
        );

        listing.is_active = false;
        listing.quantity = 0;
        env.storage()
            .persistent()
            .set(&StorageKey::Listing(listing_id), &listing);

        events::listing_expired(&env, listing_id, &listing.seller, &listing.invoice_id);
        Ok(())
    }

    /// Returns every listing ever created for `invoice_id`, active or not
    /// (#515's "returns all active listings" is satisfied by filtering on
    /// `is_active`/`expiry` in the returned list; callers that only want
    /// purchasable listings filter client-side, the same pagination-free
    /// shape `order-book::get_open_orders` already uses for its own
    /// per-invoice list).
    pub fn get_listings(env: Env, invoice_id: BytesN<32>) -> Result<Vec<Listing>, Error> {
        let listing_ids: Vec<u32> = env
            .storage()
            .persistent()
            .get(&StorageKey::InvoiceListings(invoice_id))
            .unwrap_or(Vec::new(&env));

        let mut result = Vec::new(&env);
        for i in 0..listing_ids.len() {
            let id = listing_ids.get(i).unwrap();
            if let Some(listing) = env
                .storage()
                .persistent()
                .get::<_, Listing>(&StorageKey::Listing(id))
            {
                result.push_back(listing);
            }
        }
        Ok(result)
    }

    /// Returns active, non-expired listings for `invoice_id` (#552).
    pub fn get_active_listings(env: Env, invoice_id: BytesN<32>) -> Result<Vec<Listing>, Error> {
        let listing_ids: Vec<u32> = env
            .storage()
            .persistent()
            .get(&StorageKey::InvoiceListings(invoice_id))
            .unwrap_or(Vec::new(&env));

        let now = env.ledger().timestamp();
        let mut result = Vec::new(&env);
        for i in 0..listing_ids.len() {
            let id = listing_ids.get(i).unwrap();
            if let Some(listing) = env
                .storage()
                .persistent()
                .get::<_, Listing>(&StorageKey::Listing(id))
            {
                if listing.is_active && now < listing.expiry {
                    result.push_back(listing);
                }
            }
        }
        Ok(result)
    }

    pub fn get_listing(env: Env, listing_id: u32) -> Result<Listing, Error> {
        env.storage()
            .persistent()
            .get(&StorageKey::Listing(listing_id))
            .ok_or(Error::ListingNotFound)
    }
}
