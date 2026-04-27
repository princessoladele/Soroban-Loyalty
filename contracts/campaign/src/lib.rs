#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, Address, Env, Symbol,
};

// ── Types ─────────────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug)]
pub struct Campaign {
    pub id: u64,
    pub merchant: Address,
    pub reward_amount: i128,
    pub expiration: u64, // Unix timestamp (seconds)
    pub active: bool,
    pub total_claimed: u64,
}

#[contracttype]
pub enum DataKey {
    Campaign(u64),
    NextId,
    Admin,
}

// ── Events ────────────────────────────────────────────────────────────────────

const CAMPAIGN_CREATED: Symbol = symbol_short!("CAM_CRT");
const CAMPAIGN_UPDATED: Symbol = symbol_short!("CAM_UPD");

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct CampaignContract;

#[contractimpl]
impl CampaignContract {
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("already initialized");
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
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

    // ── Public interface ──────────────────────────────────────────────────────

    /// Create a new campaign. Only the merchant (caller) can create it.
    pub fn create_campaign(
        env: Env,
        merchant: Address,
        reward_amount: i128,
        expiration: u64,
    ) -> u64 {
        merchant.require_auth();
        assert!(reward_amount > 0, "reward_amount must be positive");
        assert!(
            expiration > env.ledger().timestamp(),
            "expiration must be in the future"
        );

        let id = Self::bump_id(&env);
        let campaign = Campaign {
            id,
            merchant: merchant.clone(),
            reward_amount,
            expiration,
            active: true,
            total_claimed: 0,
        };
        env.storage()
            .persistent()
            .set(&DataKey::Campaign(id), &campaign);

        // Invariant: newly created campaign starts with zero claims and positive reward.
        #[cfg(debug_assertions)]
        {
            debug_assert_eq!(campaign.total_claimed, 0, "invariant: new campaign total_claimed == 0");
            debug_assert!(campaign.reward_amount > 0, "invariant: reward_amount > 0");
        }

        env.events()
            .publish((CAMPAIGN_CREATED, symbol_short!("id"), id), merchant);

        id
    }

    /// Deactivate / reactivate a campaign. Only the merchant can do this.
    pub fn set_active(env: Env, campaign_id: u64, active: bool) {
        let mut campaign = Self::get_campaign_internal(&env, campaign_id);
        campaign.merchant.require_auth();
        campaign.active = active;
        env.storage()
            .persistent()
            .set(&DataKey::Campaign(campaign_id), &campaign);

        env.events()
            .publish((CAMPAIGN_UPDATED, symbol_short!("id"), campaign_id), active);
    }

    /// Called by the rewards contract to increment the claim counter.
    pub fn record_claim(env: Env, campaign_id: u64) {
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

    pub fn get_campaign(env: Env, campaign_id: u64) -> Campaign {
        Self::get_campaign_internal(&env, campaign_id)
    }

    fn get_campaign_internal(env: &Env, campaign_id: u64) -> Campaign {
        env.storage()
            .persistent()
            .get(&DataKey::Campaign(campaign_id))
            .expect("campaign not found")
    }

    pub fn is_active(env: Env, campaign_id: u64) -> bool {
        let c = Self::get_campaign_internal(&env, campaign_id);
        c.active && env.ledger().timestamp() < c.expiration
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::{Address as _, Ledger}, Env};

    fn setup() -> (Env, Address, CampaignContractClient<'static>) {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let contract_id = env.register_contract(None, CampaignContract);
        let client = CampaignContractClient::new(&env, &contract_id);
        client.initialize(&admin);
        (env, admin, client)
    }

    #[test]
    fn test_create_campaign() {
        let (env, _admin, client) = setup();
        let merchant = Address::generate(&env);
        let expiry = env.ledger().timestamp() + 86400;
        let id = client.create_campaign(&merchant, &100, &expiry);
        assert_eq!(id, 1);
        let c = client.get_campaign(&id);
        assert_eq!(c.merchant, merchant);
        assert_eq!(c.reward_amount, 100);
        assert!(c.active);
    }

    #[test]
    #[should_panic(expected = "expiration must be in the future")]
    fn test_expired_campaign_rejected() {
        let (env, _admin, client) = setup();
        let merchant = Address::generate(&env);
        // expiration in the past
        client.create_campaign(&merchant, &100, &0);
    }

    #[test]
    fn test_set_active() {
        let (env, _admin, client) = setup();
        let merchant = Address::generate(&env);
        let expiry = env.ledger().timestamp() + 86400;
        let id = client.create_campaign(&merchant, &100, &expiry);
        client.set_active(&id, &false);
        assert!(!client.get_campaign(&id).active);
    }

    #[test]
    fn test_is_active_after_expiry() {
        let (env, _admin, client) = setup();
        let merchant = Address::generate(&env);
        let expiry = env.ledger().timestamp() + 10;
        let id = client.create_campaign(&merchant, &100, &expiry);
        assert!(client.is_active(&id));

        // advance ledger past expiry
        env.ledger().with_mut(|l| l.timestamp = expiry + 1);
        assert!(!client.is_active(&id));
    }

    // ── Invariant tests ───────────────────────────────────────────────────────

    #[test]
    fn test_invariant_new_campaign_total_claimed_zero() {
        let (env, _admin, client) = setup();
        let merchant = Address::generate(&env);
        let expiry = env.ledger().timestamp() + 86400;
        let id = client.create_campaign(&merchant, &100, &expiry);
        let c = client.get_campaign(&id);
        assert_eq!(c.total_claimed, 0, "invariant: new campaign total_claimed == 0");
        assert!(c.reward_amount > 0, "invariant: reward_amount > 0");
    }

    #[test]
    fn test_invariant_total_claimed_monotonic() {
        let (env, _admin, client) = setup();
        let merchant = Address::generate(&env);
        let expiry = env.ledger().timestamp() + 86400;
        let id = client.create_campaign(&merchant, &100, &expiry);

        client.record_claim(&id);
        let after_first = client.get_campaign(&id).total_claimed;
        assert_eq!(after_first, 1);

        client.record_claim(&id);
        let after_second = client.get_campaign(&id).total_claimed;
        assert!(after_second > after_first, "invariant: total_claimed is monotonically increasing");
    }

    #[test]
    #[should_panic(expected = "reward_amount must be positive")]
    fn test_invariant_zero_reward_rejected() {
        let (env, _admin, client) = setup();
        let merchant = Address::generate(&env);
        let expiry = env.ledger().timestamp() + 86400;
        client.create_campaign(&merchant, &0, &expiry);
    }
}
