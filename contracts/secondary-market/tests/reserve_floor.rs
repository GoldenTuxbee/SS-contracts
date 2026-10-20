use secondary_market::{Error, SecondaryMarket, SecondaryMarketClient};
use soroban_sdk::{
    testutils::{Address as _, EnvTestConfig},
    token::{Client as TokenClient, StellarAssetClient},
    Address, BytesN, Env,
};

struct Fixture {
    env: Env,
    client: SecondaryMarketClient<'static>,
    seller: Address,
    buyer: Address,
    fractions: Address,
    payment: Address,
    id: BytesN<32>,
}

impl Fixture {
    fn new() -> Self {
        let env = Env::new_with_config(EnvTestConfig {
            capture_snapshot_at_drop: false,
        });
        env.mock_all_auths();
        let contract = if let Some(directory) = std::env::var_os("SS_CONTRACT_WASM_DIR") {
            let path = std::path::Path::new(&directory).join("secondary_market.wasm");
            let wasm = std::fs::read(path).expect("built secondary-market WASM");
            env.register(wasm.as_slice(), ())
        } else {
            env.register(SecondaryMarket, ())
        };
        let client = SecondaryMarketClient::new(&env, &contract);
        let admin = Address::generate(&env);
        let seller = Address::generate(&env);
        let buyer = Address::generate(&env);
        let fractions = env
            .register_stellar_asset_contract_v2(admin.clone())
            .address();
        let payment = env
            .register_stellar_asset_contract_v2(admin.clone())
            .address();
        StellarAssetClient::new(&env, &fractions).mint(&seller, &100);
        StellarAssetClient::new(&env, &payment).mint(&buyer, &100_000);
        client.initialize(&admin);
        let id = BytesN::from_array(&env, &[1; 32]);
        Self {
            env,
            client,
            seller,
            buyer,
            fractions,
            payment,
            id,
        }
    }

    fn configure(&self, remaining: i128, total: i128) {
        self.client
            .set_floor_inputs(&self.id, &self.fractions, &self.payment, &remaining, &total);
    }

    fn list(&self, price: i128) -> Result<u32, Result<Error, soroban_sdk::InvokeError>> {
        self.client
            .try_create_listing(
                &self.seller,
                &self.id,
                &self.fractions,
                &self.payment,
                &10,
                &price,
                &1_000_000,
            )
            .map(|result| result.unwrap())
    }
}

#[test]
fn listings_below_reserve_fail_before_tokens_move_and_exact_floor_succeeds() {
    let f = Fixture::new();
    f.configure(10_000, 100);
    assert_eq!(f.list(99), Err(Ok(Error::FloorPriceViolation)));
    assert_eq!(
        TokenClient::new(&f.env, &f.fractions).balance(&f.seller),
        100
    );
    assert_eq!(
        TokenClient::new(&f.env, &f.fractions).balance(&f.client.address),
        0
    );
    let listing = f.list(100).unwrap();
    f.client.buy_listing(&f.buyer, &listing, &10);
    assert_eq!(
        TokenClient::new(&f.env, &f.payment).balance(&f.seller),
        1000
    );
    assert_eq!(TokenClient::new(&f.env, &f.fractions).balance(&f.buyer), 10);
}

#[test]
fn floor_getter_reports_formula_and_live_yield_and_multiplier_changes() {
    let f = Fixture::new();
    f.configure(10_000, 100);
    f.client.set_floor_multiplier(&7500);
    let result = f.client.get_floor_price(&f.id);
    assert_eq!(
        (
            result.minimum_price,
            result.remaining_yield,
            result.total_fractions,
            result.multiplier_bps
        ),
        (75, 10_000, 100, 7500)
    );
    assert_eq!(result.fraction_token, f.fractions);
    assert_eq!(result.payment_token, f.payment);
    f.client.set_remaining_yield(&f.id, &20_000);
    assert_eq!(f.client.compute_floor_price(&f.id), 150);
    f.client.set_floor_multiplier(&5000);
    assert_eq!(f.client.compute_floor_price(&f.id), 100);
    f.client.set_remaining_yield(&f.id, &0);
    assert_eq!(f.client.compute_floor_price(&f.id), 0);
}

