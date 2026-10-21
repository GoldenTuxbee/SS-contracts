# Invoice custody and risk controls

Issues #651, #652 and #655 extend `invoice-escrow`; #653 extends
`secondary-market`. These are callable methods on the existing contracts, so
deployments do not need four extra contracts or callback addresses. New state
uses separate storage keys; existing invoice and listing struct encodings are
unchanged. Configure controls explicitly before opening the relevant invoices.

## Beneficial ownership (#651)

Custody applies to token-backed `BytesN<32>` invoices created with
`create_invoice` or `create_invoice_legacy`:

1. An investor acquires a position through `invest`.
2. The initialized administrator calls
   `register_custody(custodian, beneficiary, invoice_id, amount)` to reserve
   some or all of that position for its beneficiary. An amount above the
   custodian's actual position and duplicate registration are rejected.
3. `get_beneficial_owner(custodian, invoice_id)` returns the beneficiary;
   `get_custody` additionally returns the reserved amount.
4. `transfer_custody(custodian, new_custodian, invoice_id)` moves the reserved
   position and ownership record together. The old custodian, beneficiary and
   receiving custodian authorize the invocation. The receiving custodian must
   not already have a custody record for the same invoice. Investor-count and
   per-investor amount caps still apply.

Ordinary position transfers cannot dispose of custody claims. Partial refunds
may withdraw only the position above the reserved amount. Additional investments
are direct holdings until explicitly registered; they do not silently change an
existing beneficiary's claim.

For settlement, release the raised principal with `release_to_issuer`, then:

1. Repay through `deposit_repayment(invoice_id, payer, amount)`, which pulls
   actual payment tokens from the authenticated payer. Multiple deposits are
   allowed before a settlement proposal.
2. Configure two administrators through `set_emergency_config` with threshold
   two, as required by the existing settlement authorization flow.
3. Call `propose_settlement` (or its `settle_invoice` alias) for exactly the
   deposited total, at least the raised principal.
4. A different administrator calls `approve_settlement`.

Settlement transfers each reserved share to its beneficiary, with any
unreserved share paid directly to its holder. Proportional amounts use full-width
arithmetic, with final rounding dust assigned to the last investor. Repayment
deposits are accounted separately from principal and collateral. The invoice
becomes terminal, claims are consumed, and repeat settlement is rejected.

`CustodyRegistered` records invoice, custodian, beneficiary and amount.
`CustodyTransferred` records invoice, old/new custodians, beneficiary and amount.
The legacy admin-registered metadata-only invoice API retains its existing
event-only settlement behavior; custody requires a token-backed invoice.

## Issuer collateral (#652)

The administrator first calls
`configure_collateral(issuer, payment_token, minimum_bps)`. For example, `2000`
requires collateral of 20% of outstanding debt. An account has one payment token
so collateral and invoice values use the same unit. Its configuration cannot be
overwritten to erase liabilities or deposits.

New funding invoices automatically add their target to outstanding debt. Legacy
Symbol escrows add face value less recorded repayments and remove debt at
settlement, cancellation or refund. For token-backed funding invoices,
`set_invoice_outstanding(invoice_id, amount)` is an administrator-authorized
update for accrued yield, partial repayment or importing an existing invoice;
it adjusts the issuer's aggregate by the change in debt. Configure accounts
before issuance when possible: historical funding invoices must be imported,
and historical Symbol escrows enter totals on their next state save.

`check_collateral_ratio(issuer_wallet)` returns `(ratio_bps, minimum_bps)`.
`get_collateral_status` additionally returns total collateral, total outstanding,
required top-up, payment token and issuance-block status. No outstanding debt
is represented by ratio `i128::MAX` and zero required top-up. Required deposits
round up to avoid leaving a fractional shortfall.

When a ratio is breached, the administrator calls `request_topup(issuer_wallet)`.
This emits `CollateralTopUpRequired`, including issuer, shortfall amount, totals
and ratios, and blocks new invoices from that issuer. The block covers the
threshold and legacy funding creation methods and Symbol escrow aliases.

The issuer calls `top_up_collateral(invoice_id, amount)` using one of its funding
invoice IDs. Actual payment tokens move from that issuer into escrow.
`CollateralToppedUp` includes issuer, invoice, deposit amount, new collateral,
ratio and block status. A partial deposit retains the block; restoring the ratio
clears it. Reducing outstanding debt to a healthy level also clears the block.
Collateral remains reserved when an invoice settles. All outgoing escrow token
payments preserve the aggregate collateral reserve for that token.

