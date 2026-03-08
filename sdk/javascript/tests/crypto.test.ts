import { describe, it, expect } from "vitest";
import { Keypair, deriveAddress } from "../src/crypto.js";

describe("Keypair", () => {
  it("generates a random keypair with valid lengths", () => {
    const kp = Keypair.generate();
    expect(kp.secretKey).toHaveLength(64);
    expect(kp.publicKey).toHaveLength(64);
    expect(kp.address).toHaveLength(64);
  });

  it("generates distinct keypairs on each call", () => {
    const a = Keypair.generate();
    const b = Keypair.generate();
    expect(a.secretKey).not.toBe(b.secretKey);
    expect(a.address).not.toBe(b.address);
  });

  it("reconstructs from a secret key hex string", () => {
    const original = Keypair.generate();
    const restored = Keypair.fromSecretKey(original.secretKey);
    expect(restored.secretKey).toBe(original.secretKey);
    expect(restored.publicKey).toBe(original.publicKey);
    expect(restored.address).toBe(original.address);
  });

  it("constructs from a 32-byte seed", () => {
    const seed = new Uint8Array(32).fill(0xfa);
    const kp = Keypair.fromBytes(seed);
    expect(kp.secretKey).toBe("fa".repeat(32));
    expect(kp.publicKey).toHaveLength(64);
    expect(kp.address).toHaveLength(64);
  });

  it("produces a deterministic address for a known seed", () => {
    const seed = new Uint8Array(32).fill(0xfa);
    const kp1 = Keypair.fromBytes(seed);
    const kp2 = Keypair.fromBytes(seed);
    expect(kp1.address).toBe(kp2.address);
    expect(kp1.publicKey).toBe(kp2.publicKey);
  });

  it("produces lowercase hex", () => {
    const kp = Keypair.generate();
    expect(kp.secretKey).toMatch(/^[0-9a-f]{64}$/);
    expect(kp.publicKey).toMatch(/^[0-9a-f]{64}$/);
    expect(kp.address).toMatch(/^[0-9a-f]{64}$/);
  });

  it("throws on invalid secret key length", () => {
    expect(() => Keypair.fromSecretKey("abcd")).toThrow(
      "Secret key must be exactly 32 bytes",
    );
  });

  it("throws on invalid seed length", () => {
    expect(() => Keypair.fromBytes(new Uint8Array(16))).toThrow(
      "Seed must be exactly 32 bytes",
    );
  });

  it("faucet seed matches known pattern", () => {
    const faucet = Keypair.fromBytes(new Uint8Array(32).fill(0xfa));
    const dev = Keypair.fromBytes(new Uint8Array(32).fill(0xde));
    expect(faucet.address).not.toBe(dev.address);
  });
});

describe("deriveAddress", () => {
  it("returns 64-char lowercase hex from a 32-byte public key", () => {
    const pubkey = new Uint8Array(32).fill(0x01);
    const addr = deriveAddress(pubkey);
    expect(addr).toHaveLength(64);
    expect(addr).toMatch(/^[0-9a-f]{64}$/);
  });

  it("is deterministic", () => {
    const pubkey = new Uint8Array(32).fill(0xab);
    expect(deriveAddress(pubkey)).toBe(deriveAddress(pubkey));
  });

  it("produces different addresses for different keys", () => {
    const a = deriveAddress(new Uint8Array(32).fill(0x01));
    const b = deriveAddress(new Uint8Array(32).fill(0x02));
    expect(a).not.toBe(b);
  });
});
