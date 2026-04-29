# ADR-0003: PostgreSQL as Off-Chain Read Store

- **Status:** Accepted
- **Date:** 2026-04-26
- **Authors:** Platform Team

---

## Context

Soroban contracts are the authoritative source of truth for all value operations, but querying the chain directly for every read request is impractical: the Soroban RPC does not support arbitrary queries (e.g., "all campaigns for merchant X"), responses are slow relative to a local database, and pagination/filtering must be implemented client-side.

The system needs a queryable, low-latency read store that mirrors on-chain state. The backend must serve paginated campaign lists, per-user reward histories, and analytics aggregates — none of which are efficiently expressible as RPC calls.

Key constraints:
- The team has strong PostgreSQL experience.
- The data model is relational: rewards reference campaigns, campaigns reference merchants.
- The read store is derived data; it can be rebuilt from on-chain events if lost.
- Production runs on Kubernetes with managed PostgreSQL (AWS RDS or equivalent).

## Decision

We will use PostgreSQL as the off-chain read store. The schema mirrors on-chain state with four tables:

- `users` — indexed Stellar addresses (created on first claim).
- `campaigns` — one row per on-chain campaign, keyed by the contract-assigned `BIGINT` ID.
- `rewards` — one row per (user, campaign) pair with claim and redemption state.
- `transactions` — append-only ledger of indexed events (`claim`, `redeem`, `campaign_created`).

Foreign keys enforce referential integrity between rewards → users and rewards → campaigns. A `UNIQUE (user_address, campaign_id)` constraint on `rewards` mirrors the on-chain double-claim guard at the database level. Indexes are placed on the most common query patterns: rewards by user, rewards by campaign, transactions by user, campaigns by merchant.

Schema changes are managed via `node-pg-migrate` versioned migrations (see ADR-0003 consequences and `backend/migrations/`).

## Consequences

### Positive
- Sub-millisecond query latency for all read endpoints.
- Full SQL expressiveness for analytics aggregates, pagination, and joins.
- The `transactions` table provides a queryable audit log independent of RPC event history windows.
- `UNIQUE (user_address, campaign_id)` provides a database-level safety net against indexer bugs that could otherwise insert duplicate rewards.
- Migrations give a versioned, reviewable history of schema evolution.

### Negative
- The read store is eventually consistent with the chain; there is a lag of up to one poll interval (~5 s) between an on-chain event and its appearance in the API.
- The backend must handle re-indexing if the database is lost or corrupted (no automated replay tooling exists yet).
- `BIGINT` campaign IDs are sourced from the contract's auto-increment counter; if the contract is redeployed, IDs restart from 1 and could collide with existing rows.
- `VARCHAR(56)` for Stellar addresses is correct for Ed25519 public keys in Stellar's base32 encoding but would need to change if the address format ever changes.

### Neutral
- Using `uuid-ossp` for UUID primary keys on `rewards` and `transactions` avoids exposing sequential IDs externally but requires the extension to be available on the PostgreSQL instance.
- `TIMESTAMPTZ` is used throughout to avoid timezone ambiguity; all application code must treat timestamps as UTC.

---

## Alternatives Considered

| Option | Reason not chosen |
|--------|-------------------|
| Query Soroban RPC directly on every request | No support for arbitrary queries; high latency; no pagination or aggregation. |
| Redis as read cache | Lacks relational query capability needed for analytics; adds operational complexity without eliminating the need for a persistent store. |
| SQLite | Not suitable for multi-instance production deployments; no managed cloud offering. |
| MongoDB | No relational integrity; team has stronger PostgreSQL expertise; joins are needed for reward-campaign queries. |

## References

- `database/schema.sql` — canonical schema definition
- `backend/migrations/` — versioned migration history
- `backend/src/services/campaign.service.ts` — query patterns
- `backend/src/services/reward.service.ts` — upsert patterns
