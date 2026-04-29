# ADR-0005: Wallet-Signed Auth, No Backend JWT

- **Status:** Accepted
- **Date:** 2026-04-26
- **Authors:** Platform Team

---

## Context

SorobanLoyalty needs to answer two authentication questions:

1. **Write operations** (claim reward, redeem, create campaign): who is authorised to submit a transaction that mutates on-chain state?
2. **Read operations** (fetch rewards for a user, list campaigns): does the backend API need to verify the caller's identity?

Traditional web applications answer both with a session token or JWT issued after a username/password login. However, this system's users are identified by their Stellar public key, not a username. All write operations are Soroban transactions that must be signed by the user's private key — a key that never leaves the user's Freighter wallet.

Key constraints:
- Private keys must never be sent to or stored by the backend.
- The Freighter browser extension is the only supported wallet at launch.
- The backend API currently exposes only read endpoints (indexed on-chain data).
- Adding a separate identity layer (OAuth, Auth0, custom JWT service) would require users to link a wallet to an account — adding friction with no clear benefit at this stage.

## Decision

We will use Soroban's native `require_auth()` as the sole authentication mechanism for all write operations. The frontend builds a Soroban transaction, simulates it via the RPC, presents it to Freighter for signing, and submits the signed XDR directly to the Soroban RPC. The backend never participates in write flows.

The backend API requires no authentication. All endpoints are read-only and return public on-chain data. The user's Stellar address is passed as a URL path parameter (e.g., `GET /user/:address/rewards`) and is treated as a public identifier, not a secret.

This means:
- No JWT issuance, validation, or refresh logic in the backend.
- No session storage.
- No private key handling anywhere in the stack.

## Consequences

### Positive
- Zero private key exposure risk: keys never leave the user's Freighter extension.
- Authentication correctness is enforced by the Soroban VM on every validator — it cannot be bypassed by a compromised backend.
- The backend is stateless with respect to auth, simplifying horizontal scaling and deployment.
- No token expiry, rotation, or revocation logic to maintain.
- New users can interact immediately without a registration step.

### Negative
- The backend API is fully public; any caller can query any user's reward history by knowing their Stellar address. Stellar addresses are pseudonymous but not private — this is an accepted trade-off for a public blockchain application.
- If the backend ever needs to expose write endpoints (e.g., admin operations, off-chain metadata), a separate auth mechanism will need to be retrofitted.
- There is no rate-limiting per authenticated user; abuse prevention relies on IP-based rate limiting or API gateway controls.
- Freighter is the only supported wallet; users without it cannot interact with the platform.

### Neutral
- `require_auth()` in Soroban validates that the transaction was signed by the address passed as the `user` or `merchant` argument. This is equivalent to "the caller owns this address" without any additional challenge-response.
- The frontend stores the connected public key in React state (`WalletContext`); it is not persisted to `localStorage` by default, so users must reconnect after a page refresh.

---

## Alternatives Considered

| Option | Reason not chosen |
|--------|-------------------|
| JWT issued after wallet signature challenge | Adds complexity (challenge generation, JWT signing keys, token storage) with no benefit since the backend has no write endpoints to protect. |
| OAuth2 / Auth0 with wallet linking | Requires users to create an account and link their wallet; significant friction for a crypto-native audience. |
| API keys for backend access | Appropriate for machine-to-machine access; not suitable for end-user browser flows. |
| Session cookies | Requires CSRF protection and sticky sessions; incompatible with stateless horizontal scaling. |

## References

- `backend/docs/api-reference.md` — "Authentication" section confirming no auth required
- `frontend/src/lib/freighter.ts` — wallet connection and transaction signing
- `frontend/src/context/WalletContext.tsx` — public key state management
- `contracts/rewards/src/lib.rs` — `user.require_auth()` in `claim_reward` and `redeem_reward`
- `contracts/campaign/src/lib.rs` — `merchant.require_auth()` in `create_campaign`
- [Soroban auth documentation](https://developers.stellar.org/docs/build/smart-contracts/example-contracts/auth)
