/**
 * Client-side pocket-claim challenge builder and signer.
 *
 * When a user adds a pocket "@alice.savings → wallet_B", wallet_B must
 * prove it consents to being listed. The proof is an Ed25519 signature
 * over a deterministic challenge that binds (parent_name, label, target_addr).
 *
 * This must match the Rust layout in name_registry.rs pocket_claim_bytes():
 *   POCKET_CLAIM_DOMAIN || parent_name || 0x00 || label || 0x00 || target_addr(20 bytes)
 */

import * as ed from '@noble/ed25519';

const POCKET_CLAIM_DOMAIN = new TextEncoder().encode('ultradag-pocket-claim');

/**
 * Build the pocket-claim challenge bytes. Byte-exact match to the Rust
 * encoder in name_registry.rs::pocket_claim_bytes().
 */
export function buildPocketClaim(
  parentName: string,
  label: string,
  targetAddressHex: string,
): Uint8Array {
  const parentBytes = new TextEncoder().encode(parentName.toLowerCase());
  const labelBytes = new TextEncoder().encode(label.toLowerCase());
  // Target address is 20 bytes (40 hex chars).
  const addrBytes = hexToBytes(targetAddressHex);
  if (addrBytes.length !== 20) {
    throw new Error(`Expected 20-byte address, got ${addrBytes.length}`);
  }

  const buf = new Uint8Array(
    POCKET_CLAIM_DOMAIN.length + parentBytes.length + 1 + labelBytes.length + 1 + 20,
  );
  let offset = 0;
  buf.set(POCKET_CLAIM_DOMAIN, offset); offset += POCKET_CLAIM_DOMAIN.length;
  buf.set(parentBytes, offset); offset += parentBytes.length;
  buf[offset++] = 0x00; // separator
  buf.set(labelBytes, offset); offset += labelBytes.length;
  buf[offset++] = 0x00; // separator
  buf.set(addrBytes, offset);

  return buf;
}

/**
 * Sign a pocket-claim with an Ed25519 secret key (keystore wallet).
 *
 * Returns the hex-encoded pubkey (32 bytes → 64 hex) and proof signature
 * (64 bytes → 128 hex), ready to include in a pocket entry.
 */
export async function signPocketClaim(
  parentName: string,
  label: string,
  secretKeyHex: string,
  targetAddressHex: string,
): Promise<{ pubkeyHex: string; proofHex: string }> {
  const challenge = buildPocketClaim(parentName, label, targetAddressHex);
  const skBytes = hexToBytes(secretKeyHex);
  if (skBytes.length !== 32) {
    throw new Error(`Expected 32-byte secret key, got ${skBytes.length}`);
  }

  const pubkey = await ed.getPublicKeyAsync(skBytes);
  const signature = await ed.signAsync(challenge, skBytes);

  return {
    pubkeyHex: bytesToHex(pubkey),
    proofHex: bytesToHex(signature),
  };
}

// ── Helpers ──────────────────────────────────────────────────────────────

function hexToBytes(hex: string): Uint8Array {
  const clean = hex.startsWith('0x') ? hex.slice(2) : hex;
  const bytes = new Uint8Array(clean.length / 2);
  for (let i = 0; i < clean.length; i += 2) {
    bytes[i / 2] = parseInt(clean.substring(i, i + 2), 16);
  }
  return bytes;
}

function bytesToHex(bytes: Uint8Array): string {
  return Array.from(bytes).map(b => b.toString(16).padStart(2, '0')).join('');
}
