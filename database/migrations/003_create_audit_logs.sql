-- Migration: create audit_logs table
-- Append-only: no UPDATE or DELETE is permitted on this table.
-- Enforced via a trigger that raises an exception on any attempt.

CREATE TABLE IF NOT EXISTS audit_logs (
    id          UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    actor       VARCHAR(56) NOT NULL,          -- Stellar public key of the actor
    action      VARCHAR(64) NOT NULL,           -- e.g. 'campaign.create', 'campaign.deactivate', 'reward.claim', 'reward.redeem'
    entity_type VARCHAR(64) NOT NULL,           -- e.g. 'campaign', 'reward'
    entity_id   VARCHAR(64) NOT NULL,           -- ID of the affected entity
    metadata    JSONB NOT NULL DEFAULT '{}',    -- additional context (amounts, tx_hash, etc.)
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_audit_logs_actor      ON audit_logs(actor);
CREATE INDEX IF NOT EXISTS idx_audit_logs_action     ON audit_logs(action);
CREATE INDEX IF NOT EXISTS idx_audit_logs_entity     ON audit_logs(entity_type, entity_id);
CREATE INDEX IF NOT EXISTS idx_audit_logs_created_at ON audit_logs(created_at DESC);

-- Enforce append-only: block UPDATE and DELETE at the DB level
CREATE OR REPLACE FUNCTION audit_logs_immutable()
RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
  RAISE EXCEPTION 'audit_logs is append-only: % is not permitted', TG_OP;
END;
$$;

DROP TRIGGER IF EXISTS trg_audit_logs_no_update ON audit_logs;
CREATE TRIGGER trg_audit_logs_no_update
  BEFORE UPDATE ON audit_logs
  FOR EACH ROW EXECUTE FUNCTION audit_logs_immutable();

DROP TRIGGER IF EXISTS trg_audit_logs_no_delete ON audit_logs;
CREATE TRIGGER trg_audit_logs_no_delete
  BEFORE DELETE ON audit_logs
  FOR EACH ROW EXECUTE FUNCTION audit_logs_immutable();
