#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, Address, Env, String, Symbol, Vec,
};

// ── Storage keys ──────────────────────────────────────────────────────────────

#[contracttype]
pub enum DataKey {
    /// Multi-sig admin config
    MultiSig,
    /// Designated minter (e.g. rewards contract) — can call mint() directly
    Minter,
    Balance(Address),
    Allowance(Address, Address),
    TotalSupply,
    Name,
    Symbol,
    Decimals,
    /// Pending set_admin proposal awaiting threshold signatures
    SetAdminProposal,
}

// ── Multi-sig types ───────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone)]
pub struct MultiSigConfig {
    pub signers: Vec<Address>,
    pub threshold: u32,
}

#[contracttype]
#[derive(Clone)]
pub struct SetAdminProposal {
    pub new_config: MultiSigConfig,
    pub new_minter: Address,
    pub signatures: Vec<Address>,
}

// ── Events ────────────────────────────────────────────────────────────────────

const MINT: Symbol = symbol_short!("MINT");
const TRANSFER: Symbol = symbol_short!("TRANSFER");
const BURN: Symbol = symbol_short!("BURN");
const APPROVAL: Symbol = symbol_short!("APPROVAL");
const PAUSED: Symbol = symbol_short!("PAUSED");
const UNPAUSED: Symbol = symbol_short!("UNPAUSED");

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct TokenContract;

