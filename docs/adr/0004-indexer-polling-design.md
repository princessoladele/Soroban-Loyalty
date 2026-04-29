# ADR-0004: Event Indexer via RPC Polling

- **Status:** Accepted
- **Date:** 2026-04-26
- **Authors:** Platform Team

---

## Context

The PostgreSQL read store must be kept in sync with on-chain state. Soroban contracts emit typed events (`CAM_CRT`, `RWD_CLM`, `RWD_RDM`) that encode all state transitions. The backend needs a reliable mechanism to consume these events and persist them to the database.

Soroban RPC exposes a `getEvents` endpoint that returns contract events filtered by contract ID, with cursor-based pagination. There is no native push/webhook mechanism from the Stellar network to an off-chain service.

Key constraints:
- The indexer must survive restarts without re-processing events (cursor persistence).
- The indexer runs in the same process as the Express server to keep the deployment footprint small (single container).
- Event ordering must be preserved; the indexer processes events sequentially within each poll.
- The team does not want to operate a separate message broker (Kafka, RabbitMQ) at this stage.

## Decision

We will implement a polling-based event indexer that runs as a background `setInterval` loop within the Express server process. On each tick (every 5 seconds):

1. Read the last processed event cursor from the `indexer_state` table in PostgreSQL.
2. Call `rpcServer.getEvents()` with the cursor and a filter for both contract IDs.
3. Process each event sequentially: decode XDR topics/values, upsert the relevant database rows, and record a transaction entry.
4. After processing all events in the batch, persist the paging token of the last event as the new cursor.

The cursor is stored in a dedicated `indexer_state` key-value table, ensuring the indexer resumes from the correct position after a restart. On first start (no cursor), indexing begins from ledger 1.

A Prometheus gauge (`indexer_lag_blocks`) tracks the difference between the latest chain ledger and the last indexed ledger, enabling alerting on indexer lag.

## Consequences

### Positive
- Simple to operate: no additional infrastructure (no Kafka, no separate worker process).
- Cursor persistence means restarts are safe and do not cause duplicate processing.
- The 5-second poll interval is well within acceptable eventual-consistency bounds for the use case.
- Sequential event processing within a poll guarantees ordering; no out-of-order writes.
- The lag metric enables proactive alerting before users notice stale data.

### Negative
- Polling introduces up to 5 seconds of lag between an on-chain event and its appearance in the API. This is visible to users immediately after submitting a transaction.
- The indexer shares the Express process; a crash loop in the indexer could affect API availability (mitigated by catching errors per-poll and continuing).
- If the RPC node is unavailable for an extended period, the cursor falls behind and a large backlog of events must be processed on recovery, temporarily increasing DB write load.
- There is no dead-letter queue for events that fail to process; a persistent decode error would stall the cursor at that event (mitigated by logging and manual intervention).
- Running in-process means the indexer cannot be scaled independently of the API.

### Neutral
- The `indexer_state` table is created with `CREATE TABLE IF NOT EXISTS` at startup, so it does not require a separate migration for initial deployment.
- The poll interval (5 s) is hardcoded as `POLL_INTERVAL_MS`; it can be made configurable via environment variable if needed.

---

## Alternatives Considered

| Option | Reason not chosen |
|--------|-------------------|
| Webhook / push from Stellar node | Stellar network has no native push mechanism to off-chain services. |
| Separate indexer process / worker | Increases operational complexity; not justified at current scale. |
| Kafka + consumer | Significant infrastructure overhead; no existing Kafka expertise on the team. |
| Horizon API (Stellar's REST layer) | Horizon does not expose Soroban contract events; only classic Stellar operations. |
| Streaming via `getEvents` long-poll | Not supported by the Soroban RPC; polling is the documented approach. |

## References

- `backend/src/indexer/indexer.ts` — full indexer implementation
- `backend/src/metrics.ts` — `indexer_lag_blocks` and `indexer_events_total` metrics
- [Soroban RPC `getEvents` reference](https://developers.stellar.org/docs/data/rpc/api-reference/methods/getEvents)
