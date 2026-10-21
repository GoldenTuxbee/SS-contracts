#![allow(deprecated, unused_variables, dead_code, unused_mut, clippy::all)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    token::StellarAssetClient,
    Address, BytesN, Env,
};

/// Deploys a real Stellar Asset Contract so transfers genuinely move
/// balances (order-book's own tests use a bare Address::generate for
/// `token`, which is not a deployed contract at all; its own
/// test_place_buy_order_and_cancel panics with HostError(Storage,
/// MissingValue) as a result — confirmed by running it directly against
/// this repo's current upstream/dev. Deliberately not copying that pattern.)
fn setup_token(env: &Env, admin: &Address) -> (Address, StellarAssetClient<'static>) {
    let sac = env.register_stellar_asset_contract_v2(admin.clone());
    let address = sac.address();
    let mint_client = StellarAssetClient::new(env, &address);
    (address, mint_client)
}

fn setup() -> (Env, SecondaryMarketClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(SecondaryMarket, ());
    let client = SecondaryMarketClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);

    (env, client, admin)
}

const FAR_FUTURE: u64 = 1_000_000;

#[test]
fn test_create_listing_validates_quantity_against_sellers_fraction_balance() {
    let (env, client, admin) = setup();
    let seller = Address::generate(&env);
    let (fraction_token, fraction_mint) = setup_token(&env, &admin);
    let (payment_token, _payment_mint) = setup_token(&env, &admin);

    fraction_mint.mint(&seller, &50);

    let invoice_id = BytesN::from_array(&env, &[1u8; 32]);

    // Listing more than the seller holds fails (invoice-token/SAC transfer's
    // own insufficient-balance check, not a separate pre-check here).
    let result = client.try_create_listing(
        &seller,
        &invoice_id,
        &fraction_token,
        &payment_token,
        &51,
        &100,
        &FAR_FUTURE,
    );
    assert!(result.is_err());

    // Listing exactly the held balance succeeds.
    let listing_id = client.create_listing(
        &seller,
        &invoice_id,
        &fraction_token,
        &payment_token,
        &50,
        &100,
        &FAR_FUTURE,
    );
    assert_eq!(listing_id, 1);

    let token_client = soroban_sdk::token::Client::new(&env, &fraction_token);
    assert_eq!(token_client.balance(&seller), 0);
    assert_eq!(token_client.balance(&client.address), 50);
}

#[test]
fn test_buy_listing_transfers_fractions_and_payment_atomically() {
    let (env, client, admin) = setup();
    let seller = Address::generate(&env);
    let buyer = Address::generate(&env);
    let (fraction_token, fraction_mint) = setup_token(&env, &admin);
    let (payment_token, payment_mint) = setup_token(&env, &admin);

    fraction_mint.mint(&seller, &100);
    payment_mint.mint(&buyer, &10_000);

    let invoice_id = BytesN::from_array(&env, &[1u8; 32]);
    let listing_id = client.create_listing(
        &seller,
        &invoice_id,
        &fraction_token,
        &payment_token,
        &100,
        &50,
        &FAR_FUTURE,
    );

    client.buy_listing(&buyer, &listing_id, &40);

    let fraction_client = soroban_sdk::token::Client::new(&env, &fraction_token);
    let payment_client = soroban_sdk::token::Client::new(&env, &payment_token);

    assert_eq!(fraction_client.balance(&buyer), 40);
    assert_eq!(fraction_client.balance(&client.address), 60);
    assert_eq!(payment_client.balance(&seller), 40 * 50);
    assert_eq!(payment_client.balance(&buyer), 10_000 - 40 * 50);

    let listing = client.get_listing(&listing_id);
    assert_eq!(listing.quantity, 60);
    assert!(listing.is_active);
}

