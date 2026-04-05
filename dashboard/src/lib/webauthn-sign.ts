/**
 * WebAuthn transaction signing for SmartAccount passkey wallets.
 *
 * Uses navigator.credentials.get() to sign a transaction challenge
 * with the device's secure enclave (Face ID, fingerprint, etc.).
 */

import { getNodeUrl } from './api';
import { getPasskeyWallet, hasPasskeyWallet } from './passkey-wallet';
import type { PasskeyWallet } from './passkey-wallet';

/** @deprecated Use getPasskeyWallet() from passkey-wallet.ts instead. */
export function getPasskeyInfo(): PasskeyWallet | null {
  return getPasskeyWallet();
}

/** @deprecated Use hasPasskeyWallet() from passkey-wallet.ts instead. */
export function isPasskeyWallet(): boolean {
  return hasPasskeyWallet();
}

/**
 * Sign and submit a SmartTransfer using WebAuthn (passkey).
 *
 * Flow:
 * 1. Build signable_bytes for SmartTransferTx
 * 2. Compute challenge = SHA-256(signable_bytes)
 * 3. Call navigator.credentials.get() with challenge
 * 4. Extract authenticatorData, clientDataJSON, P256 signature
 * 5. Submit SmartTransferTx with WebAuthn envelope to /tx/submit
 */
export async function signAndSubmitWithPasskey(
  to: string,
  amountSats: number,
  feeSats: number,
  nonce: number,
  memo?: string,
): Promise<{ tx_hash: string }> {
  const passkey = getPasskeyWallet();
  if (!passkey) throw new Error('No passkey wallet found');

  // Build signable_bytes matching Rust SmartTransferTx::signable_bytes()
  const networkStr = localStorage.getItem('ultradag_network') === 'mainnet' ? 'ultradag-mainnet-v1' : 'ultradag-testnet-v1';
  const NETWORK_ID = new TextEncoder().encode(networkStr);
  const TYPE_TAG = new TextEncoder().encode('smart_transfer');

  const fromBytesRaw = hexToBytes(passkey.address);
  // Address is always 20 bytes — truncate if stored as 32 bytes (full hash)
  const fromBytes = fromBytesRaw.length > 20 ? fromBytesRaw.slice(0, 20) : fromBytesRaw;
  const toBytes = hexToBytes(to);
  const keyIdBytes = hexToBytes(passkey.keyId);

  const parts: Uint8Array[] = [
    NETWORK_ID,
    TYPE_TAG,
    fromBytes,
    toBytes,
    u64ToLE(BigInt(amountSats)),
    u64ToLE(BigInt(feeSats)),
    u64ToLE(BigInt(nonce)),
    keyIdBytes,
  ];

  if (memo) {
    const memoBytes = new TextEncoder().encode(memo);
    parts.push(u32ToLE(memoBytes.length), memoBytes);
  }

  const signableBytes = concat(parts);

  // Compute challenge = SHA-256(signable_bytes)
  const challengeBuffer = await crypto.subtle.digest('SHA-256', signableBytes.buffer as ArrayBuffer);
  const challenge = new Uint8Array(challengeBuffer);

  // Call WebAuthn to sign
  const credential = await navigator.credentials.get({
    publicKey: {
      challenge: challenge.buffer as ArrayBuffer,
      rpId: window.location.hostname,
      allowCredentials: [{
        id: base64urlToBytes(passkey.credentialId).buffer as ArrayBuffer,
        type: 'public-key',
      }],
      userVerification: 'required',
      timeout: 60000,
    },
  }) as PublicKeyCredential | null;

  if (!credential) throw new Error('WebAuthn signing cancelled');

  const assertion = credential.response as AuthenticatorAssertionResponse;
  const authenticatorData = new Uint8Array(assertion.authenticatorData);
  const clientDataJSON = new Uint8Array(assertion.clientDataJSON);
  const signature = new Uint8Array(assertion.signature);

  // Convert DER signature to raw r||s (64 bytes) if needed
  const rawSig = derToRaw(signature);

  // Build the SmartTransferTx with WebAuthn envelope
  const tx = {
    SmartTransfer: {
      from: Array.from(fromBytes),
      to: Array.from(toBytes),
      amount: amountSats,
      fee: feeSats,
      nonce,
      signing_key_id: Array.from(keyIdBytes),
      signature: [],
      memo: memo ? Array.from(new TextEncoder().encode(memo)) : null,
      webauthn: {
        authenticator_data: Array.from(authenticatorData),
        client_data_json: Array.from(clientDataJSON),
        signature: Array.from(rawSig),
      },
    },
  };

  // Submit
  const res = await fetch(`${getNodeUrl()}/tx/submit`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(tx),
    signal: AbortSignal.timeout(10000),
  });

  if (!res.ok) {
    const err = await res.json().catch(() => ({ error: 'Unknown error' }));
    throw new Error(err.error || `Server error: ${res.status}`);
  }

  return res.json();
}

