import { pool } from "../db";
import { writeAuditLog } from "./audit.service";

export interface Campaign {
  id: number;
  merchant: string;
  name?: string | null;
  reward_amount: number;
  expiration: number;
  active: boolean;
  total_claimed: number;
  display_order: number;
  tx_hash?: string;
  image_url?: string;
  created_at: Date;
  deleted_at?: Date | null;
}

export async function upsertCampaign(c: Omit<Campaign, "created_at" | "display_order" | "deleted_at">): Promise<void> {
  const client = await pool.connect();
  try {
    await client.query("BEGIN");
    await client.query(
      `INSERT INTO campaigns (id, merchant, reward_amount, expiration, active, total_claimed, tx_hash, image_url)
       VALUES ($1,$2,$3,$4,$5,$6,$7, $8)
       ON CONFLICT (id) DO UPDATE SET
         active = EXCLUDED.active,
         total_claimed = EXCLUDED.total_claimed,
         image_url = COALESCE(EXCLUDED.image_url, campaigns.image_url),
         updated_at = NOW()`,
      [c.id, c.merchant, c.reward_amount, c.expiration, c.active, c.total_claimed, c.tx_hash ?? null, (c as any).image_url ?? null]
    );
    await writeAuditLog({
      actor: c.merchant,
      action: "campaign.create",
      entity_type: "campaign",
      entity_id: String(c.id),
      metadata: { reward_amount: c.reward_amount, expiration: c.expiration, tx_hash: c.tx_hash },
    }, client);
    await client.query("COMMIT");
  } catch (err) {
    await client.query("ROLLBACK");
    throw err;
  } finally {
    client.release();
  }
}

/**
 * Temporarily maps a transaction hash to an image URL.
 */
export async function saveCampaignImageMapping(txHash: string, imageUrl: string): Promise<void> {
  await pool.query(
    `INSERT INTO campaign_image_mappings (tx_hash, image_url) VALUES ($1, $2)
     ON CONFLICT (tx_hash) DO UPDATE SET image_url = EXCLUDED.image_url`,
    [txHash, imageUrl]
  );
}

/**
 * Retrieves the image URL for a given transaction hash, if any.
 */
export async function getCampaignImageByTxHash(txHash: string): Promise<string | null> {
  const { rows } = await pool.query<{ image_url: string }>(
    `SELECT image_url FROM campaign_image_mappings WHERE tx_hash = $1`,
    [txHash]
  );
  return rows[0]?.image_url ?? null;
}

export interface CampaignFilters {
  search?: string;
  status?: "active" | "inactive";
  expires_before?: number;
  expires_after?: number;
}

export async function getCampaigns(
  limit = 20,
  offset = 0,
  filters: CampaignFilters = {}
): Promise<{ campaigns: Campaign[]; total: number }> {
  const conditions: string[] = ["deleted_at IS NULL"];
  const params: unknown[] = [];

  if (filters.search) {
    params.push(`%${filters.search}%`);
    conditions.push(`name ILIKE $${params.length}`);
  }
  if (filters.status !== undefined) {
    params.push(filters.status === "active");
    conditions.push(`active = $${params.length}`);
  }
  if (filters.expires_before !== undefined) {
    params.push(filters.expires_before);
    conditions.push(`expiration <= $${params.length}`);
  }
  if (filters.expires_after !== undefined) {
    params.push(filters.expires_after);
    conditions.push(`expiration >= $${params.length}`);
  }

  const where = conditions.join(" AND ");

  const listParams = [...params, limit, offset];
  const { rows } = await pool.query<Campaign>(
    `SELECT * FROM campaigns WHERE ${where} ORDER BY display_order ASC, created_at DESC LIMIT $${listParams.length - 1} OFFSET $${listParams.length}`,
    listParams
  );
  const { rows: countRows } = await pool.query<{ count: string }>(
    `SELECT COUNT(*) FROM campaigns WHERE ${where}`,
    params
  );
  return { campaigns: rows, total: parseInt(countRows[0].count, 10) };
}

export async function getCampaignById(id: number): Promise<Campaign | null> {
  const { rows } = await pool.query<Campaign>(
    `SELECT * FROM campaigns WHERE id = $1 AND deleted_at IS NULL`,
    [id]
  );
  return rows[0] ?? null;
}

export async function softDeleteCampaign(id: number, actor?: string): Promise<boolean> {
  const client = await pool.connect();
  try {
    await client.query("BEGIN");
    const { rowCount } = await client.query(
      `UPDATE campaigns SET deleted_at = NOW() WHERE id = $1 AND deleted_at IS NULL`,
      [id]
    );
    if ((rowCount ?? 0) > 0 && actor) {
      await writeAuditLog({
        actor,
        action: "campaign.deactivate",
        entity_type: "campaign",
        entity_id: String(id),
        metadata: {},
      }, client);
    }
    await client.query("COMMIT");
    return (rowCount ?? 0) > 0;
  } catch (err) {
    await client.query("ROLLBACK");
    throw err;
  } finally {
    client.release();
  }
}

export async function restoreCampaign(id: number): Promise<boolean> {
  const { rowCount } = await pool.query(
    `UPDATE campaigns SET deleted_at = NULL WHERE id = $1 AND deleted_at IS NOT NULL`,
    [id]
  );
  return (rowCount ?? 0) > 0;
}

/**
 * Persists the display order of campaigns.
 * @param order - Array of campaign IDs in the desired display order.
 */
export async function reorderCampaigns(order: number[]): Promise<void> {
  const client = await pool.connect();
  try {
    await client.query("BEGIN");
    for (let i = 0; i < order.length; i++) {
      await client.query(
        `UPDATE campaigns SET display_order = $1 WHERE id = $2`,
        [i, order[i]]
      );
    }
    await client.query("COMMIT");
  } catch (err) {
    await client.query("ROLLBACK");
    throw err;
  } finally {
    client.release();
  }
}
