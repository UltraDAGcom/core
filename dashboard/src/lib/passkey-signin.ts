/**
 * Passkey sign-in flow for users who already have a passkey registered
 * (e.g., synced via iCloud Keychain / Google Password Manager) but don't
 * have the passkey wallet info in this browser's localStorage.
 *
 * Flow:
 *   1. User enters their @name (or udag1… bech32 address).
 *   2. We resolve name → address via GET /name/resolve/:name.
 *   3. We fetch GET /smart-account/:addr → list of authorized P256 pubkeys.
 *   4. We call navigator.credentials.get() with discoverable credentials so
 *      the browser/OS surfaces every passkey for this RP. User picks one and
 *      authenticates with biometrics.
 *   5. The assertion response contains `credential.id` (credential ID) and a
 *      P256 signature over `authenticatorData || sha256(clientDataJSON)`.
 *   6. We verify the signature against each authorized pubkey using WebCrypto.
 *      Whichever pubkey verifies is the one matching the selected passkey.
 *   7. We reconstruct a PasskeyWallet from the matching pubkey + credential ID
 *      and save it to localStorage.
 *
 * No protocol change required — the /smart-account RPC was extended to include
 * the raw pubkey (which is already public on-chain state).
 */

import { getNodeUrl } from './api';
import { savePasskeyWallet, type PasskeyWallet } from './passkey-wallet';
import { blake3 } from '@noble/hashes/blake3.js';

// ═════════════════════════════════════════════════════════════════════════
// P-256 point decompression (pure BigInt, no external deps)
// ═════════════════════════════════════════════════════════════════════════

/** NIST P-256 / secp256r1 prime `p`. */
const P256_P = 0xffffffff00000001000000000000000000000000ffffffffffffffffffffffffn;
/** NIST P-256 curve parameter `a` = -3 mod p. */
const P256_A = 0xffffffff00000001000000000000000000000000fffffffffffffffffffffffcn;
/** NIST P-256 curve parameter `b`. */
const P256_B = 0x5ac635d8aa3a93e7b3ebbd55769886bc651d06b0cc53b0f63bce3c3e27d2604bn;

/** Modular exponentiation: `(base ** exp) mod m`. */
function modPow(base: bigint, exp: bigint, m: bigint): bigint {
  let result = 1n;
  base = ((base % m) + m) % m;
  while (exp > 0n) {
    if (exp & 1n) result = (result * base) % m;
    exp >>= 1n;
    base = (base * base) % m;
  }
  return result;
}

/**
 * Decompress a 33-byte P-256 compressed point (`0x02/0x03 || x`) into a
 * 65-byte uncompressed point (`0x04 || x || y`).
 *
 * For P-256 the prime `p ≡ 3 (mod 4)`, so `sqrt(v) = v^((p+1)/4) mod p`.
 */
function decompressP256(compressed: Uint8Array): Uint8Array {
  if (compressed.length !== 33 || (compressed[0] !== 0x02 && compressed[0] !== 0x03)) {
    throw new Error(`invalid compressed P-256 point (len=${compressed.length}, tag=0x${compressed[0]?.toString(16)})`);
  }
  const yOdd = compressed[0] === 0x03;

  // Parse x as a big-endian 256-bit integer.
  let x = 0n;
  for (let i = 1; i < 33; i++) {
    x = (x << 8n) | BigInt(compressed[i]);
  }
  if (x >= P256_P) throw new Error('x coordinate out of range');

  // Compute y² = x³ + a·x + b mod p.
  const x2 = (x * x) % P256_P;
  const x3 = (x2 * x) % P256_P;
  const ax = (P256_A * x) % P256_P;
  const y2 = (((x3 + ax) % P256_P) + P256_B) % P256_P;

  // y = y²^((p+1)/4) mod p  (since p ≡ 3 mod 4).
  const exp = (P256_P + 1n) / 4n;
  let y = modPow(y2, exp, P256_P);

  // Verify: y² should equal our computed y2. If not, the point isn't on the curve.
  if ((y * y) % P256_P !== y2) {
    throw new Error('point not on P-256 curve (sqrt failed)');
  }

  // Pick the correct root based on the parity bit from the compressed prefix.
  if ((y & 1n) !== (yOdd ? 1n : 0n)) {
    y = P256_P - y;
  }

  // Encode as uncompressed: 0x04 || x(32) || y(32).
  const out = new Uint8Array(65);
  out[0] = 0x04;
  for (let i = 0; i < 32; i++) {
    out[1 + i] = Number((x >> BigInt(8 * (31 - i))) & 0xffn);
    out[33 + i] = Number((y >> BigInt(8 * (31 - i))) & 0xffn);
  }
  return out;
}

