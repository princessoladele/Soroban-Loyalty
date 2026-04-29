import { Request } from "express";
import { env } from "./env";
import { getCorrelationId } from "./correlation";

export type AlertLevel = "critical" | "error" | "warn" | "info" | "debug";

interface AlertPayload {
  level: AlertLevel;
  message: string;
  error?: Error;
  context?: Record<string, unknown>;
}

async function sendSlackAlert(payload: AlertPayload): Promise<void> {
  if (!env.SLACK_WEBHOOK_URL || env.MAINTENANCE_MODE) return;
  const { level, message, error, context } = payload;
  const emoji = level === "critical" ? "🚨" : "⚠️";
  const runbook = "https://github.com/Dev-Odun-oss/Soroban-Loyalty/wiki/Runbooks";

  const text = [
    `${emoji} *[${level.toUpperCase()}]* ${message}`,
    error ? `\`\`\`${error.stack ?? error.message}\`\`\`` : null,
    context ? `*Context:* \`${JSON.stringify(context)}\`` : null,
    `*Runbook:* ${runbook}`,
  ]
    .filter(Boolean)
    .join("\n");

  try {
    await fetch(env.SLACK_WEBHOOK_URL, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ text }),
    });
  } catch {
    // Never throw from alerting path
  }
}

function withCorrelation(obj: Record<string, unknown>): Record<string, unknown> {
  const id = getCorrelationId();
  return id ? { correlationId: id, ...obj } : obj;
}

export const logger = {
  debug(message: string, context?: Record<string, unknown>) {
    console.debug(JSON.stringify(withCorrelation({ level: "debug", message, ...context, ts: new Date().toISOString() })));
  },

  info(message: string, context?: Record<string, unknown>) {
    console.log(JSON.stringify(withCorrelation({ level: "info", message, ...context, ts: new Date().toISOString() })));
  },

  warn(message: string, context?: Record<string, unknown>) {
    console.warn(JSON.stringify(withCorrelation({ level: "warn", message, ...context, ts: new Date().toISOString() })));
  },

  error(message: string, error?: Error, context?: Record<string, unknown>) {
    console.error(
      JSON.stringify(
        withCorrelation({
          level: "error",
          message,
          error: error?.message,
          stack: error?.stack,
          ...context,
          ts: new Date().toISOString(),
        })
      )
    );
    sendSlackAlert({ level: "error", message, error, context });
  },

  critical(message: string, error?: Error, context?: Record<string, unknown>) {
    console.error(
      JSON.stringify(
        withCorrelation({
          level: "critical",
          message,
          error: error?.message,
          stack: error?.stack,
          ...context,
          ts: new Date().toISOString(),
        })
      )
    );
    sendSlackAlert({ level: "critical", message, error, context });
  },
};

/** Express middleware: attaches request context to errors and alerts on 5xx */
export function requestLogger(
  req: Request,
  _res: unknown,
  next: () => void
): void {
  (req as Request & { _startAt: number })._startAt = Date.now();
  next();
}

/** Express error handler: logs + alerts on unhandled errors */
export function errorAlertMiddleware(
  err: Error,
  req: Request,
  res: { status: (c: number) => { json: (b: unknown) => void } },
  _next: unknown
): void {
  const context = {
    method: req.method,
    path: req.path,
    query: req.query,
    durationMs: Date.now() - ((req as Request & { _startAt: number })._startAt ?? 0),
  };
  logger.critical("Unhandled exception", err, context);
  res.status(500).json({ error: "Internal server error" });
}
