#!/usr/bin/env bash
# backup-postgres.sh — pg_dump → gzip → S3
# Required env vars:
#   PGHOST, PGPORT, PGUSER, PGPASSWORD, PGDATABASE
#   S3_BACKUP_BUCKET
#   SLACK_WEBHOOK_URL  (optional — skip alert if unset)
set -euo pipefail

TIMESTAMP=$(date -u +"%Y%m%dT%H%M%SZ")
FILENAME="backup-${TIMESTAMP}.sql.gz"
S3_KEY="postgres/${FILENAME}"
TMP_FILE="/tmp/${FILENAME}"

alert_slack() {
  local msg="$1"
  [[ -z "${SLACK_WEBHOOK_URL:-}" ]] && return 0
  curl -s -X POST "${SLACK_WEBHOOK_URL}" \
    -H "Content-Type: application/json" \
    -d "{\"text\":\"${msg}\"}" || true
}

cleanup() { rm -f "${TMP_FILE}"; }
trap cleanup EXIT

on_error() {
  alert_slack ":x: *[soroban-loyalty]* PostgreSQL backup FAILED at $(date -u +%Y-%m-%dT%H:%M:%SZ). Check pod logs for details."
  exit 1
}
trap on_error ERR

echo "[backup] Starting pg_dump of ${PGDATABASE} at ${TIMESTAMP}"
pg_dump \
  --host="${PGHOST}" \
  --port="${PGPORT:-5432}" \
  --username="${PGUSER}" \
  --no-password \
  --format=plain \
  "${PGDATABASE}" \
  | gzip -9 > "${TMP_FILE}"

echo "[backup] Uploading ${FILENAME} to s3://${S3_BACKUP_BUCKET}/${S3_KEY}"
aws s3 cp "${TMP_FILE}" "s3://${S3_BACKUP_BUCKET}/${S3_KEY}" \
  --sse AES256 \
  --no-progress

echo "[backup] Done — s3://${S3_BACKUP_BUCKET}/${S3_KEY}"
alert_slack ":white_check_mark: *[soroban-loyalty]* PostgreSQL backup succeeded: \`${S3_KEY}\`"
