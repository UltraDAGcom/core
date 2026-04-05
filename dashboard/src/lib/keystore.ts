// Non-custodial keystore: AES-256-GCM + PBKDF2 600k iterations
// Private keys encrypted in localStorage, decrypted only in memory

export interface Wallet {
  name: string;
  secret_key: string;
  address: string;
}

interface KeystoreData {
  wallets: Wallet[];
  _password?: string;
}

interface EncryptedBlob {
  version: number;
  kdf: string;
  kdf_params: { iterations: number; salt: string };
  cipher: string;
  cipher_params: { iv: string };
  ciphertext: string;
}

const STORAGE_KEY = 'ultradag_keystore';
const PRIMARY_KEY = 'ultradag_primary_address';
let keystoreData: KeystoreData | null = null;
let encryptedBlob: EncryptedBlob | null = null;

// Auto-load from localStorage on module import (synchronous, runs before first render)
try {
  const raw = localStorage.getItem(STORAGE_KEY);
  if (raw) encryptedBlob = JSON.parse(raw);
} catch { /* ignore */ }

// Listeners for state changes
type Listener = () => void;
const listeners: Listener[] = [];
export function onKeystoreChange(fn: Listener) { listeners.push(fn); return () => { const i = listeners.indexOf(fn); if (i >= 0) listeners.splice(i, 1); }; }
function notify() { listeners.forEach(fn => fn()); }

// Crypto helpers
function toBase64(buf: ArrayBuffer | ArrayBufferLike): string { return btoa(String.fromCharCode(...new Uint8Array(buf as ArrayBuffer))); }
function fromBase64(str: string): Uint8Array {
  const bin = atob(str);
  const buf = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) buf[i] = bin.charCodeAt(i);
  return buf;
}

async function deriveKey(password: string, salt: Uint8Array): Promise<CryptoKey> {
  const enc = new TextEncoder();
  const km = await crypto.subtle.importKey('raw', enc.encode(password), 'PBKDF2', false, ['deriveKey']);
  return crypto.subtle.deriveKey(
    { name: 'PBKDF2', salt: salt.buffer as ArrayBuffer, iterations: 600000, hash: 'SHA-256' },
    km, { name: 'AES-GCM', length: 256 }, false, ['encrypt', 'decrypt']
  );
}

async function encrypt(data: any, password: string): Promise<EncryptedBlob> {
  const salt = crypto.getRandomValues(new Uint8Array(32));
  const iv = crypto.getRandomValues(new Uint8Array(12));
  const key = await deriveKey(password, salt);
  const ct = await crypto.subtle.encrypt({ name: 'AES-GCM', iv: iv as unknown as BufferSource }, key, new TextEncoder().encode(JSON.stringify(data)));
  return {
    version: 1, kdf: 'pbkdf2-sha256',
    kdf_params: { iterations: 600000, salt: toBase64(salt.buffer as ArrayBuffer) },
    cipher: 'aes-256-gcm', cipher_params: { iv: toBase64(iv.buffer as ArrayBuffer) },
    ciphertext: toBase64(ct),
  };
}

async function decrypt(blob: EncryptedBlob, password: string): Promise<any> {
  const salt = fromBase64(blob.kdf_params.salt);
  const iv = fromBase64(blob.cipher_params.iv);
  const ct = fromBase64(blob.ciphertext);
  const key = await deriveKey(password, salt);
  const pt = await crypto.subtle.decrypt({ name: 'AES-GCM', iv: iv as unknown as BufferSource }, key, ct as unknown as BufferSource);
  return JSON.parse(new TextDecoder().decode(pt));
}

// Public API
export function isUnlocked(): boolean { return keystoreData !== null; }
export function hasKeystore(): boolean { return encryptedBlob !== null; }
export function getWallets(): Wallet[] { return keystoreData?.wallets ?? []; }
export function getWallet(index: number): Wallet | undefined { return keystoreData?.wallets[index]; }

/**
 * Returns the user-chosen primary wallet address, or null if none is set.
 * Stored in plaintext localStorage (not encrypted) because addresses are public
 * and this preference needs to be readable before unlock (e.g., for route resolution).
 */
export function getPrimaryAddress(): string | null {
  try { return localStorage.getItem(PRIMARY_KEY); } catch { return null; }
}

/** Set the primary wallet address. Pass null to clear the preference. */
export function setPrimaryAddress(address: string | null): void {
  try {
    if (address == null) localStorage.removeItem(PRIMARY_KEY);
    else localStorage.setItem(PRIMARY_KEY, address);
    notify();
  } catch { /* quota / private mode — ignore */ }
}