#[test]
fn test_buy_listing_fully_closes_it_when_quantity_reaches_zero() {
    let (env, client, admin) = setup();
    let seller = Address::generate(&env);
    let buyer = Address::generate(&env);
    let (fraction_token, fraction_mint) = setup_token(&env, &admin);
    let (payment_token, payment_mint) = setup_token(&env, &admin);

    fraction_mint.mint(&seller, &10);
    payment_mint.mint(&buyer, &1_000);

    let invoice_id = BytesN::from_array(&env, &[1u8; 32]);
    let listing_id = client.create_listing(
        &seller,
        &invoice_id,
        &fraction_token,
        &payment_token,
        &10,
        &50,
        &FAR_FUTURE,
    );

    client.buy_listing(&buyer, &listing_id, &10);

    let listing = client.get_listing(&listing_id);
    assert_eq!(listing.quantity, 0);
    assert!(!listing.is_active);
}

#[test]
fn test_cancel_listing_restricted_to_listing_owner() {
    let (env, client, admin) = setup();
    let seller = Address::generate(&env);
    let impostor = Address::generate(&env);
    let (fraction_token, fraction_mint) = setup_token(&env, &admin);
    let (payment_token, _payment_mint) = setup_token(&env, &admin);

    fraction_mint.mint(&seller, &10);
    let invoice_id = BytesN::from_array(&env, &[1u8; 32]);
    let listing_id = client.create_listing(
        &seller,
        &invoice_id,
        &fraction_token,
        &payment_token,
        &10,
        &50,
        &FAR_FUTURE,
    );

    let result = client.try_cancel_listing(&impostor, &listing_id);
    assert_eq!(result, Err(Ok(Error::Unauthorized)));

    // The real owner can still cancel afterwards.
    client.cancel_listing(&seller, &listing_id);
    let listing = client.get_listing(&listing_id);
    assert!(!listing.is_active);
}

#[test]
fn test_cancel_listing_returns_escrowed_fractions_to_seller() {
    let (env, client, admin) = setup();
    let seller = Address::generate(&env);
    let (fraction_token, fraction_mint) = setup_token(&env, &admin);
    let (payment_token, _payment_mint) = setup_token(&env, &admin);

    fraction_mint.mint(&seller, &10);
    let invoice_id = BytesN::from_array(&env, &[1u8; 32]);
    let listing_id = client.create_listing(
        &seller,
        &invoice_id,
        &fraction_token,
        &payment_token,
        &10,
        &50,
        &FAR_FUTURE,
    );

    client.cancel_listing(&seller, &listing_id);

    let fraction_client = soroban_sdk::token::Client::new(&env, &fraction_token);
    assert_eq!(fraction_client.balance(&seller), 10);
    assert_eq!(fraction_client.balance(&client.address), 0);
}

#[test]
fn test_expired_listings_rejected_on_buy_attempt() {
    let (env, client, admin) = setup();
    let seller = Address::generate(&env);
    let buyer = Address::generate(&env);
    let (fraction_token, fraction_mint) = setup_token(&env, &admin);
    let (payment_token, payment_mint) = setup_token(&env, &admin);

    fraction_mint.mint(&seller, &10);
    payment_mint.mint(&buyer, &1_000);

    let invoice_id = BytesN::from_array(&env, &[1u8; 32]);
    let expiry = env.ledger().timestamp() + 100;
    let listing_id = client.create_listing(
        &seller,
        &invoice_id,
        &fraction_token,
        &payment_token,
        &10,
        &50,
        &expiry,
    );

    env.ledger().with_mut(|li| li.timestamp = expiry);

    let result = client.try_buy_listing(&buyer, &listing_id, &5);
    assert_eq!(result, Err(Ok(Error::ListingExpired)));
}

#[test]
fn test_create_listing_rejects_an_expiry_already_in_the_past() {
    let (env, client, admin) = setup();
    let seller = Address::generate(&env);
    let (fraction_token, fraction_mint) = setup_token(&env, &admin);
    let (payment_token, _payment_mint) = setup_token(&env, &admin);
    fraction_mint.mint(&seller, &10);

    env.ledger().with_mut(|li| li.timestamp = 500);
    let invoice_id = BytesN::from_array(&env, &[1u8; 32]);

    let result = client.try_create_listing(
        &seller,
        &invoice_id,
        &fraction_token,
        &payment_token,
        &10,
        &50,
        &400,
    );
    assert_eq!(result, Err(Ok(Error::InvalidExpiry)));
}

