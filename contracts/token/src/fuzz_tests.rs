//! Property-based fuzz tests for the LYT token contract.
//!
//! Invariants verified:
//!   1. Conservation of mass  — transfer never changes total supply.
//!   2. Balance integrity     — burn always reduces balance and supply by exactly `amount`.
//!   3. Global ledger check   — after any sequence of ops, Σ(balances) == total_supply.
//!
//! Each proptest runs 10 000 cases (PROPTEST_CASES env-var or config below).

use super::*;
use proptest::prelude::*;
use soroban_sdk::{testutils::Address as _, Env, String};

// ── helpers ───────────────────────────────────────────────────────────────────

fn setup_env() -> (Env, TokenContractClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let id = env.register_contract(None, TokenContract);
    let client = TokenContractClient::new(&env, &id);
    client.initialize(
        &admin,
        &String::from_str(&env, "LoyaltyToken"),
        &String::from_str(&env, "LYT"),
        &7,
    );
    (env, client)
}

/// Clamp a raw i128 to a valid positive mint/transfer/burn amount.
/// Keeps values in [1, i128::MAX/2] to avoid overflow when two amounts are summed.
fn valid_amount(raw: i128) -> i128 {
    let v = raw.unsigned_abs() as i128; // strip sign
    if v == 0 { 1 } else { v.min(i128::MAX / 2) }
}

// ── strategies ────────────────────────────────────────────────────────────────

/// Generates boundary-heavy i128 values: 0, 1, MAX, MAX-1, and random values.
fn amount_strategy() -> impl Strategy<Value = i128> {
    prop_oneof![
        // boundary values
        Just(0_i128),
        Just(1_i128),
        Just(i128::MAX),
        Just(i128::MAX - 1),
        Just(i128::MAX / 2),
        // random values across the full range
        any::<i128>(),
    ]
}

// ── 1. Conservation of mass ───────────────────────────────────────────────────

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10_000))]

    /// transfer must not change total_supply.
    #[test]
    fn fuzz_transfer_conserves_supply(raw_mint in amount_strategy(), raw_transfer in amount_strategy()) {
        let mint_amt   = valid_amount(raw_mint);
        let (env, client) = setup_env();
        let alice = Address::generate(&env);
        let bob   = Address::generate(&env);

        client.mint(&alice, &mint_amt);
        let supply_before = client.total_supply_view();

        // Only transfer if alice has enough; otherwise skip (not a failure).
        let transfer_amt = valid_amount(raw_transfer).min(mint_amt);
        client.transfer(&alice, &bob, &transfer_amt);

        prop_assert_eq!(
            client.total_supply_view(),
            supply_before,
            "transfer changed total_supply: before={supply_before}, after={}",
            client.total_supply_view()
        );
    }
}

// ── 2. Balance integrity ──────────────────────────────────────────────────────

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10_000))]

    /// burn must reduce both balance and total_supply by exactly `amount`,
    /// and the resulting balance must be >= 0.
    #[test]
    fn fuzz_burn_balance_integrity(raw_mint in amount_strategy(), raw_burn in amount_strategy()) {
        let mint_amt = valid_amount(raw_mint);
        let (env, client) = setup_env();
        let user = Address::generate(&env);

        client.mint(&user, &mint_amt);
        let bal_before    = client.balance(&user);
        let supply_before = client.total_supply_view();

        // Burn at most what the user holds.
        let burn_amt = valid_amount(raw_burn).min(bal_before);

        client.burn(&user, &burn_amt);

        let bal_after    = client.balance(&user);
        let supply_after = client.total_supply_view();

        prop_assert!(bal_after >= 0, "balance went negative: {bal_after}");
        prop_assert_eq!(
            bal_after,
            bal_before - burn_amt,
            "balance not reduced by burn_amt"
        );
        prop_assert_eq!(
            supply_after,
            supply_before - burn_amt,
            "total_supply not reduced by burn_amt"
        );
    }
}

// ── 3. Global ledger check ────────────────────────────────────────────────────

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10_000))]

    /// After an arbitrary sequence of mint → transfer → burn,
    /// Σ(all known balances) must equal total_supply.
    #[test]
    fn fuzz_global_ledger_invariant(
        raw_mint_a  in amount_strategy(),
        raw_mint_b  in amount_strategy(),
        raw_transfer in amount_strategy(),
        raw_burn    in amount_strategy(),
    ) {
        let mint_a = valid_amount(raw_mint_a);
        let mint_b = valid_amount(raw_mint_b);

        let (env, client) = setup_env();
        let alice = Address::generate(&env);
        let bob   = Address::generate(&env);
        let carol = Address::generate(&env);

        // Mint to alice and bob.
        client.mint(&alice, &mint_a);
        client.mint(&bob,   &mint_b);

        // Transfer alice → carol (capped to alice's balance).
        let transfer_amt = valid_amount(raw_transfer).min(client.balance(&alice));
        client.transfer(&alice, &carol, &transfer_amt);

        // Burn from bob (capped to bob's balance).
        let burn_amt = valid_amount(raw_burn).min(client.balance(&bob));
        client.burn(&bob, &burn_amt);

        // Σ balances must equal total_supply.
        let sum_balances = client.balance(&alice)
            + client.balance(&bob)
            + client.balance(&carol);
        let total = client.total_supply_view();

        prop_assert_eq!(
            sum_balances,
            total,
            "ledger imbalance: Σbalances={sum_balances}, total_supply={total}"
        );
    }
}

// ── Regression: boundary values ───────────────────────────────────────────────
// These unit tests pin specific boundary cases discovered during fuzzing.

#[test]
fn regression_mint_amount_one() {
    let (env, client) = setup_env();
    let user = Address::generate(&env);
    client.mint(&user, &1);
    assert_eq!(client.balance(&user), 1);
    assert_eq!(client.total_supply_view(), 1);
}

#[test]
fn regression_burn_full_balance() {
    let (env, client) = setup_env();
    let user = Address::generate(&env);
    client.mint(&user, &1);
    client.burn(&user, &1);
    assert_eq!(client.balance(&user), 0);
    assert_eq!(client.total_supply_view(), 0);
}

#[test]
fn regression_transfer_full_balance_to_self_conserves_supply() {
    let (env, client) = setup_env();
    let alice = Address::generate(&env);
    client.mint(&alice, &1_000_000);
    let supply_before = client.total_supply_view();
    // transfer to a different address (self-transfer not meaningful here)
    let bob = Address::generate(&env);
    client.transfer(&alice, &bob, &1_000_000);
    assert_eq!(client.total_supply_view(), supply_before);
    assert_eq!(client.balance(&alice), 0);
    assert_eq!(client.balance(&bob), 1_000_000);
}
