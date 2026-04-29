/// Property-based tests for campaign expiry logic.
///
/// Core invariant under test:
///   is_active(campaign_id) ⟺ campaign.active && ledger.timestamp() < campaign.expiration
use crate::{CampaignContract, CampaignContractClient};
use proptest::prelude::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, Bytes, Env,
};

// ── Helpers ───────────────────────────────────────────────────────────────────

fn setup_env() -> (Env, CampaignContractClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();
    let id = env.register_contract(None, CampaignContract);
    let client = CampaignContractClient::new(&env, &id);
    let mut admins = soroban_sdk::Vec::new(&env);
    admins.push_back(Address::generate(&env));
    client.initialize(&admins, &1);
    (env, client)
}

fn short_bytes(env: &Env, s: &[u8]) -> Bytes {
    Bytes::from_slice(env, s)
}

/// Create a campaign that expires at `expiration` (must be > current ledger ts).
/// Returns the campaign id.
fn create_campaign(env: &Env, client: &CampaignContractClient, expiration: u64) -> u64 {
    let merchant = Address::generate(env);
    client.create_campaign(
        &merchant,
        &100,
        &expiration,
        &short_bytes(env, b"Test"),
        &short_bytes(env, b"Test desc"),
        &0,
    )
}

// ── Strategies ────────────────────────────────────────────────────────────────

/// Unix epoch 0 through year 2106 (u32::MAX seconds).
fn ts_strategy() -> impl Strategy<Value = u64> {
    0u64..=u32::MAX as u64
}

/// Year 2100 timestamp (far future edge case): 2100-01-01 00:00:00 UTC
const YEAR_2100: u64 = 4_102_444_800;

// ── Properties ────────────────────────────────────────────────────────────────

proptest! {
    #![proptest_config(ProptestConfig::with_cases(
        std::env::var("PROPTEST_CASES")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1_000)
    ))]

    /// Property 1: campaign is active iff current_time < expiration (and active flag is set).
    ///
    /// For any `now` and `expiration` where `expiration > now` at creation time,
    /// after advancing the ledger to `query_time`:
    ///   is_active == (query_time < expiration)
    #[test]
    fn prop_active_iff_before_expiry(
        // creation_offset: how far in the future expiration is from t=0
        creation_offset in 1u64..=86_400u64,
        // query_delta: signed offset from expiration to query at
        // negative = before expiry, 0 = at expiry, positive = after expiry
        query_delta in -10i64..=10i64,
    ) {
        let (env, client) = setup_env();
        // Start ledger at t=1 so we can set expiration > 0
        env.ledger().with_mut(|l| l.timestamp = 1);

        let expiration = 1u64 + creation_offset;
        let cid = create_campaign(&env, &client, expiration);

        // Advance ledger to query_time (clamped to 0 to avoid underflow)
        let query_time = (expiration as i64 + query_delta).max(0) as u64;
        env.ledger().with_mut(|l| l.timestamp = query_time);

        let expected = query_time < expiration;
        prop_assert_eq!(client.is_active(&cid), expected,
            "is_active mismatch: query_time={query_time}, expiration={expiration}");
    }

    /// Property 2: expired campaigns cannot be claimed regardless of active flag.
    ///
    /// Once ledger.timestamp() >= expiration, is_active must return false
    /// even if campaign.active == true.
    #[test]
    fn prop_expired_campaign_never_active(
        expiration in 2u64..=86_400u64,
        past_offset in 0u64..=3_600u64,
    ) {
        let (env, client) = setup_env();
        env.ledger().with_mut(|l| l.timestamp = 1);

        let cid = create_campaign(&env, &client, expiration);

        // Advance to at or past expiration
        let query_time = expiration + past_offset;
        env.ledger().with_mut(|l| l.timestamp = query_time);

        prop_assert!(!client.is_active(&cid),
            "expired campaign must not be active: query_time={query_time}, expiration={expiration}");
    }

    /// Property 3: inactive flag overrides timestamp — deactivated campaigns are
    /// never active regardless of expiration.
    #[test]
    fn prop_inactive_flag_overrides_timestamp(
        creation_offset in 1u64..=86_400u64,
        query_offset in 0u64..=3_600u64,
    ) {
        let (env, client) = setup_env();
        env.ledger().with_mut(|l| l.timestamp = 1);

        let expiration = 1u64 + creation_offset;
        let cid = create_campaign(&env, &client, expiration);

        // Deactivate while still before expiry
        client.set_active(&cid, &false);

        // Query at any time before expiry
        let query_time = (1u64 + query_offset).min(expiration - 1);
        env.ledger().with_mut(|l| l.timestamp = query_time);

        prop_assert!(!client.is_active(&cid),
            "deactivated campaign must not be active: query_time={query_time}, expiration={expiration}");
    }

    /// Property 4: is_active is consistent — it returns the same value when
    /// called twice with the same ledger state (no side effects).
    #[test]
    fn prop_is_active_is_idempotent(
        creation_offset in 1u64..=86_400u64,
        query_delta in -5i64..=5i64,
    ) {
        let (env, client) = setup_env();
        env.ledger().with_mut(|l| l.timestamp = 1);

        let expiration = 1u64 + creation_offset;
        let cid = create_campaign(&env, &client, expiration);

        let query_time = (expiration as i64 + query_delta).max(0) as u64;
        env.ledger().with_mut(|l| l.timestamp = query_time);

        prop_assert_eq!(client.is_active(&cid), client.is_active(&cid),
            "is_active must be idempotent");
    }
}

// ── Explicit edge-case tests ──────────────────────────────────────────────────

#[test]
fn edge_expiry_at_unix_epoch_plus_one() {
    // Minimum meaningful expiration: t=1 (one second past epoch).
    // Campaign created at t=0, expires at t=1.
    let (env, client) = setup_env();
    // ledger starts at 0 by default in Soroban test env
    let expiration = 1u64;
    let cid = create_campaign(&env, &client, expiration);

    // At t=0: active
    assert!(client.is_active(&cid), "should be active before expiry");

    // At t=1 (== expiration): expired
    env.ledger().with_mut(|l| l.timestamp = 1);
    assert!(!client.is_active(&cid), "should be inactive at expiration");
}

#[test]
fn edge_expiry_one_second_before_and_after() {
    let (env, client) = setup_env();
    let expiration = 1_000u64;
    let cid = create_campaign(&env, &client, expiration);

    // One second before: active
    env.ledger().with_mut(|l| l.timestamp = expiration - 1);
    assert!(client.is_active(&cid), "one second before expiry: must be active");

    // Exactly at expiry: inactive
    env.ledger().with_mut(|l| l.timestamp = expiration);
    assert!(!client.is_active(&cid), "at expiry: must be inactive");

    // One second after: inactive
    env.ledger().with_mut(|l| l.timestamp = expiration + 1);
    assert!(!client.is_active(&cid), "one second after expiry: must be inactive");
}

#[test]
fn edge_expiry_far_future_year_2100() {
    // Expiration set to 2100-01-01 00:00:00 UTC.
    let (env, client) = setup_env();
    let cid = create_campaign(&env, &client, YEAR_2100);

    // Query at a recent timestamp (2026): active
    let now_2026: u64 = 1_756_512_000; // approx 2025-09-01
    env.ledger().with_mut(|l| l.timestamp = now_2026);
    assert!(client.is_active(&cid), "far-future campaign must be active in 2026");

    // Query at exactly year 2100: inactive
    env.ledger().with_mut(|l| l.timestamp = YEAR_2100);
    assert!(!client.is_active(&cid), "campaign must be inactive at year-2100 expiry");
}
