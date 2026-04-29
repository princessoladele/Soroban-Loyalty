import { pool } from "../db";
import { PoolClient } from "pg";

export type AuditAction =
  | "campaign.create"
  | "campaign.deactivate"
  | "reward.claim"
  | "reward.redeem";

export interface AuditLogEntry {
  id: string;
  actor: string;
  action: AuditAction;
  entity_type: string;
  entity_id: string;
  metadata: Record<string, unknown>;
  created_at: Date;
}

/**
 * Writes an audit log entry.
 * Pass a PoolClient to write within an existing transaction (recommended).
 * Omit client to use the pool directly (auto-commit).
 */
export async function writeAuditLog(
  entry: Omit<AuditLogEntry, "id" | "created_at">,
  client?: PoolClient
): Promise<void> {
  const db = client ?? pool;
  await db.query(
    `INSERT INTO audit_logs (actor, action, entity_type, entity_id, metadata)
     VALUES ($1, $2, $3, $4, $5)`,
    [entry.actor, entry.action, entry.entity_type, entry.entity_id, JSON.stringify(entry.metadata)]
  );
}

export interface AuditLogFilters {
  actor?: string;
  action?: AuditAction;
  entity_type?: string;
  entity_id?: string;
  since?: Date;
  until?: Date;
  limit?: number;
  offset?: number;
}

export async function queryAuditLogs(
  filters: AuditLogFilters = {}
): Promise<{ logs: AuditLogEntry[]; total: number }> {
  const conditions: string[] = [];
  const params: unknown[] = [];

  if (filters.actor) { params.push(filters.actor); conditions.push(`actor = $${params.length}`); }
  if (filters.action) { params.push(filters.action); conditions.push(`action = $${params.length}`); }
  if (filters.entity_type) { params.push(filters.entity_type); conditions.push(`entity_type = $${params.length}`); }
  if (filters.entity_id) { params.push(filters.entity_id); conditions.push(`entity_id = $${params.length}`); }
  if (filters.since) { params.push(filters.since); conditions.push(`created_at >= $${params.length}`); }
  if (filters.until) { params.push(filters.until); conditions.push(`created_at <= $${params.length}`); }

  const where = conditions.length > 0 ? `WHERE ${conditions.join(" AND ")}` : "";

  const { rows: countRows } = await pool.query<{ count: string }>(
    `SELECT COUNT(*) FROM audit_logs ${where}`, params
  );

  const limit = filters.limit ?? 50;
  const offset = filters.offset ?? 0;
  params.push(limit, offset);

  const { rows } = await pool.query<AuditLogEntry>(
    `SELECT * FROM audit_logs ${where} ORDER BY created_at DESC LIMIT $${params.length - 1} OFFSET $${params.length}`,
    params
  );

  return { logs: rows, total: parseInt(countRows[0].count, 10) };
}
