#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, Address, Env, Symbol,
};

// ── Cross-contract interfaces ─────────────────────────────────────────────────
// We define minimal client traits via contractimport for production.
// Tests use the real crate clients directly.

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
    use soroban_sdk::{contractclient, contracttype, Address, Env};

    #[contracttype]
    #[derive(Clone)]
    pub struct Campaign {
        pub id: u64,
        pub merchant: Address,
        pub reward_amount: i128,
        pub expiration: u64,
        pub active: bool,
        pub total_claimed: u64,
    }

    #[contractclient(name = "CampaignClient")]
    pub trait CampaignTrait {
        fn is_active(env: Env, campaign_id: u64) -> bool;
        fn get_campaign(env: Env, campaign_id: u64) -> Campaign;
        fn record_claim(env: Env, campaign_id: u64);
    }
}

use campaign::Campaign;

// ── Storage keys ──────────────────────────────────────────────────────────────

#[contracttype]
pub enum DataKey {
    Claimed(Address, u64),
    TokenContract,
    CampaignContract,
    Admin,
}

// ── Events ────────────────────────────────────────────────────────────────────

const REWARD_CLAIMED: Symbol = symbol_short!("RWD_CLM");
const REWARD_REDEEMED: Symbol = symbol_short!("RWD_RDM");

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct RewardsContract;

#[contractimpl]
impl RewardsContract {
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
        env.storage()
            .persistent()
            .has(&DataKey::Claimed(user.clone(), campaign_id))
    }

    pub fn claim_reward(env: Env, user: Address, campaign_id: u64) {
        user.require_auth();

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

        // Write claimed state before external mint (reentrancy guard)
        env.storage()
            .persistent()
            .set(&DataKey::Claimed(user.clone(), campaign_id), &true);

        // Invariant: claimed flag must be persisted before any external call.
        #[cfg(debug_assertions)]
        debug_assert!(
            Self::has_claimed(&env, &user, campaign_id),
            "invariant: claimed flag must be set before mint"
        );

        #[cfg(debug_assertions)]
        let balance_before = Self::token_client(&env).balance(&user);

        campaign_client.record_claim(&campaign_id);
        Self::token_client(&env).mint(&user, &campaign.reward_amount);

        // Invariant: user balance increased by exactly reward_amount.
        #[cfg(debug_assertions)]
        debug_assert_eq!(
            Self::token_client(&env).balance(&user),
            balance_before + campaign.reward_amount,
            "invariant: balance must increase by reward_amount after claim"
        );

        env.events().publish(
            (REWARD_CLAIMED, symbol_short!("user"), user.clone()),
            (campaign_id, campaign.reward_amount),
        );
    }

    pub fn redeem_reward(env: Env, user: Address, amount: i128) {
        user.require_auth();
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

    pub fn has_claimed_view(env: Env, user: Address, campaign_id: u64) -> bool {
        Self::has_claimed(&env, &user, campaign_id)
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
        Env,
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

        let token_id = env.register_contract(None, TokenContract);
        let token = soroban_loyalty_token::TokenContractClient::new(&env, &token_id);
        token.initialize(
            &admin,
            &soroban_sdk::String::from_str(&env, "LoyaltyToken"),
            &soroban_sdk::String::from_str(&env, "LYT"),
            &7,
        );

        let campaign_id_addr = env.register_contract(None, CampaignContract);
        let campaign =
            soroban_loyalty_campaign::CampaignContractClient::new(&env, &campaign_id_addr);
        campaign.initialize(&admin);

        let rewards_id = env.register_contract(None, RewardsContract);
        let rewards = RewardsContractClient::new(&env, &rewards_id);
        rewards.initialize(&admin, &token_id, &campaign_id_addr);

        // Give rewards contract mint authority
        token.set_admin(&rewards_id);

        TestSetup { env, token, campaign, rewards }
    }

    #[test]
    fn test_claim_mints_tokens() {
        let t = setup();
        let merchant = Address::generate(&t.env);
        let user = Address::generate(&t.env);
        let expiry = t.env.ledger().timestamp() + 86400;

        let cid = t.campaign.create_campaign(&merchant, &500, &expiry);
        t.rewards.claim_reward(&user, &cid);

        assert_eq!(t.token.balance(&user), 500);
        assert!(t.rewards.has_claimed_view(&user, &cid));
    }

    #[test]
    #[should_panic(expected = "already claimed")]
    fn test_double_claim_prevented() {
        let t = setup();
        let merchant = Address::generate(&t.env);
        let user = Address::generate(&t.env);
        let expiry = t.env.ledger().timestamp() + 86400;

        let cid = t.campaign.create_campaign(&merchant, &500, &expiry);
        t.rewards.claim_reward(&user, &cid);
        t.rewards.claim_reward(&user, &cid);
    }

    #[test]
    #[should_panic(expected = "campaign not active")]
    fn test_claim_inactive_campaign_rejected() {
        let t = setup();
        let merchant = Address::generate(&t.env);
        let user = Address::generate(&t.env);
        let expiry = t.env.ledger().timestamp() + 86400;

        let cid = t.campaign.create_campaign(&merchant, &500, &expiry);
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

        let cid = t.campaign.create_campaign(&merchant, &500, &expiry);
        t.env.ledger().with_mut(|l| l.timestamp = expiry + 1);
        t.rewards.claim_reward(&user, &cid);
    }

    #[test]
    fn test_redeem_burns_tokens() {
        let t = setup();
        let merchant = Address::generate(&t.env);
        let user = Address::generate(&t.env);
        let expiry = t.env.ledger().timestamp() + 86400;

        let cid = t.campaign.create_campaign(&merchant, &500, &expiry);
        t.rewards.claim_reward(&user, &cid);
        t.rewards.redeem_reward(&user, &200);

        assert_eq!(t.token.balance(&user), 300);
        assert_eq!(t.token.total_supply_view(), 300);
    }

    #[test]
    fn test_multiple_users_same_campaign() {
        let t = setup();
        let merchant = Address::generate(&t.env);
        let user1 = Address::generate(&t.env);
        let user2 = Address::generate(&t.env);
        let expiry = t.env.ledger().timestamp() + 86400;

        let cid = t.campaign.create_campaign(&merchant, &100, &expiry);
        t.rewards.claim_reward(&user1, &cid);
        t.rewards.claim_reward(&user2, &cid);

        assert_eq!(t.token.balance(&user1), 100);
        assert_eq!(t.token.balance(&user2), 100);
        assert_eq!(t.token.total_supply_view(), 200);
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
