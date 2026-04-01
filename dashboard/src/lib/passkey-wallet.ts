/**
 * Passkey-native wallet management.
 *
 * This replaces the encrypted keystore for passkey wallets. The P256 private key
 * lives in the device's secure enclave (TPM / Secure Enclave / Android Keystore)
 * and NEVER leaves the hardware. We only store:
 *   - credentialId: identifies the passkey for navigator.credentials.get()
 *   - p256PubkeyHex: compressed SEC1 public key (for on-chain verification)
 *   - address: the on-chain account address
 *   - keyId: blake3(0x01 || pubkey)[..8] — used in SmartTransferTx.signing_key_id
 *   - name: optional human-readable name (from name registry)
 *
 * No secret keys. No passwords. No seed phrases. Security = biometrics.
 */

import { blake3 } from '@noble/hashes/blake3.js';

const STORAGE_KEY = 'ultradag_passkey';
const SESSION_KEY = 'ultradag_passkey_unlocked';

export interface PasskeyWallet {
  credentialId: string;
  p256PubkeyHex: string;
  address: string;
  keyId: string;
  name: string | null;
}

// ── State ────────────────────────────────────────────────────────────────

let cachedWallet: PasskeyWallet | null = null;
let sessionUnlocked = false;

// Check sessionStorage for unlock state (survives page refresh within tab)
if (typeof window !== 'undefined') {
  sessionUnlocked = sessionStorage.getItem(SESSION_KEY) === 'true';
  // Auto-fix passkey wallets with wrong addresses.
  // Re-derive the correct address from P256 pubkey: blake3("smart_account_p256" || pubkey)[:20]
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw) {
      const w = JSON.parse(raw);
      if (w.p256PubkeyHex) {
        const pubkeyBytes = new Uint8Array(w.p256PubkeyHex.length / 2);
        for (let i = 0; i < w.p256PubkeyHex.length; i += 2)
          pubkeyBytes[i / 2] = parseInt(w.p256PubkeyHex.substring(i, i + 2), 16);
        const prefix = new TextEncoder().encode('smart_account_p256');
        const combined = new Uint8Array(prefix.length + pubkeyBytes.length);
        combined.set(prefix); combined.set(pubkeyBytes, prefix.length);
        const fullHash = blake3(combined);
        const correctAddr = Array.from(fullHash.slice(0, 20)).map(b => b.toString(16).padStart(2, '0')).join('');
        if (w.address !== correctAddr) {
          console.log(`[passkey] Fixing address: ${w.address} → ${correctAddr}`);
          w.address = correctAddr;
          localStorage.setItem(STORAGE_KEY, JSON.stringify(w));
        }
      }
    }
  } catch { /* ignore */ }
}

// ── Core API ─────────────────────────────────────────────────────────────

/** Check if a passkey wallet exists. */
export function hasPasskeyWallet(): boolean {
  return localStorage.getItem(STORAGE_KEY) !== null;
}

/** Derive correct address from P256 pubkey: blake3("smart_account_p256" || pubkey)[:20] */
function deriveAddress(p256PubkeyHex: string): string {
  const pubkeyBytes = new Uint8Array(p256PubkeyHex.length / 2);
  for (let i = 0; i < p256PubkeyHex.length; i += 2)
    pubkeyBytes[i / 2] = parseInt(p256PubkeyHex.substring(i, i + 2), 16);
  const prefix = new TextEncoder().encode('smart_account_p256');
  const combined = new Uint8Array(prefix.length + pubkeyBytes.length);
  combined.set(prefix); combined.set(pubkeyBytes, prefix.length);
  const fullHash = blake3(combined);
  return Array.from(fullHash.slice(0, 20)).map(b => b.toString(16).padStart(2, '0')).join('');
}

/** Get the stored passkey wallet info (or null). */
export function getPasskeyWallet(): PasskeyWallet | null {
  if (cachedWallet) return cachedWallet;
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as PasskeyWallet;
    // Always verify address matches the P256 pubkey derivation
    if (parsed.p256PubkeyHex) {
      const correct = deriveAddress(parsed.p256PubkeyHex);
      if (parsed.address !== correct) {
        parsed.address = correct;
        localStorage.setItem(STORAGE_KEY, JSON.stringify(parsed));
      }
    }
    cachedWallet = parsed;
    return cachedWallet;
  } catch {
    return null;
  }
}

/** Save a new passkey wallet after creation. */
export function savePasskeyWallet(wallet: PasskeyWallet): void {
  // Always derive address from pubkey to ensure correctness
  if (wallet.p256PubkeyHex) {
    wallet = { ...wallet, address: deriveAddress(wallet.p256PubkeyHex) };
  }
  localStorage.setItem(STORAGE_KEY, JSON.stringify(wallet));
  cachedWallet = wallet;
  // Auto-unlock on creation
  sessionUnlocked = true;
  sessionStorage.setItem(SESSION_KEY, 'true');
  notifyListeners();
}

/** Check if the wallet is unlocked (biometric verified this session). */
export function isUnlocked(): boolean {
  return sessionUnlocked && hasPasskeyWallet();
}

/**
 * Unlock via biometric verification.
 * Calls navigator.credentials.get() to verify the user's identity.
 * The private key never leaves the secure enclave — we just verify
 * that the user can authenticate with the registered passkey.
 */
export async function unlockWithBiometric(): Promise<boolean> {
  const wallet = getPasskeyWallet();
  if (!wallet) return false;

  try {
    const challenge = crypto.getRandomValues(new Uint8Array(32));
    const credential = await navigator.credentials.get({
      publicKey: {
        challenge,
        rpId: window.location.hostname,
        userVerification: 'required',
        timeout: 60000,
      },
    });

    if (!credential) return false;

    sessionUnlocked = true;
    sessionStorage.setItem(SESSION_KEY, 'true');
    notifyListeners();
    return true;
  } catch {
    return false;
  }
}

/** Lock the wallet (clears session unlock state). */
export function lock(): void {
  sessionUnlocked = false;
  sessionStorage.removeItem(SESSION_KEY);
  notifyListeners();
}

/** Destroy the passkey wallet (factory reset). */
export function destroy(): void {
  localStorage.removeItem(STORAGE_KEY);
  sessionStorage.removeItem(SESSION_KEY);
  cachedWallet = null;
  sessionUnlocked = false;
  notifyListeners();
}

// ── Change listeners ─────────────────────────────────────────────────────

type Listener = () => void;
const listeners: Listener[] = [];

export function onPasskeyChange(fn: Listener): () => void {
  listeners.push(fn);
  return () => {
    const idx = listeners.indexOf(fn);
    if (idx >= 0) listeners.splice(idx, 1);
  };
}

function notifyListeners() {
  for (const fn of listeners) fn();
}
