import { describe, it, expect } from "vitest";
import * as ed from "@noble/ed25519";
import { createHash } from "blake3";
import { createHash as nodeCryptoHash } from "node:crypto";

import {
  transferSignableBytes,
  stakeSignableBytes,
  unstakeSignableBytes,
  delegateSignableBytes,
  undelegateSignableBytes,
  setCommissionSignableBytes,
  createProposalSignableBytes,
  voteSignableBytes,
  signTransaction,
  buildSignedTransferTx,
  buildSignedStakeTx,
  buildSignedUnstakeTx,
  buildSignedDelegateTx,
  buildSignedUndelegateTx,
  buildSignedSetCommissionTx,
  buildSignedCreateProposalTx,
  buildSignedVoteTx,
  hexToBytes,
  bytesToHex,
  deriveAddressBytes,
  u64ToLeBytes,
  u32ToLeBytes,
} from "../src/transactions.js";

// Ensure @noble/ed25519 has a sync SHA-512 hasher.
if (!ed.etc.sha512Sync) {
  ed.etc.sha512Sync = (...msgs: Uint8Array[]): Uint8Array => {
    const h = nodeCryptoHash("sha512");
    for (const m of msgs) h.update(m);
    return new Uint8Array(h.digest());
  };
}

// ---------------------------------------------------------------------------
// Helper: derive address bytes from a secret key
// ---------------------------------------------------------------------------