Accounts are opt-in. Unconfigured issuers retain existing behavior. Debt accrual
is supplied by the authorized accounting integration through
`set_invoice_outstanding`; the contract does not invent an accrual schedule.

## Secondary-market reserve floor (#653)

The market administrator calls
`set_floor_inputs(invoice_id, fraction_token, payment_token, remaining_yield, total_fractions)`.
Yield is nonnegative, total fractions must be positive, and both token addresses
are bound to that invoice. A configured fraction token cannot be listed under
another invoice ID to evade the floor. Assets cannot be changed after binding.

The minimum price per fraction, in payment-token units, is:

```text
ceil(remaining_yield * multiplier_bps / (total_fractions * 10_000))
```

The default multiplier is `10_000`. Only the administrator may call
`set_floor_multiplier`, with values from 1 to 10,000. One full-width division
avoids early truncation and overflow for valid `i128` inputs.

`compute_floor_price(invoice_id)` returns the minimum; `get_floor_price` also
returns remaining yield, total fractions, multiplier and token addresses.
`set_remaining_yield` updates the authoritative outstanding yield as invoices
accrue, repay or approach maturity. No separate time decay is applied beyond
the issue's yield/fractions/multiplier formula. Floor enforcement is opt-in per
invoice, preserving unconfigured listings' behavior.

`create_listing` rejects an ask below the current floor before moving fractions.
`buy_listing` rechecks the floor so a stale listing cannot evade an increased
reserve. Sellers may still cancel and recover fractions.

## Per-invoice investor caps (#655)

`set_investor_cap(invoice_id, max_investors)` requires the issuer's signature
before the first investment. The seller is the issuer for token-backed invoices;
the administrator is the issuer for the admin-only metadata registration API.
Caps range from 1 to the protocol's existing hard limit of 500. Once investment
starts, even withdrawing all positions does not reopen cap configuration.

`get_investor_count(invoice_id)` returns `{ current, max }`. Counts track unique
active position holders, including custody transfers and full withdrawals.
`invest` and its currency entry point share admission checks; existing holders
can increase their positions at the cap. Position transfers free the outgoing
holder's slot before admitting a replacement. Splitting custody into an extra
holder still requires a free slot.

An explicit invoice cap returns `InvestorCapReached` (escrow code 68). Existing
metadata invoices that use only the global default retain `MaxInvestorsReached`
(57). The global default also bounds new token-backed invoices.

Soroban contract errors contain numeric codes rather than dynamic payloads.
Clients obtain `current`/`max` from `get_investor_count`, and the minimum and
calculation breakdown from `get_floor_price` for `FloorPriceViolation` (market
code 12). Debug simulation diagnostics also include current/max or
attempted/minimum values. The TypeScript error SDK exports both contracts' codes.

## Verification

```sh
cargo test -p invoice-escrow --test institutional_controls --test fuzz_refund
cargo test -p secondary-market
# Stellar CLI 26.0.0 (validated) builds optimized, deployment-sized artifacts:
bash scripts/build-institutional.sh
# Exercise the same acceptance cases against the compiled release WASM:
export SS_CONTRACT_WASM_DIR="$PWD/target/institutional"
cargo test -p invoice-escrow --test institutional_controls
cargo test -p secondary-market --test reserve_floor
unset SS_CONTRACT_WASM_DIR
npm ci --prefix sdk
npm test --prefix sdk
```

The acceptance tests use deployed Stellar Asset Contracts and check token
balances, missing signatures, event fields, count changes, partial-custody
settlements, rounding, undercollateralization, reserved funds and price-floor
bypass attempts. They do not depend on the pre-existing escrow unit-test modules.
Release builds enable overflow checks and strip debug symbol names. The standard
Stellar optimized build retains contract specifications and metadata. The build
script enforces a 130,000-byte artifact budget, leaving room under Soroban's
128 KiB code-entry limit. Use these optimized artifacts for deployment; plain
Cargo output may be too large. WASM tests use default resource limits, resetting
the execution budget between contract deployment and subsequent invocations.
The dedicated `Institutional Invoice Controls` workflow runs native and WASM
acceptance tests independently of the existing escrow unit-test modules.
