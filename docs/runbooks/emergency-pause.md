# Emergency Pause Runbook

**Audience:** On-call engineers, security team  
**Last updated:** 2026-04-27

---

## Overview

All three Soroban contracts (token, campaign, rewards) expose an `emergency_pause` / `emergency_unpause` function. When paused, every state-changing operation reverts with `"contract is paused"`. Read-only views continue to work.

| Contract | Pause auth | Guarded functions |
|---|---|---|
| `token` | Single admin (`require_admin`) | `mint`, `burn`, `transfer`, `approve`, `transfer_from` |
| `campaign` | Multi-sig admin list + threshold | `create_campaign`, `set_active`, `record_claim` |
| `rewards` | Single admin (`require_admin`) | `claim_reward`, `redeem_reward` |

Each pause/unpause emits a `PAUSED` / `UNPAUSED` event that the indexer picks up for monitoring.

---

## When to pause

Pause immediately if any of the following are confirmed or strongly suspected:

- Active exploit draining user balances or minting tokens without authorization
- Critical vulnerability disclosed that affects state-changing contract functions
- Compromised admin key with evidence of malicious transactions
- Abnormal on-chain activity (e.g., unexpected large mints, rapid claim loops)

Do **not** pause for routine maintenance, frontend outages, or backend indexer lag.

---

## Prerequisites

- Stellar CLI installed and on `$PATH`
- Admin secret key available (hardware wallet or secure vault — never plaintext on disk)
- Contract addresses from `.env` (`TOKEN_CONTRACT_ID`, `CAMPAIGN_CONTRACT_ID`, `REWARDS_CONTRACT_ID`)
- For campaign contract: at least `threshold` admin signers available

---

## Step 1 — Confirm the incident

1. Check Grafana / alertmanager for anomalous event spikes (`MINT`, `RWD_CLM`).
2. Query on-chain state to verify the suspected behaviour:
   ```bash
   stellar contract invoke \
     --id $TOKEN_CONTRACT_ID \
     --network $NETWORK \
     -- paused
   ```
3. Open a war-room channel (Slack `#incident-response`) and assign an incident commander.
4. Record the incident start time and initial findings in the post-mortem doc (`docs/post-mortem-template.md`).

---

## Step 2 — Pause all three contracts

Run these commands in order. Each requires the admin key to sign.

### Token contract
```bash
stellar contract invoke \
  --id $TOKEN_CONTRACT_ID \
  --source $ADMIN_SECRET_KEY \
  --network $NETWORK \
  -- emergency_pause
```

### Rewards contract
```bash
stellar contract invoke \
  --id $REWARDS_CONTRACT_ID \
  --source $ADMIN_SECRET_KEY \
  --network $NETWORK \
  -- emergency_pause
```

### Campaign contract (multi-sig — requires `threshold` admins)

Each admin signs independently:
```bash
# Admin 1
stellar contract invoke \
  --id $CAMPAIGN_CONTRACT_ID \
  --source $ADMIN1_SECRET_KEY \
  --network $NETWORK \
  -- emergency_pause \
  --admin $ADMIN1_ADDRESS

# Admin 2 (repeat for each required signer up to threshold)
stellar contract invoke \
  --id $CAMPAIGN_CONTRACT_ID \
  --source $ADMIN2_SECRET_KEY \
  --network $NETWORK \
  -- emergency_pause \
  --admin $ADMIN2_ADDRESS
```

> **Note:** The campaign `emergency_pause` requires a single call from any one admin in the admin list — the `require_admin` check verifies the caller is in the list and calls `require_auth()`. Only one admin call is needed to pause; the multi-sig threshold applies to upgrades, not pause.

### Verify all three are paused
```bash
for CONTRACT in $TOKEN_CONTRACT_ID $REWARDS_CONTRACT_ID $CAMPAIGN_CONTRACT_ID; do
  echo -n "$CONTRACT paused: "
  stellar contract invoke --id $CONTRACT --network $NETWORK -- paused
done
```

Expected output: `true` for each contract.

---

## Step 3 — Notify stakeholders

- Post in `#incident-response`: contracts paused, time, reason.
- Update status page (if applicable) to "Degraded — transactions temporarily halted."
- Notify merchant partners if campaigns are affected.

---

## Step 4 — Investigate and remediate

1. Pull recent events from the indexer DB:
   ```sql
   SELECT * FROM events
   WHERE event_type IN ('MINT','RWD_CLM','TRANSFER')
   ORDER BY created_at DESC
   LIMIT 100;
   ```
2. Identify affected accounts and quantify impact.
3. If a contract upgrade is required, follow `docs/runbooks/operations.md` → Upgrade section.
4. If an admin key is compromised, rotate it via `set_admin` (token/rewards) or re-deploy (campaign) **before** unpausing.

---

## Step 5 — Unpause

Only unpause after the root cause is fixed and the fix is verified on testnet.

```bash
# Token
stellar contract invoke \
  --id $TOKEN_CONTRACT_ID \
  --source $ADMIN_SECRET_KEY \
  --network $NETWORK \
  -- emergency_unpause

# Rewards
stellar contract invoke \
  --id $REWARDS_CONTRACT_ID \
  --source $ADMIN_SECRET_KEY \
  --network $NETWORK \
  -- emergency_unpause

# Campaign (any one admin in the list)
stellar contract invoke \
  --id $CAMPAIGN_CONTRACT_ID \
  --source $ADMIN1_SECRET_KEY \
  --network $NETWORK \
  -- emergency_unpause \
  --admin $ADMIN1_ADDRESS
```

Verify:
```bash
for CONTRACT in $TOKEN_CONTRACT_ID $REWARDS_CONTRACT_ID $CAMPAIGN_CONTRACT_ID; do
  echo -n "$CONTRACT paused: "
  stellar contract invoke --id $CONTRACT --network $NETWORK -- paused
done
```

Expected output: `false` for each contract.

---

## Step 6 — Post-incident

1. Fill in `docs/post-mortem-template.md` within 48 hours.
2. Update this runbook if the procedure revealed gaps.
3. Add monitoring alert rules for the `PAUSED` / `UNPAUSED` events in `monitoring/alerts.yml`.

---

## Quick-reference card

| Action | Command suffix |
|---|---|
| Check pause state | `-- paused` |
| Pause (token / rewards) | `-- emergency_pause` |
| Pause (campaign) | `-- emergency_pause --admin <ADDR>` |
| Unpause (token / rewards) | `-- emergency_unpause` |
| Unpause (campaign) | `-- emergency_unpause --admin <ADDR>` |
