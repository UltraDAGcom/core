# Security Audit Scope

## Overview

Before mainnet launch, an independent security audit firm must review the following critical components. This document defines the minimum audit scope.

## Critical Path 1: Cryptographic Signatures

**Files:** `crates/ultradag-coin/src/address/keys.rs`, `crates/ultradag-coin/src/consensus/vertex.rs`

- Ed25519 signature creation and `verify_strict()` usage
- `signable_bytes()` construction for all transaction types (Transfer, Stake, Unstake, CreateProposal, Vote)
- NETWORK_ID domain separation prefix
- Transaction type discriminator bytes (`b"transfer"`, `b"stake"`, etc.)
- DagVertex signature covering all consensus-critical fields
- Address derivation: `blake3(ed25519_pubkey)`

**Risk:** Wrong byte ordering, missing fields in signable_bytes, or weak verification allows signature forgery or cross-type replay.

## Critical Path 2: BFT Finality Logic

**Files:** `crates/ultradag-coin/src/consensus/finality.rs`, `crates/ultradag-coin/src/consensus/dag.rs`

- Quorum threshold calculation: `ceil(2n/3)` correctness
- `find_newly_finalized()` — forward propagation through children
- Parent finality guarantee (parents finalized before children)
- Descendant validator tracking via BitVec + ValidatorIndex
- Pruned parent handling (pruned == deeply finalized)
- Stuck parent escape hatch (>100 rounds)
- Equivocation detection in `try_insert()`

**Risk:** Off-by-one in quorum, incorrect finality propagation, or missed equivocation allows double-spend or consensus split.

## Critical Path 3: State Engine & Supply Invariant

**Files:** `crates/ultradag-coin/src/state/engine.rs`

- `apply_finalized_vertices()` — deterministic ordering by (round, hash)
- Deterministic slashing during finality batch processing
- Supply invariant: `sum(liquid) + sum(staked) == total_supply`
- `credit()` / `debit()` overflow protection (saturating arithmetic)
- Coinbase validation: expected_height computation, supply cap enforcement
- Staking: `apply_stake_tx()`, `apply_unstake_tx()`, `slash()`
- Governance: `tick_governance()`, `apply_change()`, DAO activation gate

**Risk:** Supply inflation, balance overflow, non-deterministic state across nodes.

## Critical Path 4: P2P Message Handling

**Files:** `crates/ultradag-network/src/node/server.rs`

- Lock ordering (finality → state, never reversed)
- Signature verification before any state mutation
- Rate limiting and size bounds on all message types
- Orphan buffer bounds (per-peer caps, total caps)
- CheckpointSync trust model and chain verification

**Risk:** Deadlocks, denial of service, state corruption from malicious peers.

## Critical Path 5: Checkpoint Chain

**Files:** `crates/ultradag-coin/src/consensus/checkpoint.rs`, `crates/ultradag-coin/src/constants.rs`

- GENESIS_CHECKPOINT_HASH hardcoded trust anchor
- `verify_checkpoint_chain()` — backwards walk, cycle detection
- Checkpoint signature verification before storage
- Fast-sync state_root validation

**Risk:** Eclipse attack via forged checkpoints, broken chain allowing state injection.

## Out of Scope (Lower Priority)

- RPC endpoint input validation (already hardened)
- CLI argument parsing
- Test infrastructure
- SDK implementations
- Website/frontend code

## Deliverables

1. Full report with severity classifications (Critical/High/Medium/Low/Informational)
2. Proof-of-concept for any Critical or High findings
3. Verification of all supply invariant paths
4. Confirmation of BFT safety properties under f < n/3 Byzantine validators
5. Review of all `saturating_*` arithmetic for correctness (not just overflow prevention)

## Estimated Scope

- ~3,000 lines of consensus-critical Rust code
- ~2,000 lines of P2P message handling
- ~1,500 lines of state engine
- Total: ~6,500 lines requiring careful review
