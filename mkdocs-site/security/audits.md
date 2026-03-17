---
title: Audit Reports
---

# Audit Reports

UltraDAG has undergone extensive multi-pass auditing, resulting in 188+ bugs found and fixed. This page summarizes audit methodology, findings, and current security posture.

---

## Audit Summary

| Metric | Value |
|--------|-------|
| Total bugs found and fixed | 188+ |
| Critical bugs fixed | 15+ |
| High severity bugs fixed | 20+ |
| Audit passes completed | 12+ |
| Tests passing | 836+ (core) + 64 (sim) |
| Jepsen fault injection tests | 14 |
| Unsafe Rust instances | 0 |
| Cargo audit vulnerabilities | 0 |
| Cargo audit warnings | 0 |

---

## Audit Methodology

### Internal Multi-Pass Auditing

The codebase was audited through 12+ systematic passes, each targeting a specific concern:

| Pass | Focus Area | Key Findings |
|------|-----------|-------------|
| Consensus correctness | DAG-BFT finality, vertex ordering, equivocation | Non-deterministic slashing, ordering by topo_level |
| Supply invariant | Balance tracking, minting, burning | Multiple arithmetic overflow paths, fee clawback failures |
| P2P hardening | Message handlers, rate limiting, sync | Unbounded responses, deadlock in lock ordering |
| RPC validation | Input validation, authentication, rate limits | Zero-amount transfers, missing fee checks |
| Persistence safety | fsync, redb, checkpoint chain | No fsync before rename, state root serde-dependency |
| Cryptographic audit | Signatures, hashing, domain separation | Hash collisions in variable-length fields |
| Staking economics | Reward distribution, delegation, slashing | Undelegating amounts inflating reward denominator |
| Governance | Proposal lifecycle, parameter changes, council | Non-deterministic proposal execution order |
| Transport encryption | Noise protocol, identity binding | Handshake parse().unwrap() panics |
| Cross-batch analysis | Equivocation across finality batches | Double/triple slash for single equivocation |
| Eclipse attack | Checkpoint trust model, fresh nodes | Fresh node accepts fabricated checkpoint chain |
| Dependency audit | Third-party crate security | 1 unmaintained dependency (fixed) |

### External Vulnerability Report

An external security report with 20 claimed vulnerabilities was triaged:

| Result | Count |
|--------|-------|
| Valid (all previously known) | 3 |
| False positives or already mitigated | 17 |

The 3 valid findings were:

1. **CheckpointSync trust on fresh nodes** (VULN-01) — mitigated via checkpoint chain verification
2. **Dynamic validator inflation** (VULN-02) — mitigated via `--validators N` flag
3. **Private keys in RPC** (VULN-03) — mitigated via testnet gating

---

## Notable Bug Categories

### Consensus Determinism

Non-deterministic behavior across nodes is a consensus split vector. Findings and fixes:

| Bug | Description | Fix |
|-----|-------------|-----|
| Reward distribution order | HashMap iteration in `distribute_round_rewards()` | Sort by address before iteration |
| Governance execution order | HashMap iteration in `tick_governance()` | Sort proposals by ID |
| Vertex ordering | `topo_level` in sort key (locally computed) | Order by `(round, hash)` only |
| Council emission order | HashMap iteration for council credits | Sort council members by address |
| Pre-staking reward order | HashSet iteration of producers | Sort by address |
| Parent selection tiebreak | Unstable sort on score collision | Add hash-based tiebreaker |

### Supply Integrity

Bugs that could have caused supply inflation or deflation:

| Bug | Description | Fix |
|-----|-------------|-----|
| Fee clawback failure | Governance tx fee clawback logged but ignored | Return `SupplyInvariantBroken` |
| Undelegating in denominator | Reward denominator included undelegating amounts | Use `sum(effective_stake_of(v))` |
| Observer reward hardcoded | Governance parameter for observer rate ignored | Read from `governance_params` |
| Unchecked `.sum()` | 11 instances of overflow-unsafe summation | `saturating_add` folds |
| Credit overflow | `credit()` used unchecked `+=` | `saturating_add()` |
| Coinbase height trusted | Engine trusted proposer-supplied height for reward | Compute from `last_finalized_round` |

### P2P Hardening

Network-layer vulnerabilities:

| Bug | Description | Fix |
|-----|-------------|-----|
| Eclipse attack | Fresh node CheckpointSync skipped chain verification | Always verify chain to genesis hash |
| Chunk amplification | 4MB message with 1-byte chunks = 4M decrypt ops | Cap max chunks |
| Orphan buffer unbounded per peer | Single peer fills entire buffer | Per-peer cap of 100 |
| DagVertices unbounded | No cap on incoming vertex count | `.take(500)` |
| RoundHashes amplification | Unbounded hash count generates GetParents flood | Cap at 1000 rounds, 100 hashes |
| Deadlock (3 instances) | Inconsistent lock ordering between handlers | Uniform finality-before-state ordering |

### Persistence Safety

Data durability bugs:

| Bug | Description | Fix |
|-----|-------------|-----|
| No fsync before rename | 5 persistence paths used write + rename without fsync | `write_and_fsync()` + `fsync_directory()` |
| State root serde-dependent | `postcard::to_allocvec()` not version-stable | Hand-rolled canonical bytes |
| Broken checkpoint chain | Missing predecessor produces `[0u8; 32]` chain link | Skip production, log error |
| configured_validator_count not persisted | Lost on restart, changed reward splitting | Save in redb METADATA table |

---

## Dependency Audit

### Cargo Audit

Full scan of all 316 crate dependencies against the RustSec advisory database:

```
cargo audit
    Fetching advisory database from `https://github.com/RustSec/advisory-db.git`
    Scanning Cargo.lock for vulnerabilities (949 advisories)

    0 vulnerabilities found
    0 warnings found
```

### Unmaintained Dependencies

One unmaintained dependency was found and resolved:

- **`rustls-pemfile` v1.0.4** (RUSTSEC-2025-0134) via `reqwest` v0.11.27
- **Fix**: updated `reqwest` from v0.11.27 to v0.13.2

---

## Unsafe Code Audit

Comprehensive scan of all source files across all UltraDAG crates:

| Crate | Unsafe Instances |
|-------|-----------------|
| `ultradag-coin` | 0 |
| `ultradag-network` | 0 |
| `ultradag-node` | 0 |
| `ultradag-sdk` | 0 |
| **Total** | **0** |

UltraDAG achieves **100% safe Rust** — no `unsafe` blocks, no manual memory management, no undefined behavior risks.

---

## Test Coverage

### Test Breakdown

| Category | Count |
|----------|-------|
| Unit tests (ultradag-coin) | 168 |
| Integration tests (ultradag-coin) | 407 |
| Unit tests (ultradag-network) | 25 |
| Integration tests (ultradag-network) | 12 |
| Fault injection tests | 49 |
| Jepsen fault injection | 14 |
| Adversarial integration | 5 |
| Simulation tests (ultradag-sim) | 64 |
| RPC tests | 25 |
| SDK tests | 146 |
| **Total** | **1,000+** |

### Jepsen Fault Injection

14 Jepsen-style tests validate consensus under adversarial conditions:

- Split-brain partition with heal
- Partition with clock skew recovery
- Extreme chaos (combined faults)
- Single and simultaneous node crashes
- Repeated crash-restart cycles
- Message chaos (delays, drops, reordering)
- Future timestamp rejection
- Minority partition liveness check

### Simulation Harness

64 deterministic simulation tests run real consensus logic with a virtual network:

- Base consensus convergence (100-1000 rounds)
- Staking lifecycle with commission
- Delegation rewards and splits
- Governance parameter change execution
- Epoch transitions
- Equivocation detection under message reorder
- 21-validator stress with 5% loss
- Mixed Byzantine strategies (2/7 Byzantine)
- Late-joiner convergence

---

## Formal Verification

The consensus protocol is specified in TLA+ and model-checked:

- **32.9 million states explored**
- **6 invariants verified** with zero violations
- **Model**: N=4 validators, 1 Byzantine, MAX_ROUNDS=2

See [Formal Verification](../technical/formal-verification.md) for full details.

---

## Current Security Posture

| Aspect | Status |
|--------|--------|
| Cargo audit | Clean (0 vulnerabilities, 0 warnings) |
| Unsafe code | Zero instances |
| Supply invariant | FATAL on violation (exit code 101) |
| Transport encryption | All P2P traffic encrypted (Noise protocol) |
| Rate limiting | Per-IP, per-peer, per-endpoint |
| Formal verification | 32.9M states, zero violations |
| Test coverage | 900+ tests passing (core + sim) |
| Arithmetic safety | Saturating operations throughout |

---

## Next Steps

- [Security Model](model.md) — full security architecture
- [Bug Bounty](bug-bounty.md) — report a vulnerability
- [Formal Verification](../technical/formal-verification.md) — TLA+ specification
