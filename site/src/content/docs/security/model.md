---
title: "Security Model"
description: "Cryptographic primitives, BFT assumptions, transport security, and defense-in-depth strategy"
order: 1
section: "security"
---

# Security Model

This page describes UltraDAG's security architecture, cryptographic choices, and defense-in-depth strategy.

---

## Cryptographic Primitives

| Function | Algorithm | Implementation | Purpose |
|----------|-----------|---------------|---------|
| Signatures | Ed25519 | `ed25519-dalek` (`verify_strict`) | Transaction and vertex authentication |
| Hashing | Blake3 | `blake3` crate | Address derivation, state root, vertex hashing |
| P2P Encryption | Noise_XX_25519_ChaChaPoly_BLAKE2s | `snow` crate v0.9 | Transport encryption with forward secrecy |
| Key Agreement | X25519 | via Noise framework | Ephemeral Diffie-Hellman for session keys |

### Ed25519 Signatures

All transactions and DAG vertices are signed with Ed25519. The `verify_strict` mode is used throughout, which:

- Rejects non-canonical signatures (prevents malleability)
- Rejects small-order public keys
- Provides stronger guarantees than standard Ed25519 verification

### Blake3 Hashing

Blake3 is used for:

- **Address derivation**: `address = blake3(ed25519_public_key)`
- **Vertex hashing**: `hash = blake3(author || round || parents || txs || timestamp)`
- **State root**: canonical byte hashing of the entire state
- **Parent selection scoring**: `blake3(proposer || candidate_hash)` for deterministic selection

### Why These Choices

- **Ed25519**: smallest signatures (64 bytes), fastest verification, widely audited
- **Blake3**: fastest cryptographic hash, tree-structured parallelism, designed as a drop-in SHA-256 replacement
- **Noise framework**: proven protocol composition, forward secrecy, no TLS complexity

---

## BFT Assumptions

UltraDAG's consensus safety relies on the standard BFT fault threshold:

$$
n \geq 3f + 1
$$

With 100 validators, the protocol tolerates up to **33 Byzantine validators**. Safety holds if fewer than 1/3 of validators (by stake weight) are Byzantine.

| Validators | Byzantine Tolerance |
|-----------|-------------------|
| 4 | 1 |
| 7 | 2 |
| 13 | 4 |
| 100 | 33 |

### What Byzantine Means

A Byzantine validator can:

- Produce conflicting vertices (equivocation)
- Withhold vertices
- Send different data to different peers
- Crash and restart at arbitrary times

A Byzantine validator **cannot**:

- Forge signatures of honest validators
- Break Ed25519 or Blake3
- Revert finalized transactions

---

## NETWORK_ID Domain Separation

Every signed message includes a network identifier prefix:

| Mode | NETWORK_ID |
|------|-----------|
| Testnet | `b"ultradag-testnet-v1"` |
| Mainnet | `b"ultradag-mainnet-v1"` |

This provides:

- **Cross-network replay protection**: a testnet transaction cannot be replayed on mainnet
- **Type-discriminated signatures**: each transaction type adds a unique discriminator byte string after the NETWORK_ID
- **Version-aware**: the `v1` suffix allows future protocol upgrades with different signing schemes

```
signable_bytes = NETWORK_ID || type_discriminator || field_bytes
```

---

## Transport Security

### Noise Protocol

All P2P connections are encrypted using the Noise_XX pattern:

- **Forward secrecy**: ephemeral X25519 keys generated per connection
- **Mutual authentication**: both peers exchange static keys
- **Validator identity binding**: Ed25519 validator key signs the Noise static public key during handshake

### Observer Support

Nodes without a validator identity connect with encryption but without authentication. They receive `[0x00]` as the identity payload instead of a signed validator key.

### Message Integrity

All messages after the Noise handshake are:

- Encrypted with ChaChaPoly1305 (authenticated encryption)
- Chunked for messages > 65,535 bytes (Noise spec limit)
- Protected against replay by the Noise protocol's nonce mechanism

---

## Supply Invariant

The supply invariant is the most critical safety property:

$$
\text{liquid} + \text{staked} + \text{delegated} + \text{treasury} = \text{total\_supply}
$$

This is verified after **every state transition** in release builds (not just debug). A violation triggers:

1. FATAL error log with diagnostic breakdown
2. Graceful state save to disk
3. Exit with code 101

