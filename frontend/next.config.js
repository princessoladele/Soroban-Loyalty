const withNextIntl = require('next-intl/plugin')();
const { withSentryConfig } = require('@sentry/nextjs');

/** @type {import('next').NextConfig} */
const nextConfig = {
  reactStrictMode: true,
  output: "standalone",
  images: {
    remotePatterns: [
      {
        protocol: 'https',
        hostname: '**',
      },
    ],
  },
  // Support CDN asset prefix if provided via environment variable
  assetPrefix: process.env.NEXT_PUBLIC_CDN_URL || undefined,
  env: {
    NEXT_PUBLIC_API_URL: process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:3001",
    NEXT_PUBLIC_SOROBAN_RPC_URL: process.env.NEXT_PUBLIC_SOROBAN_RPC_URL ?? "http://localhost:8000/soroban/rpc",
    NEXT_PUBLIC_NETWORK_PASSPHRASE: process.env.NEXT_PUBLIC_NETWORK_PASSPHRASE ?? "Test SDF Network ; September 2015",
    NEXT_PUBLIC_REWARDS_CONTRACT_ID: process.env.NEXT_PUBLIC_REWARDS_CONTRACT_ID ?? "",
    NEXT_PUBLIC_CAMPAIGN_CONTRACT_ID: process.env.NEXT_PUBLIC_CAMPAIGN_CONTRACT_ID ?? "",
  },
  async headers() {
    return [
      {
        // Immutable assets (hashed filenames)
        source: '/_next/static/(.*)',
        headers: [
          {
            key: 'Cache-Control',
            value: 'public, max-age=31536000, immutable',
          },
        ],
      },
      {
        // Static assets in the public/static folder (if used)
        source: '/static/(.*)',
        headers: [
          {
            key: 'Cache-Control',
            value: 'public, max-age=31536000, immutable',
          },
        ],
      },
      {
        // Default cache control for HTML pages and other assets
        // We use a short TTL for HTML to allow for updates while still benefiting from CDN
        source: '/((?!_next/static|static|api|favicon.ico).*)',
        headers: [
          {
            key: 'Cache-Control',
            value: 'public, max-age=60, stale-while-revalidate=59',
          },
        ],
      },
    ];
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