/**
 * Sign and submit a SmartOp (stake, delegate, vote, register name, etc.) via WebAuthn.
 * This enables passkey wallets to do everything, not just transfers.
 */
/**
 * Convert operation object for JSON serialization.
 * Rust's serde expects Address as [u8; 20] (byte array), not hex string.
 * Also converts [u8; 32] fields (like stream_id) to byte arrays.
 */
/**
 * Convert a hex address to exactly 20 bytes (Address size in Rust).
 * If 32 bytes given (full blake3 hash), truncate to first 20 bytes.
 * If 20 bytes, use as-is.
 */
function hexToAddress(hex: string): number[] {
  // Strip 0x prefix if present
  let clean = hex.startsWith('0x') ? hex.slice(2) : hex;
  // Pad to even length
  if (clean.length % 2 !== 0) clean = '0' + clean;
  const bytes = hexToBytes(clean);
  // Always return exactly 20 bytes (Address size in Rust)
  if (bytes.length >= 20) return Array.from(bytes.slice(0, 20));
  // Pad short addresses with leading zeros
  const padded = new Uint8Array(20);
  padded.set(bytes, 20 - bytes.length);
  return Array.from(padded);
}

function serializeOperation(op: Record<string, unknown>): Record<string, unknown> {
  if ('Stake' in op) return { Stake: op.Stake };
  if ('Unstake' in op) return { Unstake: op.Unstake ?? {} };
  if ('Delegate' in op) {
    const d = op.Delegate as { validator: string; amount: number };
    return { Delegate: { validator: hexToAddress(d.validator), amount: d.amount } };
  }
  if ('Undelegate' in op) return { Undelegate: op.Undelegate ?? {} };
  if ('Vote' in op) return { Vote: op.Vote };
  if ('RegisterName' in op) return { RegisterName: op.RegisterName };
  if ('StreamCreate' in op) {
    const s = op.StreamCreate as { recipient: string; rate_sats_per_round: number; deposit: number; cliff_rounds?: number };
    return { StreamCreate: { recipient: hexToAddress(s.recipient), rate_sats_per_round: s.rate_sats_per_round, deposit: s.deposit, cliff_rounds: s.cliff_rounds ?? 0 } };
  }
  if ('StreamWithdraw' in op) {
    const s = op.StreamWithdraw as { stream_id: string };
    return { StreamWithdraw: { stream_id: Array.from(hexToBytes(s.stream_id)) } };
  }
  if ('StreamCancel' in op) {
    const s = op.StreamCancel as { stream_id: string };
    return { StreamCancel: { stream_id: Array.from(hexToBytes(s.stream_id)) } };
  }
  if ('AddKey' in op) {
    const a = op.AddKey as { key_type: 'p256' | 'ed25519'; pubkey: string; label: string };
    return {
      AddKey: {
        // Rust enum variants serialize with their exact identifier.
        key_type: a.key_type === 'p256' ? 'P256' : 'Ed25519',
        pubkey: Array.from(hexToBytes(a.pubkey)),
        label: a.label,
      },
    };
  }
  return op;
}

