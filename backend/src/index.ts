import { initSentry, Sentry } from "./sentry";
// Sentry must be initialized before any other imports that may throw
initSentry();

import express from "express";
import cors from "cors";
import dotenv from "dotenv";
import { loadSecrets } from "./secrets";
import { campaignRouter } from "./routes/campaign.routes";
import { rewardRouter } from "./routes/reward.routes";
import { analyticsRouter } from "./routes/analytics.routes";
import { startIndexer } from "./indexer/indexer";
import { rpcServer } from "./soroban";
import { pool } from "./db";
import { registry, httpRequestsTotal, httpRequestDuration, dbPoolActive, dbPoolIdle, dbPoolWaiting } from "./metrics";
import { correlationMiddleware } from "./correlation";

// Load .env first (no-op in production where env vars are injected),
// then fetch secrets from AWS Secrets Manager before any other init.
dotenv.config();
await loadSecrets();

const app = express();
app.use(cors());
app.use(correlationMiddleware);
app.use(express.json());
app.use(requestLogger);

// ── Prometheus HTTP instrumentation ──────────────────────────────────────────
app.use((req, res, next) => {
  const end = httpRequestDuration.startTimer();
  res.on("finish", () => {
    const route = req.route?.path ?? req.path;
    const labels = { method: req.method, route, status: String(res.statusCode) };
    httpRequestsTotal.inc(labels);
    end(labels);
  });
  next();
});

// ── /metrics endpoint for Prometheus scraping ─────────────────────────────────
app.get("/metrics", async (_req, res) => {
  // Snapshot DB pool stats
  dbPoolActive.set(pool.totalCount - pool.idleCount);
  dbPoolIdle.set(pool.idleCount);
  dbPoolWaiting.set(pool.waitingCount);

  res.set("Content-Type", registry.contentType);
  res.end(await registry.metrics());
});

app.get("/health", async (_req, res) => {
  const startTime = Date.now();
  const checks: any = {
    stellar: { reachable: false, latency: 0 },
    database: { connected: false, responseTime: 0 },
    indexer: { running: true }
  };

  // Check Stellar network
  try {
    const stellarStart = Date.now();
    await rpcServer.getHealth();
    checks.stellar.reachable = true;
    checks.stellar.latency = Date.now() - stellarStart;
  } catch (err) {
    checks.stellar.reachable = false;
  }

  // Check database
  try {
    const dbStart = Date.now();
    await pool.query('SELECT 1');
    checks.database.connected = true;
    checks.database.responseTime = Date.now() - dbStart;
  } catch (err) {
    checks.database.connected = false;
  }

  const allHealthy = checks.stellar.reachable && checks.database.connected;
  const status = allHealthy ? 'healthy' : (checks.stellar.reachable || checks.database.connected) ? 'degraded' : 'unhealthy';

  res.json({
    status,
    checks,
    timestamp: new Date().toISOString(),
    uptime: process.uptime()
  });
});

app.use("/campaigns", campaignRouter);
app.use("/", rewardRouter);
app.use("/analytics", analyticsRouter);

// Sentry error handler must be before other error handlers
Sentry.setupExpressErrorHandler(app);

// Global error handler — logs + alerts on unhandled errors
app.use(errorAlertMiddleware);

// Catch unhandled promise rejections and exceptions
process.on("unhandledRejection", (reason) => {
  const err = reason instanceof Error ? reason : new Error(String(reason));
  Sentry.captureException(err);
  logger.critical("Unhandled promise rejection", err);
});
process.on("uncaughtException", (err) => {
  Sentry.captureException(err);
  logger.critical("Uncaught exception", err);
  process.exit(1);
});

const PORT = process.env.PORT ?? 3001;

app.listen(PORT, async () => {
  logger.info(`Server listening on port ${PORT}`);
  if (process.env.ENABLE_INDEXER !== "false") {
    await startIndexer();
  }
});

export default app;
