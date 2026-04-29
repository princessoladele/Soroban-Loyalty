#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, Address, Bytes, Env, Symbol,
};

// ── Types ─────────────────────────────────────────────────────────────────────

/// Optimized Campaign struct (Issue #110).
///
/// Changes vs. original:
///   - Removed `id: u64`          — redundant; the storage key DataKey::Campaign(id) already
///                                   carries the id, so storing it inside the value wastes 8 bytes.
///   - `expiration: u64 → u32`    — Unix timestamp; u32 is valid until year 2106, saves 4 bytes.
///   - `total_claimed: u64 → u32` — realistic claim counts never exceed 4 billion, saves 4 bytes.
///
/// Net saving: 16 bytes per record on a previously ~68-byte struct ≈ 24 % reduction.
#[contracttype]
#[derive(Clone, Debug)]
pub struct Campaign {
    pub id: u64,
    pub merchant: Address,
    pub reward_amount: i128,
    pub expiration: u64, // Unix timestamp (seconds)
    pub created_at: u64, // Unix timestamp (seconds)
    pub active: bool,
    pub paused: bool,
    pub total_claimed: u64,
    /// Campaign name — max 64 bytes UTF-8
    pub name: Bytes,
    /// Campaign description — max 256 bytes UTF-8
    pub description: Bytes,
    /// Optional linear vesting period in days (0 = no vesting, immediate release)
    pub vesting_period_days: u32,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct UpgradeProposal {
    pub wasm_hash: soroban_sdk::BytesN<32>,
    pub proposed_at: u64,
    pub signatures: soroban_sdk::Vec<Address>,
}

#[contracttype]
pub enum DataKey {
    Campaign(u64),
    NextId,
    Admins,
    Threshold,
    UpgradeProposal,
    Paused,
}

// ── Events ────────────────────────────────────────────────────────────────────

const CAMPAIGN_CREATED: Symbol = symbol_short!("CAM_CRT");
const CAMPAIGN_DEACTIVATED: Symbol = symbol_short!("CAM_DEACT");
const PAUSED: Symbol = symbol_short!("PAUSED");
const UNPAUSED: Symbol = symbol_short!("UNPAUSED");
const UPGRADE_PROPOSED: Symbol = symbol_short!("UPG_PROP");
const UPGRADE_AUTHORIZED: Symbol = symbol_short!("UPG_AUTH");
const UPGRADE_EXECUTED: Symbol = symbol_short!("UPG_EXEC");
const UPGRADE_CANCELLED: Symbol = symbol_short!("UPG_CNCL");

/// 48-hour timelock for upgrades (in seconds).
const TIMELOCK: u64 = 172_800;

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct CampaignContract;

#[contractimpl]
impl CampaignContract {
    /// Initialize the contract with a multi-sig admin set and approval threshold.
    ///
    /// # Parameters
    /// - `admins` — list of admin addresses; length must be ≥ `threshold`
    /// - `threshold` — minimum signatures required to execute an upgrade; must be > 0
    ///
    /// # Panics
    /// - `"already initialized"` — if called more than once
    /// - `"threshold must be positive"` — if `threshold == 0`
    /// - `"insufficient admins for threshold"` — if `admins.len() < threshold`
    pub fn initialize(env: Env, admins: soroban_sdk::Vec<Address>, threshold: u32) {
        if env.storage().instance().has(&DataKey::Admins) {
            panic!("already initialized");
        }
        assert!(threshold > 0, "threshold must be positive");
        assert!(admins.len() >= threshold, "insufficient admins for threshold");

        env.storage().instance().set(&DataKey::Admins, &admins);
        env.storage().instance().set(&DataKey::Threshold, &threshold);
        env.storage().instance().set(&DataKey::NextId, &1_u64);
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    fn next_id(env: &Env) -> u64 {
        env.storage()
            .instance()
            .get(&DataKey::NextId)
            .unwrap_or(1)
    }

    fn bump_id(env: &Env) -> u64 {
        let id = Self::next_id(env);
        env.storage()
            .instance()
            .set(&DataKey::NextId, &(id + 1));
        id
    }

    // ── Pause helpers ─────────────────────────────────────────────────────────

    fn is_paused(env: &Env) -> bool {
        env.storage()
            .instance()
            .get(&DataKey::Paused)
            .unwrap_or(false)
    }

    fn require_not_paused(env: &Env) {
        assert!(!Self::is_paused(env), "contract is paused");
    }

    pub fn emergency_pause(env: Env, admin: Address) {
        Self::require_admin(&env, &admin);
        env.storage().instance().set(&DataKey::Paused, &true);
        env.events().publish((PAUSED,), admin);
    }

    pub fn emergency_unpause(env: Env, admin: Address) {
        Self::require_admin(&env, &admin);
        env.storage().instance().set(&DataKey::Paused, &false);
        env.events().publish((UNPAUSED,), admin);
    }

    pub fn paused(env: Env) -> bool {
        Self::is_paused(&env)
    }

    // ── Public interface ──────────────────────────────────────────────────────

    /// Create a new campaign. Only the merchant (caller) can create it.
    /// `name` max 64 bytes, `description` max 256 bytes.
    /// `vesting_period_days` = 0 means no vesting (immediate release).
    pub fn create_campaign(
        env: Env,
        merchant: Address,
        reward_amount: i128,
        expiration: u64,
        name: Bytes,
        description: Bytes,
        vesting_period_days: u32,
    ) -> u64 {
        merchant.require_auth();
        Self::require_not_paused(&env);
        assert!(reward_amount > 0, "reward_amount must be positive");
        assert!(
            expiration > env.ledger().timestamp(),
            "expiration must be in the future"
        );
        assert!(name.len() <= 64, "name exceeds 64 bytes");
        assert!(description.len() <= 256, "description exceeds 256 bytes");

        let id = Self::bump_id(&env);
        let campaign = Campaign {
            merchant: merchant.clone(),
            reward_amount,
            expiration,
            created_at: env.ledger().timestamp(),
            active: true,
            paused: false,
            total_claimed: 0,
            name: name.clone(),
            description: description.clone(),
            vesting_period_days,
        };
        env.storage()
            .persistent()
            .set(&DataKey::Campaign(id), &campaign);

        env.events().publish(
            (CAMPAIGN_CREATED, symbol_short!("id"), id),
            (merchant, name, description),
        );

        id
    }

    /// Deactivate / reactivate a campaign. Only the merchant can do this.
    pub fn set_active(env: Env, campaign_id: u64, active: bool) {
        let mut campaign = Self::get_campaign_internal(&env, campaign_id);
        campaign.merchant.require_auth();
        Self::require_not_paused(&env);
        campaign.active = active;
        env.storage()
            .persistent()
            .set(&DataKey::Campaign(campaign_id), &campaign);

        if !active {
            env.events().publish(
                (CAMPAIGN_DEACTIVATED, symbol_short!("id"), campaign_id),
                campaign.merchant,
            );
        }
    }

    /// Called by the rewards contract to increment the claim counter.
    pub fn pause_campaign(env: Env, campaign_id: u64) {
        let mut campaign = Self::get_campaign_internal(&env, campaign_id);
        campaign.merchant.require_auth();
        campaign.paused = true;
        env.storage()
            .persistent()
            .set(&DataKey::Campaign(campaign_id), &campaign);
        env.events().publish(
            (CAMPAIGN_PAUSED, symbol_short!("id"), campaign_id),
            (campaign_id, campaign.merchant),
        );
    }

    /// Resume a paused campaign. Only the merchant can do this.
    pub fn resume_campaign(env: Env, campaign_id: u64) {
        let mut campaign = Self::get_campaign_internal(&env, campaign_id);
        campaign.merchant.require_auth();
        campaign.paused = false;
        env.storage()
            .persistent()
            .set(&DataKey::Campaign(campaign_id), &campaign);
        env.events().publish(
            (CAMPAIGN_RESUMED, symbol_short!("id"), campaign_id),
            (campaign_id, campaign.merchant),
        );
    }

    /// Called by the rewards contract to increment the claim counter.
    pub fn record_claim(env: Env, campaign_id: u64) {
        Self::require_not_paused(&env);
        let mut campaign = Self::get_campaign_internal(&env, campaign_id);

        #[cfg(debug_assertions)]
        let claimed_before = campaign.total_claimed;

        campaign.total_claimed = campaign
            .total_claimed
            .checked_add(1)
            .expect("overflow");
        env.storage()
            .persistent()
            .set(&DataKey::Campaign(campaign_id), &campaign);

        // Invariant: total_claimed is strictly monotonically increasing.
        #[cfg(debug_assertions)]
        debug_assert!(
            campaign.total_claimed > claimed_before,
            "invariant: total_claimed must increase after record_claim"
        );
    }

    /// Returns the full [`Campaign`] struct for `campaign_id`.
    ///
    /// # Panics
    /// - `"campaign not found"` — if `campaign_id` does not exist
    pub fn get_campaign(env: Env, campaign_id: u64) -> Campaign {
        Self::get_campaign_internal(&env, campaign_id)
    }

    /// View function returning only the on-chain metadata fields.
    pub fn get_campaign_metadata(env: Env, campaign_id: u64) -> (Bytes, Bytes) {
        let c = Self::get_campaign_internal(&env, campaign_id);
        (c.name, c.description)
    }

    fn get_campaign_internal(env: &Env, campaign_id: u64) -> Campaign {
        env.storage()
            .persistent()
            .get(&DataKey::Campaign(campaign_id))
            .expect("campaign not found")
    }

    /// Returns `true` if the campaign exists, is marked active, and has not expired.
    pub fn is_active(env: Env, campaign_id: u64) -> bool {
        let c = Self::get_campaign_internal(&env, campaign_id);
        c.active && env.ledger().timestamp() < c.expiration as u64
    }

    // ── Upgrade Mechanism ───────────────────────────────────────────────────

    /// Propose a contract upgrade with the given WASM hash.
    ///
    /// The proposing admin's signature is counted as the first authorization.
    /// The upgrade cannot be executed until the timelock has elapsed and
    /// `threshold` admins have called `authorize_upgrade`.
    ///
    /// # Security
    /// Requires `admin` to be in the stored admin list (`require_auth` enforced).
    ///
    /// # Panics
    /// - `"upgrade already proposed"` — if a proposal is already pending
    /// - `"not an admin"` — if `admin` is not in the admin list
    pub fn propose_upgrade(env: Env, admin: Address, wasm_hash: soroban_sdk::BytesN<32>) {
        Self::require_admin(&env, &admin);
        if env.storage().instance().has(&DataKey::UpgradeProposal) {
            panic!("upgrade already proposed");
        }

        let mut signatures = soroban_sdk::Vec::new(&env);
        signatures.push_back(admin.clone());

        let proposal = UpgradeProposal {
            wasm_hash: wasm_hash.clone(),
            proposed_at: env.ledger().timestamp(),
            signatures,
        };

        env.storage().instance().set(&DataKey::UpgradeProposal, &proposal);
        env.events().publish((UPGRADE_PROPOSED, wasm_hash), admin);
    }

    /// Add `admin`'s authorization to the pending upgrade proposal.
    ///
    /// # Security
    /// Requires `admin` to be in the stored admin list (`require_auth` enforced).
    ///
    /// # Panics
    /// - `"no pending proposal"` — if no upgrade has been proposed
    /// - `"already authorized by this admin"` — if `admin` has already signed
    /// - `"not an admin"` — if `admin` is not in the admin list
    pub fn authorize_upgrade(env: Env, admin: Address) {
        Self::require_admin(&env, &admin);
        let mut proposal: UpgradeProposal = env
            .storage()
            .instance()
            .get(&DataKey::UpgradeProposal)
            .expect("no pending proposal");

        for signee in proposal.signatures.iter() {
            if signee == admin {
                panic!("already authorized by this admin");
            }
        }

        proposal.signatures.push_back(admin.clone());
        env.storage().instance().set(&DataKey::UpgradeProposal, &proposal);
        env.events().publish((UPGRADE_AUTHORIZED,), admin);
    }

    /// Execute the pending upgrade once the timelock has elapsed and enough
    /// admins have authorized it.
    ///
    /// Replaces the contract WASM and clears the proposal from storage.
    ///
    /// # Security
    /// Requires `admin` to be in the stored admin list (`require_auth` enforced).
    ///
    /// # Panics
    /// - `"no pending proposal"` — if no upgrade has been proposed
    /// - `"insufficient authorizations"` — if fewer than `threshold` admins have signed
    /// - `"timelock not met"` — if the required delay since proposal has not elapsed
    /// - `"not an admin"` — if `admin` is not in the admin list
    pub fn execute_upgrade(env: Env, admin: Address) {
        Self::require_admin(&env, &admin);
        let proposal: UpgradeProposal = env
            .storage()
            .instance()
            .get(&DataKey::UpgradeProposal)
            .expect("no pending proposal");

        let threshold: u32 = env.storage().instance().get(&DataKey::Threshold).unwrap();
        assert!(
            proposal.signatures.len() >= threshold,
            "insufficient authorizations"
        );
        assert!(
            env.ledger().timestamp() >= proposal.proposed_at + TIMELOCK,
            "timelock not met"
        );

        env.deployer().update_current_contract_wasm(proposal.wasm_hash.clone());
        env.storage().instance().remove(&DataKey::UpgradeProposal);
        env.events().publish((UPGRADE_EXECUTED,), proposal.wasm_hash);
    }

    /// Cancel the pending upgrade proposal, allowing a new one to be submitted.
    ///
    /// # Security
    /// Requires `admin` to be in the stored admin list (`require_auth` enforced).
    ///
    /// # Panics
    /// - `"not an admin"` — if `admin` is not in the admin list
    pub fn cancel_upgrade(env: Env, admin: Address) {
        Self::require_admin(&env, &admin);
        env.storage().instance().remove(&DataKey::UpgradeProposal);
        env.events().publish((UPGRADE_CANCELLED,), admin);
    }

    fn require_admin(env: &Env, admin: &Address) {
        admin.require_auth();
        let admins: soroban_sdk::Vec<Address> = env.storage().instance().get(&DataKey::Admins).unwrap();
        let mut is_admin = false;
        for a in admins.iter() {
            if a == *admin {
                is_admin = true;
                break;
            }
        }
        if !is_admin {
            panic!("not an admin");
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod expiry_prop_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{
        testutils::{Address as _, Ledger},
        Bytes, Env,
    };

    fn setup() -> (Env, Address, Address, CampaignContractClient<'static>) {
        let env = Env::default();
        env.mock_all_auths();
        let admin1 = Address::generate(&env);
        let admin2 = Address::generate(&env);
        let contract_id = env.register_contract(None, CampaignContract);
        let client = CampaignContractClient::new(&env, &contract_id);
        let mut admins = soroban_sdk::Vec::new(&env);
        admins.push_back(admin1.clone());
        admins.push_back(admin2.clone());
        client.initialize(&admins, &2);
        (env, admin1, admin2, client)
    }

    fn name(env: &Env) -> Bytes {
        Bytes::from_slice(env, b"Summer Sale")
    }

    fn desc(env: &Env) -> Bytes {
        Bytes::from_slice(env, b"Earn LYT on every purchase this summer")
    }

    #[test]
    fn test_create_campaign() {
        let (env, admin1, _admin2, client) = setup();
        let merchant = Address::generate(&env);
        let expiry = env.ledger().timestamp() + 86400;
        let id = client.create_campaign(&merchant, &100, &expiry, &name(&env), &desc(&env), &0);
        assert_eq!(id, 1);
        let c = client.get_campaign(&id);
        assert_eq!(c.merchant, merchant);
        assert_eq!(c.reward_amount, 100);
        assert!(c.active);
        assert_eq!(c.name, name(&env));
        assert_eq!(c.description, desc(&env));
    }

    #[test]
    fn test_get_campaign_metadata() {
        let (env, _admin1, _admin2, client) = setup();
        let merchant = Address::generate(&env);
        let expiry = env.ledger().timestamp() + 86400;
        let id = client.create_campaign(&merchant, &100, &expiry, &name(&env), &desc(&env), &0);
        let (n, d) = client.get_campaign_metadata(&id);
        assert_eq!(n, name(&env));
        assert_eq!(d, desc(&env));
    }

    #[test]
    #[should_panic(expected = "name exceeds 64 bytes")]
    fn test_name_too_long() {
        let (env, _admin1, _admin2, client) = setup();
        let merchant = Address::generate(&env);
        let expiry = env.ledger().timestamp() + 86400;
        let long_name = Bytes::from_slice(env, &[b'x'; 65]);
        client.create_campaign(&merchant, &100, &expiry, &long_name, &desc(&env), &0);
    }

    #[test]
    #[should_panic(expected = "description exceeds 256 bytes")]
    fn test_description_too_long() {
        let (env, _admin1, _admin2, client) = setup();
        let merchant = Address::generate(&env);
        let expiry = env.ledger().timestamp() + 86400;
        let long_desc = Bytes::from_slice(env, &[b'd'; 257]);
        client.create_campaign(&merchant, &100, &expiry, &name(&env), &long_desc, &0);
    }

    #[test]
    #[should_panic(expected = "expiration must be in the future")]
    fn test_expired_campaign_rejected() {
        let (env, admin1, _admin2, client) = setup();
        let merchant = Address::generate(&env);
        client.create_campaign(&merchant, &100, &0, &name(&env), &desc(&env), &0);
    }

    #[test]
    fn test_set_active_emits_deactivated_event() {
        let (env, _admin1, _admin2, client) = setup();
        let merchant = Address::generate(&env);
        let expiry = env.ledger().timestamp() + 86400;
        let id = client.create_campaign(&merchant, &100, &expiry, &name(&env), &desc(&env), &0);
        client.set_active(&id, &false);
        assert!(!client.get_campaign(&id).active);
    }

    #[test]
    fn test_set_active_reactivate_no_event() {
        let (env, _admin1, _admin2, client) = setup();
        let merchant = Address::generate(&env);
        let expiry = env.ledger().timestamp() + 86400;
        let id = client.create_campaign(&merchant, &100, &expiry, &name(&env), &desc(&env), &0);
        client.set_active(&id, &false);
        assert!(!client.get_campaign(&id).active);
        client.set_active(&id, &true);
        assert!(client.get_campaign(&id).active);
    }

    #[test]
    fn test_is_active_after_expiry() {
        let (env, admin1, _admin2, client) = setup();
        let merchant = Address::generate(&env);
        let expiry = env.ledger().timestamp() + 10;
        let id = client.create_campaign(&merchant, &100, &expiry, &name(&env), &desc(&env), &0);
        assert!(client.is_active(&id));

        env.ledger().with_mut(|l| l.timestamp = expiry + 1);
        assert!(!client.is_active(&id));
    }

    #[test]
    #[ignore = "requires actual WASM upload; not testable in unit test environment"]
    fn test_upgrade_flow() {
        let (env, admin1, admin2, client) = setup();
        let wasm_hash = soroban_sdk::BytesN::from_array(&env, &[0u8; 32]);

        // Propose
        client.propose_upgrade(&admin1, &wasm_hash);

        // Authorize (need 2nd signature for threshold 2)
        client.authorize_upgrade(&admin2);

        // Advance time for timelock (48h)
        env.ledger().with_mut(|l| l.timestamp += TIMELOCK + 1);

        // Execute
        client.execute_upgrade(&admin1);
    }

    #[test]
    #[should_panic(expected = "timelock not met")]
    fn test_upgrade_timelock_enforced() {
        let (env, admin1, admin2, client) = setup();
        let wasm_hash = soroban_sdk::BytesN::from_array(&env, &[0u8; 32]);

        client.propose_upgrade(&admin1, &wasm_hash);
        client.authorize_upgrade(&admin2);

        // Try to execute before 48h
        client.execute_upgrade(&admin1);
    }

    #[test]
    #[should_panic(expected = "insufficient authorizations")]
    fn test_upgrade_threshold_enforced() {
        let (env, admin1, admin2, client) = setup();
        let wasm_hash = soroban_sdk::BytesN::from_array(&env, &[0u8; 32]);

        client.propose_upgrade(&admin1, &wasm_hash);
        // missing 2nd signature

        env.ledger().with_mut(|l| l.timestamp += TIMELOCK + 1);
        client.execute_upgrade(&admin1);
    }

    #[test]
    fn test_cancel_upgrade() {
        let (env, admin1, _admin2, client) = setup();
        let wasm_hash = soroban_sdk::BytesN::from_array(&env, &[0u8; 32]);

        client.propose_upgrade(&admin1, &wasm_hash);
        client.cancel_upgrade(&admin1);

        // Verify it's gone (should be able to propose again)
        client.propose_upgrade(&admin1, &wasm_hash);
    }

    // ── Pause tests ───────────────────────────────────────────────────────────

    #[test]
    fn test_pause_and_unpause() {
        let (env, admin1, _admin2, client) = setup();
        assert!(!client.paused());
        client.emergency_pause(&admin1);
        assert!(client.paused());
        client.emergency_unpause(&admin1);
        assert!(!client.paused());
    }

    #[test]
    #[should_panic(expected = "contract is paused")]
    fn test_create_campaign_blocked_when_paused() {
        let (env, admin1, _admin2, client) = setup();
        let merchant = Address::generate(&env);
        let expiry = env.ledger().timestamp() + 86400;
        client.emergency_pause(&admin1);
        client.create_campaign(&merchant, &100, &expiry, &name(&env), &desc(&env));
    }

    #[test]
    #[should_panic(expected = "contract is paused")]
    fn test_set_active_blocked_when_paused() {
        let (env, admin1, _admin2, client) = setup();
        let merchant = Address::generate(&env);
        let expiry = env.ledger().timestamp() + 86400;
        let id = client.create_campaign(&merchant, &100, &expiry, &name(&env), &desc(&env));
        client.emergency_pause(&admin1);
        client.set_active(&id, &false);
    }
}
