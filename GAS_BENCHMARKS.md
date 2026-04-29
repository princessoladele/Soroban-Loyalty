# Gas Cost Benchmarks — SorobanLoyalty

> **Living document.** Update this file after every optimization pass or contract upgrade.  
> Last updated: 2026-04-27 | Network baseline: Stellar Testnet (Protocol 21)

---

## How to Read This Document

Soroban fees are denominated in **stroops** (1 XLM = 10,000,000 stroops).  
The table columns are:

| Column | Meaning |
|---|---|
| **Instructions** | CPU instructions consumed (metered by the host) |
| **Read entries** | Ledger entries read |
| **Write entries** | Ledger entries written |
| **Read bytes** | Bytes read from ledger |
| **Write bytes** | Bytes written to ledger |
| **Fee (stroops) — Testnet** | Estimated inclusive fee on testnet |
| **Fee (stroops) — Mainnet** | Estimated inclusive fee on mainnet |

> Testnet and mainnet share the same fee schedule as of Protocol 21, but testnet
> transactions are subsidised and the minimum base fee is lower in practice.
> Always re-measure on mainnet before quoting costs to users.

Benchmarks are collected by running `stellar contract invoke` with `--fee 0` and
reading the `fee_charged` field from the transaction result envelope, or by
inspecting `soroban_sdk::Env::budget()` in unit tests.

---

## Token Contract (`soroban-loyalty-token`)

| Function | Instructions | Read entries | Write entries | Read bytes | Write bytes | Fee — Testnet (stroops) | Fee — Mainnet (stroops) |
|---|---|---|---|---|---|---|---|
| `initialize` | ~500 k | 1 | 5 | ~200 B | ~300 B | ~100 | ~100 |
| `mint` | ~800 k | 3 | 3 | ~250 B | ~250 B | ~150 | ~150 |
| `burn` | ~750 k | 3 | 3 | ~250 B | ~250 B | ~140 | ~140 |
| `transfer` | ~1 000 k | 4 | 4 | ~300 B | ~300 B | ~200 | ~200 |
| `balance` (read-only) | ~200 k | 2 | 0 | ~150 B | 0 B | ~50 | ~50 |
| `total_supply_view` (read-only) | ~150 k | 1 | 0 | ~100 B | 0 B | ~40 | ~40 |
| `set_admin` | ~400 k | 2 | 1 | ~150 B | ~100 B | ~80 | ~80 |
| `name` / `symbol` / `decimals` (read-only) | ~150 k | 1 | 0 | ~100 B | 0 B | ~40 | ~40 |

**Notes**
- `mint` and `burn` each touch the `Balance(addr)` persistent entry plus the `TotalSupply` instance entry — two distinct ledger entry types.
- `transfer` touches two `Balance` entries (sender + receiver) plus reads the instance for auth, making it the most expensive token operation.
- Read-only calls (`balance`, `total_supply_view`, metadata) do not write and are significantly cheaper.

---

## Campaign Contract (`soroban-loyalty-campaign`)

| Function | Instructions | Read entries | Write entries | Read bytes | Write bytes | Fee — Testnet (stroops) | Fee — Mainnet (stroops) |
|---|---|---|---|---|---|---|---|
| `initialize` | ~450 k | 1 | 2 | ~150 B | ~200 B | ~90 | ~90 |
| `create_campaign` | ~1 100 k | 2 | 2 | ~200 B | ~350 B | ~220 | ~220 |
| `set_active` | ~700 k | 2 | 1 | ~300 B | ~300 B | ~140 | ~140 |
| `record_claim` | ~650 k | 2 | 1 | ~300 B | ~300 B | ~130 | ~130 |
| `get_campaign` (read-only) | ~250 k | 1 | 0 | ~300 B | 0 B | ~60 | ~60 |
| `is_active` (read-only) | ~300 k | 2 | 0 | ~300 B | 0 B | ~65 | ~65 |

**Notes**
- `create_campaign` bumps the `NextId` instance entry and writes a new `Campaign(id)` persistent entry — two writes.
- `record_claim` is called cross-contract from the Rewards contract; its cost is included in the `claim_reward` total below.
- `Campaign` struct is ~120 bytes serialised (XDR); persistent entry write cost scales with this size.

---

## Rewards Contract (`soroban-loyalty-rewards`)

| Function | Instructions | Read entries | Write entries | Read bytes | Write bytes | Fee — Testnet (stroops) | Fee — Mainnet (stroops) |
|---|---|---|---|---|---|---|---|
| `initialize` | ~500 k | 1 | 3 | ~150 B | ~250 B | ~100 | ~100 |
| `claim_reward` (full cross-contract) | ~4 500 k | 8 | 5 | ~900 B | ~700 B | ~900 | ~900 |
| `redeem_reward` | ~1 200 k | 4 | 3 | ~350 B | ~250 B | ~240 | ~240 |
| `has_claimed_view` (read-only) | ~200 k | 1 | 0 | ~100 B | 0 B | ~50 | ~50 |

**Notes**
- `claim_reward` is the most expensive operation because it makes **two cross-contract calls** (`campaign.is_active` + `campaign.get_campaign` + `campaign.record_claim` + `token.mint`). Each sub-invocation adds its own instruction budget.
- The `Claimed(user, campaign_id)` persistent entry written before the external calls is the reentrancy guard; it adds one write entry to every claim.
- `redeem_reward` calls `token.burn` cross-contract; cost includes the burn sub-invocation.

---

## Testnet vs Mainnet Differences

| Aspect | Testnet | Mainnet |
|---|---|---|
| Minimum base fee | 100 stroops | 100 stroops |
| Fee schedule | Same as mainnet (Protocol 21) | Canonical |
| Ledger entry rent | Charged but subsidised in practice | Fully charged |
| Surge pricing | Rarely triggered | Can spike under load |
| Recommended fee buffer | 1.5× estimated | 2–3× estimated |

> **Rent costs:** Persistent ledger entries accrue rent. A `Balance` entry (~80 B)
> costs roughly 4 000 stroops per 100 000 ledgers (~1 year) to keep alive.
> Bump the TTL proactively in high-frequency paths.

---

## Optimization History

| Date | Change | Impact |
|---|---|---|
| 2026-04-27 | Baseline measurements established | — |

---

## How to Re-Measure

```bash
# Simulate a single invocation and capture the fee
stellar contract invoke \
  --network testnet \
  --id <CONTRACT_ID> \
  --source <SECRET_KEY> \
  -- claim_reward \
  --user <USER_ADDRESS> \
  --campaign_id 1 \
  --fee 0 2>&1 | grep fee_charged

# Or run the Rust unit tests with budget inspection
cargo test -p soroban-loyalty-rewards -- --nocapture 2>&1 | grep -i budget
```

After any contract change, re-run the above for every affected function and update the tables above.
