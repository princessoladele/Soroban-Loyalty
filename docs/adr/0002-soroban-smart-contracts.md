# ADR-0002: Use Soroban for On-Chain Logic

- **Status:** Accepted
- **Date:** 2026-04-26
- **Authors:** Platform Team

---

## Context

SorobanLoyalty requires tamper-proof, trustless execution of three core operations: minting reward tokens when a user claims a campaign, burning tokens when a user redeems them, and enforcing campaign rules (expiry, single-claim-per-user). These operations involve value transfer and must be auditable by any participant without trusting the platform operator.

The Stellar network added Soroban — its native smart contract platform — in Protocol 20 (early 2024). Soroban contracts are written in Rust, compiled to WASM, and executed deterministically on every validator. The alternative was to keep all business logic in the off-chain backend, relying on the platform's database as the source of truth.

Key constraints:
- The team already uses Stellar for payments and has existing Stellar infrastructure.
- Token issuance must be permissionless and verifiable on-chain.
- Double-claim prevention must be enforced without trusting the backend.
- The team has Rust experience.

## Decision

We will implement all value-bearing logic — token mint/burn, campaign lifecycle, and claim/redeem enforcement — as Soroban smart contracts on the Stellar network. Three contracts are deployed:

- `token` — a fungible LYT token with admin-controlled minting.
- `campaign` — merchants create campaigns with a reward amount and expiry timestamp.
- `rewards` — users call `claim_reward` (mints LYT) and `redeem_reward` (burns LYT); double-claim is prevented by writing claimed state before any external call.

The backend and frontend are read/write clients of these contracts, not the authoritative source of truth.

## Consequences

### Positive
- All value operations are auditable on-chain; no trust in the platform operator is required.
- Double-claim prevention is enforced at the contract level and cannot be bypassed by a compromised backend.
- Reentrancy is mitigated by writing state before external calls (checks-effects-interactions pattern).
- Soroban's WASM sandbox provides deterministic, metered execution with no network I/O.
- Rust's type system and `overflow-checks = true` in release builds prevent integer overflow.

### Negative
- Contract upgrades require a governance process; bugs cannot be patched with a simple deploy.
- Soroban is relatively new; tooling, documentation, and community resources are less mature than EVM.
- Every user action that mutates state requires a signed transaction and a ledger confirmation (~5 s), adding latency compared to a pure off-chain system.
- Local development requires running a Soroban-enabled Stellar node (via Docker).

### Neutral
- The backend becomes an indexer and read cache rather than the authoritative store, which changes the mental model for backend developers.
- Cross-contract calls (rewards → campaign, rewards → token) add complexity but are necessary for composability.

---

## Alternatives Considered

| Option | Reason not chosen |
|--------|-------------------|
| Off-chain backend as source of truth | Cannot provide trustless guarantees; double-claim prevention relies on database integrity and backend honesty. |
| Ethereum / EVM chain | Team has no EVM infrastructure; Stellar fees are significantly lower; existing Stellar payment rails are reused. |
| Stellar Classic (non-Soroban) operations | Stellar Classic has no general-purpose programmability; complex claim logic cannot be expressed as native operations. |

## References

- [Soroban documentation](https://developers.stellar.org/docs/build/smart-contracts/overview)
- [Stellar Protocol 20 announcement](https://stellar.org/blog/developers/protocol-20-is-live-soroban-is-here)
- `contracts/rewards/src/lib.rs` — reentrancy guard implementation
- `contracts/campaign/src/lib.rs` — campaign lifecycle
