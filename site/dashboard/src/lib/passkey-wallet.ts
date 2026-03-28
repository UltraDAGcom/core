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

const STORAGE_KEY = 'ultradag_passkey'; // v2: address normalization
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
  // Auto-fix old passkey wallets with 64-char addresses (should be 40)
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw) {
      const w = JSON.parse(raw);
      if (w.address && w.address.replace(/^0x/, '').length > 40) {
        w.address = w.address.replace(/^0x/, '').toLowerCase().slice(0, 40);
        localStorage.setItem(STORAGE_KEY, JSON.stringify(w));
      }
    }
  } catch { /* ignore */ }
}

// ── Core API ─────────────────────────────────────────────────────────────

/** Check if a passkey wallet exists. */
export function hasPasskeyWallet(): boolean {
  return localStorage.getItem(STORAGE_KEY) !== null;
}

/** Normalize address to exactly 40 hex chars (20 bytes = UltraDAG Address size). */
function normalizeAddr(addr: string): string {
  const clean = addr.replace(/^0x/, '').toLowerCase();
  return clean.length > 40 ? clean.slice(0, 40) : clean;
}

/** Get the stored passkey wallet info (or null). */
export function getPasskeyWallet(): PasskeyWallet | null {
  if (cachedWallet) return cachedWallet;
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as PasskeyWallet;
    // Fix old wallets that stored 64-char addresses (32 bytes) instead of 40-char (20 bytes)
    if (parsed.address && parsed.address.replace(/^0x/, '').length > 40) {
      parsed.address = normalizeAddr(parsed.address);
      localStorage.setItem(STORAGE_KEY, JSON.stringify(parsed));
    }
    cachedWallet = parsed;
    return cachedWallet;
  } catch {
    return null;
  }
}

/** Save a new passkey wallet after creation. */
export function savePasskeyWallet(wallet: PasskeyWallet): void {
  wallet = { ...wallet, address: normalizeAddr(wallet.address) };
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
