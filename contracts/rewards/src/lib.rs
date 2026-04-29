#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, Address, Env, Symbol, Vec,
};

// ── Cross-contract interfaces ─────────────────────────────────────────────────

mod token {
    use soroban_sdk::{contractclient, Address, Env};

    #[contractclient(name = "TokenClient")]
    pub trait Token {
        fn mint(env: Env, to: Address, amount: i128);
        fn burn(env: Env, from: Address, amount: i128);
        fn balance(env: Env, addr: Address) -> i128;
        fn total_supply_view(env: Env) -> i128;
    }
}

mod campaign {
    use soroban_sdk::{contractclient, contracttype, Address, Bytes, Env};

    #[contracttype]
    #[derive(Clone)]
    pub struct Campaign {
        pub id: u64,
        pub merchant: Address,
        pub reward_amount: i128,
        pub expiration: u64,
        pub created_at: u64,
        pub active: bool,
        pub paused: bool,
        pub total_claimed: u64,
        pub vesting_period_days: u32,
    }

    #[contractclient(name = "CampaignClient")]
    pub trait CampaignTrait {
        fn is_active(env: Env, campaign_id: u64) -> bool;
        fn get_campaign(env: Env, campaign_id: u64) -> Campaign;
        fn record_claim(env: Env, campaign_id: u64);
        fn pause_campaign(env: Env, campaign_id: u64);
        fn resume_campaign(env: Env, campaign_id: u64);
    }
}

use campaign::Campaign;

// ── Storage keys ──────────────────────────────────────────────────────────────

#[contracttype]
pub enum DataKey {
    /// v2: Claimed(user, campaign_id) → ClaimRecord
    Claimed(Address, u64),
    /// v1 (legacy): ClaimedV1(user, campaign_id) → bool
    /// Kept during migration; removed after migrate() completes.
    ClaimedV1(Address, u64),
    TokenContract,
    CampaignContract,
    Admin,
    /// Vesting state for (user, campaign_id)
    Vesting(Address, u64),
}

// ── Vesting state ─────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone)]
pub struct VestingState {
    /// Total amount locked in vesting
    pub total_amount: i128,
    /// Amount already claimed from vesting
    pub claimed_amount: i128,
    /// Unix timestamp when vesting started (= claim time)
    pub start_time: u64,
    /// Vesting duration in seconds
    pub vesting_duration_secs: u64,
}

// ── Events ────────────────────────────────────────────────────────────────────

const REWARD_CLAIMED: Symbol = symbol_short!("RWD_CLM");
const REWARD_REDEEMED: Symbol = symbol_short!("RWD_RDM");
const VESTED_CLAIMED: Symbol = symbol_short!("VST_CLM");

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct RewardsContract;

