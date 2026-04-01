// Client-side Ed25519 key generation + blake3 address derivation
// No server involved — keys never leave the browser.

import * as ed from '@noble/ed25519';
import { blake3 } from '@noble/hashes/blake3.js';

function bytesToHex(bytes: Uint8Array): string {
  return Array.from(bytes).map(b => b.toString(16).padStart(2, '0')).join('');
}

/**
 * Generate a new Ed25519 keypair and derive the UltraDAG address.
 * Uses browser CSPRNG (crypto.getRandomValues) for the private key.
 * Address = blake3(ed25519_pubkey)[..20] (20 bytes, matching the Rust implementation).
 */
function hexToBytes(hex: string): Uint8Array {
  const bytes = new Uint8Array(hex.length / 2);
  for (let i = 0; i < hex.length; i += 2) {
    bytes[i / 2] = parseInt(hex.substring(i, i + 2), 16);
  }
  return bytes;
}

/**
 * Derive the UltraDAG address from an existing Ed25519 secret key (hex).
 * Address = blake3(ed25519_pubkey)[..20]
 */
export async function deriveAddress(secretKeyHex: string): Promise<string> {
  const seed = hexToBytes(secretKeyHex);
  const publicKey = await ed.getPublicKeyAsync(seed);
  const hash = blake3(publicKey);
  return bytesToHex(hash.slice(0, 20));
}

export async function generateKeypair(): Promise<{ secret_key: string; address: string }> {
  // Generate 32 random bytes as the Ed25519 seed
  const seed = crypto.getRandomValues(new Uint8Array(32));

  // Derive Ed25519 public key from seed
  const publicKey = await ed.getPublicKeyAsync(seed);

  // Address = blake3(pubkey) truncated to 20 bytes
  const hash = blake3(publicKey);
  const addressBytes = hash.slice(0, 20);

  return {
    secret_key: bytesToHex(seed),
    address: bytesToHex(addressBytes),
  };
}