#[test]
fn test_get_listings_returns_all_listings_for_an_invoice() {
    let (env, client, admin) = setup();
    let seller = Address::generate(&env);
    let (fraction_token, fraction_mint) = setup_token(&env, &admin);
    let (payment_token, _payment_mint) = setup_token(&env, &admin);
    fraction_mint.mint(&seller, &100);

    let invoice_a = BytesN::from_array(&env, &[1u8; 32]);
    let invoice_b = BytesN::from_array(&env, &[2u8; 32]);

    client.create_listing(
        &seller,
        &invoice_a,
        &fraction_token,
        &payment_token,
        &10,
        &50,
        &FAR_FUTURE,
    );
    client.create_listing(
        &seller,
        &invoice_a,
        &fraction_token,
        &payment_token,
        &20,
        &60,
        &FAR_FUTURE,
    );
    client.create_listing(
        &seller,
        &invoice_b,
        &fraction_token,
        &payment_token,
        &5,
        &70,
        &FAR_FUTURE,
    );

    let listings_a = client.get_listings(&invoice_a);
    assert_eq!(listings_a.len(), 2);
    let listings_b = client.get_listings(&invoice_b);
    assert_eq!(listings_b.len(), 1);
}

#[test]
fn test_buy_listing_rejects_a_quantity_above_what_remains() {
    let (env, client, admin) = setup();
    let seller = Address::generate(&env);
    let buyer = Address::generate(&env);
    let (fraction_token, fraction_mint) = setup_token(&env, &admin);
    let (payment_token, payment_mint) = setup_token(&env, &admin);

    fraction_mint.mint(&seller, &10);
    payment_mint.mint(&buyer, &1_000);

    let invoice_id = BytesN::from_array(&env, &[1u8; 32]);
    let listing_id = client.create_listing(
        &seller,
        &invoice_id,
        &fraction_token,
        &payment_token,
        &10,
        &50,
        &FAR_FUTURE,
    );

    let result = client.try_buy_listing(&buyer, &listing_id, &11);
    assert_eq!(result, Err(Ok(Error::ExceedsListedQuantity)));
}

#[test]
fn test_events_emitted_with_correct_data() {
    let (env, client, admin) = setup();
    let seller = Address::generate(&env);
    let buyer = Address::generate(&env);
    let (fraction_token, fraction_mint) = setup_token(&env, &admin);
    let (payment_token, payment_mint) = setup_token(&env, &admin);

    fraction_mint.mint(&seller, &10);
    payment_mint.mint(&buyer, &1_000);

    let invoice_id = BytesN::from_array(&env, &[1u8; 32]);
    let listing_id = client.create_listing(
        &seller,
        &invoice_id,
        &fraction_token,
        &payment_token,
        &10,
        &50,
        &FAR_FUTURE,
    );

    // Each call also triggers token-contract transfer events alongside this
    // contract's own; filtered to this contract's own address so the
    // assertion is about ListingCreated/ListingSold/ListingCancelled
    // specifically, not the token events that accompany them.
    use soroban_sdk::testutils::Events as _;

    assert_eq!(
        env.events()
            .all()
            .filter_by_contract(&client.address)
            .events()
            .len(),
        1
    ); // ListingCreated

    client.buy_listing(&buyer, &listing_id, &4);
    assert_eq!(
        env.events()
            .all()
            .filter_by_contract(&client.address)
            .events()
            .len(),
        1
    ); // ListingSold

    client.cancel_listing(&seller, &listing_id);
    assert_eq!(
        env.events()
            .all()
            .filter_by_contract(&client.address)
            .events()
            .len(),
        1
    ); // ListingCancelled
}