#[test]
fn a_live_floor_increase_also_blocks_stale_listings_at_purchase() {
    let f = Fixture::new();
    f.configure(10_000, 100);
    let listing = f.list(100).unwrap();
    f.client.set_remaining_yield(&f.id, &20_000);
    assert_eq!(
        f.client.try_buy_listing(&f.buyer, &listing, &10),
        Err(Ok(Error::FloorPriceViolation))
    );
    assert_eq!(
        TokenClient::new(&f.env, &f.payment).balance(&f.buyer),
        100_000
    );
    assert_eq!(f.client.get_listing(&listing).quantity, 10);
    f.client.set_remaining_yield(&f.id, &5000);
    f.client.buy_listing(&f.buyer, &listing, &10);
    assert_eq!(TokenClient::new(&f.env, &f.fractions).balance(&f.buyer), 10);
}

#[test]
fn floor_math_preserves_small_prices_and_full_i128_range() {
    let f = Fixture::new();
    f.configure(3, 2);
    f.client.set_floor_multiplier(&7500);
    // ceil(3 * .75 / 2) = 2; dividing yield first would incorrectly give zero.
    assert_eq!(f.client.compute_floor_price(&f.id), 2);
    f.configure(1, 100);
    assert_eq!(f.client.compute_floor_price(&f.id), 1);
    f.configure(i128::MAX, 1);
    f.client.set_floor_multiplier(&10_000);
    assert_eq!(f.client.compute_floor_price(&f.id), i128::MAX);
    f.configure(i128::MAX, i128::MAX);
    assert_eq!(f.client.compute_floor_price(&f.id), 1);
}

#[test]
fn only_admin_can_change_yield_supply_assets_or_multiplier() {
    let f = Fixture::new();
    f.configure(10_000, 100);
    f.env.set_auths(&[]);
    assert!(f.client.try_set_floor_multiplier(&5000).is_err());
    assert!(f.client.try_set_remaining_yield(&f.id, &0).is_err());
    assert!(f
        .client
        .try_set_floor_inputs(&f.id, &f.fractions, &f.payment, &0, &100)
        .is_err());
    assert_eq!(f.client.get_floor_price(&f.id).minimum_price, 100);
}

#[test]
fn invalid_inputs_are_rejected_and_asset_substitution_cannot_bypass_floor() {
    let f = Fixture::new();
    assert_eq!(
        f.client.try_get_floor_price(&f.id),
        Err(Ok(Error::FloorNotConfigured))
    );
    assert_eq!(
        f.client
            .try_set_floor_inputs(&f.id, &f.fractions, &f.payment, &-1, &100),
        Err(Ok(Error::InvalidFloorInputs))
    );
    assert_eq!(
        f.client
            .try_set_floor_inputs(&f.id, &f.fractions, &f.payment, &10_000, &0),
        Err(Ok(Error::InvalidFloorInputs))
    );
    assert_eq!(
        f.client.try_set_floor_multiplier(&0),
        Err(Ok(Error::InvalidFloorMultiplier))
    );
    assert_eq!(
        f.client.try_set_floor_multiplier(&10_001),
        Err(Ok(Error::InvalidFloorMultiplier))
    );
    f.configure(10_000, 100);
    assert_eq!(
        f.client
            .try_create_listing(&f.seller, &f.id, &f.payment, &f.payment, &1, &100, &1_000_000),
        Err(Ok(Error::FloorAssetMismatch))
    );
    let forged_id = BytesN::from_array(&f.env, &[99; 32]);
    assert_eq!(
        f.client.try_create_listing(
            &f.seller,
            &forged_id,
            &f.fractions,
            &f.payment,
            &1,
            &1,
            &1_000_000
        ),
        Err(Ok(Error::FloorAssetMismatch))
    );
}
