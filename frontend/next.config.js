const withNextIntl = require('next-intl/plugin')();
const { withSentryConfig } = require('@sentry/nextjs');

/** @type {import('next').NextConfig} */
const nextConfig = {
  reactStrictMode: true,
  output: "standalone",
  env: {
    NEXT_PUBLIC_API_URL: process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:3001",
    NEXT_PUBLIC_SOROBAN_RPC_URL: process.env.NEXT_PUBLIC_SOROBAN_RPC_URL ?? "http://localhost:8000/soroban/rpc",
    NEXT_PUBLIC_NETWORK_PASSPHRASE: process.env.NEXT_PUBLIC_NETWORK_PASSPHRASE ?? "Test SDF Network ; September 2015",
    NEXT_PUBLIC_REWARDS_CONTRACT_ID: process.env.NEXT_PUBLIC_REWARDS_CONTRACT_ID ?? "",
    NEXT_PUBLIC_CAMPAIGN_CONTRACT_ID: process.env.NEXT_PUBLIC_CAMPAIGN_CONTRACT_ID ?? "",
  },
};

module.exports = withSentryConfig(withNextIntl(nextConfig), {
  // Upload source maps to Sentry on build (requires SENTRY_AUTH_TOKEN)
  silent: true,
  org: process.env.SENTRY_ORG,
  project: process.env.SENTRY_PROJECT,
  // Automatically tree-shake Sentry logger statements in production
  disableLogger: true,
  // Hides source maps from the browser bundle
  hideSourceMaps: true,
});
