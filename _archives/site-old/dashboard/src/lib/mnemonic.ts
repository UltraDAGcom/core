// BIP39 mnemonic generation & Ed25519 key derivation
// Uses @scure/bip39 (same author as @noble/ed25519, audited)

import { generateMnemonic, mnemonicToSeedSync, validateMnemonic } from '@scure/bip39';
import { wordlist } from '@scure/bip39/wordlists/english.js';
import * as ed from '@noble/ed25519';
import { blake3 } from '@noble/hashes/blake3.js';

function bytesToHex(bytes: Uint8Array): string {
  return Array.from(bytes).map(b => b.toString(16).padStart(2, '0')).join('');
}

/**
 * Generate a 12-word BIP39 mnemonic (128 bits of entropy).
 */
export function generateMnemonicPhrase(): string {
  return generateMnemonic(wordlist, 128);
}

/**
 * Validate a mnemonic phrase (word count, wordlist, checksum).
 */
export function isValidMnemonic(phrase: string): boolean {
  return validateMnemonic(phrase.trim().toLowerCase(), wordlist);
}

/**
 * Derive an Ed25519 keypair from a BIP39 mnemonic.
 * Uses the standard BIP39 seed derivation (PBKDF2-SHA512, 2048 rounds),
 * then takes the first 32 bytes as the Ed25519 seed.
 * This matches the approach used by Solana, Cardano, and other Ed25519 chains.
 */
export async function mnemonicToKeypair(phrase: string): Promise<{
  secret_key: string;
  address: string;
  mnemonic: string;
}> {
  const mnemonic = phrase.trim().toLowerCase();
  // BIP39 standard: PBKDF2-HMAC-SHA512(mnemonic, "mnemonic", 2048) → 64 bytes
  const seed64 = mnemonicToSeedSync(mnemonic);
  // Use first 32 bytes as Ed25519 seed (standard for non-BIP32 Ed25519 chains)
  const seed32 = seed64.slice(0, 32);

  const publicKey = await ed.getPublicKeyAsync(seed32);
  const hash = blake3(publicKey);
  const addressBytes = hash.slice(0, 20);

  return {
    secret_key: bytesToHex(seed32),
    address: bytesToHex(addressBytes),
    mnemonic,
  };
}

/**
 * Generate a new mnemonic and derive the keypair from it.
 */
export async function generateWithMnemonic(): Promise<{
  secret_key: string;
  address: string;
  mnemonic: string;
}> {
  const mnemonic = generateMnemonicPhrase();
  return mnemonicToKeypair(mnemonic);
}
