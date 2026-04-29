# PostgreSQL Backup & Restoration Runbook

**Audience:** On-call engineers  
**Last updated:** 2026-04-27

---

## Backup overview

| Property | Value |
|---|---|
| Schedule | Daily at **02:00 UTC** (Kubernetes CronJob) |
| Tool | `pg_dump --format=plain` piped through `gzip -9` |
| Storage | S3 — `soroban-loyalty-<env>-db-backups` |
| Path pattern | `postgres/backup-<YYYYMMDDTHHMMSSZ>.sql.gz` |
| Encryption | AES256 server-side (S3 SSE) |
| Retention | 30 days (S3 lifecycle rule auto-expires) |
| Alerting | Slack `#alerts` on failure; success message on completion |

---

## Verify a backup ran

```bash
# List the last 5 backups
aws s3 ls s3://${S3_BACKUP_BUCKET}/postgres/ \
  --recursive --human-readable \
  | sort | tail -5
```

Check the CronJob history in Kubernetes:
```bash
kubectl get jobs -n soroban-loyalty -l app=postgres-backup
kubectl logs -n soroban-loyalty job/<job-name>
```

---

## Restoration procedure

### Prerequisites
- `psql` and `aws` CLI installed
- Access to the S3 backup bucket
- Target PostgreSQL instance reachable

### Step 1 — Identify the backup to restore

```bash
aws s3 ls s3://${S3_BACKUP_BUCKET}/postgres/ --recursive | sort
```

Pick the desired `backup-<TIMESTAMP>.sql.gz` key.

### Step 2 — Download and decompress

```bash
BACKUP_KEY="postgres/backup-<TIMESTAMP>.sql.gz"
aws s3 cp "s3://${S3_BACKUP_BUCKET}/${BACKUP_KEY}" /tmp/restore.sql.gz
gunzip /tmp/restore.sql.gz
# Result: /tmp/restore.sql
```

### Step 3 — Stop application traffic (optional but recommended)

Scale down the backend to prevent writes during restore:
```bash
kubectl scale deployment backend -n soroban-loyalty --replicas=0
```

### Step 4 — Drop and recreate the target database

```bash
psql -h $PGHOST -U $PGUSER -d postgres \
  -c "SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname='soroban_loyalty';"
psql -h $PGHOST -U $PGUSER -d postgres \
  -c "DROP DATABASE IF EXISTS soroban_loyalty;"
psql -h $PGHOST -U $PGUSER -d postgres \
  -c "CREATE DATABASE soroban_loyalty OWNER loyalty;"
```

### Step 5 — Restore

```bash
psql -h $PGHOST -U $PGUSER -d soroban_loyalty < /tmp/restore.sql
```

### Step 6 — Verify

```bash
psql -h $PGHOST -U $PGUSER -d soroban_loyalty \
  -c "\dt" \
  -c "SELECT COUNT(*) FROM campaigns;" \
  -c "SELECT COUNT(*) FROM rewards;"
```

### Step 7 — Resume traffic

```bash
kubectl scale deployment backend -n soroban-loyalty --replicas=2
rm /tmp/restore.sql
```

---

## Monthly restoration test

Run this on the first Monday of each month against the **staging** environment:

1. Identify the latest production backup.
2. Follow Steps 2–6 above targeting the staging database.
3. Run the backend smoke tests:
   ```bash
   curl -sf https://staging.soroban-loyalty.example.com/health
   ```
4. Record the result in the monthly ops log (date, backup timestamp used, row counts before/after, pass/fail).

---

## Trigger a manual backup

```bash
kubectl create job --from=cronjob/postgres-backup \
  manual-backup-$(date +%Y%m%d) \
  -n soroban-loyalty
```

---

## Troubleshooting

| Symptom | Check |
|---|---|
| Job fails immediately | `kubectl logs` — likely missing env var or secret key |
| `pg_dump: error: connection failed` | Verify `PGHOST`/`PGPASSWORD` in `soroban-loyalty-secrets` |
| `aws s3 cp` permission denied | Verify IRSA role ARN annotation on `postgres-backup` ServiceAccount |
| No Slack alert | `SLACK_WEBHOOK_URL` secret is unset or webhook is revoked |
| Backup older than 25 hours | CronJob may have been suspended — check `kubectl get cronjob postgres-backup -n soroban-loyalty` |