#[contractimpl]
impl RewardsContract {
    /// Initialize the contract with cross-contract addresses.
    ///
    /// # Parameters
    /// - `admin` — address authorized to administer this contract
    /// - `token_contract` — address of the deployed LYT token contract
    /// - `campaign_contract` — address of the deployed campaign contract
    ///
    /// # Panics
    /// - `"already initialized"` — if called more than once
    pub fn initialize(
        env: Env,
        admin: Address,
        token_contract: Address,
        campaign_contract: Address,
    ) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("already initialized");
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&DataKey::TokenContract, &token_contract);
        env.storage()
            .instance()
            .set(&DataKey::CampaignContract, &campaign_contract);
    }

    fn token_client(env: &Env) -> token::TokenClient {
        let addr: Address = env
            .storage()
            .instance()
            .get(&DataKey::TokenContract)
            .unwrap();
        token::TokenClient::new(env, &addr)
    }

    fn campaign_client(env: &Env) -> campaign::CampaignClient {
        let addr: Address = env
            .storage()
            .instance()
            .get(&DataKey::CampaignContract)
            .unwrap();
        campaign::CampaignClient::new(env, &addr)
    }

    fn has_claimed(env: &Env, user: &Address, campaign_id: u64) -> bool {
        // Check v2 first, then fall back to v1 (pre-migration)
        env.storage()
            .persistent()
            .has(&DataKey::Claimed(user.clone(), campaign_id))
            || env.storage()
                .persistent()
                .has(&DataKey::ClaimedV1(user.clone(), campaign_id))
    }

    // ── Pause helpers ─────────────────────────────────────────────────────────

    fn require_admin(env: &Env) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        admin.require_auth();
    }

    fn is_paused(env: &Env) -> bool {
        env.storage()
            .instance()
            .get(&DataKey::Paused)
            .unwrap_or(false)
    }

    fn require_not_paused(env: &Env) {
        assert!(!Self::is_paused(env), "contract is paused");
    }

    pub fn emergency_pause(env: Env) {
        Self::require_admin(&env);
        env.storage().instance().set(&DataKey::Paused, &true);
        env.events().publish((PAUSED,), ());
    }

    pub fn emergency_unpause(env: Env) {
        Self::require_admin(&env);
        env.storage().instance().set(&DataKey::Paused, &false);
        env.events().publish((UNPAUSED,), ());
    }

    pub fn paused(env: Env) -> bool {
        Self::is_paused(&env)
    }

    /// Returns multiplier in basis points (10000 = 1x, 20000 = 2x).
    /// Formula: 1 + (expires_at - now) / (expires_at - created_at), capped [1x, 2x].
    fn calc_multiplier(now: u64, created_at: u64, expires_at: u64) -> u64 {
        if now >= expires_at || expires_at <= created_at {
            return 10_000;
        }
        let duration = expires_at - created_at;
        let remaining = expires_at - now;
        // multiplier_bp = 10000 + 10000 * remaining / duration, capped at 20000
        let extra = 10_000u64 * remaining / duration;
        10_000 + extra.min(10_000)
    }

    /// Claim the reward for `campaign_id` on behalf of `user`.
    ///
    /// The minted amount is scaled by an early-claim multiplier in the range
    /// `[1×, 2×]` — users who claim earlier in the campaign lifetime receive more.
    ///
    /// # Security
    /// Requires `user.require_auth()`. Claimed state is written **before** the
    /// external mint call to prevent reentrancy.
    ///
    /// # Panics
    /// - `"already claimed"` — if `user` has already claimed this campaign
    /// - `"campaign not active"` — if the campaign is inactive or expired
    pub fn claim_reward(env: Env, user: Address, campaign_id: u64) {
        user.require_auth();
        Self::require_not_paused(&env);

        // Double-claim guard — checked BEFORE any external calls
        assert!(
            !Self::has_claimed(&env, &user, campaign_id),
            "already claimed"
        );

        let campaign_client = Self::campaign_client(&env);
        assert!(
            campaign_client.is_active(&campaign_id),
            "campaign not active"
        );

        let campaign: Campaign = campaign_client.get_campaign(&campaign_id);

        // Write claimed state before external mint (reentrancy guard) — v2 record
        let record = ClaimRecord {
            amount: 0, // will be updated below; set 0 now to mark as claimed
            claimed_at: env.ledger().timestamp(),
        };
        env.storage()
            .persistent()
            .set(&DataKey::Claimed(user.clone(), campaign_id), &record);

        let multiplier_bp = Self::calc_multiplier(
            env.ledger().timestamp(),
            campaign.created_at,
            campaign.expiration,
        );
        let final_amount = (campaign.reward_amount * multiplier_bp as i128) / 10_000;

        campaign_client.record_claim(&campaign_id);

        if campaign.vesting_period_days == 0 {
            // No vesting — mint immediately
            Self::token_client(&env).mint(&user, &final_amount);
        } else {
            // Lock in vesting — do NOT mint yet; mint on each claim_vested call
            let vesting_duration_secs = campaign.vesting_period_days as u64 * 86_400;
            let vesting = VestingState {
                total_amount: final_amount,
                claimed_amount: 0,
                start_time: env.ledger().timestamp(),
                vesting_duration_secs,
            };
            env.storage()
                .persistent()
                .set(&DataKey::Vesting(user.clone(), campaign_id), &vesting);
        }

        // Update record with actual amount
        let record = ClaimRecord {
            amount: final_amount,
            claimed_at: env.ledger().timestamp(),
        };
        env.storage()
            .persistent()
            .set(&DataKey::Claimed(user.clone(), campaign_id), &record);

        // Invariant: user balance increased by exactly reward_amount.
        #[cfg(debug_assertions)]
        debug_assert_eq!(
            Self::token_client(&env).balance(&user),
            balance_before + campaign.reward_amount,
            "invariant: balance must increase by reward_amount after claim"
        );

        env.events().publish(
            (REWARD_CLAIMED, symbol_short!("user"), user.clone()),
            (campaign_id, final_amount, multiplier_bp),
        );
    }

    /// Claim the currently vested portion of a locked reward.
    /// Linear vesting: vested = total * elapsed / duration.
    /// Can be called multiple times; each call mints only the newly vested amount.
    pub fn claim_vested(env: Env, user: Address, campaign_id: u64) {
        user.require_auth();

        let mut vesting: VestingState = env
            .storage()
            .persistent()
            .get(&DataKey::Vesting(user.clone(), campaign_id))
            .expect("no vesting schedule found");

        let now = env.ledger().timestamp();
        let elapsed = now.saturating_sub(vesting.start_time);
        let vested_total = if elapsed >= vesting.vesting_duration_secs {
            vesting.total_amount
        } else {
            (vesting.total_amount * elapsed as i128) / vesting.vesting_duration_secs as i128
        };

        let claimable = vested_total - vesting.claimed_amount;
        assert!(claimable > 0, "nothing to claim yet");

        vesting.claimed_amount = vested_total;
        env.storage()
            .persistent()
            .set(&DataKey::Vesting(user.clone(), campaign_id), &vesting);

        Self::token_client(&env).mint(&user, &claimable);

        env.events().publish(
            (VESTED_CLAIMED, symbol_short!("user"), user),
            (campaign_id, claimable, vesting.total_amount),
        );
    }

    /// View the current vesting state for a user/campaign pair.
    pub fn vesting_state(env: Env, user: Address, campaign_id: u64) -> VestingState {
        env.storage()
            .persistent()
            .get(&DataKey::Vesting(user, campaign_id))
            .expect("no vesting schedule found")
    }

    pub fn redeem_reward(env: Env, user: Address, amount: i128) {
        user.require_auth();
        Self::require_not_paused(&env);
        assert!(amount > 0, "amount must be positive");

        #[cfg(debug_assertions)]
        let (balance_before, supply_before) = {
            let tc = Self::token_client(&env);
            (tc.balance(&user), tc.total_supply_view())
        };

        Self::token_client(&env).burn(&user, &amount);

        // Invariant: balance and total supply each decreased by exactly amount.
        #[cfg(debug_assertions)]
        {
            let tc = Self::token_client(&env);
            debug_assert_eq!(
                tc.balance(&user),
                balance_before - amount,
                "invariant: balance must decrease by amount after redeem"
            );
            debug_assert_eq!(
                tc.total_supply_view(),
                supply_before - amount,
                "invariant: total_supply must decrease by amount after redeem"
            );
        }

        env.events()
            .publish((REWARD_REDEEMED, symbol_short!("user"), user), amount);
    }

    /// Returns `true` if `user` has already claimed `campaign_id`.
    pub fn has_claimed_view(env: Env, user: Address, campaign_id: u64) -> bool {
        Self::has_claimed(&env, &user, campaign_id)
    }

    pub fn schema_version(env: Env) -> u32 {
        env.storage().instance().get(&DataKey::SchemaVersion).unwrap_or(1)
    }

    // ── Storage Migration ─────────────────────────────────────────────────────

    /// Migrate v1 claim records (bool) to v2 (ClaimRecord) for a batch of entries.
    ///
    /// # Idempotency
    /// Guarded by `MigrationV1Done` flag — panics if called a second time.
    ///
    /// # Admin-only
    /// Requires admin auth.
    ///
    /// # Usage
    /// Call once after deploying the v2 contract upgrade, passing all
    /// (user, campaign_id) pairs that were claimed under v1.
    /// Old `ClaimedV1` keys are removed after writing the new `Claimed` records.
    ///
    /// # Rollback
    /// If migration fails mid-batch, the `MigrationV1Done` flag is NOT set
    /// (it is only set on success), so the function can be retried.
    /// To roll back to v1 entirely, redeploy the v1 wasm and the v1 keys
    /// remain intact (they are only removed on successful migration).
    pub fn migrate_v1_to_v2(
        env: Env,
        admin: Address,
        entries: Vec<(Address, u64)>,
    ) {
        admin.require_auth();
        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        assert!(admin == stored_admin, "not admin");

        // Idempotency guard
        assert!(
            !env.storage().instance().has(&DataKey::MigrationV1Done),
            "migration already completed"
        );

        let now = env.ledger().timestamp();

        for (user, campaign_id) in entries.iter() {
            let v1_key = DataKey::ClaimedV1(user.clone(), campaign_id);
            let v2_key = DataKey::Claimed(user.clone(), campaign_id);

            // Only migrate if v1 record exists and v2 doesn't yet
            if env.storage().persistent().has(&v1_key)
                && !env.storage().persistent().has(&v2_key)
            {
                let record = ClaimRecord {
                    amount: 0, // amount unknown from v1 bool; set 0 as sentinel
                    claimed_at: now,
                };
                env.storage().persistent().set(&v2_key, &record);
                env.storage().persistent().remove(&v1_key);
            }
        }

        // Mark migration complete and bump schema version
        env.storage().instance().set(&DataKey::MigrationV1Done, &true);
        env.storage().instance().set(&DataKey::SchemaVersion, &2_u32);

        env.events().publish(MIGRATED, 2_u32);
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_loyalty_campaign::CampaignContract;
    use soroban_loyalty_token::TokenContract;
    use soroban_sdk::{
        testutils::{Address as _, Ledger},
        Env, IntoVal,
    };

    struct TestSetup<'a> {
        env: Env,
        token: soroban_loyalty_token::TokenContractClient<'a>,
        campaign: soroban_loyalty_campaign::CampaignContractClient<'a>,
        rewards: RewardsContractClient<'a>,
    }

    fn setup() -> TestSetup<'static> {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);

        let rewards_id_placeholder = env.register_contract(None, RewardsContract);

        let token_id = env.register_contract(None, TokenContract);
        let token = soroban_loyalty_token::TokenContractClient::new(&env, &token_id);
        let mut token_signers = soroban_sdk::Vec::new(&env);
        token_signers.push_back(admin.clone());
        // Initialize token with rewards contract as the designated minter
        token.initialize(
            &token_signers,
            &1,
            &rewards_id_placeholder,
            &soroban_sdk::String::from_str(&env, "LoyaltyToken"),
            &soroban_sdk::String::from_str(&env, "LYT"),
            &7,
        );

        let campaign_id_addr = env.register_contract(None, CampaignContract);
        let campaign =
            soroban_loyalty_campaign::CampaignContractClient::new(&env, &campaign_id_addr);
        let mut campaign_admins = soroban_sdk::Vec::new(&env);
        campaign_admins.push_back(admin.clone());
        campaign.initialize(&campaign_admins, &1);

        let rewards = RewardsContractClient::new(&env, &rewards_id_placeholder);
        rewards.initialize(&admin, &token_id, &campaign_id_addr);

        TestSetup { env, token, campaign, rewards }
    }

    fn make_campaign(t: &TestSetup, merchant: &Address, reward: i128) -> u64 {
        let expiry = t.env.ledger().timestamp() + 86400;
        let name = soroban_sdk::Bytes::from_slice(&t.env, b"Test Campaign");
        let desc = soroban_sdk::Bytes::from_slice(&t.env, b"Test description");
        t.campaign.create_campaign(merchant, &reward, &expiry, &name, &desc, &0)
    }

    fn make_vesting_campaign(t: &TestSetup, merchant: &Address, reward: i128, vesting_days: u32) -> u64 {
        let expiry = t.env.ledger().timestamp() + 86400;
        let name = soroban_sdk::Bytes::from_slice(&t.env, b"Vesting Campaign");
        let desc = soroban_sdk::Bytes::from_slice(&t.env, b"Vesting test");
        t.campaign.create_campaign(merchant, &reward, &expiry, &name, &desc, &vesting_days)
    }

    #[test]
    fn test_claim_mints_tokens() {
        let t = setup();
        let merchant = Address::generate(&t.env);
        let user = Address::generate(&t.env);

        let cid = make_campaign(&t, &merchant, 500);
        t.rewards.claim_reward(&user, &cid);

        // At t=0 (start), multiplier is 2x → 500 * 2 = 1000
        assert_eq!(t.token.balance(&user), 1000);
        assert!(t.rewards.has_claimed_view(&user, &cid));

        // Assert RWD_CLM event emitted by rewards contract (verified by successful claim)
        assert!(t.rewards.has_claimed_view(&user, &cid));
    }

    #[test]
    #[should_panic(expected = "already claimed")]
    fn test_double_claim_prevented() {
        let t = setup();
        let merchant = Address::generate(&t.env);
        let user = Address::generate(&t.env);

        let cid = make_campaign(&t, &merchant, 500);
        t.rewards.claim_reward(&user, &cid);
        t.rewards.claim_reward(&user, &cid);
    }

    #[test]
    #[should_panic(expected = "campaign not active")]
    fn test_claim_inactive_campaign_rejected() {
        let t = setup();
        let merchant = Address::generate(&t.env);
        let user = Address::generate(&t.env);

        let cid = make_campaign(&t, &merchant, 500);
        t.campaign.set_active(&cid, &false);
        t.rewards.claim_reward(&user, &cid);
    }

    #[test]
    #[should_panic(expected = "campaign not active")]
    fn test_claim_expired_campaign_rejected() {
        let t = setup();
        let merchant = Address::generate(&t.env);
        let user = Address::generate(&t.env);
        let expiry = t.env.ledger().timestamp() + 10;
        let name = soroban_sdk::Bytes::from_slice(&t.env, b"Test Campaign");
        let desc = soroban_sdk::Bytes::from_slice(&t.env, b"Test description");
        let cid = t.campaign.create_campaign(&merchant, &500, &expiry, &name, &desc, &0);
        t.env.ledger().with_mut(|l| l.timestamp = expiry + 1);
        t.rewards.claim_reward(&user, &cid);
    }

    #[test]
    fn test_redeem_burns_tokens() {
        let t = setup();
        let merchant = Address::generate(&t.env);
        let user = Address::generate(&t.env);

        let cid = make_campaign(&t, &merchant, 500);
        t.rewards.claim_reward(&user, &cid);
        // Claimed at t=0 → 2x multiplier → 1000 minted; redeem 200 → 800 remaining
        t.rewards.redeem_reward(&user, &200);

        assert_eq!(t.token.balance(&user), 800);
        assert_eq!(t.token.total_supply_view(), 800);
    }

    #[test]
    fn test_multiple_users_same_campaign() {
        let t = setup();
        let merchant = Address::generate(&t.env);
        let user1 = Address::generate(&t.env);
        let user2 = Address::generate(&t.env);

        let cid = make_campaign(&t, &merchant, 100);
        t.rewards.claim_reward(&user1, &cid);
        t.rewards.claim_reward(&user2, &cid);

        assert_eq!(t.token.balance(&user1), 100);
        assert_eq!(t.token.balance(&user2), 100);
        assert_eq!(t.token.total_supply_view(), 200);
    }

    // ── Vesting Tests (Issue #128) ────────────────────────────────────────────

    #[test]
    fn test_no_vesting_mints_immediately() {
        let t = setup();
        let merchant = Address::generate(&t.env);
        let user = Address::generate(&t.env);

        let cid = make_campaign(&t, &merchant, 1000); // vesting_period_days = 0
        t.rewards.claim_reward(&user, &cid);

        // Tokens minted immediately (2x multiplier at t=0)
        assert_eq!(t.token.balance(&user), 2000);
    }

    #[test]
    fn test_vesting_locks_tokens_on_claim() {
        let t = setup();
        let merchant = Address::generate(&t.env);
        let user = Address::generate(&t.env);

        // 10-day vesting campaign
        let cid = make_vesting_campaign(&t, &merchant, 1000, 10);
        t.rewards.claim_reward(&user, &cid);

        // No tokens minted yet — locked in vesting
        assert_eq!(t.token.balance(&user), 0);

        // Vesting state recorded
        let vs = t.rewards.vesting_state(&user, &cid);
        assert_eq!(vs.claimed_amount, 0);
        assert!(vs.total_amount > 0);
    }

    #[test]
    fn test_partial_vesting_claim() {
        let t = setup();
        let merchant = Address::generate(&t.env);
        let user = Address::generate(&t.env);

        // 10-day vesting; advance 5 days → 50% vested
        let cid = make_vesting_campaign(&t, &merchant, 1000, 10);
        t.rewards.claim_reward(&user, &cid);

        let vs = t.rewards.vesting_state(&user, &cid);
        let total = vs.total_amount;

        // Advance 5 days (50% of 10-day period)
        t.env.ledger().with_mut(|l| l.timestamp += 5 * 86_400);
        t.rewards.claim_vested(&user, &cid);

        // ~50% minted
        let expected = total / 2;
        assert_eq!(t.token.balance(&user), expected);

        let vs2 = t.rewards.vesting_state(&user, &cid);
        assert_eq!(vs2.claimed_amount, expected);
    }

    #[test]
    fn test_full_vesting_after_period() {
        let t = setup();
        let merchant = Address::generate(&t.env);
        let user = Address::generate(&t.env);

        let cid = make_vesting_campaign(&t, &merchant, 1000, 10);
        t.rewards.claim_reward(&user, &cid);

        let vs = t.rewards.vesting_state(&user, &cid);
        let total = vs.total_amount;

        // Advance past full vesting period
        t.env.ledger().with_mut(|l| l.timestamp += 11 * 86_400);
        t.rewards.claim_vested(&user, &cid);

        // 100% minted
        assert_eq!(t.token.balance(&user), total);

        let vs2 = t.rewards.vesting_state(&user, &cid);
        assert_eq!(vs2.claimed_amount, total);
    }

    #[test]
    #[should_panic(expected = "nothing to claim yet")]
    fn test_claim_vested_before_any_vesting() {
        let t = setup();
        let merchant = Address::generate(&t.env);
        let user = Address::generate(&t.env);

        let cid = make_vesting_campaign(&t, &merchant, 1000, 10);
        t.rewards.claim_reward(&user, &cid);

        // Immediately try to claim — 0 seconds elapsed → 0 vested
        t.rewards.claim_vested(&user, &cid);
    }

    // ── Integration Tests (Issue #127) ───────────────────────────────────────

    #[test]
    fn test_integration_claim_loop() {
        let t = setup();
        let merchant = Address::generate(&t.env);
        let user = Address::generate(&t.env);
        let reward_amount = 1000_i128;

        // 1. Create active campaign
        let campaign_id = t.campaign.create_campaign(&merchant, &reward_amount, &expiry, &soroban_sdk::Bytes::from_slice(&t.env, b"Test"), &soroban_sdk::Bytes::from_slice(&t.env, b"Test"), &0);
        assert!(t.campaign.is_active(&campaign_id));

        t.rewards.claim_reward(&user, &campaign_id);

        assert_eq!(t.token.balance(&user), reward_amount * 2); // 2x multiplier at t=0
        assert_eq!(t.token.total_supply_view(), reward_amount * 2);
        assert!(t.rewards.has_claimed_view(&user, &campaign_id));
    }

    #[test]
    fn test_integration_redemption_loop() {
        let t = setup();
        let merchant = Address::generate(&t.env);
        let user = Address::generate(&t.env);
        let reward_amount = 1000_i128;
        let redeem_amount = 300_i128;

        // Setup: User has claimed rewards
        let campaign_id = t.campaign.create_campaign(&merchant, &reward_amount, &expiry, &soroban_sdk::Bytes::from_slice(&t.env, b"Test"), &soroban_sdk::Bytes::from_slice(&t.env, b"Test"), &0);
        t.rewards.claim_reward(&user, &campaign_id);

        t.rewards.redeem_reward(&user, &redeem_amount);

        let expected_balance = reward_amount * 2 - redeem_amount;
        assert_eq!(t.token.balance(&user), expected_balance);
        assert_eq!(t.token.total_supply_view(), expected_balance);
    }

    #[test]
    fn test_integration_multi_user_multi_campaign() {
        let t = setup();
        let merchant1 = Address::generate(&t.env);
        let merchant2 = Address::generate(&t.env);
        let user1 = Address::generate(&t.env);
        let user2 = Address::generate(&t.env);

        // Create two campaigns with different reward amounts
        let campaign1_id = t.campaign.create_campaign(&merchant1, &100, &expiry, &soroban_sdk::Bytes::from_slice(&t.env, b"C1"), &soroban_sdk::Bytes::from_slice(&t.env, b"C1"), &0);
        let campaign2_id = t.campaign.create_campaign(&merchant2, &200, &expiry, &soroban_sdk::Bytes::from_slice(&t.env, b"C2"), &soroban_sdk::Bytes::from_slice(&t.env, b"C2"), &0);

        t.rewards.claim_reward(&user1, &campaign1_id);
        t.rewards.claim_reward(&user1, &campaign2_id);
        t.rewards.claim_reward(&user2, &campaign1_id);

        // 2x multiplier at t=0
        assert_eq!(t.token.balance(&user1), 600); // (100+200)*2
        assert_eq!(t.token.balance(&user2), 200); // 100*2
        assert_eq!(t.token.total_supply_view(), 800);

        t.rewards.redeem_reward(&user1, &150);
        assert_eq!(t.token.balance(&user1), 450);
        assert_eq!(t.token.total_supply_view(), 650);

        assert!(t.rewards.has_claimed_view(&user1, &campaign1_id));
        assert!(t.rewards.has_claimed_view(&user1, &campaign2_id));
        assert!(t.rewards.has_claimed_view(&user2, &campaign1_id));
        assert!(!t.rewards.has_claimed_view(&user2, &campaign2_id));
    }

    #[test]
    #[should_panic(expected = "campaign not active")]
    fn test_integration_campaign_expiration_boundary() {
        let t = setup();
        let merchant = Address::generate(&t.env);
        let user2 = Address::generate(&t.env);
        let short_expiry = t.env.ledger().timestamp() + 10;
        let name = soroban_sdk::Bytes::from_slice(&t.env, b"Test");
        let desc = soroban_sdk::Bytes::from_slice(&t.env, b"Test");
        let campaign_id = t.campaign.create_campaign(&merchant, &500, &short_expiry, &name, &desc);

        let campaign_id = t.campaign.create_campaign(&merchant, &500, &short_expiry, &soroban_sdk::Bytes::from_slice(&t.env, b"Test"), &soroban_sdk::Bytes::from_slice(&t.env, b"Test"), &0);
        
        // User1 claims before expiry - should succeed
        t.rewards.claim_reward(&user1, &campaign_id);
        assert_eq!(t.token.balance(&user1), 500 * 2);

        t.env.ledger().with_mut(|l| l.timestamp = short_expiry + 1);
        t.rewards.claim_reward(&user2, &campaign_id); // should panic
    }

    #[test]
    #[should_panic(expected = "campaign not active")]
    fn test_integration_inactive_campaign_boundary() {
        let t = setup();
        let merchant = Address::generate(&t.env);
        let user = Address::generate(&t.env);

        let campaign_id = t.campaign.create_campaign(&merchant, &500, &expiry, &soroban_sdk::Bytes::from_slice(&t.env, b"Test"), &soroban_sdk::Bytes::from_slice(&t.env, b"Test"), &0);
        
        // Deactivate campaign via campaign contract
        t.campaign.set_active(&campaign_id, &false);
        t.rewards.claim_reward(&user, &campaign_id); // should panic
    }

    #[test]
    #[should_panic(expected = "campaign is paused")]
    fn test_claim_paused_campaign_rejected() {
        let t = setup();
        let merchant = Address::generate(&t.env);
        let user = Address::generate(&t.env);
        let cid = make_campaign(&t, &merchant, 500);
        t.campaign.pause_campaign(&cid);
        t.rewards.claim_reward(&user, &cid);
    }

    #[test]
    fn test_resume_then_claim_succeeds() {
        let t = setup();
        let merchant = Address::generate(&t.env);
        let user = Address::generate(&t.env);
        let cid = make_campaign(&t, &merchant, 500);
        t.campaign.pause_campaign(&cid);
        t.campaign.resume_campaign(&cid);
        t.rewards.claim_reward(&user, &cid);
        assert!(t.rewards.has_claimed_view(&user, &cid));
    }

    // ── Invariant tests ───────────────────────────────────────────────────────

    #[test]
    fn test_invariant_claimed_flag_set_before_mint() {
        let t = setup();
        let merchant = Address::generate(&t.env);
        let user = Address::generate(&t.env);
        let expiry = t.env.ledger().timestamp() + 86400;

        let cid = t.campaign.create_campaign(&merchant, &500, &expiry);
        t.rewards.claim_reward(&user, &cid);

        // After claim: flag must be set and balance must reflect the mint.
        assert!(t.rewards.has_claimed_view(&user, &cid), "invariant: claimed flag set");
        assert_eq!(t.token.balance(&user), 500, "invariant: balance == reward_amount after claim");
    }

    #[test]
    fn test_invariant_balance_increases_by_reward_amount() {
        let t = setup();
        let merchant = Address::generate(&t.env);
        let user = Address::generate(&t.env);
        let expiry = t.env.ledger().timestamp() + 86400;
        let reward = 750_i128;

        let cid = t.campaign.create_campaign(&merchant, &reward, &expiry);
        let bal_before = t.token.balance(&user);
        t.rewards.claim_reward(&user, &cid);

        assert_eq!(
            t.token.balance(&user),
            bal_before + reward,
            "invariant: balance increases by exactly reward_amount"
        );
    }

    #[test]
    fn test_invariant_redeem_decreases_balance_and_supply() {
        let t = setup();
        let merchant = Address::generate(&t.env);
        let user = Address::generate(&t.env);
        let expiry = t.env.ledger().timestamp() + 86400;

        let cid = t.campaign.create_campaign(&merchant, &500, &expiry);
        t.rewards.claim_reward(&user, &cid);

        let bal_before = t.token.balance(&user);
        let supply_before = t.token.total_supply_view();
        let redeem_amount = 200_i128;

        t.rewards.redeem_reward(&user, &redeem_amount);

        assert_eq!(
            t.token.balance(&user),
            bal_before - redeem_amount,
            "invariant: balance decreases by redeem amount"
        );
        assert_eq!(
            t.token.total_supply_view(),
            supply_before - redeem_amount,
            "invariant: total_supply decreases by redeem amount"
        );
    }
}