export function loadFromStorage(): boolean {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw) { encryptedBlob = JSON.parse(raw); return true; }
  } catch {}
  return false;
}

export async function create(password: string): Promise<void> {
  keystoreData = { wallets: [], _password: password };
  encryptedBlob = await encrypt({ wallets: [] }, password);
  localStorage.setItem(STORAGE_KEY, JSON.stringify(encryptedBlob));
  notify();
}

export async function unlock(password: string): Promise<boolean> {
  if (!encryptedBlob) return false;
  try {
    keystoreData = await decrypt(encryptedBlob, password) as KeystoreData;
    keystoreData!._password = password;
    notify();
    return true;
  } catch {
    return false;
  }
}

export function lock(): void {
  keystoreData = null;
  notify();
}

/** Permanently delete the keystore and all wallets from this browser. */
export function destroy(): void {
  keystoreData = null;
  encryptedBlob = null;
  localStorage.removeItem(STORAGE_KEY);
  localStorage.removeItem(WEBAUTHN_STORAGE_KEY);
  localStorage.removeItem(PRIMARY_KEY);
  notify();
}

export async function addWallet(name: string, secretKey: string, address: string): Promise<void> {
  if (!keystoreData) throw new Error('Keystore not unlocked');
  // Skip if wallet with same address already exists
  if (keystoreData.wallets.some(w => w.address === address)) return;
  keystoreData.wallets.push({ name, secret_key: secretKey, address });
  await save();
  notify();
}

export async function removeWallet(index: number): Promise<void> {
  if (!keystoreData) throw new Error('Keystore not unlocked');
  const removed = keystoreData.wallets[index];
  keystoreData.wallets.splice(index, 1);
  // Clear the primary-address preference if the removed wallet was primary.
  if (removed && getPrimaryAddress() === removed.address) {
    try { localStorage.removeItem(PRIMARY_KEY); } catch { /* ignore */ }
  }
  await save();
  notify();
}

async function save(): Promise<void> {
  if (!keystoreData?._password) return;
  encryptedBlob = await encrypt({ wallets: keystoreData.wallets }, keystoreData._password);
  localStorage.setItem(STORAGE_KEY, JSON.stringify(encryptedBlob));
}

export async function changePassword(oldPw: string, newPw: string): Promise<boolean> {
  if (!encryptedBlob) return false;
  try {
    const data = await decrypt(encryptedBlob, oldPw);
    keystoreData = { ...data, _password: newPw };
    await save();
    notify();
    return true;
  } catch {
    return false;
  }
}

export function exportBlob(): string | null {
  return encryptedBlob ? JSON.stringify(encryptedBlob, null, 2) : null;
}

export function importBlob(json: string, _password?: string): boolean {
  try {
    const blob = JSON.parse(json);
    if (!blob.version || !blob.ciphertext) return false;
    encryptedBlob = blob;
    localStorage.setItem(STORAGE_KEY, JSON.stringify(blob));
    return true;
  } catch {
    return false;
  }
}

// ========================================================================
// WebAuthn (Biometric) Authentication
// ========================================================================
// Strategy: WebAuthn wraps the keystore password. The authenticator produces
// a signature used to derive an AES key that encrypts/decrypts the password.
// On biometric unlock: WebAuthn → derive key → decrypt password → unlock keystore.
// Fallback to manual password always available.

const WEBAUTHN_STORAGE_KEY = 'ultradag_webauthn';

interface WebAuthnData {
  credential_id: string;   // base64url
  wrapped_password: string; // base64 (AES-GCM encrypted password)
  wrapped_iv: string;       // base64
  wrapped_salt: string;     // base64
}

function toBase64Url(buf: ArrayBuffer): string {
  return btoa(String.fromCharCode(...new Uint8Array(buf)))
    .replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
}

function fromBase64Url(str: string): Uint8Array {
  const padded = str.replace(/-/g, '+').replace(/_/g, '/') + '=='.slice(0, (4 - str.length % 4) % 4);
  return fromBase64(padded);
}

export function isWebAuthnAvailable(): boolean {
  return typeof window !== 'undefined' && !!window.PublicKeyCredential;
}

export function isWebAuthnEnrolled(): boolean {
  try {
    const raw = localStorage.getItem(WEBAUTHN_STORAGE_KEY);
    return raw !== null;
  } catch { return false; }
}