<div class="callout callout-danger"><div class="callout-title">Halt over corruption</div>The node will halt rather than continue with inconsistent state. On mainnet, any supply drift would be unrecoverable without a hard fork. Halting is the safe choice.</div>

---

## Rate Limiting

### Per-IP RPC Limits

| Endpoint | Limit | Window |
|----------|-------|--------|
| `/tx` | 100 | per minute |
| `/faucet` | 1 | per 10 minutes |
| `/stake`, `/unstake` | 5 each | per minute |
| `/delegate`, `/undelegate` | 5 each | per minute |
| Global | 1,000 | per minute |

### Per-Peer P2P Limits

| Mechanism | Limit |
|-----------|-------|
| Aggregate messages | 500 per 60-second window |
| `GetDagVertices` | 2-second per-peer cooldown |
| `GetRoundHashes` | 10-second per-peer cooldown |
| `GetCheckpoint` | 30-second per-peer cooldown |

Peers exceeding limits are disconnected.

### Trusted Proxy Detection

Behind a reverse proxy (e.g., Fly.io), the real client IP is extracted from `Fly-Client-IP` or `X-Forwarded-For` headers. Only trusted proxy IPs are honored:

- Loopback (127.0.0.0/8, ::1)
- RFC 1918 private ranges
- Fly.io fdaa::/16

---

## Equivocation Detection and Slashing

### Detection

Equivocation (two different vertices from the same validator in the same round) is detected at multiple layers:

1. **DAG insertion**: `try_insert()` rejects the second vertex and stores equivocation evidence
2. **P2P gossip**: evidence is broadcast to all peers
3. **Finality application**: `apply_finalized_vertices()` detects duplicate (validator, round) pairs in the sorted batch
4. **Cross-batch detection**: `applied_validators_per_round` HashMap tracks producers across separate finality batches

### Deterministic Slashing

Slashing occurs in `apply_finalized_vertices()`, not in P2P handlers. This ensures all honest nodes apply the same slash at the same logical point in the state machine. P2P handlers only broadcast evidence for awareness.

---

## Canonical State Root

The state root uses hand-rolled canonical byte encoding, not serde serialization:

```
blake3(
  "ultradag-state-root-v1" ||
  sorted_accounts ||
  sorted_stakes ||
  sorted_delegations ||
  sorted_proposals ||
  governance_params ||
  configured_validator_count ||
  total_supply ||
  latest_round
)
```

This is:

- **Version-prefixed**: `"ultradag-state-root-v1"` allows future schema changes
- **Deterministic**: little-endian integers, length-prefixed strings, explicit enum discriminants
- **Serde-independent**: immune to serialization library version changes
- **Regression-tested**: 6 tests with a known-fixture hash anchor

---

## 100% Safe Rust

The UltraDAG codebase contains **zero instances of `unsafe` code**. This eliminates:

- Manual memory management vulnerabilities
- Undefined behavior
- Buffer overflows
- Use-after-free bugs

All arithmetic in financial paths uses `saturating_add` / `saturating_sub` / `saturating_mul` to prevent overflow.

---

## Eclipse Attack Prevention

Fresh nodes joining the network are protected by:

1. **Checkpoint chain verification**: `CheckpointSync` includes the full checkpoint chain, verified back to the hardcoded `GENESIS_CHECKPOINT_HASH`
2. **Chain never skipped**: even fresh nodes with zero local checkpoints verify the full chain
3. **Quorum signatures**: checkpoints require >2/3 validator co-signatures
4. **Hardcoded genesis**: `GENESIS_CHECKPOINT_HASH` is baked into the binary at compile time

---

## Additional Defenses

| Defense | Description |
|---------|-------------|
| **Mempool limits** | 10K transactions max, fee-based eviction, 100 per sender |
| **Orphan buffer** | 1000 entries max, 100 per peer, lowest-round eviction |
| **Message size** | 4 MB maximum before deserialization |
| **Future round** | Vertices > 10 rounds ahead rejected |
| **Timestamp** | Vertices > 5 minutes in future rejected |
| **Read timeout** | 30-second timeout on peer connections |
| **Connection limits** | Max inbound/outbound connections enforced |
| **Chunk amplification** | Noise chunk count capped to prevent CPU exhaustion |

---

## Next Steps

- [Bug Bounty](/docs/security/bug-bounty) — report vulnerabilities
- [Audit Reports](/docs/security/audits) — audit findings and fixes
