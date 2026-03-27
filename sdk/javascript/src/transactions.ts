/**
 * Client-side transaction signing for UltraDAG.
 *
 * Each function constructs the exact same byte sequence as the Rust
 * `signable_bytes()` method for the corresponding transaction type.
 * These bytes are then signed with Ed25519 and the result is submitted
 * to the node via POST /tx/submit.
 *
 * IMPORTANT: The byte layouts here MUST match the Rust implementations
 * exactly, or signature verification will fail on the server.
 */

import * as ed from "@noble/ed25519";
import { createHash } from "blake3";
import { createHash as nodeCryptoHash } from "node:crypto";

// Ensure @noble/ed25519 has a sync SHA-512 hasher (same as crypto.ts).
if (!ed.etc.sha512Sync) {
  ed.etc.sha512Sync = (...msgs: Uint8Array[]): Uint8Array => {
    const h = nodeCryptoHash("sha512");
    for (const m of msgs) h.update(m);
    return new Uint8Array(h.digest());
  };
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/** Network identifier prepended to all signable bytes (testnet). */
const NETWORK_ID = new TextEncoder().encode("ultradag-testnet-v1");

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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

function bytesToHex(bytes: Uint8Array): string {
  const hex: string[] = [];
  for (const b of bytes) {
    hex.push(b.toString(16).padStart(2, "0"));
  }
  return hex.join("");
}

/** Write a u64 (bigint) as 8 bytes in little-endian order. */
function u64ToLeBytes(value: bigint): Uint8Array {
  const buf = new Uint8Array(8);
  let v = BigInt.asUintN(64, value);
  for (let i = 0; i < 8; i++) {
    buf[i] = Number(v & 0xFFn);
    v >>= 8n;
  }
  return buf;
}

/** Write a u32 (number) as 4 bytes in little-endian order. */
function u32ToLeBytes(value: number): Uint8Array {
  const buf = new Uint8Array(4);
  buf[0] = value & 0xFF;
  buf[1] = (value >>> 8) & 0xFF;
  buf[2] = (value >>> 16) & 0xFF;
  buf[3] = (value >>> 24) & 0xFF;
  return buf;
}

/** Derive the UltraDAG address (blake3 hash) from a 32-byte Ed25519 public key. */
function deriveAddressBytes(publicKey: Uint8Array): Uint8Array {
  const hash = createHash();
  hash.update(publicKey);
  return new Uint8Array(hash.digest());
}

/** Concatenate multiple Uint8Arrays into one. */
function concat(...arrays: Uint8Array[]): Uint8Array {
  let totalLen = 0;
  for (const a of arrays) totalLen += a.length;
  const result = new Uint8Array(totalLen);
  let offset = 0;
  for (const a of arrays) {
    result.set(a, offset);
    offset += a.length;
  }
  return result;
}

// ---------------------------------------------------------------------------
// Signable bytes — one function per transaction type
// ---------------------------------------------------------------------------

/**
 * Build signable bytes for a Transfer transaction.
 *
 * Layout: NETWORK_ID | "transfer" | from(32) | to(32) | amount(u64 LE) |
 *         fee(u64 LE) | nonce(u64 LE) | [memo_len(u32 LE) | memo bytes]
 */
export function transferSignableBytes(
  from: Uint8Array,
  to: Uint8Array,
  amount: bigint,
  fee: bigint,
  nonce: bigint,
  memo?: Uint8Array,
): Uint8Array {
  const parts: Uint8Array[] = [
    NETWORK_ID,
    new TextEncoder().encode("transfer"),
    from,
    to,
    u64ToLeBytes(amount),
    u64ToLeBytes(fee),
    u64ToLeBytes(nonce),
  ];
  if (memo !== undefined && memo.length > 0) {
    parts.push(u32ToLeBytes(memo.length));
    parts.push(memo);
  }
  return concat(...parts);
}

/**
 * Build signable bytes for a Stake transaction.
 *
 * Layout: NETWORK_ID | "stake" | from(32) | amount(u64 LE) | nonce(u64 LE)
 */
export function stakeSignableBytes(
  from: Uint8Array,
  amount: bigint,
  nonce: bigint,
): Uint8Array {
  return concat(
    NETWORK_ID,
    new TextEncoder().encode("stake"),
    from,
    u64ToLeBytes(amount),
    u64ToLeBytes(nonce),
  );
}

/**
 * Build signable bytes for an Unstake transaction.
 *
 * Layout: NETWORK_ID | "unstake" | from(32) | nonce(u64 LE)
 */
export function unstakeSignableBytes(
  from: Uint8Array,
  nonce: bigint,
): Uint8Array {
  return concat(
    NETWORK_ID,
    new TextEncoder().encode("unstake"),
    from,
    u64ToLeBytes(nonce),
  );
}

/**
 * Build signable bytes for a Delegate transaction.
 *
 * Layout: NETWORK_ID | "delegate" | from(32) | validator(32) | amount(u64 LE) | nonce(u64 LE)
 */
export function delegateSignableBytes(
  from: Uint8Array,
  validator: Uint8Array,
  amount: bigint,
  nonce: bigint,
): Uint8Array {
  return concat(
    NETWORK_ID,
    new TextEncoder().encode("delegate"),
    from,
    validator,
    u64ToLeBytes(amount),
    u64ToLeBytes(nonce),
  );
}

/**
 * Build signable bytes for an Undelegate transaction.
 *
 * Layout: NETWORK_ID | "undelegate" | from(32) | nonce(u64 LE)
 */
export function undelegateSignableBytes(
  from: Uint8Array,
  nonce: bigint,
): Uint8Array {
  return concat(
    NETWORK_ID,
    new TextEncoder().encode("undelegate"),
    from,
    u64ToLeBytes(nonce),
  );
}

/**
 * Build signable bytes for a SetCommission transaction.
 *
 * Layout: NETWORK_ID | "set_commission" | from(32) | commission_percent(u8) | nonce(u64 LE)
 */
export function setCommissionSignableBytes(
  from: Uint8Array,
  commissionPercent: number,
  nonce: bigint,
): Uint8Array {
  return concat(
    NETWORK_ID,
    new TextEncoder().encode("set_commission"),
    from,
    new Uint8Array([commissionPercent & 0xFF]),
    u64ToLeBytes(nonce),
  );
}

// ---------------------------------------------------------------------------
// Governance proposal types for signable bytes
// ---------------------------------------------------------------------------

export interface TextProposal {
  type: "TextProposal";
}

export interface ParameterChangeProposal {
  type: "ParameterChange";
  param: string;
  newValue: string;
}

export interface CouncilMembershipProposal {
  type: "CouncilMembership";
  action: "Add" | "Remove";
  address: Uint8Array;
  category: string; // "Technical" | "Business" | "Legal" | "Academic" | "Community" | "Foundation"
}

export interface TreasurySpendProposal {
  type: "TreasurySpend";
  recipient: Uint8Array;
  amount: bigint;
}

export type ProposalTypeInput =
  | TextProposal
  | ParameterChangeProposal
  | CouncilMembershipProposal
  | TreasurySpendProposal;

/**
 * Build signable bytes for a CreateProposal transaction.
 *
 * Layout: NETWORK_ID | "proposal" | from(32) | proposal_id(u64 LE) |
 *         title_len(u32 LE) | title | desc_len(u32 LE) | desc |
 *         proposal_type_bytes | fee(u64 LE) | nonce(u64 LE)
 */
export function createProposalSignableBytes(
  from: Uint8Array,
  proposalId: bigint,
  title: string,
  description: string,
  proposalType: ProposalTypeInput,
  fee: bigint,
  nonce: bigint,
): Uint8Array {
  const encoder = new TextEncoder();
  const titleBytes = encoder.encode(title);
  const descBytes = encoder.encode(description);

  const parts: Uint8Array[] = [
    NETWORK_ID,
    encoder.encode("proposal"),
    from,
    u64ToLeBytes(proposalId),
    u32ToLeBytes(titleBytes.length),
    titleBytes,
    u32ToLeBytes(descBytes.length),
    descBytes,
  ];

  switch (proposalType.type) {
    case "TextProposal":
      parts.push(new Uint8Array([0]));
      break;
    case "ParameterChange": {
      const paramBytes = encoder.encode(proposalType.param);
      const valueBytes = encoder.encode(proposalType.newValue);
      parts.push(new Uint8Array([1]));
      parts.push(u32ToLeBytes(paramBytes.length));
      parts.push(paramBytes);
      parts.push(u32ToLeBytes(valueBytes.length));
      parts.push(valueBytes);
      break;
    }
    case "CouncilMembership": {
      parts.push(new Uint8Array([2]));
      parts.push(new Uint8Array([proposalType.action === "Add" ? 0 : 1]));
      parts.push(proposalType.address);
      parts.push(encoder.encode(proposalType.category));
      break;
    }
    case "TreasurySpend":
      parts.push(new Uint8Array([3]));
      parts.push(proposalType.recipient);
      parts.push(u64ToLeBytes(proposalType.amount));
      break;
  }

  parts.push(u64ToLeBytes(fee));
  parts.push(u64ToLeBytes(nonce));

  return concat(...parts);
}

/**
 * Build signable bytes for a Vote transaction.
 *
 * Layout: NETWORK_ID | "vote" | from(32) | proposal_id(u64 LE) | vote(1 byte) |
 *         fee(u64 LE) | nonce(u64 LE)
 */
export function voteSignableBytes(
  from: Uint8Array,
  proposalId: bigint,
  vote: boolean,
  fee: bigint,
  nonce: bigint,
): Uint8Array {
  return concat(
    NETWORK_ID,
    new TextEncoder().encode("vote"),
    from,
    u64ToLeBytes(proposalId),
    new Uint8Array([vote ? 1 : 0]),
    u64ToLeBytes(fee),
    u64ToLeBytes(nonce),
  );
}

// ---------------------------------------------------------------------------
// Signing
// ---------------------------------------------------------------------------

/**
 * Sign a message (signable bytes) with an Ed25519 secret key.
 *
 * @param signableBytes - The exact byte sequence to sign.
 * @param secretKey - 32-byte Ed25519 secret key.
 * @returns 64-byte Ed25519 signature.
 */
export function signTransaction(
  signableBytes: Uint8Array,
  secretKey: Uint8Array,
): Uint8Array {
  return ed.sign(signableBytes, secretKey);
}

// ---------------------------------------------------------------------------
// Build full signed transaction JSON (ready for /tx/submit)
// ---------------------------------------------------------------------------

/**
 * Build a signed Transfer transaction object ready for POST /tx/submit.
 *
 * @param secretKeyHex - 64-char hex secret key.
 * @param toHex - 64-char hex recipient address.
 * @param amount - Amount in sats (bigint).
 * @param fee - Fee in sats (bigint).
 * @param nonce - Sender nonce (bigint).
 * @param memo - Optional memo bytes.
 * @returns JSON object matching the server's Transaction::Transfer serde format.
 */
export function buildSignedTransferTx(
  secretKeyHex: string,
  toHex: string,
  amount: bigint,
  fee: bigint,
  nonce: bigint,
  memo?: Uint8Array,
): object {
  const secretKey = hexToBytes(secretKeyHex);
  const publicKey = ed.getPublicKey(secretKey);
  const fromBytes = deriveAddressBytes(publicKey);
  const toBytes = hexToBytes(toHex);

  if (toBytes.length !== 32) {
    throw new Error("Recipient address must be 32 bytes (64 hex characters)");
  }

  const signable = transferSignableBytes(fromBytes, toBytes, amount, fee, nonce, memo);
  const signature = signTransaction(signable, secretKey);

  return {
    Transfer: {
      from: Array.from(fromBytes),
      to: Array.from(toBytes),
      amount: Number(amount),
      fee: Number(fee),
      nonce: Number(nonce),
      pub_key: Array.from(publicKey),
      signature: bytesToHex(signature),
      memo: memo ? Array.from(memo) : null,
    },
  };
}

/**
 * Build a signed Stake transaction object ready for POST /tx/submit.
 */
export function buildSignedStakeTx(
  secretKeyHex: string,
  amount: bigint,
  nonce: bigint,
): object {
  const secretKey = hexToBytes(secretKeyHex);
  const publicKey = ed.getPublicKey(secretKey);
  const fromBytes = deriveAddressBytes(publicKey);

  const signable = stakeSignableBytes(fromBytes, amount, nonce);
  const signature = signTransaction(signable, secretKey);

  return {
    Stake: {
      from: Array.from(fromBytes),
      amount: Number(amount),
      nonce: Number(nonce),
      pub_key: Array.from(publicKey),
      signature: bytesToHex(signature),
    },
  };
}

/**
 * Build a signed Unstake transaction object ready for POST /tx/submit.
 */
export function buildSignedUnstakeTx(
  secretKeyHex: string,
  nonce: bigint,
): object {
  const secretKey = hexToBytes(secretKeyHex);
  const publicKey = ed.getPublicKey(secretKey);
  const fromBytes = deriveAddressBytes(publicKey);

  const signable = unstakeSignableBytes(fromBytes, nonce);
  const signature = signTransaction(signable, secretKey);

  return {
    Unstake: {
      from: Array.from(fromBytes),
      nonce: Number(nonce),
      pub_key: Array.from(publicKey),
      signature: bytesToHex(signature),
    },
  };
}

/**
 * Build a signed Delegate transaction object ready for POST /tx/submit.
 */
export function buildSignedDelegateTx(
  secretKeyHex: string,
  validatorHex: string,
  amount: bigint,
  nonce: bigint,
): object {
  const secretKey = hexToBytes(secretKeyHex);
  const publicKey = ed.getPublicKey(secretKey);
  const fromBytes = deriveAddressBytes(publicKey);
  const validatorBytes = hexToBytes(validatorHex);

  if (validatorBytes.length !== 32) {
    throw new Error("Validator address must be 32 bytes (64 hex characters)");
  }

  const signable = delegateSignableBytes(fromBytes, validatorBytes, amount, nonce);
  const signature = signTransaction(signable, secretKey);

  return {
    Delegate: {
      from: Array.from(fromBytes),
      validator: Array.from(validatorBytes),
      amount: Number(amount),
      nonce: Number(nonce),
      pub_key: Array.from(publicKey),
      signature: bytesToHex(signature),
    },
  };
}

/**
 * Build a signed Undelegate transaction object ready for POST /tx/submit.
 */
export function buildSignedUndelegateTx(
  secretKeyHex: string,
  nonce: bigint,
): object {
  const secretKey = hexToBytes(secretKeyHex);
  const publicKey = ed.getPublicKey(secretKey);
  const fromBytes = deriveAddressBytes(publicKey);

  const signable = undelegateSignableBytes(fromBytes, nonce);
  const signature = signTransaction(signable, secretKey);

  return {
    Undelegate: {
      from: Array.from(fromBytes),
      nonce: Number(nonce),
      pub_key: Array.from(publicKey),
      signature: bytesToHex(signature),
    },
  };
}

/**
 * Build a signed SetCommission transaction object ready for POST /tx/submit.
 */
export function buildSignedSetCommissionTx(
  secretKeyHex: string,
  commissionPercent: number,
  nonce: bigint,
): object {
  const secretKey = hexToBytes(secretKeyHex);
  const publicKey = ed.getPublicKey(secretKey);
  const fromBytes = deriveAddressBytes(publicKey);

  const signable = setCommissionSignableBytes(fromBytes, commissionPercent, nonce);
  const signature = signTransaction(signable, secretKey);

  return {
    SetCommission: {
      from: Array.from(fromBytes),
      commission_percent: commissionPercent,
      nonce: Number(nonce),
      pub_key: Array.from(publicKey),
      signature: bytesToHex(signature),
    },
  };
}

/**
 * Build a signed CreateProposal transaction object ready for POST /tx/submit.
 */
export function buildSignedCreateProposalTx(
  secretKeyHex: string,
  proposalId: bigint,
  title: string,
  description: string,
  proposalType: ProposalTypeInput,
  fee: bigint,
  nonce: bigint,
): object {
  const secretKey = hexToBytes(secretKeyHex);
  const publicKey = ed.getPublicKey(secretKey);
  const fromBytes = deriveAddressBytes(publicKey);

  const signable = createProposalSignableBytes(
    fromBytes, proposalId, title, description, proposalType, fee, nonce,
  );
  const signature = signTransaction(signable, secretKey);

  // Build the serde-compatible proposal_type JSON
  let proposalTypeJson: unknown;
  switch (proposalType.type) {
    case "TextProposal":
      proposalTypeJson = "TextProposal";
      break;
    case "ParameterChange":
      proposalTypeJson = {
        ParameterChange: {
          param: proposalType.param,
          new_value: proposalType.newValue,
        },
      };
      break;
    case "CouncilMembership":
      proposalTypeJson = {
        CouncilMembership: {
          action: proposalType.action,
          address: Array.from(proposalType.address),
          category: proposalType.category,
        },
      };
      break;
    case "TreasurySpend":
      proposalTypeJson = {
        TreasurySpend: {
          recipient: Array.from(proposalType.recipient),
          amount: Number(proposalType.amount),
        },
      };
      break;
  }

  return {
    CreateProposal: {
      from: Array.from(fromBytes),
      proposal_id: Number(proposalId),
      title,
      description,
      proposal_type: proposalTypeJson,
      fee: Number(fee),
      nonce: Number(nonce),
      pub_key: Array.from(publicKey),
      signature: bytesToHex(signature),
    },
  };
}

/**
 * Build a signed Vote transaction object ready for POST /tx/submit.
 */
export function buildSignedVoteTx(
  secretKeyHex: string,
  proposalId: bigint,
  vote: boolean,
  fee: bigint,
  nonce: bigint,
): object {
  const secretKey = hexToBytes(secretKeyHex);
  const publicKey = ed.getPublicKey(secretKey);
  const fromBytes = deriveAddressBytes(publicKey);

  const signable = voteSignableBytes(fromBytes, proposalId, vote, fee, nonce);
  const signature = signTransaction(signable, secretKey);

  return {
    Vote: {
      from: Array.from(fromBytes),
      proposal_id: Number(proposalId),
      vote,
      fee: Number(fee),
      nonce: Number(nonce),
      pub_key: Array.from(publicKey),
      signature: bytesToHex(signature),
    },
  };
}

// ---------------------------------------------------------------------------
// SmartAccount Transaction Types
// ---------------------------------------------------------------------------

/** Compute a key_id from key type (0=Ed25519, 1=P256) and public key bytes. */
export function computeKeyId(keyType: number, pubkey: Uint8Array): Uint8Array {
  const hasher = createHash();
  hasher.update(new Uint8Array([keyType]));
  hasher.update(pubkey);
  const hash = hasher.digest();
  return new Uint8Array(hash.slice(0, 8));
}

/** Build signable bytes for AddKeyTx. */
function addKeySignableBytes(
  from: Uint8Array, keyId: Uint8Array, keyType: number,
  pubkey: Uint8Array, label: string, fee: bigint, nonce: bigint,
): Uint8Array {
  const labelBytes = new TextEncoder().encode(label);
  const parts: Uint8Array[] = [
    NETWORK_ID, new TextEncoder().encode("smart_add_key"),
    from, keyId, new Uint8Array([keyType]),
    u32ToLeBytes(pubkey.length), pubkey,
    u32ToLeBytes(labelBytes.length), labelBytes,
    u64ToLeBytes(fee), u64ToLeBytes(nonce),
  ];
  return concatBytes(parts);
}

/** Build and sign an AddKey transaction (Ed25519 signer). */
export function buildAddKeyTx(
  secretKeyHex: string, keyType: number, newPubkey: Uint8Array,
  label: string, fee: bigint, nonce: bigint,
): object {
  const secretKey = hexToBytes(secretKeyHex);
  const publicKey = ed.getPublicKey(secretKey);
  const fromBytes = deriveAddressBytes(publicKey);
  const keyId = computeKeyId(keyType, newPubkey);

  const signable = addKeySignableBytes(fromBytes, keyId, keyType, newPubkey, label, fee, nonce);
  const signature = signTransaction(signable, secretKey);

  return {
    AddKey: {
      from: Array.from(fromBytes),
      new_key: {
        key_id: Array.from(keyId),
        key_type: keyType === 0 ? "Ed25519" : "P256",
        pubkey: Array.from(newPubkey),
        label,
        daily_limit: null,
        daily_spent: [0, 0],
      },
      fee: Number(fee),
      nonce: Number(nonce),
      pub_key: Array.from(publicKey),
      signature: bytesToHex(signature),
    },
  };
}

/** Build signable bytes for SmartTransferTx. */
function smartTransferSignableBytes(
  from: Uint8Array, to: Uint8Array, amount: bigint,
  fee: bigint, nonce: bigint, signingKeyId: Uint8Array,
  memo: Uint8Array | null,
): Uint8Array {
  const parts: Uint8Array[] = [
    NETWORK_ID, new TextEncoder().encode("smart_transfer"),
    from, to, u64ToLeBytes(amount), u64ToLeBytes(fee),
    u64ToLeBytes(nonce), signingKeyId,
  ];
  if (memo && memo.length > 0) {
    parts.push(u32ToLeBytes(memo.length), memo);
  }
  return concatBytes(parts);
}

/** Build and sign a SmartTransfer transaction (Ed25519 signer). */
export function buildSmartTransferTx(
  secretKeyHex: string, toHex: string, amount: bigint,
  fee: bigint, nonce: bigint, memo?: Uint8Array,
): object {
  const secretKey = hexToBytes(secretKeyHex);
  const publicKey = ed.getPublicKey(secretKey);
  const fromBytes = deriveAddressBytes(publicKey);
  const toBytes = hexToBytes(toHex);
  const keyId = computeKeyId(0, publicKey); // Ed25519

  const signable = smartTransferSignableBytes(
    fromBytes, toBytes, amount, fee, nonce, keyId, memo ?? null,
  );
  const signature = signTransaction(signable, secretKey);

  return {
    SmartTransfer: {
      from: Array.from(fromBytes),
      to: Array.from(toBytes),
      amount: Number(amount),
      fee: Number(fee),
      nonce: Number(nonce),
      signing_key_id: Array.from(keyId),
      signature: Array.from(signature),
      memo: memo ? Array.from(memo) : null,
      webauthn: null,
    },
  };
}

/** Build signable bytes for RegisterNameTx. */
function registerNameSignableBytes(
  from: Uint8Array, name: string, durationYears: number,
  fee: bigint, nonce: bigint,
): Uint8Array {
  const nameBytes = new TextEncoder().encode(name);
  const parts: Uint8Array[] = [
    NETWORK_ID, new TextEncoder().encode("name_register"),
    from, u32ToLeBytes(nameBytes.length), nameBytes,
    new Uint8Array([durationYears]),
    u64ToLeBytes(fee), u64ToLeBytes(nonce),
  ];
  return concatBytes(parts);
}

/** Build and sign a RegisterName transaction. */
export function buildRegisterNameTx(
  secretKeyHex: string, name: string, durationYears: number,
  fee: bigint, nonce: bigint,
): object {
  const secretKey = hexToBytes(secretKeyHex);
  const publicKey = ed.getPublicKey(secretKey);
  const fromBytes = deriveAddressBytes(publicKey);

  const signable = registerNameSignableBytes(fromBytes, name, durationYears, fee, nonce);
  const signature = signTransaction(signable, secretKey);

  return {
    RegisterName: {
      from: Array.from(fromBytes),
      name,
      duration_years: durationYears,
      fee: Number(fee),
      nonce: Number(nonce),
      pub_key: Array.from(publicKey),
      signature: bytesToHex(signature),
    },
  };
}

/** Helper to concatenate multiple Uint8Arrays. */
function concatBytes(arrays: Uint8Array[]): Uint8Array {
  const totalLen = arrays.reduce((acc, a) => acc + a.length, 0);
  const result = new Uint8Array(totalLen);
  let offset = 0;
  for (const a of arrays) {
    result.set(a, offset);
    offset += a.length;
  }
  return result;
}

// ---------------------------------------------------------------------------
// Re-exports for convenience
// ---------------------------------------------------------------------------

export { hexToBytes, bytesToHex, deriveAddressBytes, u64ToLeBytes, u32ToLeBytes };
