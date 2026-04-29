import { resolve } from "path";
import runner from "node-pg-migrate";
import { logger } from "./logger";

/**
 * Runs all pending migrations. Called automatically on startup in development,
 * and explicitly via `npm run migrate:up` in any environment.
 */
export async function runMigrations(): Promise<void> {
  const databaseUrl = process.env.DATABASE_URL;
  if (!databaseUrl) {
    throw new Error("DATABASE_URL is not set — cannot run migrations");
  }

  logger.info("Running database migrations...");

  await runner({
    databaseUrl,
    migrationsTable: "pgmigrations",
    dir: resolve(__dirname, "../migrations"),
    direction: "up",
    log: (msg) => logger.info(msg),
  });

  logger.info("Migrations complete");
}
