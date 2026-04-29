# Deployment Guide: Testnet and Mainnet

This guide covers the full lifecycle of deploying the three Soroban Loyalty contracts
(`token`, `campaign`, `rewards`) to Stellar testnet and mainnet, including prerequisites,
step-by-step walkthroughs, post-deployment verification, and rollback procedures.

---

## Table of Contents

1. [Prerequisites](#1-prerequisites)
2. [Pre-Deployment Checklist](#2-pre-deployment-checklist)
3. [Testnet Deployment](#3-testnet-deployment)
4. [Mainnet Deployment](#4-mainnet-deployment)
5. [Post-Deployment Verification](#5-post-deployment-verification)
6. [Updating .env After Deployment](#6-updating-env-after-deployment)
7. [Rollback Procedure](#7-rollback-procedure)
8. [Troubleshooting](#8-troubleshooting)

---

## 1. Prerequisites

### Rust

The workspace requires Rust **1.81.0** (pinned in `rust-toolchain.toml`).
`rustup` will automatically read this file and switch to the correct toolchain
when you are inside the project directory.

```bash
# Install rustup if not present
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Verify the active toolchain (must be 1.81.0)
rustup show active-toolchain
# expected: 1.81.0-<host-triple>

# Add the WebAssembly target
rustup target add wasm32-unknown-unknown
```

### Stellar CLI

The contracts use **soroban-sdk 21.0.0**, which requires **stellar-cli ≥ 21.0.0**.

```bash
# Install via cargo
cargo install --locked stellar-cli@21.5.0

# Or use the official install script (Linux / macOS)
curl --proto '=https' --tlsv1.2 -sSf https://stellar.sh | sh

# Verify
stellar --version
# expected: stellar 21.x.x
```

> **Windows users:** run all commands inside WSL 2 (Ubuntu 22.04 or later).
> The `sed -i` call in `deploy-contracts.sh` is not compatible with native Windows.

### Funded deployer account

| Network | How to fund |
|---------|-------------|
| **Testnet** | Use [Stellar Friendbot](https://friendbot.stellar.org/?addr=YOUR_PUBLIC_KEY) — 10 000 test XLM, free |
| **Mainnet** | Purchase XLM from a CEX (Coinbase, Kraken, Binance) and send to your deployer address. Keep at least **20 XLM** for deployment fees. |

---

## 2. Pre-Deployment Checklist

Run through this list before every deployment regardless of network.

```
[ ] Rust 1.81.0 is active  (rustup show active-toolchain)
[ ] stellar-cli ≥ 21.0.0   (stellar --version)
[ ] wasm32-unknown-unknown target installed  (rustup target list --installed)
[ ] cargo build --release passes with no errors on a clean checkout
[ ] Deployer secret key (S…) is available — never commit it to git
[ ] Deployer account is funded (≥ 5 XLM testnet | ≥ 20 XLM mainnet)
[ ] .env or .env.local exists and contains correct RPC URL / passphrase
[ ] You are NOT re-deploying to an address that already has initialized contracts
    (initialize() panics on a second call — see §7 Rollback for recovery)
[ ] For mainnet: a second team member has reviewed the WASM hashes (§4.3)
```

---

## 3. Testnet Deployment

### 3.1 Generate or import a deployer key

```bash
# Option A — generate a new key
stellar keys generate deployer --network testnet
stellar keys address deployer          # copy the G… address

# Option B — import an existing secret key
stellar keys add deployer --secret-key SXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX
```

### 3.2 Fund the testnet account

```bash
# Friendbot
curl "https://friendbot.stellar.org/?addr=$(stellar keys address deployer)"

# Verify balance (should show ~10 000 XLM)
stellar account info $(stellar keys address deployer) --network testnet
```

### 3.3 Run the deploy script

```bash
chmod +x scripts/deploy-contracts.sh

./scripts/deploy-contracts.sh testnet SXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX
```

**What the script does — step by step:**

| Step | Command | What happens |
|------|---------|--------------|
| 1 | `cargo build --release` × 3 | Compiles each contract to `*.wasm` under `target/wasm32-unknown-unknown/release/` |
| 2 | `stellar contract deploy` (token) | Uploads the token WASM and registers it on-chain; prints `TOKEN_CONTRACT_ID` |
| 3 | `stellar contract deploy` (campaign) | Same for the campaign contract; prints `CAMPAIGN_CONTRACT_ID` |
| 4 | `stellar contract deploy` (rewards) | Same for the rewards contract; prints `REWARDS_CONTRACT_ID` |
| 5 | `token.initialize(admin=REWARDS_ID, …)` | Sets the **rewards contract** as the token mint authority |
| 6 | `campaign.initialize(admin=REWARDS_ID)` | Sets the rewards contract as campaign admin |
| 7 | `rewards.initialize(admin=REWARDS_ID, token=TOKEN_ID, campaign=CAMPAIGN_ID)` | Wires all three contracts together |
| 8 | `.env` update | If `.env` exists, patches the three `*_CONTRACT_ID` lines in place |

> **Deployment order is critical.** Token and Campaign must be deployed first because
> the Rewards contract's `initialize` call needs both addresses. The script enforces
> this order — do not reorder steps.

### 3.4 Expected output

```
==> Building contracts...
==> Deploying Token contract...
TOKEN_CONTRACT_ID=CABC...
==> Deploying Campaign contract...
CAMPAIGN_CONTRACT_ID=CDEF...
==> Deploying Rewards contract...
REWARDS_CONTRACT_ID=CGHI...
==> Initializing Token contract...
==> Initializing Campaign contract...
==> Initializing Rewards contract...
==> Deployment complete. Add these to your .env:
TOKEN_CONTRACT_ID=CABC...
CAMPAIGN_CONTRACT_ID=CDEF...
REWARDS_CONTRACT_ID=CGHI...
==> .env updated.
```

If you see any `Error` line, stop immediately — do not proceed to the next step.
See §7 for recovery.

---

## 4. Mainnet Deployment

Mainnet deployment is irreversible and involves real funds. Take every safety step.

### 4.1 Additional safety checks before mainnet

```
[ ] Testnet deployment completed successfully at least once
[ ] All three contract IDs from testnet have been validated end-to-end
    (backend can read campaigns, frontend can claim rewards)
[ ] WASM hashes reviewed by a second team member (§4.3)
[ ] Deployer account has ≥ 20 XLM on mainnet
[ ] Secret key is stored in a password manager or HSM — not in shell history
[ ] A block explorer tab is open: https://stellar.expert/explorer/public
```

### 4.2 Mainnet deploy (manual — script does not support mainnet by default)

The `deploy-contracts.sh` script only supports `local` and `testnet`. For mainnet,
run the equivalent commands manually to retain full visibility of each step.

```bash
# ── Configuration ─────────────────────────────────────────────────────────────
NETWORK="mainnet"
RPC_URL="https://mainnet.stellar.validationcloud.io/v1/<YOUR_API_KEY>"
# Alternative public RPC (no API key required, lower rate limits):
# RPC_URL="https://rpc.stellar.org"
PASSPHRASE="Public Global Stellar Network ; September 2015"
ADMIN_SECRET="SXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"

# ── Build ─────────────────────────────────────────────────────────────────────
cargo build --release --target wasm32-unknown-unknown \
  --manifest-path contracts/token/Cargo.toml

cargo build --release --target wasm32-unknown-unknown \
  --manifest-path contracts/campaign/Cargo.toml

cargo build --release --target wasm32-unknown-unknown \
  --manifest-path contracts/rewards/Cargo.toml

# ── Deploy ────────────────────────────────────────────────────────────────────
TOKEN_ID=$(stellar contract deploy \
  --wasm target/wasm32-unknown-unknown/release/soroban_loyalty_token.wasm \
  --source "$ADMIN_SECRET" \
  --rpc-url "$RPC_URL" \
  --network-passphrase "$PASSPHRASE")
echo "TOKEN_CONTRACT_ID=$TOKEN_ID"

CAMPAIGN_ID=$(stellar contract deploy \
  --wasm target/wasm32-unknown-unknown/release/soroban_loyalty_campaign.wasm \
  --source "$ADMIN_SECRET" \
  --rpc-url "$RPC_URL" \
  --network-passphrase "$PASSPHRASE")
echo "CAMPAIGN_CONTRACT_ID=$CAMPAIGN_ID"

REWARDS_ID=$(stellar contract deploy \
  --wasm target/wasm32-unknown-unknown/release/soroban_loyalty_rewards.wasm \
  --source "$ADMIN_SECRET" \
  --rpc-url "$RPC_URL" \
  --network-passphrase "$PASSPHRASE")
echo "REWARDS_CONTRACT_ID=$REWARDS_ID"

# ── Initialize ────────────────────────────────────────────────────────────────
stellar contract invoke \
  --id "$TOKEN_ID" \
  --source "$ADMIN_SECRET" \
  --rpc-url "$RPC_URL" \
  --network-passphrase "$PASSPHRASE" \
  -- initialize \
  --admin "$REWARDS_ID" \
  --name "LoyaltyToken" \
  --symbol "LYT" \
  --decimals 7

stellar contract invoke \
  --id "$CAMPAIGN_ID" \
  --source "$ADMIN_SECRET" \
  --rpc-url "$RPC_URL" \
  --network-passphrase "$PASSPHRASE" \
  -- initialize \
  --admin "$REWARDS_ID"

stellar contract invoke \
  --id "$REWARDS_ID" \
  --source "$ADMIN_SECRET" \
  --rpc-url "$RPC_URL" \
  --network-passphrase "$PASSPHRASE" \
  -- initialize \
  --admin "$REWARDS_ID" \
  --token_contract "$TOKEN_ID" \
  --campaign_contract "$CAMPAIGN_ID"
```

> **Pause after each `stellar contract deploy`.** Record the contract ID before running the next command.
> If a later step fails you will know exactly which contracts were already deployed.

### 4.3 Verify WASM hashes before deploying

Compare the SHA-256 hash of each built WASM against what a second team member built
independently from the same git commit. Hashes must match exactly.

```bash
# Team member A runs this and shares output
sha256sum \
  target/wasm32-unknown-unknown/release/soroban_loyalty_token.wasm \
  target/wasm32-unknown-unknown/release/soroban_loyalty_campaign.wasm \
  target/wasm32-unknown-unknown/release/soroban_loyalty_rewards.wasm

# Expected: all three hashes match across both machines
# If they differ: do NOT deploy. Investigate build environment differences.
```

---

## 5. Post-Deployment Verification

### 5.1 Verify contracts on Stellar Expert

Open `https://stellar.expert/explorer/testnet` (or `/public` for mainnet) and
search each contract ID.

For each contract you should see:

- **Created** date/ledger
- **WASM upload** transaction
- **Initialization** invocation (the `initialize` call)

If a contract ID returns "not found", the deployment transaction failed or
was not yet confirmed. Wait 30 seconds and refresh.

### 5.2 Verify the token admin is the rewards contract

```bash
stellar contract invoke \
  --id "$TOKEN_ID" \
  --source "$ADMIN_SECRET" \
  --rpc-url "$RPC_URL" \
  --network-passphrase "$PASSPHRASE" \
  -- admin_address

# Expected output: "$REWARDS_ID"
# If it returns the deployer address instead, the token.initialize step failed
# or was called with the wrong --admin argument.
```

### 5.3 Verify the rewards contract wiring

```bash
# Confirm token contract is set
stellar contract invoke \
  --id "$REWARDS_ID" \
  --source "$ADMIN_SECRET" \
  --rpc-url "$RPC_URL" \
  --network-passphrase "$PASSPHRASE" \
  -- get_token_contract   # or check storage directly via explorer
```

### 5.4 Smoke test: create a campaign and claim a reward

```bash
# 1. Create a test campaign (expiration = 1 hour from now in unix seconds)
EXPIRY=$(( $(date +%s) + 3600 ))

stellar contract invoke \
  --id "$CAMPAIGN_ID" \
  --source "$ADMIN_SECRET" \
  --rpc-url "$RPC_URL" \
  --network-passphrase "$PASSPHRASE" \
  -- create_campaign \
  --merchant $(stellar keys address deployer) \
  --reward_amount 1000000000 \
  --expiration $EXPIRY

# Expected: returns a campaign ID (e.g. "1")

# 2. Claim the reward
stellar contract invoke \
  --id "$REWARDS_ID" \
  --source "$ADMIN_SECRET" \
  --rpc-url "$RPC_URL" \
  --network-passphrase "$PASSPHRASE" \
  -- claim_reward \
  --user $(stellar keys address deployer) \
  --campaign_id 1

# 3. Verify LYT balance
stellar contract invoke \
  --id "$TOKEN_ID" \
  --source "$ADMIN_SECRET" \
  --rpc-url "$RPC_URL" \
  --network-passphrase "$PASSPHRASE" \
  -- balance \
  --addr $(stellar keys address deployer)

# Expected: 1000000000 (1 LYT with 7 decimal places)
```

If any of these steps fail, treat the deployment as failed and follow §7.

---

## 6. Updating .env After Deployment

After a successful deployment, update the three contract IDs in your environment file.

### 6.1 Automatic update (if `.env` exists)

The deploy script patches `.env` automatically:

```bash
sed -i "s|^TOKEN_CONTRACT_ID=.*|TOKEN_CONTRACT_ID=$TOKEN_ID|" .env
sed -i "s|^CAMPAIGN_CONTRACT_ID=.*|CAMPAIGN_CONTRACT_ID=$CAMPAIGN_ID|" .env
sed -i "s|^REWARDS_CONTRACT_ID=.*|REWARDS_CONTRACT_ID=$REWARDS_ID|" .env
```

### 6.2 Manual update

Open `.env` and set all six contract-related variables:

```dotenv
# ── Backend ────────────────────────────────────────────────────────────────────
SOROBAN_RPC_URL=https://soroban-testnet.stellar.org          # or mainnet RPC
NETWORK_PASSPHRASE="Test SDF Network ; September 2015"        # or mainnet passphrase

TOKEN_CONTRACT_ID=CABC...
CAMPAIGN_CONTRACT_ID=CDEF...
REWARDS_CONTRACT_ID=CGHI...

# ── Frontend (Next.js public vars) ─────────────────────────────────────────────
NEXT_PUBLIC_SOROBAN_RPC_URL=https://soroban-testnet.stellar.org
NEXT_PUBLIC_NETWORK_PASSPHRASE="Test SDF Network ; September 2015"
NEXT_PUBLIC_REWARDS_CONTRACT_ID=CGHI...
NEXT_PUBLIC_CAMPAIGN_CONTRACT_ID=CDEF...
```

> The frontend only needs `REWARDS_CONTRACT_ID` and `CAMPAIGN_CONTRACT_ID`.
> `TOKEN_CONTRACT_ID` is used by the backend only.

### 6.3 Network passphrases reference

| Network | Passphrase |
|---------|-----------|
| Local | `Standalone Network ; February 2017` |
| Testnet | `Test SDF Network ; September 2015` |
| Mainnet | `Public Global Stellar Network ; September 2015` |

Passphrase must match the network exactly — including spaces and capitalisation.
A wrong passphrase causes all transactions to be rejected with an auth error.

### 6.4 Restart the backend after updating .env

```bash
# Docker Compose
docker compose restart backend

# Direct (development)
# Kill the process and re-run: npm run dev in backend/
```

---

## 7. Rollback Procedure

### Understanding the constraints

Soroban contracts **cannot be deleted** once deployed. The `initialize` function
panics if called a second time, so a partially-initialised contract cannot be
re-initialised without redeploying.

The rollback procedure is always **deploy-fresh**: deploy all three contracts again
with a new deployer transaction, get new contract IDs, and update your `.env` to
point at the new addresses. The old contract IDs become permanently inert (they hold
state but are unreachable from the application).

### Failure scenarios

#### Scenario A — Build failed

The `cargo build` step failed. No contracts were deployed.

```bash
# Fix the compilation error, then re-run from scratch
./scripts/deploy-contracts.sh testnet $ADMIN_SECRET
```

#### Scenario B — Deployment of Token or Campaign failed mid-way

One or two contracts deployed but not all three. Initialization was never called.

1. Note which contract IDs were printed before the failure.
2. Do **not** try to continue from the failed step — the script has no resume logic.
3. Start a fresh deployment:

```bash
./scripts/deploy-contracts.sh testnet $ADMIN_SECRET
```

The new run deploys three brand-new contracts. The partially-deployed ones are
abandoned (they were never initialized so they hold no user state).

#### Scenario C — Deployment succeeded but initialization failed

All three contracts are deployed, but one or more `initialize` calls failed.
An uninitialised contract will reject all function calls.

```bash
# Check which initialize calls succeeded
stellar contract invoke --id "$TOKEN_ID" ... -- admin_address
# Returns an error → token was never initialized

# Recovery: deploy all three fresh, then re-initialize
./scripts/deploy-contracts.sh testnet $ADMIN_SECRET
```

#### Scenario D — Contracts are initialized but smoke tests fail

The contracts are live but something is wrong with the wiring (e.g. wrong admin set).

1. Do NOT update `.env` — keep pointing at old contract IDs if they still work.
2. Deploy a fresh set:

```bash
./scripts/deploy-contracts.sh testnet $ADMIN_SECRET
```

3. Re-run the smoke tests (§5.4) against the new IDs.
4. Only update `.env` after all smoke tests pass.

#### Scenario E — Mainnet deployment succeeded but a critical bug is found

Because contracts cannot be deleted:

1. **Deactivate all campaigns** via `campaign.set_active(id, false)` to prevent
   new reward claims while the issue is being resolved.
2. Deploy a patched version of the affected contract(s) to new contract IDs.
3. Call `initialize` on the new contracts with the new IDs.
4. Update `.env` and restart the backend.
5. Communicate the new contract IDs to any external integrators.

> If the bug is in the **token** contract: the rewards contract's admin address
> still points at the old token contract. You must redeploy all three contracts
> and re-initialize to restore correct wiring.

---

## 8. Troubleshooting

### `stellar: command not found`

Install stellar-cli (see §1) and ensure `~/.cargo/bin` is in your `PATH`:

```bash
export PATH="$HOME/.cargo/bin:$PATH"
```

### `error: toolchain '1.81.0-...' is not installed`

```bash
rustup toolchain install 1.81.0
rustup target add wasm32-unknown-unknown --toolchain 1.81.0
```

### `error: failed to run custom build command for soroban-sdk`

The wasm32 target is missing:

```bash
rustup target add wasm32-unknown-unknown
```

### `Error: transaction simulation failed: HostError: ... AlreadyInitialized`

You are trying to call `initialize` on a contract that was already initialized.
This is unrecoverable — deploy a fresh set of contracts.

### `Error: account not found` during deploy

The deployer account does not exist on the network or has zero XLM.

- **Testnet:** re-run Friendbot: `curl "https://friendbot.stellar.org/?addr=GXXX..."`
- **Mainnet:** send XLM to the deployer address from an exchange or another account.

### `Error: insufficient XLM balance`

Each `stellar contract deploy` and `stellar contract invoke` costs a small XLM fee.
With three deploys and three initializations, budget at least **5 XLM** on testnet
and **20 XLM** on mainnet.

### `.env` update fails on macOS (`sed -i`)

macOS `sed` requires an explicit backup suffix with `-i`:

```bash
sed -i '' "s|^TOKEN_CONTRACT_ID=.*|TOKEN_CONTRACT_ID=$TOKEN_ID|" .env
```

Consider using `gsed` (GNU sed via Homebrew) for cross-platform compatibility:

```bash
brew install gnu-sed
gsed -i "s|^TOKEN_CONTRACT_ID=.*|TOKEN_CONTRACT_ID=$TOKEN_ID|" .env
```

### Contract ID looks wrong (too short / not starting with `C`)

Stellar contract IDs are 56-character strings starting with `C` (StrKey-encoded).
If the output of `stellar contract deploy` looks different, the deployment failed
and the command printed an error message instead. Re-check the exit code:

```bash
stellar contract deploy ... || echo "DEPLOY FAILED"
```

### RPC timeout or `502 Bad Gateway`

The public testnet RPC (`https://soroban-testnet.stellar.org`) occasionally
experiences high load. Retry after 30–60 seconds, or use an alternative provider:

```
https://soroban-testnet.stellar.org    # SDF public
https://rpc.stellar.org                # SDF mainnet public
```

For production, consider a dedicated RPC node via
[Validation Cloud](https://validationcloud.io/),
[NOWNodes](https://nownodes.io/), or running your own
[stellar/quickstart](https://github.com/stellar/quickstart) container.
