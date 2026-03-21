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
let keystoreData: KeystoreData | null = null;
let encryptedBlob: EncryptedBlob | null = null;

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

export async function addWallet(name: string, secretKey: string, address: string): Promise<void> {
  if (!keystoreData) throw new Error('Keystore not unlocked');
  keystoreData.wallets.push({ name, secret_key: secretKey, address });
  await save();
  notify();
}

export async function removeWallet(index: number): Promise<void> {
  if (!keystoreData) throw new Error('Keystore not unlocked');
  keystoreData.wallets.splice(index, 1);
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
