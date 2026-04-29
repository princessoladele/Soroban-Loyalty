/* eslint-disable camelcase */
"use strict";

/** @param {import('node-pg-migrate').MigrationBuilder} pgm */
exports.up = (pgm) => {
  pgm.createExtension("uuid-ossp", { ifNotExists: true });

  pgm.createTable("users", {
    address: { type: "varchar(56)", primaryKey: true },
    created_at: {
      type: "timestamptz",
      notNull: true,
      default: pgm.func("NOW()"),
    },
  });

  pgm.createTable("campaigns", {
    id: { type: "bigint", primaryKey: true },
    merchant: { type: "varchar(56)", notNull: true },
    reward_amount: { type: "bigint", notNull: true },
    expiration: { type: "bigint", notNull: true },
    active: { type: "boolean", notNull: true, default: true },
    total_claimed: { type: "bigint", notNull: true, default: 0 },
    display_order: { type: "int", notNull: true, default: 0 },
    tx_hash: { type: "varchar(64)" },
    created_at: {
      type: "timestamptz",
      notNull: true,
      default: pgm.func("NOW()"),
    },
    updated_at: {
      type: "timestamptz",
      notNull: true,
      default: pgm.func("NOW()"),
    },
  });

  pgm.createTable("rewards", {
    id: {
      type: "uuid",
      primaryKey: true,
      default: pgm.func("uuid_generate_v4()"),
    },
    user_address: {
      type: "varchar(56)",
      notNull: true,
      references: '"users"',
      onDelete: "CASCADE",
    },
    campaign_id: {
      type: "bigint",
      notNull: true,
      references: '"campaigns"',
      onDelete: "CASCADE",
    },
    amount: { type: "bigint", notNull: true },
    redeemed: { type: "boolean", notNull: true, default: false },
    redeemed_amount: { type: "bigint", notNull: true, default: 0 },
    claimed_at: {
      type: "timestamptz",
      notNull: true,
      default: pgm.func("NOW()"),
    },
    redeemed_at: { type: "timestamptz" },
  });

  pgm.addConstraint("rewards", "rewards_user_campaign_unique", {
    unique: ["user_address", "campaign_id"],
  });

  pgm.createTable("transactions", {
    id: {
      type: "uuid",
      primaryKey: true,
      default: pgm.func("uuid_generate_v4()"),
    },
    tx_hash: { type: "varchar(64)", notNull: true, unique: true },
    type: { type: "varchar(32)", notNull: true },
    user_address: { type: "varchar(56)" },
    campaign_id: { type: "bigint" },
    amount: { type: "bigint" },
    ledger: { type: "bigint" },
    created_at: {
      type: "timestamptz",
      notNull: true,
      default: pgm.func("NOW()"),
    },
  });

  pgm.createIndex("rewards", "user_address", { name: "idx_rewards_user" });
  pgm.createIndex("rewards", "campaign_id", { name: "idx_rewards_campaign" });
  pgm.createIndex("transactions", "user_address", {
    name: "idx_transactions_user",
  });
  pgm.createIndex("campaigns", "merchant", { name: "idx_campaigns_merchant" });
};

/** @param {import('node-pg-migrate').MigrationBuilder} pgm */
exports.down = (pgm) => {
  pgm.dropIndex("campaigns", "merchant", { name: "idx_campaigns_merchant" });
  pgm.dropIndex("transactions", "user_address", {
    name: "idx_transactions_user",
  });
  pgm.dropIndex("rewards", "campaign_id", { name: "idx_rewards_campaign" });
  pgm.dropIndex("rewards", "user_address", { name: "idx_rewards_user" });

  pgm.dropTable("transactions");
  pgm.dropTable("rewards");
  pgm.dropTable("campaigns");
  pgm.dropTable("users");
};
