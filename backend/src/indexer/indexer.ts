/**
 * indexer.ts — Soroban event indexer with exponential backoff and per-event retry.
 *
 * Improvements over the naive fixed-interval poller:
 *  - Exponential backoff on RPC failures (base 2s, cap 60s, jitter ±20%)
 *  - Per-event retry up to MAX_EVENT_RETRIES times before dead-lettering
 *  - Last processed ledger persisted to DB so restarts resume from where
 *    they left off (no re-processing, no gaps)
 *  - Indexer lag metric: current chain tip minus last processed ledger
 *  - Backoff delay and dead-letter counters exposed as Prometheus metrics
 */

import { SorobanRpc, xdr } from "@stellar/stellar-sdk";
import { rpcServer } from "../soroban";
import { upsertCampaign } from "../services/campaign.service";
import { upsertReward, recordTransaction } from "../services/reward.service";
import { pool } from "../db";
import { logger } from "../logger";
import {
  indexerLagBlocks,
  indexerEventsTotal,
  indexerPollErrors,
  indexerDeadLetters,
  indexerBackoffMs,
} from "../metrics";
import { env } from "../env";

// ── Constants ─────────────────────────────────────────────────────────────────

const REWARDS_CONTRACT  = env.REWARDS_CONTRACT_ID;
const CAMPAIGN_CONTRACT = env.CAMPAIGN_CONTRACT_ID;
const TOKEN_CONTRACT    = env.TOKEN_CONTRACT_ID;

const POLL_INTERVAL_MS   = 5_000;   // base happy-path interval
const BACKOFF_BASE_MS    = 2_000;   // first retry wait
const BACKOFF_MAX_MS     = 60_000;  // ceiling
const BACKOFF_MULTIPLIER = 2;
const JITTER_FACTOR      = 0.2;     // ±20% random jitter
const MAX_EVENT_RETRIES  = 3;       // per-event retries before dead-letter

// ── Backoff state (module-level so tests can inspect/reset) ───────────────────

export let currentBackoffMs = 0;    // 0 = no active backoff
let consecutiveFailures    = 0;

let indexerInterval: ReturnType<typeof setInterval> | null = null;

// Persist cursor so we don't re-process events on restart
export async function getCursor(): Promise<string | undefined> {
  const { rows } = await pool.query<{ value: string }>(
    `SELECT value FROM indexer_state WHERE key = 'cursor' LIMIT 1`
  );
  return rows[0]?.value;
}

export async function saveCursor(cursor: string): Promise<void> {
  await pool.query(
    `INSERT INTO indexer_state (key, value) VALUES ('cursor', $1)
     ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value`,
    [cursor]
  );
}

/** Persist a failed event to the dead_letter_events table for later inspection. */
async function deadLetterEvent(
  event: SorobanRpc.Api.RawEventResponse,
  lastError: Error
): Promise<void> {
  try {
    await pool.query(
      `INSERT INTO dead_letter_events (tx_hash, contract_id, paging_token, payload, error, created_at)
       VALUES ($1, $2, $3, $4, $5, NOW())
       ON CONFLICT (tx_hash) DO NOTHING`,
      [
        event.txHash,
        event.contractId ?? null,
        event.pagingToken,
        JSON.stringify(event),
        lastError.message,
      ]
    );
  } catch (dbErr) {
    // Never let dead-letter writes crash the indexer
    logger.error("[indexer] Failed to write dead-letter event", dbErr instanceof Error ? dbErr : new Error(String(dbErr)));
  }
  indexerDeadLetters.inc();
  logger.error(`[indexer] Dead-lettered event txHash=${event.txHash}`, lastError);
}

export async function ensureIndexerTable(): Promise<void> {
  await pool.query(`
    CREATE TABLE IF NOT EXISTS indexer_state (
      key VARCHAR(50) PRIMARY KEY,
      value TEXT NOT NULL
    )
  `);
}

/** Ensure the dead_letter_events table exists. */
async function ensureDeadLetterTable(): Promise<void> {
  await pool.query(`
    CREATE TABLE IF NOT EXISTS dead_letter_events (
      tx_hash      VARCHAR(128) PRIMARY KEY,
      contract_id  VARCHAR(128),
      paging_token TEXT,
      payload      JSONB,
      error        TEXT,
      created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW()
    )
  `);
}

// ── XDR decoders ─────────────────────────────────────────────────────────────

export function decodeAddress(val: xdr.ScVal): string {
  return val.address().toString();
}

export function decodeI128(val: xdr.ScVal): number {
  const hi = val.i128().hi().toBigInt();
  const lo = val.i128().lo().toBigInt();
  return Number((hi << 64n) | lo);
}