/**
 * Wrap a 65-byte uncompressed P-256 point (`0x04 || x || y`) in the
 * SubjectPublicKeyInfo DER prefix so it can be imported by
 * `crypto.subtle.importKey('spki', …, { name: 'ECDSA', namedCurve: 'P-256' })`.
 *
 * The prefix is fixed for all P-256 keys and encodes:
 *   SEQUENCE {
 *     SEQUENCE {
 *       OID 1.2.840.10045.2.1 ecPublicKey
 *       OID 1.2.840.10045.3.1.7 prime256v1
 *     }
 *     BIT STRING { 0x00 || <uncompressed point> }
 *   }
 */
function uncompressedToSpki(uncompressed: Uint8Array): Uint8Array {
  if (uncompressed.length !== 65 || uncompressed[0] !== 0x04) {
    throw new Error('expected 65-byte uncompressed P-256 point');
  }
  const prefix = new Uint8Array([
    0x30, 0x59, // SEQUENCE, 89 bytes
    0x30, 0x13, // SEQUENCE, 19 bytes (AlgorithmIdentifier)
    0x06, 0x07, 0x2a, 0x86, 0x48, 0xce, 0x3d, 0x02, 0x01, // OID ecPublicKey
    0x06, 0x08, 0x2a, 0x86, 0x48, 0xce, 0x3d, 0x03, 0x01, 0x07, // OID prime256v1
    0x03, 0x42, 0x00, // BIT STRING, 66 bytes, 0 unused bits
  ]);
  const out = new Uint8Array(prefix.length + 65);
  out.set(prefix);
  out.set(uncompressed, prefix.length);
  return out;
}

// ═════════════════════════════════════════════════════════════════════════
// WebAuthn signature verification (client-side, pure WebCrypto)
// ═════════════════════════════════════════════════════════════════════════

/**
 * Verify a WebAuthn P-256 assertion against a single candidate public key.
 *
 * WebAuthn signs `authenticatorData || sha256(clientDataJSON)` using ES256
 * (ECDSA P-256 with SHA-256). The signature on the wire is ASN.1 DER-encoded;
 * WebCrypto expects raw r||s, so we convert first.
 *
 * Returns `true` if the pubkey verifies this assertion.
 */
async function verifyAssertionAgainstPubkey(
  compressedPubkeyHex: string,
  authenticatorData: Uint8Array,
  clientDataJSON: Uint8Array,
  derSignature: Uint8Array,
): Promise<boolean> {
  let cryptoKey: CryptoKey;
  try {
    const compressed = hexToBytes(compressedPubkeyHex);
    const uncompressed = decompressP256(compressed);
    const spki = uncompressedToSpki(uncompressed);
    cryptoKey = await crypto.subtle.importKey(
      'spki',
      spki.buffer.slice(spki.byteOffset, spki.byteOffset + spki.byteLength) as ArrayBuffer,
      { name: 'ECDSA', namedCurve: 'P-256' },
      false,
      ['verify'],
    );
  } catch (e) {
    console.warn('[passkey-signin] failed to import pubkey', compressedPubkeyHex.slice(0, 16), e);
    return false;
  }

  // Build the signed data blob.
  const clientDataHashBuf = await crypto.subtle.digest(
    'SHA-256',
    clientDataJSON.buffer.slice(clientDataJSON.byteOffset, clientDataJSON.byteOffset + clientDataJSON.byteLength) as ArrayBuffer,
  );
  const clientDataHash = new Uint8Array(clientDataHashBuf);
  const signedData = new Uint8Array(authenticatorData.length + clientDataHash.length);
  signedData.set(authenticatorData);
  signedData.set(clientDataHash, authenticatorData.length);

  const rawSig = derToRaw(derSignature);

  try {
    return await crypto.subtle.verify(
      { name: 'ECDSA', hash: 'SHA-256' },
      cryptoKey,
      rawSig.buffer.slice(rawSig.byteOffset, rawSig.byteOffset + rawSig.byteLength) as ArrayBuffer,
      signedData.buffer.slice(signedData.byteOffset, signedData.byteOffset + signedData.byteLength) as ArrayBuffer,
    );
  } catch {
    return false;
  }
}

// ═════════════════════════════════════════════════════════════════════════
// Sign-in flow
// ═════════════════════════════════════════════════════════════════════════

export interface SignInResult {
  wallet: PasskeyWallet;
}

/**
 * One of the fields returned by /smart-account/:addr for each authorized key.
 */
interface SmartAccountKey {
  key_id: string;       // 16-hex (8 bytes)
  key_type: 'p256' | 'ed25519';
  pubkey: string;       // 66-hex (33 bytes compressed) for P-256
  label: string;
  daily_limit?: number | null;
}

