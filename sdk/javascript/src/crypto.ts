import * as ed from "@noble/ed25519";
import { createHash } from "blake3";
import { createHash as nodeCryptoHash } from "node:crypto";

// Configure @noble/ed25519 with a synchronous SHA-512 implementation using
// Node.js built-in crypto. This is required because @noble/ed25519 v2 does
// not ship with a default sync hasher.
ed.etc.sha512Sync = (...msgs: Uint8Array[]): Uint8Array => {
  const h = nodeCryptoHash("sha512");
  for (const m of msgs) h.update(m);
  return new Uint8Array(h.digest());
};

// ---------------------------------------------------------------------------
// Hex helpers
// ---------------------------------------------------------------------------

function bytesToHex(bytes: Uint8Array): string {
  const hex: string[] = [];
  for (const b of bytes) {
    hex.push(b.toString(16).padStart(2, "0"));
  }
  return hex.join("");
}

function hexToBytes(hex: string): Uint8Array {
  if (hex.length % 2 !== 0) {
    throw new Error("Hex string must have even length");
  }
  const bytes = new Uint8Array(hex.length / 2);
  for (let i = 0; i < hex.length; i += 2) {
    bytes[i / 2] = parseInt(hex.substring(i, i + 2), 16);
  }
  return bytes;
}

// ---------------------------------------------------------------------------
// Address derivation
// ---------------------------------------------------------------------------

/**
 * Derive an UltraDAG address from an Ed25519 public key.
 *
 * An address is the Blake3 hash of the 32-byte Ed25519 public key, encoded as
 * a 64-character lowercase hex string.
 *
 * @param publicKey - 32-byte Ed25519 public key.
 * @returns 64-character hex address string.
 */
export function deriveAddress(publicKey: Uint8Array): string {
  const hash = createHash();
  hash.update(publicKey);
  const digest: Uint8Array = hash.digest();
  return bytesToHex(digest);
}

// ---------------------------------------------------------------------------
// Keypair
// ---------------------------------------------------------------------------

/**
 * An Ed25519 keypair with the corresponding UltraDAG address.
 */
export class Keypair {
  /** 32-byte secret key encoded as 64-character hex. */
  public readonly secretKey: string;
  /** 32-byte Ed25519 public key encoded as 64-character hex. */
  public readonly publicKey: string;
  /** 32-byte Blake3 hash of the public key, encoded as 64-character hex. */
  public readonly address: string;

  private constructor(secretKey: string, publicKey: string, address: string) {
    this.secretKey = secretKey;
    this.publicKey = publicKey;
    this.address = address;
  }

  /**
   * Generate a new random Ed25519 keypair.
   *
   * Uses `crypto.getRandomValues` which is available in Node 18+ and all
   * modern browsers.
   *
   * @returns A new Keypair instance.
   */
  static generate(): Keypair {
    const secret = ed.utils.randomPrivateKey();
    return Keypair.fromSecretKey(bytesToHex(secret));
  }

  /**
   * Reconstruct a Keypair from an existing 64-character hex secret key.
   *
   * @param secretKeyHex - 64-character hex-encoded 32-byte Ed25519 secret key.
   * @returns A Keypair instance derived from the provided secret.
   */
  static fromSecretKey(secretKeyHex: string): Keypair {
    const secretBytes = hexToBytes(secretKeyHex);
    if (secretBytes.length !== 32) {
      throw new Error("Secret key must be exactly 32 bytes (64 hex characters)");
    }
    const publicBytes = ed.getPublicKey(secretBytes);
    const address = deriveAddress(publicBytes);
    return new Keypair(
      secretKeyHex.toLowerCase(),
      bytesToHex(publicBytes),
      address,
    );
  }

  /**
   * Construct a Keypair from a byte array seed (for example `[0xFA; 32]` for
   * the faucet key).
   *
   * @param seed - 32-byte Uint8Array.
   * @returns A Keypair instance.
   */
  static fromBytes(seed: Uint8Array): Keypair {
    if (seed.length !== 32) {
      throw new Error("Seed must be exactly 32 bytes");
    }
    return Keypair.fromSecretKey(bytesToHex(seed));
  }
}