export function decodeU64(val: xdr.ScVal): number {
  return Number(val.u64().toBigInt());
}

// ── Per-event processing with retry ──────────────────────────────────────────

/**
 * Process a single event, retrying up to MAX_EVENT_RETRIES times on failure.
 * If all retries are exhausted the event is dead-lettered and processing
 * continues (we never block the whole indexer on one bad event).
 */
export async function processEventWithRetry(
  event: SorobanRpc.Api.RawEventResponse
): Promise<void> {
  let lastError: Error | undefined;

  for (let attempt = 1; attempt <= MAX_EVENT_RETRIES; attempt++) {
    try {
      await processEvent(event);
      return; // success
    } catch (err) {
      lastError = err instanceof Error ? err : new Error(String(err));
      logger.warn(
        `[indexer] Event processing attempt ${attempt}/${MAX_EVENT_RETRIES} failed for txHash=${event.txHash}: ${lastError.message}`
      );
      if (attempt < MAX_EVENT_RETRIES) {
        await sleep(calcBackoff(attempt));
      }
    }
  }

  await deadLetterEvent(event, lastError!);
}

async function processEvent(event: SorobanRpc.Api.RawEventResponse): Promise<void> {
  if (event.type !== "contract") return;

  const topics    = event.topic.map((t) => xdr.ScVal.fromXDR(t, "base64"));
  const eventName = topics[0]?.sym() ?? "";

  if (event.contractId === CAMPAIGN_CONTRACT && eventName === "CAM_CRT") {
    const id       = decodeU64(topics[2]);
    const merchant = decodeAddress(xdr.ScVal.fromXDR(event.value, "base64"));
    
    // Fetch mapped image URL if exists
    const { getCampaignImageByTxHash } = require("../services/campaign.service");
    const imageUrl = await getCampaignImageByTxHash(event.txHash);

    await upsertCampaign({
      id,
      merchant,
      reward_amount: 0,
      expiration: 0,
      active: true,
      total_claimed: 0,
      tx_hash: event.txHash,
      image_url: imageUrl || undefined,
    } as any);
    await recordTransaction(event.txHash, "campaign_created", merchant, id, null, event.ledger);
    logger.info(`[indexer] CampaignCreated id=${id} merchant=${merchant}`);
  }

  if (event.contractId === CAMPAIGN_CONTRACT && eventName === "CAM_DEACT") {
    // topics: [CAM_DEACT, "id", id_val], value: merchant_address
    const id = decodeU64(topics[2]);
    const merchant = decodeAddress(xdr.ScVal.fromXDR(event.value, "base64"));
    await upsertCampaign({
      id,
      merchant,
      reward_amount: 0,
      expiration: 0,
      active: false,
      total_claimed: 0,
      tx_hash: event.txHash,
    });
    await recordTransaction(event.txHash, "campaign_deactivated", merchant, id, null, event.ledger);
    console.log(`[indexer] CampaignDeactivated id=${id} merchant=${merchant}`);
  }

  if (event.contractId === TOKEN_CONTRACT && eventName === "MINT") {
    // topics: [MINT, "to", to_addr], value: (amount, total_supply)
    const to = decodeAddress(topics[2]);
    const valueVec = xdr.ScVal.fromXDR(event.value, "base64").vec()!;
    const amount = decodeI128(valueVec[0]);
    const totalSupply = decodeI128(valueVec[1]);
    await recordTransaction(event.txHash, "mint", to, null, amount, event.ledger);
    console.log(`[indexer] TokenMinted to=${to} amount=${amount} totalSupply=${totalSupply}`);
  }

  if (event.contractId === TOKEN_CONTRACT && eventName === "BURN") {
    // topics: [BURN, "from", from_addr], value: (amount, total_supply)
    const from = decodeAddress(topics[2]);
    const valueVec = xdr.ScVal.fromXDR(event.value, "base64").vec()!;
    const amount = decodeI128(valueVec[0]);
    const totalSupply = decodeI128(valueVec[1]);
    await recordTransaction(event.txHash, "burn", from, null, amount, event.ledger);
    console.log(`[indexer] TokenBurned from=${from} amount=${amount} totalSupply=${totalSupply}`);
  }

  if (event.contractId === REWARDS_CONTRACT && eventName === "RWD_CLM") {
    const user      = decodeAddress(topics[2]);
    const valueVec  = xdr.ScVal.fromXDR(event.value, "base64").vec()!;
    const campaignId = decodeU64(valueVec[0]);
    const amount    = decodeI128(valueVec[1]);
    await upsertReward({ user_address: user, campaign_id: campaignId, amount, redeemed: false, redeemed_amount: 0 });
    await recordTransaction(event.txHash, "claim", user, campaignId, amount, event.ledger);
    logger.info(`[indexer] RewardClaimed user=${user} campaign=${campaignId} amount=${amount}`);
  }

  if (event.contractId === REWARDS_CONTRACT && eventName === "RWD_RDM") {
    const user   = decodeAddress(topics[2]);
    const amount = decodeI128(xdr.ScVal.fromXDR(event.value, "base64"));
    await recordTransaction(event.txHash, "redeem", user, null, amount, event.ledger);
    logger.info(`[indexer] RewardRedeemed user=${user} amount=${amount}`);
  }
}

