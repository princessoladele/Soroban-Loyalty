/**
 * JWT authentication for merchant endpoints.
 *
 * Flow:
 *   1. POST /auth/challenge  — returns a nonce for a given public key
 *   2. POST /auth/verify     — validates the Stellar-signed nonce, returns a JWT
 *   3. Merchant routes use requireAuth middleware to validate the JWT
 *
 * JWT is a minimal HS256 implementation using Node's built-in crypto module.
 * Nonces are stored in-memory (replace with Redis for multi-instance deployments).
 */

import { createHmac, randomBytes, timingSafeEqual } from "crypto";
import { Request, Response, NextFunction } from "express";
import { Keypair } from "@stellar/stellar-sdk";

// ── Config ────────────────────────────────────────────────────────────────────

const JWT_SECRET = process.env.JWT_SECRET ?? randomBytes(32).toString("hex");
const JWT_EXPIRY_SECONDS = parseInt(process.env.JWT_EXPIRY_SECONDS ?? "3600", 10);
const NONCE_TTL_MS = 5 * 60 * 1000; // 5 minutes

// ── Nonce store ───────────────────────────────────────────────────────────────

interface NonceEntry { nonce: string; expiresAt: number }
const nonceStore = new Map<string, NonceEntry>();

function pruneExpiredNonces() {
  const now = Date.now();
  for (const [key, entry] of nonceStore) {
    if (entry.expiresAt < now) nonceStore.delete(key);
  }
}

// ── Minimal HS256 JWT ─────────────────────────────────────────────────────────

function b64url(buf: Buffer | string): string {
  const b = typeof buf === "string" ? Buffer.from(buf) : buf;
  return b.toString("base64url");
}

function signJwt(payload: Record<string, unknown>): string {
  const header = b64url(JSON.stringify({ alg: "HS256", typ: "JWT" }));
  const body = b64url(JSON.stringify(payload));
  const sig = createHmac("sha256", JWT_SECRET)
    .update(`${header}.${body}`)
    .digest();
  return `${header}.${body}.${b64url(sig)}`;
}

function verifyJwt(token: string): Record<string, unknown> {
  const parts = token.split(".");
  if (parts.length !== 3) throw new Error("Malformed token");
  const [header, body, sig] = parts;
  const expected = createHmac("sha256", JWT_SECRET)
    .update(`${header}.${body}`)
    .digest();
  const actual = Buffer.from(sig, "base64url");
  if (actual.length !== expected.length || !timingSafeEqual(actual, expected)) {
    throw new Error("Invalid signature");
  }
  const payload = JSON.parse(Buffer.from(body, "base64url").toString()) as Record<string, unknown>;
  if (typeof payload.exp === "number" && payload.exp < Math.floor(Date.now() / 1000)) {
    throw new Error("Token expired");
  }
  return payload;
}

// ── Route handlers ────────────────────────────────────────────────────────────

/**
 * POST /auth/challenge
 * Body: { publicKey: string }
 * Returns: { nonce: string }
 */
export function challengeHandler(req: Request, res: Response): void {
  const { publicKey } = req.body as { publicKey?: string };
  if (!publicKey || typeof publicKey !== "string") {
    res.status(400).json({ error: "publicKey is required" });
    return;
  }

  // Validate it looks like a Stellar public key
  try { Keypair.fromPublicKey(publicKey); } catch {
    res.status(400).json({ error: "Invalid Stellar public key" });
    return;
  }

  pruneExpiredNonces();
  const nonce = randomBytes(32).toString("hex");
  nonceStore.set(publicKey, { nonce, expiresAt: Date.now() + NONCE_TTL_MS });
  res.json({ nonce });
}

/**
 * POST /auth/verify
 * Body: { publicKey: string, signature: string }
 * Returns: { token: string }
 *
 * The client must sign the nonce bytes with their Stellar keypair using
 * Freighter's signMessage (or equivalent) and send the hex-encoded signature.
 */
export function verifyHandler(req: Request, res: Response): void {
  const { publicKey, signature } = req.body as { publicKey?: string; signature?: string };
  if (!publicKey || !signature) {
    res.status(400).json({ error: "publicKey and signature are required" });
    return;
  }

  const entry = nonceStore.get(publicKey);
  if (!entry || entry.expiresAt < Date.now()) {
    res.status(401).json({ error: "Nonce expired or not found. Request a new challenge." });
    return;
  }

  // Verify the Stellar signature over the nonce
  try {
    const keypair = Keypair.fromPublicKey(publicKey);
    const nonceBytes = Buffer.from(entry.nonce, "hex");
    const sigBytes = Buffer.from(signature, "hex");
    const valid = keypair.verify(nonceBytes, sigBytes);
    if (!valid) throw new Error("Signature mismatch");
  } catch {
    res.status(401).json({ error: "Signature verification failed" });
    return;
  }

  // Consume nonce (one-time use)
  nonceStore.delete(publicKey);

  const now = Math.floor(Date.now() / 1000);
  const token = signJwt({ sub: publicKey, iat: now, exp: now + JWT_EXPIRY_SECONDS });
  res.json({ token });
}

// ── Middleware ────────────────────────────────────────────────────────────────

export interface AuthRequest extends Request {
  merchantPublicKey: string;
}

/**
 * Express middleware: validates JWT in Authorization header.
 * Attaches req.merchantPublicKey on success.
 */
export function requireAuth(req: Request, res: Response, next: NextFunction): void {
  const authHeader = req.headers.authorization;
  if (!authHeader?.startsWith("Bearer ")) {
    res.status(401).json({ error: "Missing or malformed Authorization header" });
    return;
  }
  const token = authHeader.slice(7);
  try {
    const payload = verifyJwt(token);
    (req as AuthRequest).merchantPublicKey = payload.sub as string;
    next();
  } catch (err) {
    res.status(401).json({ error: (err as Error).message });
  }
}