interface SmartAccountResponse {
  address: string;
  authorized_keys: SmartAccountKey[];
  [k: string]: unknown;
}

/**
 * Sign in with an existing passkey. The user provides a name or address so we
 * can look up which SmartAccount to authenticate against.
 *
 * Throws a descriptive error on failure; returns a wallet ready to be saved.
 */
export async function signInWithPasskey(
  nameOrAddress: string,
): Promise<SignInResult> {
  if (typeof window === 'undefined' || !window.PublicKeyCredential) {
    throw new Error('WebAuthn is not available in this browser.');
  }

  const nodeUrl = getNodeUrl();

  // ─── Step 1: resolve name or address ──────────────────────────────────
  let address: string;
  let resolvedName: string | null = null;

  const cleaned = nameOrAddress.trim().replace(/^@/, '');
  if (cleaned.length === 0) {
    throw new Error('Please enter your @name or address.');
  }

  const looksLikeBech32 = /^(udag1|tudg1)/i.test(cleaned);
  const looksLikeHex = /^[0-9a-fA-F]{40}$/.test(cleaned);

  if (looksLikeBech32 || looksLikeHex) {
    // Use as-is — the /smart-account endpoint accepts both.
    address = cleaned;
  } else {
    // Treat as a name; resolve it.
    const res = await fetch(`${nodeUrl}/name/resolve/${encodeURIComponent(cleaned)}`);
    if (res.status === 404) {
      throw new Error(`No account found for @${cleaned}.`);
    }
    if (!res.ok) {
      throw new Error(`Name resolution failed (${res.status}).`);
    }
    const data = await res.json() as { address?: string };
    if (!data.address) throw new Error('Name resolver returned no address.');
    address = data.address;
    resolvedName = cleaned;
  }

  // ─── Step 2: fetch the SmartAccount's authorized keys ─────────────────
  const acctRes = await fetch(`${nodeUrl}/smart-account/${encodeURIComponent(address)}`);
  if (acctRes.status === 404) {
    throw new Error(
      `No SmartAccount found for this account. If you registered with a passkey, the account may not exist on this network yet.`,
    );
  }
  if (!acctRes.ok) {
    throw new Error(`SmartAccount lookup failed (${acctRes.status}).`);
  }
  const acct = (await acctRes.json()) as SmartAccountResponse;

  const p256Keys = (acct.authorized_keys ?? []).filter(k => k.key_type === 'p256');
  if (p256Keys.length === 0) {
    throw new Error('This account has no passkey (P-256) authorized keys.');
  }
  // Defensive: the node may be running an older build without the `pubkey` field.
  if (!p256Keys[0].pubkey) {
    throw new Error(
      'This node is running an older build that does not expose authorized key pubkeys. Try a different node or wait for the rolling upgrade.',
    );
  }

  // ─── Step 3: WebAuthn get (discoverable credentials) ──────────────────
  const challenge = crypto.getRandomValues(new Uint8Array(32));
  let credential: PublicKeyCredential | null;
  try {
    credential = (await navigator.credentials.get({
      publicKey: {
        challenge: challenge.buffer.slice(challenge.byteOffset, challenge.byteOffset + challenge.byteLength) as ArrayBuffer,
        rpId: window.location.hostname,
        // No allowCredentials → browser surfaces discoverable credentials for this RP.
        userVerification: 'required',
        timeout: 60000,
      },
    })) as PublicKeyCredential | null;
  } catch (e: unknown) {
    const msg = e instanceof Error ? e.message : String(e);
    if (msg.includes('NotAllowed')) {
      throw new Error('Biometric verification was cancelled.');
    }
    throw new Error(`Passkey selection failed: ${msg}`);
  }

  if (!credential) {
    throw new Error('No passkey was selected.');
  }

  const assertion = credential.response as AuthenticatorAssertionResponse;
  const authenticatorData = new Uint8Array(assertion.authenticatorData);
  const clientDataJSON = new Uint8Array(assertion.clientDataJSON);
  const derSignature = new Uint8Array(assertion.signature);

  // Sanity-check that the challenge in clientDataJSON matches the one we sent.
  // The browser echoes it back as base64url; a mismatch would mean something
  // fishy is going on.
  try {
    const cd = JSON.parse(new TextDecoder().decode(clientDataJSON)) as { challenge?: string; type?: string };
    if (cd.type !== 'webauthn.get') {
      throw new Error(`unexpected clientData type: ${cd.type}`);
    }
    if (!cd.challenge) throw new Error('clientData has no challenge');
    const echoed = base64urlToBytes(cd.challenge);
    if (!bytesEqual(echoed, challenge)) {
      throw new Error('challenge echo mismatch');
    }
  } catch (e) {
    throw new Error(`Passkey response validation failed: ${e instanceof Error ? e.message : String(e)}`);
  }

  // ─── Step 4: verify the signature against each authorized pubkey ──────
  let matchedPubkeyHex: string | null = null;
  for (const k of p256Keys) {
    const ok = await verifyAssertionAgainstPubkey(
      k.pubkey,
      authenticatorData,
      clientDataJSON,
      derSignature,
    );
    if (ok) {
      matchedPubkeyHex = k.pubkey;
      break;
    }
  }

  if (!matchedPubkeyHex) {
    throw new Error(
      `The passkey you selected is not authorized on this account. ` +
      `Make sure you're using the same passkey you registered with, or check that you entered the right ${resolvedName ? 'name' : 'address'}.`,
    );
  }

  // ─── Step 5: compute keyId and assemble the PasskeyWallet ─────────────
  // keyId = blake3(key_type_byte || pubkey)[..8], where key_type_byte = 1 for P256.
  // Mirrors AuthorizedKey::compute_key_id in crates/ultradag-coin/src/tx/smart_account.rs.
  const pubkeyBytes = hexToBytes(matchedPubkeyHex);
  const keyIdInput = new Uint8Array(1 + pubkeyBytes.length);
  keyIdInput[0] = 0x01; // KeyType::P256
  keyIdInput.set(pubkeyBytes, 1);
  const keyIdBytes = blake3(keyIdInput).slice(0, 8);
  const keyIdHex = Array.from(keyIdBytes).map(b => b.toString(16).padStart(2, '0')).join('');

  // Address: prefer the one returned by the node (it's the canonical 40-hex form).
  // Fall back to whatever the user typed if the node omitted it.
  const hexAddress = acct.address || address;

  // Opportunistic name fetch: if the user signed in by address, ask /balance
  // for the reverse-resolved name (BalanceResponse already includes one).
  // We don't block on this — if anything goes wrong the wallet still works.
  let displayName: string | null = resolvedName;
  if (!displayName) {
    try {
      const r = await fetch(`${nodeUrl}/balance/${encodeURIComponent(hexAddress)}`);
      if (r.ok) {
        const d = (await r.json()) as { name?: string | null };
        displayName = d.name ?? null;
      }
    } catch { /* ignore — name is optional */ }
  }

  const wallet: PasskeyWallet = {
    credentialId: credential.id, // base64url string
    p256PubkeyHex: matchedPubkeyHex,
    address: hexAddress,
    keyId: keyIdHex,
    name: displayName,
  };

  savePasskeyWallet(wallet);
  return { wallet };
}