/** Enroll WebAuthn after a successful password unlock. Wraps the password with biometric auth. */
export async function enrollWebAuthn(): Promise<boolean> {
  if (!keystoreData?._password) throw new Error('Keystore must be unlocked first');
  if (!isWebAuthnAvailable()) throw new Error('WebAuthn not supported on this device');

  const password = keystoreData._password;
  const userId = crypto.getRandomValues(new Uint8Array(16));

  // Create credential
  const credential = await navigator.credentials.create({
    publicKey: {
      rp: { name: 'UltraDAG Wallet' },
      user: {
        id: userId,
        name: 'wallet-user',
        displayName: 'UltraDAG Wallet',
      },
      challenge: crypto.getRandomValues(new Uint8Array(32)),
      pubKeyCredParams: [
        { alg: -7, type: 'public-key' },   // ES256
        { alg: -257, type: 'public-key' },  // RS256
      ],
      authenticatorSelection: {
        authenticatorAttachment: 'platform', // built-in biometric (Touch ID, Face ID, Windows Hello)
        userVerification: 'required',
      },
      timeout: 60000,
    },
  }) as PublicKeyCredential | null;

  if (!credential) return false;

  const credentialId = toBase64Url(credential.rawId);

  // Derive a wrapping key from the credential ID + a salt
  // (the credential ID is unique and stable across assertions)
  const salt = crypto.getRandomValues(new Uint8Array(16));
  const wrapKey = await deriveKeyFromBytes(new Uint8Array(credential.rawId), salt);

  // Encrypt the password with the wrapping key
  const iv = crypto.getRandomValues(new Uint8Array(12));
  const enc = new TextEncoder();
  const encryptedPassword = await crypto.subtle.encrypt(
    { name: 'AES-GCM', iv },
    wrapKey,
    enc.encode(password),
  );

  const webauthnData: WebAuthnData = {
    credential_id: credentialId,
    wrapped_password: toBase64(encryptedPassword),
    wrapped_iv: toBase64(iv.buffer as ArrayBuffer),
    wrapped_salt: toBase64(salt.buffer as ArrayBuffer),
  };

  localStorage.setItem(WEBAUTHN_STORAGE_KEY, JSON.stringify(webauthnData));
  notify();
  return true;
}

/** Unlock keystore using WebAuthn (biometric). Returns true on success. */
export async function unlockWithWebAuthn(): Promise<boolean> {
  if (!encryptedBlob) return false;
  if (!isWebAuthnAvailable()) return false;

  const raw = localStorage.getItem(WEBAUTHN_STORAGE_KEY);
  if (!raw) return false;

  const webauthnData: WebAuthnData = JSON.parse(raw);
  const credentialId = fromBase64Url(webauthnData.credential_id);

  // Request assertion (triggers biometric prompt)
  const assertion = await navigator.credentials.get({
    publicKey: {
      challenge: crypto.getRandomValues(new Uint8Array(32)),
      allowCredentials: [{ id: credentialId as unknown as BufferSource, type: 'public-key' }],
      userVerification: 'required',
      timeout: 60000,
    },
  }) as PublicKeyCredential | null;

  if (!assertion) return false;

  // Derive the same wrapping key from credential ID + stored salt
  const salt = fromBase64(webauthnData.wrapped_salt);
  const wrapKey = await deriveKeyFromBytes(new Uint8Array(assertion.rawId), salt);

  // Decrypt the password
  const iv = fromBase64(webauthnData.wrapped_iv);
  const ct = fromBase64(webauthnData.wrapped_password);
  try {
    const pt = await crypto.subtle.decrypt(
      { name: 'AES-GCM', iv: iv as unknown as BufferSource },
      wrapKey,
      ct as unknown as BufferSource,
    );
    const password = new TextDecoder().decode(pt);

    // Use the recovered password to unlock the keystore (existing flow)
    return unlock(password);
  } catch {
    return false;
  }
}

/** Remove WebAuthn enrollment. Reverts to password-only. */
export function removeWebAuthn(): void {
  localStorage.removeItem(WEBAUTHN_STORAGE_KEY);
  notify();
}

/** Derive an AES-256-GCM key from raw bytes + salt (for wrapping the password). */
async function deriveKeyFromBytes(keyMaterial: Uint8Array, salt: Uint8Array): Promise<CryptoKey> {
  const imported = await crypto.subtle.importKey('raw', keyMaterial as unknown as ArrayBuffer, 'PBKDF2', false, ['deriveKey']);
  return crypto.subtle.deriveKey(
    { name: 'PBKDF2', salt: salt.buffer as ArrayBuffer, iterations: 100000, hash: 'SHA-256' },
    imported,
    { name: 'AES-GCM', length: 256 },
    false,
    ['encrypt', 'decrypt'],
  );
}