#[contractimpl]
impl TokenContract {
    /// Initialize the token with multi-sig admin config and a designated minter.
    /// `minter` is the rewards contract address that can call `mint()` directly.
    pub fn initialize(
        env: Env,
        signers: Vec<Address>,
        threshold: u32,
        minter: Address,
        name: String,
        symbol: String,
        decimals: u32,
    ) {
        if env.storage().instance().has(&DataKey::MultiSig) {
            panic!("already initialized");
        }
        assert!(threshold > 0, "threshold must be positive");
        assert!(signers.len() >= threshold, "insufficient signers for threshold");

        let config = MultiSigConfig { signers, threshold };
        env.storage().instance().set(&DataKey::MultiSig, &config);
        env.storage().instance().set(&DataKey::Minter, &minter);
        env.storage().instance().set(&DataKey::Name, &name);
        env.storage().instance().set(&DataKey::Symbol, &symbol);
        env.storage().instance().set(&DataKey::Decimals, &decimals);
        env.storage().instance().set(&DataKey::TotalSupply, &0_i128);
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    fn config(env: &Env) -> MultiSigConfig {
        env.storage().instance().get(&DataKey::MultiSig).unwrap()
    }

    fn minter(env: &Env) -> Address {
        env.storage().instance().get(&DataKey::Minter).unwrap()
    }

    fn require_signer(env: &Env, signer: &Address) {
        signer.require_auth();
        let cfg = Self::config(env);
        let mut found = false;
        for s in cfg.signers.iter() {
            if s == *signer {
                found = true;
                break;
            }
        }
        assert!(found, "not a signer");
    }

    fn has_signed(signatures: &Vec<Address>, signer: &Address) -> bool {
        for s in signatures.iter() {
            if s == *signer {
                return true;
            }
        }
        false
    }

    #[inline(always)]
    fn read_balance(env: &Env, key: &DataKey) -> i128 {
        env.storage().persistent().get(key).unwrap_or(0)
    }

    #[inline(always)]
    fn write_balance(env: &Env, key: &DataKey, amount: i128) {
        env.storage().persistent().set(key, &amount);
    }

    fn total_supply(env: &Env) -> i128 {
        env.storage().instance().get(&DataKey::TotalSupply).unwrap_or(0)
    }

    fn set_total_supply(env: &Env, supply: i128) {
        env.storage().instance().set(&DataKey::TotalSupply, &supply);
    }

    fn get_allowance(env: &Env, owner: &Address, spender: &Address) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::Allowance(owner.clone(), spender.clone()))
            .unwrap_or(0)
    }

    fn set_allowance(env: &Env, owner: &Address, spender: &Address, amount: i128) {
        env.storage()
            .persistent()
            .set(&DataKey::Allowance(owner.clone(), spender.clone()), &amount);
    }

    fn do_mint(env: &Env, to: &Address, amount: i128) {
        let key = DataKey::Balance(to.clone());
        let new_bal = Self::read_balance(env, &key).checked_add(amount).expect("overflow");
        Self::write_balance(env, &key, new_bal);
        let new_supply = Self::total_supply(env).checked_add(amount).expect("overflow");
        Self::set_total_supply(env, new_supply);
        env.events().publish((MINT, symbol_short!("to"), to.clone()), (amount, new_supply));
    }

    // ── Minter-only mint (called by rewards contract) ─────────────────────────

    /// Direct mint callable only by the designated minter (rewards contract).
    pub fn mint(env: Env, to: Address, amount: i128) {
        let minter = Self::minter(&env);
        minter.require_auth();
        assert!(amount > 0, "amount must be positive");
        Self::do_mint(&env, &to, amount);
    }

    // ── Multi-sig admin rotation ──────────────────────────────────────────────

    /// Propose replacing the multi-sig config and minter. First signer initiates.
    pub fn propose_set_admin(
        env: Env,
        signer: Address,
        new_signers: Vec<Address>,
        new_threshold: u32,
        new_minter: Address,
    ) {
        Self::require_signer(&env, &signer);
        assert!(new_threshold > 0, "threshold must be positive");
        assert!(new_signers.len() >= new_threshold, "insufficient signers for threshold");
        assert!(
            !env.storage().instance().has(&DataKey::SetAdminProposal),
            "set_admin proposal already pending"
        );

        let new_config = MultiSigConfig { signers: new_signers, threshold: new_threshold };
        let mut signatures = Vec::new(&env);
        signatures.push_back(signer);
        let proposal = SetAdminProposal { new_config, new_minter, signatures };
        env.storage().instance().set(&DataKey::SetAdminProposal, &proposal);
    }

    /// Add a signature to the pending set_admin proposal.
    pub fn approve_set_admin(env: Env, signer: Address) {
        Self::require_signer(&env, &signer);
        let mut proposal: SetAdminProposal = env
            .storage()
            .instance()
            .get(&DataKey::SetAdminProposal)
            .expect("no pending set_admin proposal");

        assert!(!Self::has_signed(&proposal.signatures, &signer), "duplicate signature");

        proposal.signatures.push_back(signer);
        env.storage().instance().set(&DataKey::SetAdminProposal, &proposal);
    }

    /// Execute the admin rotation once threshold signatures are collected.
    pub fn execute_set_admin(env: Env, signer: Address) {
        Self::require_signer(&env, &signer);
        let proposal: SetAdminProposal = env
            .storage()
            .instance()
            .get(&DataKey::SetAdminProposal)
            .expect("no pending set_admin proposal");

        let cfg = Self::config(&env);
        assert!(proposal.signatures.len() >= cfg.threshold, "insufficient signatures");

        env.storage().instance().remove(&DataKey::SetAdminProposal);
        env.storage().instance().set(&DataKey::MultiSig, &proposal.new_config);
        env.storage().instance().set(&DataKey::Minter, &proposal.new_minter);
    }

    // ── Public token interface ────────────────────────────────────────────────

    pub fn burn(env: Env, from: Address, amount: i128) {
        from.require_auth();
        Self::require_not_paused(&env);
        assert!(amount > 0, "amount must be positive");

        let key = DataKey::Balance(from.clone());
        let bal = Self::read_balance(&env, &key);
        assert!(bal >= amount, "insufficient balance");
        let new_bal = bal - amount;
        Self::write_balance(&env, &key, new_bal);

        let new_supply = Self::total_supply(&env).checked_sub(amount).expect("underflow");
        Self::set_total_supply(&env, new_supply);

        env.events().publish((BURN, symbol_short!("from"), from), (amount, new_supply));
    }

    /// Transfer `amount` LYT tokens from `from` to `to`.
    ///
    /// # Security
    /// Requires `from.require_auth()`.
    ///
    /// # Panics
    /// - `"amount must be positive"` — if `amount <= 0`
    /// - `"insufficient balance"` — if `from`'s balance is less than `amount`
    /// - `"overflow"` — if `to`'s balance would overflow `i128`
    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
        from.require_auth();
        Self::require_not_paused(&env);
        assert!(amount > 0, "amount must be positive");

        let from_key = DataKey::Balance(from.clone());
        let to_key = DataKey::Balance(to.clone());
        let from_bal = Self::read_balance(&env, &from_key);
        assert!(from_bal >= amount, "insufficient balance");
        let to_bal = Self::read_balance(&env, &to_key);

        Self::write_balance(&env, &from_key, from_bal - amount);
        Self::write_balance(&env, &to_key, to_bal.checked_add(amount).expect("overflow"));

        env.events().publish((TRANSFER, symbol_short!("from"), from), (to, amount));
    }

    pub fn approve(env: Env, owner: Address, spender: Address, amount: i128) {
        owner.require_auth();
        Self::require_not_paused(&env);
        assert!(amount >= 0, "amount must be non-negative");
        Self::set_allowance(&env, &owner, &spender, amount);
        env.events().publish((APPROVAL, symbol_short!("owner"), owner), (spender, amount));
    }

    pub fn transfer_from(env: Env, spender: Address, from: Address, to: Address, amount: i128) {
        spender.require_auth();
        Self::require_not_paused(&env);
        assert!(amount > 0, "amount must be positive");

        let current = Self::get_allowance(&env, &from, &spender);
        assert!(current >= amount, "allowance exceeded");

        let from_key = DataKey::Balance(from.clone());
        let to_key = DataKey::Balance(to.clone());
        let from_bal = Self::read_balance(&env, &from_key);
        assert!(from_bal >= amount, "insufficient balance");
        let to_bal = Self::read_balance(&env, &to_key);

        Self::set_allowance(&env, &from, &spender, current - amount);
        Self::write_balance(&env, &from_key, from_bal - amount);
        Self::write_balance(&env, &to_key, to_bal.checked_add(amount).expect("overflow"));

        env.events().publish((TRANSFER, symbol_short!("from"), from), (to, amount));
    }

    pub fn allowance(env: Env, owner: Address, spender: Address) -> i128 {
        Self::get_allowance(&env, &owner, &spender)
    }

    /// Returns the LYT balance of `addr`.
    pub fn balance(env: Env, addr: Address) -> i128 {
        Self::read_balance(&env, &DataKey::Balance(addr))
    }

    /// Returns the current total supply of LYT tokens.
    pub fn total_supply_view(env: Env) -> i128 {
        Self::total_supply(&env)
    }

    pub fn multisig_config(env: Env) -> MultiSigConfig {
        Self::config(&env)
    }

    pub fn minter_address(env: Env) -> Address {
        Self::minter(&env)
    }

    /// Returns the token name (e.g. `"LoyaltyToken"`).
    pub fn name(env: Env) -> String {
        env.storage().instance().get(&DataKey::Name).unwrap()
    }

    /// Returns the token ticker symbol (e.g. `"LYT"`).
    pub fn symbol(env: Env) -> String {
        env.storage().instance().get(&DataKey::Symbol).unwrap()
    }

    /// Returns the number of decimal places (e.g. `7`).
    pub fn decimals(env: Env) -> u32 {
        env.storage().instance().get(&DataKey::Decimals).unwrap()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Env};

    fn setup_2of3(env: &Env) -> (Address, Address, Address, Address, TokenContractClient<'static>) {
        let s1 = Address::generate(env);
        let s2 = Address::generate(env);
        let s3 = Address::generate(env);
        let minter = Address::generate(env);
        let id = env.register_contract(None, TokenContract);
        let client = TokenContractClient::new(env, &id);
        let mut signers = Vec::new(env);
        signers.push_back(s1.clone());
        signers.push_back(s2.clone());
        signers.push_back(s3.clone());
        client.initialize(
            &signers,
            &2,
            &minter,
            &String::from_str(env, "LoyaltyToken"),
            &String::from_str(env, "LYT"),
            &7,
        );
        (s1, s2, s3, minter, client)
    }

    #[test]
    fn test_minter_can_mint_directly() {
        let env = Env::default();
        env.mock_all_auths();
        let (_s1, _s2, _s3, minter, client) = setup_2of3(&env);
        let user = Address::generate(&env);

        client.mint(&user, &1000);
        assert_eq!(client.balance(&user), 1000);
        assert_eq!(client.total_supply_view(), 1000);
    }

    #[test]
    fn test_valid_multisig_set_admin() {
        let env = Env::default();
        env.mock_all_auths();
        let (s1, s2, _s3, _minter, client) = setup_2of3(&env);
        let new_s1 = Address::generate(&env);
        let new_s2 = Address::generate(&env);
        let new_minter = Address::generate(&env);
        let mut new_signers = Vec::new(&env);
        new_signers.push_back(new_s1.clone());
        new_signers.push_back(new_s2.clone());

        client.propose_set_admin(&s1, &new_signers, &2, &new_minter);
        client.approve_set_admin(&s2);
        client.execute_set_admin(&s1);

        let cfg = client.multisig_config();
        assert_eq!(cfg.threshold, 2);
        assert_eq!(cfg.signers.len(), 2);
        assert_eq!(client.minter_address(), new_minter);
    }

    #[test]
    #[should_panic(expected = "insufficient signatures")]
    fn test_insufficient_signatures_for_set_admin() {
        let env = Env::default();
        env.mock_all_auths();
        let (s1, _s2, _s3, _minter, client) = setup_2of3(&env);
        let new_s1 = Address::generate(&env);
        let new_minter = Address::generate(&env);
        let mut new_signers = Vec::new(&env);
        new_signers.push_back(new_s1.clone());

        client.propose_set_admin(&s1, &new_signers, &1, &new_minter);
        // Only 1 signature, threshold is 2
        client.execute_set_admin(&s1);
    }

    #[test]
    #[should_panic(expected = "duplicate signature")]
    fn test_duplicate_signature_rejected() {
        let env = Env::default();
        env.mock_all_auths();
        let (s1, _s2, _s3, _minter, client) = setup_2of3(&env);
        let new_s1 = Address::generate(&env);
        let new_minter = Address::generate(&env);
        let mut new_signers = Vec::new(&env);
        new_signers.push_back(new_s1.clone());

        client.propose_set_admin(&s1, &new_signers, &1, &new_minter);
        client.approve_set_admin(&s1); // s1 already signed via propose
    }

    #[test]
    #[should_panic(expected = "not a signer")]
    fn test_non_signer_cannot_propose() {
        let env = Env::default();
        env.mock_all_auths();
        let (_s1, _s2, _s3, _minter, client) = setup_2of3(&env);
        let outsider = Address::generate(&env);
        let new_minter = Address::generate(&env);
        let mut new_signers = Vec::new(&env);
        new_signers.push_back(outsider.clone());

        client.propose_set_admin(&outsider, &new_signers, &1, &new_minter);
    }

    #[test]
    fn test_transfer_and_burn() {
        let env = Env::default();
        env.mock_all_auths();
        let (_s1, _s2, _s3, _minter, client) = setup_2of3(&env);
        let alice = Address::generate(&env);
        let bob = Address::generate(&env);

        client.mint(&alice, &500);
        client.transfer(&alice, &bob, &200);
        assert_eq!(client.balance(&alice), 300);
        assert_eq!(client.balance(&bob), 200);

        client.burn(&alice, &100);
        assert_eq!(client.balance(&alice), 200);
        assert_eq!(client.total_supply_view(), 400);
    }

    // ── Pause tests ───────────────────────────────────────────────────────────

    #[test]
    fn test_pause_and_unpause() {
        let (_env, _admin, client) = setup();
        assert!(!client.paused());
        client.emergency_pause();
        assert!(client.paused());
        client.emergency_unpause();
        assert!(!client.paused());
    }

    #[test]
    #[should_panic(expected = "contract is paused")]
    fn test_mint_blocked_when_paused() {
        let (env, _admin, client) = setup();
        let user = Address::generate(&env);
        client.emergency_pause();
        client.mint(&user, &100);
    }

    #[test]
    #[should_panic(expected = "contract is paused")]
    fn test_transfer_blocked_when_paused() {
        let (env, _admin, client) = setup();
        let alice = Address::generate(&env);
        let bob = Address::generate(&env);
        client.mint(&alice, &500);
        client.emergency_pause();
        client.transfer(&alice, &bob, &100);
    }

    #[test]
    #[should_panic(expected = "contract is paused")]
    fn test_burn_blocked_when_paused() {
        let (env, _admin, client) = setup();
        let user = Address::generate(&env);
        client.mint(&user, &300);
        client.emergency_pause();
        client.burn(&user, &100);
    }
}

#[cfg(test)]
mod fuzz_tests;