function keypairFromSeed(seed: Uint8Array): {
  secretKey: Uint8Array;
  publicKey: Uint8Array;
  address: Uint8Array;
} {
  const secretKey = seed;
  const publicKey = ed.getPublicKey(secretKey);
  const addressHash = createHash();
  addressHash.update(publicKey);
  const address = new Uint8Array(addressHash.digest());
  return { secretKey, publicKey, address };
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const NETWORK_ID = new TextEncoder().encode("ultradag-testnet-v1");

// A deterministic test keypair (same as Rust SecretKey::from_bytes([1u8; 32]))
const TEST_SEED = new Uint8Array(32).fill(0x01);
const TEST_KP = keypairFromSeed(TEST_SEED);

// Second keypair for recipient / validator
const TEST_SEED_2 = new Uint8Array(32).fill(0x02);
const TEST_KP_2 = keypairFromSeed(TEST_SEED_2);

// ---------------------------------------------------------------------------
// Utility tests
// ---------------------------------------------------------------------------

describe("u64ToLeBytes", () => {
  it("encodes 0 as 8 zero bytes", () => {
    expect(u64ToLeBytes(0n)).toEqual(new Uint8Array(8));
  });

  it("encodes 1 correctly", () => {
    const expected = new Uint8Array([1, 0, 0, 0, 0, 0, 0, 0]);
    expect(u64ToLeBytes(1n)).toEqual(expected);
  });

  it("encodes 256 correctly", () => {
    const expected = new Uint8Array([0, 1, 0, 0, 0, 0, 0, 0]);
    expect(u64ToLeBytes(256n)).toEqual(expected);
  });

  it("encodes large value correctly", () => {
    // 100_000_000 = 0x05F5E100
    const bytes = u64ToLeBytes(100_000_000n);
    expect(bytes[0]).toBe(0x00);
    expect(bytes[1]).toBe(0xE1);
    expect(bytes[2]).toBe(0xF5);
    expect(bytes[3]).toBe(0x05);
    expect(bytes[4]).toBe(0x00);
    expect(bytes[5]).toBe(0x00);
    expect(bytes[6]).toBe(0x00);
    expect(bytes[7]).toBe(0x00);
  });
});

describe("u32ToLeBytes", () => {
  it("encodes 0 as 4 zero bytes", () => {
    expect(u32ToLeBytes(0)).toEqual(new Uint8Array(4));
  });

  it("encodes 8 correctly", () => {
    const expected = new Uint8Array([8, 0, 0, 0]);
    expect(u32ToLeBytes(8)).toEqual(expected);
  });
});

describe("hexToBytes / bytesToHex", () => {
  it("round-trips correctly", () => {
    const hex = "0123456789abcdef";
    expect(bytesToHex(hexToBytes(hex))).toBe(hex);
  });

  it("handles all-zeros", () => {
    const bytes = new Uint8Array(32);
    expect(hexToBytes(bytesToHex(bytes))).toEqual(bytes);
  });
});

// ---------------------------------------------------------------------------
// Transfer signable bytes
// ---------------------------------------------------------------------------

describe("transferSignableBytes", () => {
  it("produces correct length without memo", () => {
    // NETWORK_ID(19) + "transfer"(8) + from(32) + to(32) + amount(8) + fee(8) + nonce(8) = 115
    const bytes = transferSignableBytes(
      TEST_KP.address,
      TEST_KP_2.address,
      100n,
      10n,
      0n,
    );
    expect(bytes.length).toBe(115);
  });

  it("starts with NETWORK_ID + 'transfer'", () => {
    const bytes = transferSignableBytes(
      TEST_KP.address,
      TEST_KP_2.address,
      100n,
      10n,
      0n,
    );
    const prefix = bytes.slice(0, 27); // 19 + 8
    const expected = new Uint8Array([
      ...NETWORK_ID,
      ...new TextEncoder().encode("transfer"),
    ]);
    expect(prefix).toEqual(expected);
  });

  it("includes from address at offset 27", () => {
    const bytes = transferSignableBytes(
      TEST_KP.address,
      TEST_KP_2.address,
      100n,
      10n,
      0n,
    );
    expect(bytes.slice(27, 59)).toEqual(TEST_KP.address);
  });

  it("includes to address at offset 59", () => {
    const bytes = transferSignableBytes(
      TEST_KP.address,
      TEST_KP_2.address,
      100n,
      10n,
      0n,
    );
    expect(bytes.slice(59, 91)).toEqual(TEST_KP_2.address);
  });

  it("encodes amount in little-endian at offset 91", () => {
    const bytes = transferSignableBytes(
      TEST_KP.address,
      TEST_KP_2.address,
      0x0102030405060708n,
      10n,
      0n,
    );
    expect(bytes.slice(91, 99)).toEqual(
      new Uint8Array([0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01]),
    );
  });

  it("produces correct length with memo", () => {
    const memo = new TextEncoder().encode("hello");
    // 115 + 4 (memo_len) + 5 (memo) = 124
    const bytes = transferSignableBytes(
      TEST_KP.address,
      TEST_KP_2.address,
      100n,
      10n,
      0n,
      memo,
    );
    expect(bytes.length).toBe(124);
  });

  it("encodes memo length prefix as u32 LE", () => {
    const memo = new TextEncoder().encode("hello");
    const bytes = transferSignableBytes(
      TEST_KP.address,
      TEST_KP_2.address,
      100n,
      10n,
      0n,
      memo,
    );
    // memo_len at offset 115
    expect(bytes.slice(115, 119)).toEqual(new Uint8Array([5, 0, 0, 0]));
    // memo at offset 119
    expect(bytes.slice(119, 124)).toEqual(memo);
  });

  it("is deterministic", () => {
    const a = transferSignableBytes(TEST_KP.address, TEST_KP_2.address, 100n, 10n, 0n);
    const b = transferSignableBytes(TEST_KP.address, TEST_KP_2.address, 100n, 10n, 0n);
    expect(a).toEqual(b);
  });

  it("different nonces produce different bytes", () => {
    const a = transferSignableBytes(TEST_KP.address, TEST_KP_2.address, 100n, 10n, 0n);
    const b = transferSignableBytes(TEST_KP.address, TEST_KP_2.address, 100n, 10n, 1n);
    expect(a).not.toEqual(b);
  });
});

// ---------------------------------------------------------------------------
// Stake signable bytes
// ---------------------------------------------------------------------------

describe("stakeSignableBytes", () => {
  it("produces correct length", () => {
    // NETWORK_ID(19) + "stake"(5) + from(32) + amount(8) + nonce(8) = 72
    const bytes = stakeSignableBytes(TEST_KP.address, 1000n, 0n);
    expect(bytes.length).toBe(72);
  });

  it("starts with NETWORK_ID + 'stake'", () => {
    const bytes = stakeSignableBytes(TEST_KP.address, 1000n, 0n);
    const expected = new Uint8Array([
      ...NETWORK_ID,
      ...new TextEncoder().encode("stake"),
    ]);
    expect(bytes.slice(0, 24)).toEqual(expected);
  });
});

// ---------------------------------------------------------------------------
// Unstake signable bytes
// ---------------------------------------------------------------------------

describe("unstakeSignableBytes", () => {
  it("produces correct length", () => {
    // NETWORK_ID(19) + "unstake"(7) + from(32) + nonce(8) = 66
    const bytes = unstakeSignableBytes(TEST_KP.address, 0n);
    expect(bytes.length).toBe(66);
  });
});

// ---------------------------------------------------------------------------
// Delegate signable bytes
// ---------------------------------------------------------------------------

describe("delegateSignableBytes", () => {
  it("produces correct length", () => {
    // NETWORK_ID(19) + "delegate"(8) + from(32) + validator(32) + amount(8) + nonce(8) = 107
    const bytes = delegateSignableBytes(
      TEST_KP.address,
      TEST_KP_2.address,
      1000n,
      0n,
    );
    expect(bytes.length).toBe(107);
  });
});

// ---------------------------------------------------------------------------
// Undelegate signable bytes
// ---------------------------------------------------------------------------

describe("undelegateSignableBytes", () => {
  it("produces correct length", () => {
    // NETWORK_ID(19) + "undelegate"(10) + from(32) + nonce(8) = 69
    const bytes = undelegateSignableBytes(TEST_KP.address, 0n);
    expect(bytes.length).toBe(69);
  });
});

// ---------------------------------------------------------------------------
// SetCommission signable bytes
// ---------------------------------------------------------------------------

describe("setCommissionSignableBytes", () => {
  it("produces correct length", () => {
    // NETWORK_ID(19) + "set_commission"(14) + from(32) + percent(1) + nonce(8) = 74
    const bytes = setCommissionSignableBytes(TEST_KP.address, 15, 0n);
    expect(bytes.length).toBe(74);
  });

  it("encodes commission percent as single byte", () => {
    const bytes = setCommissionSignableBytes(TEST_KP.address, 42, 0n);
    // percent at offset 19 + 14 + 32 = 65
    expect(bytes[65]).toBe(42);
  });
});

// ---------------------------------------------------------------------------
// CreateProposal signable bytes
// ---------------------------------------------------------------------------

describe("createProposalSignableBytes", () => {
  it("handles TextProposal", () => {
    const bytes = createProposalSignableBytes(
      TEST_KP.address,
      1n,
      "Test",
      "Desc",
      { type: "TextProposal" },
      10000n,
      0n,
    );
    // NETWORK_ID(19) + "proposal"(8) + from(32) + id(8) +
    // title_len(4) + "Test"(4) + desc_len(4) + "Desc"(4) +
    // type_byte(1) + fee(8) + nonce(8) = 100
    expect(bytes.length).toBe(100);
  });

  it("handles ParameterChange", () => {
    const bytes = createProposalSignableBytes(
      TEST_KP.address,
      1n,
      "T",
      "D",
      { type: "ParameterChange", param: "min_fee_sats", newValue: "20000" },
      10000n,
      0n,
    );
    // ... + type_byte(1) + param_len(4) + "min_fee_sats"(12) + value_len(4) + "20000"(5) + ...
    // Total varies; just verify it compiles and is deterministic
    const bytes2 = createProposalSignableBytes(
      TEST_KP.address,
      1n,
      "T",
      "D",
      { type: "ParameterChange", param: "min_fee_sats", newValue: "20000" },
      10000n,
      0n,
    );
    expect(bytes).toEqual(bytes2);
  });

  it("handles CouncilMembership", () => {
    const bytes = createProposalSignableBytes(
      TEST_KP.address,
      1n,
      "T",
      "D",
      {
        type: "CouncilMembership",
        action: "Add",
        address: TEST_KP_2.address,
        category: "Technical",
      },
      10000n,
      0n,
    );
    expect(bytes.length).toBeGreaterThan(0);
  });

  it("handles TreasurySpend", () => {
    const bytes = createProposalSignableBytes(
      TEST_KP.address,
      1n,
      "T",
      "D",
      {
        type: "TreasurySpend",
        recipient: TEST_KP_2.address,
        amount: 1000000n,
      },
      10000n,
      0n,
    );
    expect(bytes.length).toBeGreaterThan(0);
  });
});

// ---------------------------------------------------------------------------
// Vote signable bytes
// ---------------------------------------------------------------------------

describe("voteSignableBytes", () => {
  it("produces correct length", () => {
    // NETWORK_ID(19) + "vote"(4) + from(32) + proposal_id(8) + vote(1) + fee(8) + nonce(8) = 80
    const bytes = voteSignableBytes(TEST_KP.address, 1n, true, 10000n, 0n);
    expect(bytes.length).toBe(80);
  });

  it("encodes vote=true as 1", () => {
    const bytes = voteSignableBytes(TEST_KP.address, 1n, true, 10000n, 0n);
    // vote byte at offset 19 + 4 + 32 + 8 = 63
    expect(bytes[63]).toBe(1);
  });

  it("encodes vote=false as 0", () => {
    const bytes = voteSignableBytes(TEST_KP.address, 1n, false, 10000n, 0n);
    expect(bytes[63]).toBe(0);
  });
});

// ---------------------------------------------------------------------------
// Sign and verify
// ---------------------------------------------------------------------------

describe("signTransaction", () => {
  it("produces a 64-byte signature", () => {
    const signable = transferSignableBytes(
      TEST_KP.address,
      TEST_KP_2.address,
      100n,
      10n,
      0n,
    );
    const sig = signTransaction(signable, TEST_KP.secretKey);
    expect(sig.length).toBe(64);
  });

  it("signature verifies with correct public key", () => {
    const signable = transferSignableBytes(
      TEST_KP.address,
      TEST_KP_2.address,
      100n,
      10n,
      0n,
    );
    const sig = signTransaction(signable, TEST_KP.secretKey);
    const valid = ed.verify(sig, signable, TEST_KP.publicKey);
    expect(valid).toBe(true);
  });

  it("signature fails with wrong public key", () => {
    const signable = transferSignableBytes(
      TEST_KP.address,
      TEST_KP_2.address,
      100n,
      10n,
      0n,
    );
    const sig = signTransaction(signable, TEST_KP.secretKey);
    const valid = ed.verify(sig, signable, TEST_KP_2.publicKey);
    expect(valid).toBe(false);
  });

  it("signature fails on tampered message", () => {
    const signable = transferSignableBytes(
      TEST_KP.address,
      TEST_KP_2.address,
      100n,
      10n,
      0n,
    );
    const sig = signTransaction(signable, TEST_KP.secretKey);
    // Tamper: change amount
    const tampered = transferSignableBytes(
      TEST_KP.address,
      TEST_KP_2.address,
      999n,
      10n,
      0n,
    );
    const valid = ed.verify(sig, tampered, TEST_KP.publicKey);
    expect(valid).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// buildSignedTransferTx
// ---------------------------------------------------------------------------

describe("buildSignedTransferTx", () => {
  it("produces correctly shaped JSON", () => {
    const tx = buildSignedTransferTx(
      bytesToHex(TEST_KP.secretKey),
      bytesToHex(TEST_KP_2.address),
      100_000_000n, // 1 UDAG
      10_000n,
      0n,
    ) as { Transfer: Record<string, unknown> };

    expect(tx).toHaveProperty("Transfer");
    const t = tx.Transfer;
    expect(t.from).toBeInstanceOf(Array);
    expect((t.from as number[]).length).toBe(32);
    expect(t.to).toBeInstanceOf(Array);
    expect((t.to as number[]).length).toBe(32);
    expect(t.amount).toBe(100_000_000);
    expect(t.fee).toBe(10_000);
    expect(t.nonce).toBe(0);
    expect(t.pub_key).toBeInstanceOf(Array);
    expect((t.pub_key as number[]).length).toBe(32);
    expect(typeof t.signature).toBe("string");
    expect((t.signature as string).length).toBe(128);
    expect(t.memo).toBeNull();
  });

  it("includes memo when provided", () => {
    const memo = new TextEncoder().encode("sensor:22.4C");
    const tx = buildSignedTransferTx(
      bytesToHex(TEST_KP.secretKey),
      bytesToHex(TEST_KP_2.address),
      100n,
      10_000n,
      0n,
      memo,
    ) as { Transfer: Record<string, unknown> };

    expect(tx.Transfer.memo).toBeInstanceOf(Array);
    expect((tx.Transfer.memo as number[]).length).toBe(memo.length);
  });

  it("signature is verifiable", () => {
    const tx = buildSignedTransferTx(
      bytesToHex(TEST_KP.secretKey),
      bytesToHex(TEST_KP_2.address),
      100_000_000n,
      10_000n,
      0n,
    ) as { Transfer: Record<string, unknown> };

    const t = tx.Transfer;
    const fromBytes = new Uint8Array(t.from as number[]);
    const toBytes = new Uint8Array(t.to as number[]);
    const pubKey = new Uint8Array(t.pub_key as number[]);
    const sigBytes = hexToBytes(t.signature as string);

    // Verify address = blake3(pubkey)
    expect(fromBytes).toEqual(deriveAddressBytes(pubKey));

    // Reconstruct signable bytes and verify signature
    const signable = transferSignableBytes(
      fromBytes,
      toBytes,
      BigInt(t.amount as number),
      BigInt(t.fee as number),
      BigInt(t.nonce as number),
    );
    const valid = ed.verify(sigBytes, signable, pubKey);
    expect(valid).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// buildSignedStakeTx
// ---------------------------------------------------------------------------

describe("buildSignedStakeTx", () => {
  it("produces correctly shaped JSON", () => {
    const tx = buildSignedStakeTx(
      bytesToHex(TEST_KP.secretKey),
      1_000_000_000_000n, // 10,000 UDAG
      0n,
    ) as { Stake: Record<string, unknown> };

    expect(tx).toHaveProperty("Stake");
    expect(tx.Stake.amount).toBe(1_000_000_000_000);
    expect(tx.Stake.nonce).toBe(0);
    expect(typeof tx.Stake.signature).toBe("string");
    expect((tx.Stake.signature as string).length).toBe(128);
  });

  it("signature is verifiable", () => {
    const tx = buildSignedStakeTx(
      bytesToHex(TEST_KP.secretKey),
      1_000_000_000_000n,
      5n,
    ) as { Stake: Record<string, unknown> };

    const s = tx.Stake;
    const fromBytes = new Uint8Array(s.from as number[]);
    const pubKey = new Uint8Array(s.pub_key as number[]);
    const sigBytes = hexToBytes(s.signature as string);

    const signable = stakeSignableBytes(
      fromBytes,
      BigInt(s.amount as number),
      BigInt(s.nonce as number),
    );
    expect(ed.verify(sigBytes, signable, pubKey)).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// buildSignedUnstakeTx
// ---------------------------------------------------------------------------

describe("buildSignedUnstakeTx", () => {
  it("produces correctly shaped JSON", () => {
    const tx = buildSignedUnstakeTx(
      bytesToHex(TEST_KP.secretKey),
      3n,
    ) as { Unstake: Record<string, unknown> };

    expect(tx).toHaveProperty("Unstake");
    expect(tx.Unstake.nonce).toBe(3);
    expect(typeof tx.Unstake.signature).toBe("string");
  });
});

// ---------------------------------------------------------------------------
// buildSignedDelegateTx
// ---------------------------------------------------------------------------

describe("buildSignedDelegateTx", () => {
  it("produces correctly shaped JSON", () => {
    const tx = buildSignedDelegateTx(
      bytesToHex(TEST_KP.secretKey),
      bytesToHex(TEST_KP_2.address),
      10_000_000_000n, // 100 UDAG
      0n,
    ) as { Delegate: Record<string, unknown> };

    expect(tx).toHaveProperty("Delegate");
    expect(tx.Delegate.amount).toBe(10_000_000_000);
    expect((tx.Delegate.validator as number[]).length).toBe(32);
  });
});

// ---------------------------------------------------------------------------
// buildSignedUndelegateTx
// ---------------------------------------------------------------------------

describe("buildSignedUndelegateTx", () => {
  it("produces correctly shaped JSON", () => {
    const tx = buildSignedUndelegateTx(
      bytesToHex(TEST_KP.secretKey),
      0n,
    ) as { Undelegate: Record<string, unknown> };

    expect(tx).toHaveProperty("Undelegate");
    expect(typeof tx.Undelegate.signature).toBe("string");
  });
});

// ---------------------------------------------------------------------------
// buildSignedSetCommissionTx
// ---------------------------------------------------------------------------

describe("buildSignedSetCommissionTx", () => {
  it("produces correctly shaped JSON", () => {
    const tx = buildSignedSetCommissionTx(
      bytesToHex(TEST_KP.secretKey),
      15,
      0n,
    ) as { SetCommission: Record<string, unknown> };

    expect(tx).toHaveProperty("SetCommission");
    expect(tx.SetCommission.commission_percent).toBe(15);
  });
});

// ---------------------------------------------------------------------------
// buildSignedCreateProposalTx
// ---------------------------------------------------------------------------

describe("buildSignedCreateProposalTx", () => {
  it("produces correctly shaped JSON for TextProposal", () => {
    const tx = buildSignedCreateProposalTx(
      bytesToHex(TEST_KP.secretKey),
      1n,
      "Test Proposal",
      "A text proposal for testing",
      { type: "TextProposal" },
      10_000n,
      0n,
    ) as { CreateProposal: Record<string, unknown> };

    expect(tx).toHaveProperty("CreateProposal");
    const p = tx.CreateProposal;
    expect(p.title).toBe("Test Proposal");
    expect(p.description).toBe("A text proposal for testing");
    expect(p.proposal_type).toBe("TextProposal");
    expect(p.fee).toBe(10_000);
  });

  it("produces correctly shaped JSON for ParameterChange", () => {
    const tx = buildSignedCreateProposalTx(
      bytesToHex(TEST_KP.secretKey),
      2n,
      "Change fee",
      "Lower the min fee",
      { type: "ParameterChange", param: "min_fee_sats", newValue: "5000" },
      10_000n,
      0n,
    ) as { CreateProposal: Record<string, unknown> };

    const pt = tx.CreateProposal.proposal_type as Record<string, unknown>;
    expect(pt).toHaveProperty("ParameterChange");
    const pc = pt.ParameterChange as Record<string, unknown>;
    expect(pc.param).toBe("min_fee_sats");
    expect(pc.new_value).toBe("5000");
  });
});

// ---------------------------------------------------------------------------
// buildSignedVoteTx
// ---------------------------------------------------------------------------

describe("buildSignedVoteTx", () => {
  it("produces correctly shaped JSON", () => {
    const tx = buildSignedVoteTx(
      bytesToHex(TEST_KP.secretKey),
      1n,
      true,
      10_000n,
      0n,
    ) as { Vote: Record<string, unknown> };

    expect(tx).toHaveProperty("Vote");
    expect(tx.Vote.proposal_id).toBe(1);
    expect(tx.Vote.vote).toBe(true);
    expect(tx.Vote.fee).toBe(10_000);
  });

  it("encodes false vote correctly", () => {
    const tx = buildSignedVoteTx(
      bytesToHex(TEST_KP.secretKey),
      1n,
      false,
      10_000n,
      0n,
    ) as { Vote: Record<string, unknown> };

    expect(tx.Vote.vote).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// Cross-validation: verify all tx types produce verifiable signatures
// ---------------------------------------------------------------------------

describe("all transaction types produce verifiable signatures", () => {
  const builders = [
    {
      name: "Transfer",
      build: () =>
        buildSignedTransferTx(
          bytesToHex(TEST_KP.secretKey),
          bytesToHex(TEST_KP_2.address),
          100n,
          10_000n,
          0n,
        ),
      signableFn: (inner: Record<string, unknown>) =>
        transferSignableBytes(
          new Uint8Array(inner.from as number[]),
          new Uint8Array(inner.to as number[]),
          BigInt(inner.amount as number),
          BigInt(inner.fee as number),
          BigInt(inner.nonce as number),
        ),
    },
    {
      name: "Stake",
      build: () =>
        buildSignedStakeTx(bytesToHex(TEST_KP.secretKey), 1000n, 0n),
      signableFn: (inner: Record<string, unknown>) =>
        stakeSignableBytes(
          new Uint8Array(inner.from as number[]),
          BigInt(inner.amount as number),
          BigInt(inner.nonce as number),
        ),
    },
    {
      name: "Unstake",
      build: () =>
        buildSignedUnstakeTx(bytesToHex(TEST_KP.secretKey), 0n),
      signableFn: (inner: Record<string, unknown>) =>
        unstakeSignableBytes(
          new Uint8Array(inner.from as number[]),
          BigInt(inner.nonce as number),
        ),
    },
    {
      name: "Delegate",
      build: () =>
        buildSignedDelegateTx(
          bytesToHex(TEST_KP.secretKey),
          bytesToHex(TEST_KP_2.address),
          1000n,
          0n,
        ),
      signableFn: (inner: Record<string, unknown>) =>
        delegateSignableBytes(
          new Uint8Array(inner.from as number[]),
          new Uint8Array(inner.validator as number[]),
          BigInt(inner.amount as number),
          BigInt(inner.nonce as number),
        ),
    },
    {
      name: "Undelegate",
      build: () =>
        buildSignedUndelegateTx(bytesToHex(TEST_KP.secretKey), 0n),
      signableFn: (inner: Record<string, unknown>) =>
        undelegateSignableBytes(
          new Uint8Array(inner.from as number[]),
          BigInt(inner.nonce as number),
        ),
    },
    {
      name: "SetCommission",
      build: () =>
        buildSignedSetCommissionTx(bytesToHex(TEST_KP.secretKey), 10, 0n),
      signableFn: (inner: Record<string, unknown>) =>
        setCommissionSignableBytes(
          new Uint8Array(inner.from as number[]),
          inner.commission_percent as number,
          BigInt(inner.nonce as number),
        ),
    },
    {
      name: "Vote",
      build: () =>
        buildSignedVoteTx(bytesToHex(TEST_KP.secretKey), 1n, true, 10_000n, 0n),
      signableFn: (inner: Record<string, unknown>) =>
        voteSignableBytes(
          new Uint8Array(inner.from as number[]),
          BigInt(inner.proposal_id as number),
          inner.vote as boolean,
          BigInt(inner.fee as number),
          BigInt(inner.nonce as number),
        ),
    },
  ];

  for (const { name, build, signableFn } of builders) {
    it(`${name}: signature verifies`, () => {
      const tx = build() as Record<string, Record<string, unknown>>;
      const inner = tx[name];
      const pubKey = new Uint8Array(inner.pub_key as number[]);
      const sigBytes = hexToBytes(inner.signature as string);
      const signable = signableFn(inner);
      expect(ed.verify(sigBytes, signable, pubKey)).toBe(true);
    });
  }
});