export async function signAndSubmitSmartOp(
  operation: Record<string, unknown>,
  fee: number,
  nonce: number,
  /**
   * Optional wallet override. Pass this when the wallet hasn't been saved
   * to localStorage yet (e.g., during onboarding before the "Open Wallet"
   * handoff). Defaults to getPasskeyWallet() for normal post-onboarding use.
   */
  walletOverride?: PasskeyWallet,
): Promise<{ tx_hash: string }> {
  const passkey = walletOverride ?? getPasskeyWallet();
  if (!passkey) throw new Error('No passkey wallet found');

  const networkStr = localStorage.getItem('ultradag_network') === 'mainnet' ? 'ultradag-mainnet-v1' : 'ultradag-testnet-v1';
  const NETWORK_ID = new TextEncoder().encode(networkStr);
  const fromBytesRaw = hexToBytes(passkey.address);
  // Address is always 20 bytes — truncate if stored as 32 bytes (full hash)
  const fromBytes = fromBytesRaw.length > 20 ? fromBytesRaw.slice(0, 20) : fromBytesRaw;
  const keyIdBytes = hexToBytes(passkey.keyId);

  // Build signable_bytes for SmartOpTx
  const parts: Uint8Array[] = [
    NETWORK_ID,
    new TextEncoder().encode('smart_op'),
    fromBytes,
  ];

  // Serialize operation type (must match Rust SmartOpTx::signable_bytes exactly)
  if ('Stake' in operation) {
    const op = operation.Stake as { amount: number };
    parts.push(new Uint8Array([0]));
    parts.push(u64ToLE(BigInt(op.amount)));
  } else if ('Unstake' in operation) {
    parts.push(new Uint8Array([1]));
  } else if ('Delegate' in operation) {
    const op = operation.Delegate as { validator: string; amount: number };
    parts.push(new Uint8Array([2]));
    parts.push(hexToBytes(op.validator));
    parts.push(u64ToLE(BigInt(op.amount)));
  } else if ('Undelegate' in operation) {
    parts.push(new Uint8Array([3]));
  } else if ('Vote' in operation) {
    const op = operation.Vote as { proposal_id: number; approve: boolean };
    parts.push(new Uint8Array([6]));
    parts.push(u64ToLE(BigInt(op.proposal_id)));
    parts.push(new Uint8Array([op.approve ? 1 : 0]));
  } else if ('RegisterName' in operation) {
    const op = operation.RegisterName as { name: string; duration_years: number };
    const nameBytes = new TextEncoder().encode(op.name);
    parts.push(new Uint8Array([7]));
    parts.push(u32ToLE(nameBytes.length));
    parts.push(nameBytes);
    parts.push(new Uint8Array([op.duration_years]));
  } else if ('StreamCreate' in operation) {
    const op = operation.StreamCreate as { recipient: string; rate_sats_per_round: number; deposit: number; cliff_rounds?: number };
    parts.push(new Uint8Array([10])); // discriminant 10
    parts.push(new Uint8Array(hexToAddress(op.recipient)));
    parts.push(u64ToLE(BigInt(op.rate_sats_per_round)));
    parts.push(u64ToLE(BigInt(op.deposit)));
    parts.push(u64ToLE(BigInt(op.cliff_rounds ?? 0)));
  } else if ('StreamWithdraw' in operation) {
    const op = operation.StreamWithdraw as { stream_id: string };
    parts.push(new Uint8Array([11])); // discriminant 11
    parts.push(hexToBytes(op.stream_id));
  } else if ('StreamCancel' in operation) {
    const op = operation.StreamCancel as { stream_id: string };
    parts.push(new Uint8Array([12])); // discriminant 12
    parts.push(hexToBytes(op.stream_id));
  } else if ('AddKey' in operation) {
    // Byte-exact match to Rust SmartOpType::AddKey signable_bytes encoding.
    // Covered by test_add_key_signable_bytes_stable_encoding in Rust —
    // any change here must update that test.
    const op = operation.AddKey as { key_type: 'p256' | 'ed25519'; pubkey: string; label: string };
    const pubkeyBytes = hexToBytes(op.pubkey);
    const labelBytes = new TextEncoder().encode(op.label);
    parts.push(new Uint8Array([13])); // discriminant 13
    parts.push(new Uint8Array([op.key_type === 'p256' ? 1 : 0])); // KeyType: Ed25519=0, P256=1
    parts.push(u32ToLE(pubkeyBytes.length));
    parts.push(pubkeyBytes);
    parts.push(u32ToLE(labelBytes.length));
    parts.push(labelBytes);
  } else {
    throw new Error('Unsupported SmartOp type');
  }

  parts.push(u64ToLE(BigInt(fee)));
  parts.push(u64ToLE(BigInt(nonce)));
  parts.push(keyIdBytes);

  const signableBytes = concat(parts);

  // WebAuthn sign
  const challengeBuffer = await crypto.subtle.digest('SHA-256', signableBytes.buffer as ArrayBuffer);
  const challenge = new Uint8Array(challengeBuffer);

  const credential = await navigator.credentials.get({
    publicKey: {
      challenge: challenge.buffer as ArrayBuffer,
      rpId: window.location.hostname,
      userVerification: 'required',
      timeout: 60000,
    },
  }) as PublicKeyCredential | null;

  if (!credential) throw new Error('WebAuthn signing cancelled');

  const assertion = credential.response as AuthenticatorAssertionResponse;
  const authenticatorData = new Uint8Array(assertion.authenticatorData);
  const clientDataJSON = new Uint8Array(assertion.clientDataJSON);
  const signature = new Uint8Array(assertion.signature);
  const rawSig = derToRaw(signature);

  // Convert operation for JSON serialization — Address fields need byte arrays, not hex strings
  const jsonOperation = serializeOperation(operation);

  const tx = {
    SmartOp: {
      from: Array.from(fromBytes),
      operation: jsonOperation,
      fee,
      nonce,
      signing_key_id: Array.from(keyIdBytes),
      signature: [],
      webauthn: {
        authenticator_data: Array.from(authenticatorData),
        client_data_json: Array.from(clientDataJSON),
        signature: Array.from(rawSig),
      },
      // Include P256 pubkey for auto-registration on first SmartOp
      p256_pubkey: Array.from(hexToBytes(passkey.p256PubkeyHex)),
    },
  };

  const res = await fetch(`${getNodeUrl()}/tx/submit`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(tx),
    signal: AbortSignal.timeout(10000),
  });

  if (!res.ok) {
    const err = await res.json().catch(() => ({ error: 'Unknown error' }));
    throw new Error(err.error || `Server error: ${res.status}`);
  }

  return res.json();
}

