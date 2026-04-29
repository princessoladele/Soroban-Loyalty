# Architecture Decision Records

This directory contains Architecture Decision Records (ADRs) for SorobanLoyalty. ADRs document significant architectural choices — the context that led to them, the decision made, and the consequences.

## Index

| ADR | Title | Status |
|-----|-------|--------|
| [ADR-0001](0001-adr-template.md) | ADR Template | Meta |
| [ADR-0002](0002-soroban-smart-contracts.md) | Use Soroban for On-Chain Logic | Accepted |
| [ADR-0003](0003-postgresql-schema-design.md) | PostgreSQL as Off-Chain Read Store | Accepted |
| [ADR-0004](0004-indexer-polling-design.md) | Event Indexer via RPC Polling | Accepted |
| [ADR-0005](0005-authentication-approach.md) | Wallet-Signed Auth, No Backend JWT | Accepted |

## Creating a New ADR

1. Copy `0001-adr-template.md` to a new file: `NNNN-short-title.md` (increment the number).
2. Fill in all sections. Leave no section blank — write "N/A" if genuinely not applicable.
3. Set status to `Proposed` and open a PR for team review.
4. Once merged, update the index table above.
5. If a later ADR supersedes this one, update the status to `Superseded by ADR-NNNN`.

## Statuses

- **Proposed** — under discussion, not yet adopted
- **Accepted** — agreed and in effect
- **Deprecated** — no longer recommended but not yet replaced
- **Superseded** — replaced by a newer ADR (link to it)