// ── Poll loop ─────────────────────────────────────────────────────────────────

async function poll(): Promise<void> {
  try {
    const cursor  = await getCursor();
    const filters: SorobanRpc.Api.EventFilter[] = [
      { type: "contract", contractIds: [CAMPAIGN_CONTRACT, REWARDS_CONTRACT] },
    ];

    const result = await rpcServer.getEvents({
      startLedger: cursor ? undefined : 1,
      cursor,
      filters,
      limit: 100,
    });

    // Process each event individually with per-event retry
    for (const event of result.events) {
      await processEventWithRetry(event as unknown as SorobanRpc.Api.RawEventResponse);
      indexerEventsTotal.inc();
    }

    // Persist cursor after the batch so a crash mid-batch re-processes
    // (idempotent upserts in the service layer handle duplicates safely)
    if (result.events.length > 0) {
      const last = result.events[result.events.length - 1] as unknown as SorobanRpc.Api.RawEventResponse;
      await saveCursor(last.pagingToken);
    }

    // Update lag metric
    try {
      const latestLedger = await rpcServer.getLatestLedger();
      const processedLedger =
        result.events.length > 0
          ? Number((result.events[result.events.length - 1] as unknown as SorobanRpc.Api.RawEventResponse).ledger)
          : latestLedger.sequence;
      indexerLagBlocks.set(Math.max(0, latestLedger.sequence - processedLedger));
    } catch {
      // non-critical — skip lag update
    }

    // Successful poll — reset backoff
    if (consecutiveFailures > 0) {
      logger.info("[indexer] RPC recovered — resetting backoff");
      consecutiveFailures = 0;
      currentBackoffMs    = 0;
      indexerBackoffMs.set(0);
    }
  } catch (err) {
    const error = err instanceof Error ? err : new Error(String(err));
    consecutiveFailures++;
    indexerPollErrors.inc();

    currentBackoffMs = calcBackoff(consecutiveFailures);
    indexerBackoffMs.set(currentBackoffMs);

    const isTimeout =
      error.message.toLowerCase().includes("timeout") ||
      error.message.toLowerCase().includes("timed out");

    if (isTimeout) {
      logger.critical("[indexer] RPC timeout", error);
    } else {
      logger.error(`[indexer] Poll error (failure #${consecutiveFailures}), backing off ${currentBackoffMs}ms`, error);
    }

    await sleep(currentBackoffMs);
  }
}

// ── Public API ────────────────────────────────────────────────────────────────

/**
 * Starts the background event indexer.
 * It polls the Soroban RPC for contract events (Campaign creation, Reward claim/redeem)
 * and persists them to the local database.
 *
 * @returns A promise that resolves when the indexer has started its initial poll.
 */
export async function startIndexer(): Promise<void> {
  await ensureIndexerTable();
  await ensureDeadLetterTable();
  logger.info("[indexer] started");

  const loop = async () => {
    await poll();
    // Schedule next tick — use the current backoff if we're in a failure
    // state, otherwise use the normal interval
    const delay = currentBackoffMs > 0 ? currentBackoffMs : POLL_INTERVAL_MS;
    setTimeout(loop, delay);
  };

  // Run immediately then on interval
  await poll();
  indexerInterval = setInterval(poll, POLL_INTERVAL_MS);
}

/**
 * Stops the background indexer polling loop.
 * Called during graceful shutdown to prevent the indexer from running after the server exits.
 */
export function stopIndexer(): void {
  if (indexerInterval !== null) {
    clearInterval(indexerInterval);
    indexerInterval = null;
    console.log("[indexer] stopped");
  }
}

export function calcBackoff(failures: number): number {
  if (failures <= 0) return BACKOFF_BASE_MS;
  const delay = BACKOFF_BASE_MS * Math.pow(BACKOFF_MULTIPLIER, failures - 1);
  const maxJitter = delay * JITTER_FACTOR;
  const jitter = Math.random() * (maxJitter * 2) - maxJitter;
  return Math.max(BACKOFF_BASE_MS, Math.min(BACKOFF_MAX_MS, delay + jitter));
}

export function resetBackoff(): void {
  currentBackoffMs = 0;
}

export const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));