// ── Helpers ──────────────────────────────────────────────────────────────

function hexToBytes(hex: string): Uint8Array {
  const bytes = new Uint8Array(hex.length / 2);
  for (let i = 0; i < hex.length; i += 2) {
    bytes[i / 2] = parseInt(hex.substring(i, i + 2), 16);
  }
  return bytes;
}

function u64ToLE(value: bigint): Uint8Array {
  const buf = new Uint8Array(8);
  let v = BigInt.asUintN(64, value);
  for (let i = 0; i < 8; i++) {
    buf[i] = Number(v & 0xffn);
    v >>= 8n;
  }
  return buf;
}

function u32ToLE(value: number): Uint8Array {
  const buf = new Uint8Array(4);
  buf[0] = value & 0xff;
  buf[1] = (value >> 8) & 0xff;
  buf[2] = (value >> 16) & 0xff;
  buf[3] = (value >> 24) & 0xff;
  return buf;
}

function concat(arrays: Uint8Array[]): Uint8Array {
  const total = arrays.reduce((acc, a) => acc + a.length, 0);
  const result = new Uint8Array(total);
  let offset = 0;
  for (const a of arrays) {
    result.set(a, offset);
    offset += a.length;
  }
  return result;
}

function base64urlToBytes(b64url: string): Uint8Array {
  const b64 = b64url.replace(/-/g, '+').replace(/_/g, '/');
  const padded = b64 + '='.repeat((4 - b64.length % 4) % 4);
  const binary = atob(padded);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
  return bytes;
}

/** Convert ECDSA DER signature to raw r||s (64 bytes). */
function derToRaw(der: Uint8Array): Uint8Array {
  // If already 64 bytes, assume raw
  if (der.length === 64) return der;

  // DER format: 0x30 [len] 0x02 [r-len] [r] 0x02 [s-len] [s]
  if (der[0] !== 0x30) return der;

  let offset = 2; // skip 0x30 + length byte
  if (der[1] & 0x80) offset++; // long form length

  // Read r
  if (der[offset] !== 0x02) return der;
  offset++;
  const rLen = der[offset++];
  let r = der.slice(offset, offset + rLen);
  offset += rLen;

  // Read s
  if (der[offset] !== 0x02) return der;
  offset++;
  const sLen = der[offset++];
  let s = der.slice(offset, offset + sLen);

  // Remove leading zero padding (DER uses signed integers)
  if (r.length === 33 && r[0] === 0) r = r.slice(1);
  if (s.length === 33 && s[0] === 0) s = s.slice(1);

  // Pad to 32 bytes each
  const raw = new Uint8Array(64);
  raw.set(r, 32 - r.length);
  raw.set(s, 64 - s.length);
  return raw;
}