// ═════════════════════════════════════════════════════════════════════════
// Helpers (local copies to avoid a circular import with webauthn-sign.ts)
// ═════════════════════════════════════════════════════════════════════════

function hexToBytes(hex: string): Uint8Array {
  const clean = hex.startsWith('0x') ? hex.slice(2) : hex;
  const bytes = new Uint8Array(clean.length / 2);
  for (let i = 0; i < clean.length; i += 2) {
    bytes[i / 2] = parseInt(clean.substring(i, i + 2), 16);
  }
  return bytes;
}

function base64urlToBytes(b64url: string): Uint8Array {
  const b64 = b64url.replace(/-/g, '+').replace(/_/g, '/');
  const padded = b64 + '='.repeat((4 - (b64.length % 4)) % 4);
  const binary = atob(padded);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
  return bytes;
}

function bytesEqual(a: Uint8Array, b: Uint8Array): boolean {
  if (a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) if (a[i] !== b[i]) return false;
  return true;
}

/** Convert ECDSA DER signature to raw r||s (64 bytes for P-256). */
function derToRaw(der: Uint8Array): Uint8Array {
  // If already the expected 64 bytes, pass through.
  if (der.length === 64) return der;
  if (der[0] !== 0x30) return der;

  let offset = 2; // skip SEQUENCE + length
  if (der[1] & 0x80) offset++; // long-form length (unlikely for P-256)

  if (der[offset] !== 0x02) return der;
  offset++;
  const rLen = der[offset++];
  let r = der.slice(offset, offset + rLen);
  offset += rLen;

  if (der[offset] !== 0x02) return der;
  offset++;
  const sLen = der[offset++];
  let s = der.slice(offset, offset + sLen);

  // Strip DER's leading zero pad (used to make r/s positive as signed ints).
  if (r.length === 33 && r[0] === 0) r = r.slice(1);
  if (s.length === 33 && s[0] === 0) s = s.slice(1);

  const raw = new Uint8Array(64);
  raw.set(r, 32 - r.length);
  raw.set(s, 64 - s.length);
  return raw;
}
