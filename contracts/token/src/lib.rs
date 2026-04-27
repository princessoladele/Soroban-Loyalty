#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, Address, Env, String, Symbol,
};

// ── Storage keys ──────────────────────────────────────────────────────────────

#[contracttype]
pub enum DataKey {
    Admin,
    Balance(Address),
    TotalSupply,
    Name,
    Symbol,
    Decimals,
}

// ── Events ────────────────────────────────────────────────────────────────────

const MINT: Symbol = symbol_short!("MINT");
const TRANSFER: Symbol = symbol_short!("TRANSFER");
const BURN: Symbol = symbol_short!("BURN");

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct TokenContract;

#[contractimpl]
impl TokenContract {
    /// Initialize the token. Can only be called once.
    pub fn initialize(
        env: Env,
        admin: Address,
        name: String,
        symbol: String,
        decimals: u32,
    ) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("already initialized");
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Name, &name);
        env.storage().instance().set(&DataKey::Symbol, &symbol);
        env.storage().instance().set(&DataKey::Decimals, &decimals);
        env.storage().instance().set(&DataKey::TotalSupply, &0_i128);
    }

    // ── Admin helpers ─────────────────────────────────────────────────────────

    fn admin(env: &Env) -> Address {
        env.storage().instance().get(&DataKey::Admin).unwrap()
    }

    fn require_admin(env: &Env) {
        Self::admin(env).require_auth();
    }

    // ── Balance helpers ───────────────────────────────────────────────────────

    // Read balance for a pre-built key, avoiding a second key construction.
    #[inline(always)]
    fn read_balance(env: &Env, key: &DataKey) -> i128 {
        env.storage().persistent().get(key).unwrap_or(0)
    }

    #[inline(always)]
    fn write_balance(env: &Env, key: &DataKey, amount: i128) {
        env.storage().persistent().set(key, &amount);
    }

    fn total_supply(env: &Env) -> i128 {
        env.storage()
            .instance()
            .get(&DataKey::TotalSupply)
            .unwrap_or(0)
    }

    fn set_total_supply(env: &Env, supply: i128) {
        env.storage()
            .instance()
            .set(&DataKey::TotalSupply, &supply);
    }

    // ── Public interface ──────────────────────────────────────────────────────

    pub fn mint(env: Env, to: Address, amount: i128) {
        Self::require_admin(&env);
        assert!(amount > 0, "amount must be positive");

        let key = DataKey::Balance(to.clone());
        let new_bal = Self::read_balance(&env, &key)
            .checked_add(amount)
            .expect("overflow");
        Self::write_balance(&env, &key, new_bal);

        let new_supply = Self::total_supply(&env)
            .checked_add(amount)
            .expect("overflow");
        Self::set_total_supply(&env, new_supply);

        // Invariant: balance and total supply are non-negative after mint.
        #[cfg(debug_assertions)]
        {
            debug_assert!(new_bal >= 0, "invariant: balance >= 0 after mint");
            debug_assert!(new_supply >= 0, "invariant: total_supply >= 0 after mint");
        }

        env.events()
            .publish((MINT, symbol_short!("to"), to), amount);
    }

    pub fn burn(env: Env, from: Address, amount: i128) {
        from.require_auth();
        assert!(amount > 0, "amount must be positive");

        let key = DataKey::Balance(from.clone());
        let bal = Self::read_balance(&env, &key);
        assert!(bal >= amount, "insufficient balance");
        let new_bal = bal - amount;
        Self::write_balance(&env, &key, new_bal);

        let new_supply = Self::total_supply(&env)
            .checked_sub(amount)
            .expect("underflow");
        Self::set_total_supply(&env, new_supply);

        // Invariant: balance and total supply remain non-negative after burn.
        #[cfg(debug_assertions)]
        {
            debug_assert!(new_bal >= 0, "invariant: balance >= 0 after burn");
            debug_assert!(new_supply >= 0, "invariant: total_supply >= 0 after burn");
        }

        env.events()
            .publish((BURN, symbol_short!("from"), from), amount);
    }

    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
        from.require_auth();
        assert!(amount > 0, "amount must be positive");

        let from_key = DataKey::Balance(from.clone());
        let to_key = DataKey::Balance(to.clone());

        let from_bal = Self::read_balance(&env, &from_key);
        assert!(from_bal >= amount, "insufficient balance");
        let to_bal = Self::read_balance(&env, &to_key);

        // Capture supply before to verify it is unchanged after transfer.
        #[cfg(debug_assertions)]
        let supply_before = Self::total_supply(&env);

        let new_from_bal = from_bal - amount;
        let new_to_bal = to_bal.checked_add(amount).expect("overflow");
        Self::write_balance(&env, &from_key, new_from_bal);
        Self::write_balance(&env, &to_key, new_to_bal);

        // Invariants: both balances non-negative; total supply unchanged.
        #[cfg(debug_assertions)]
        {
            debug_assert!(new_from_bal >= 0, "invariant: sender balance >= 0 after transfer");
            debug_assert!(new_to_bal >= 0, "invariant: recipient balance >= 0 after transfer");
            debug_assert_eq!(
                Self::total_supply(&env),
                supply_before,
                "invariant: total_supply unchanged by transfer"
            );
        }

        env.events()
            .publish((TRANSFER, symbol_short!("from"), from), (to, amount));
    }

    pub fn balance(env: Env, addr: Address) -> i128 {
        Self::read_balance(&env, &DataKey::Balance(addr))
    }

    pub fn total_supply_view(env: Env) -> i128 {
        Self::total_supply(&env)
    }

    pub fn admin_address(env: Env) -> Address {
        Self::admin(&env)
    }

    pub fn name(env: Env) -> String {
        env.storage().instance().get(&DataKey::Name).unwrap()
    }

    pub fn symbol(env: Env) -> String {
        env.storage().instance().get(&DataKey::Symbol).unwrap()
    }

    pub fn decimals(env: Env) -> u32 {
        env.storage().instance().get(&DataKey::Decimals).unwrap()
    }

    pub fn set_admin(env: Env, new_admin: Address) {
        Self::require_admin(&env);
        env.storage().instance().set(&DataKey::Admin, &new_admin);
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Env};

    fn setup() -> (Env, Address, TokenContractClient<'static>) {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let contract_id = env.register_contract(None, TokenContract);
        let client = TokenContractClient::new(&env, &contract_id);
        client.initialize(
            &admin,
            &String::from_str(&env, "LoyaltyToken"),
            &String::from_str(&env, "LYT"),
            &7,
        );
        (env, admin, client)
    }

    #[test]
    fn test_mint_and_balance() {
        let (env, _admin, client) = setup();
        let user = Address::generate(&env);
        client.mint(&user, &1000);
        assert_eq!(client.balance(&user), 1000);
        assert_eq!(client.total_supply_view(), 1000);
    }

    #[test]
    fn test_transfer() {
        let (env, _admin, client) = setup();
        let alice = Address::generate(&env);
        let bob = Address::generate(&env);
        client.mint(&alice, &500);
        client.transfer(&alice, &bob, &200);
        assert_eq!(client.balance(&alice), 300);
        assert_eq!(client.balance(&bob), 200);
    }

    #[test]
    fn test_burn() {
        let (env, _admin, client) = setup();
        let user = Address::generate(&env);
        client.mint(&user, &300);
        client.burn(&user, &100);
        assert_eq!(client.balance(&user), 200);
        assert_eq!(client.total_supply_view(), 200);
    }

    #[test]
    #[should_panic(expected = "insufficient balance")]
    fn test_burn_insufficient() {
        let (env, _admin, client) = setup();
        let user = Address::generate(&env);
        client.mint(&user, &50);
        client.burn(&user, &100);
    }

    #[test]
    #[should_panic(expected = "insufficient balance")]
    fn test_transfer_insufficient() {
        let (env, _admin, client) = setup();
        let alice = Address::generate(&env);
        let bob = Address::generate(&env);
        client.mint(&alice, &50);
        client.transfer(&alice, &bob, &100);
    }

    // ── Invariant tests ───────────────────────────────────────────────────────

    #[test]
    fn test_invariant_supply_equals_sum_after_mint() {
        let (env, _admin, client) = setup();
        let u1 = Address::generate(&env);
        let u2 = Address::generate(&env);
        client.mint(&u1, &300);
        client.mint(&u2, &700);
        assert_eq!(
            client.total_supply_view(),
            client.balance(&u1) + client.balance(&u2),
            "total_supply must equal sum of balances"
        );
    }

    #[test]
    fn test_invariant_supply_equals_sum_after_burn() {
        let (env, _admin, client) = setup();
        let user = Address::generate(&env);
        client.mint(&user, &1000);
        client.burn(&user, &400);
        assert_eq!(client.total_supply_view(), client.balance(&user));
        assert!(client.balance(&user) >= 0);
    }

    #[test]
    fn test_invariant_supply_unchanged_by_transfer() {
        let (env, _admin, client) = setup();
        let alice = Address::generate(&env);
        let bob = Address::generate(&env);
        client.mint(&alice, &500);
        let supply_before = client.total_supply_view();
        client.transfer(&alice, &bob, &200);
        assert_eq!(
            client.total_supply_view(),
            supply_before,
            "transfer must not change total_supply"
        );
        assert_eq!(
            client.balance(&alice) + client.balance(&bob),
            supply_before
        );
    }

    // ── Benchmarks ────────────────────────────────────────────────────────────
    // Measure CPU instructions and memory bytes via the Soroban budget tracker.
    // Run with:  cargo test -p soroban-loyalty-token bench -- --nocapture

    #[test]
    fn bench_transfer() {
        let (env, _admin, client) = setup();
        let alice = Address::generate(&env);
        let bob = Address::generate(&env);
        client.mint(&alice, &1_000_000);

        // First transfer: bob has no existing balance (cold recipient).
        env.budget().reset_default();
        client.transfer(&alice, &bob, &100);
        let cpu1 = env.budget().cpu_instruction_cost();
        let mem1 = env.budget().memory_bytes_cost();

        // Subsequent transfer: both accounts already have balances (warm path).
        env.budget().reset_default();
        client.transfer(&alice, &bob, &100);
        let cpu2 = env.budget().cpu_instruction_cost();
        let mem2 = env.budget().memory_bytes_cost();

        soroban_sdk::log!(
            &env,
            "bench transfer cold  — cpu: {}, mem: {}",
            cpu1,
            mem1
        );
        soroban_sdk::log!(
            &env,
            "bench transfer warm  — cpu: {}, mem: {}",
            cpu2,
            mem2
        );
    }

    #[test]
    fn bench_mint() {
        let (env, _admin, client) = setup();
        let user = Address::generate(&env);

        env.budget().reset_default();
        client.mint(&user, &1000);
        let cpu1 = env.budget().cpu_instruction_cost();
        let mem1 = env.budget().memory_bytes_cost();

        env.budget().reset_default();
        client.mint(&user, &1000);
        let cpu2 = env.budget().cpu_instruction_cost();
        let mem2 = env.budget().memory_bytes_cost();

        soroban_sdk::log!(&env, "bench mint first      — cpu: {}, mem: {}", cpu1, mem1);
        soroban_sdk::log!(&env, "bench mint subsequent — cpu: {}, mem: {}", cpu2, mem2);
    }

    #[test]
    fn bench_burn() {
        let (env, _admin, client) = setup();
        let user = Address::generate(&env);
        client.mint(&user, &10_000);

        env.budget().reset_default();
        client.burn(&user, &100);
        let cpu = env.budget().cpu_instruction_cost();
        let mem = env.budget().memory_bytes_cost();

        soroban_sdk::log!(&env, "bench burn            — cpu: {}, mem: {}", cpu, mem);
    }
}
