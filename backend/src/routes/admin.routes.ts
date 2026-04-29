import { Router, Request, Response } from "express";
import { queryAuditLogs, AuditAction } from "../services/audit.service";
import { asyncHandler } from "../middleware/errorHandler";

export const adminRouter = Router();

/**
 * GET /admin/audit-logs
 * Query params: actor, action, entity_type, entity_id, since, until, limit, offset
 */
adminRouter.get("/audit-logs", asyncHandler(async (req: Request, res: Response) => {
  const limit = Math.min(parseInt(String(req.query.limit ?? "50"), 10) || 50, 200);
  const offset = parseInt(String(req.query.offset ?? "0"), 10) || 0;

  const result = await queryAuditLogs({
    actor: req.query.actor ? String(req.query.actor) : undefined,
    action: req.query.action ? String(req.query.action) as AuditAction : undefined,
    entity_type: req.query.entity_type ? String(req.query.entity_type) : undefined,
    entity_id: req.query.entity_id ? String(req.query.entity_id) : undefined,
    since: req.query.since ? new Date(String(req.query.since)) : undefined,
    until: req.query.until ? new Date(String(req.query.until)) : undefined,
    limit,
    offset,
  });

  res.json(result);
}));
