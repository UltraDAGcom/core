---
title: "UltraDAG Whitepaper v1.2"
description: "A Leaderless DAG-BFT Cryptocurrency — Minimal correct consensus in ~2,800 lines of Rust"
order: 0
section: "whitepaper"
---

**UltraDAG is a minimal cryptocurrency built on a leaderless DAG-BFT consensus protocol.** The entire consensus core is five files of Rust. The protocol achieves Byzantine fault tolerance through descendant coverage finality — a vertex is finalized when 2f+1 distinct validators have built on top of it. This implicit voting mechanism eliminates leader election, view changes, and explicit vote messages. The system has been validated through 805+ automated tests (all passing) and a 5-node Fly.io testnet. UltraDAG demonstrates that a complete, working cryptocurrency with a 21 million supply cap, halving schedule, and validator staking can be built with radical simplicity.

---

## 1. Introduction

### 1.1 Motivation

Traditional Byzantine Fault Tolerant (BFT) consensus protocols — PBFT, Tendermint, HotStuff — operate in a leader-based paradigm. In each round or view, a designated leader proposes a block, and other validators vote on it. This creates three fundamental limitations:

1. **Single point of failure per round.** If the leader is slow, crashed, or Byzantine, the round stalls until a view change occurs.
2. **Sequential throughput.** Only one block is produced per round, regardless of the number of validators.
3. **Protocol complexity.** View change mechanisms add significant complexity and are historically the most bug-prone components of BFT protocols.

DAG-based consensus protocols address these limitations by allowing all validators to produce blocks (vertices) concurrently. Recent protocols such as DAG-Rider, Tusk, Bullshark, and Shoal++ have demonstrated that DAG structures can achieve consensus without explicit voting rounds.

### 1.2 Contribution

UltraDAG implements a complete, working cryptocurrency using a custom leaderless DAG-BFT protocol with the following properties:

- **Leaderless vertex production.** All validators produce vertices concurrently every round, with no leader election.
- **Implicit finality via descendant coverage.** A vertex is finalized when ⌈2n/3⌉ distinct validators have at least one descendant of it in the DAG.
- **Parent finality guarantee.** A vertex is only finalized after all its parents are finalized, ensuring correct causal ordering.
- **Single-round vertex propagation.** Vertices are broadcast once, not voted on. The DAG structure accumulates "votes" automatically.
- **Equivocation detection with permanent banning.** Validators that produce conflicting vertices are permanently excluded.

---

## 2. Design Philosophy: Minimal Correct DAG-BFT

### 2.1 The Protocol in Three Sentences

**The Complete Consensus Rule:**

1. Every validator produces one signed vertex per round referencing all known DAG tips.
2. A vertex is final when ⌈2n/3⌉ distinct validators have built upon it and all its parents are final.
3. Equivocating validators are permanently banned.

Note that sentence 2 encodes two constraints: the quorum threshold (⌈2n/3⌉ descendants) *and* recursive parent finality ("all its parents are final"). The parent finality requirement is what ensures causal ordering and affects finality latency — a vertex in round r cannot finalize until all its ancestors finalize first, which in practice means finality propagates round-by-round. Everything else in this paper — the round gate, stall recovery, deterministic ordering, state derivation — is implementation detail required to make these three sentences operational.

### 2.2 Consensus Core Size

The complete consensus implementation is contained in five files totaling **2,769 lines** of Rust (production + tests):

| File | Lines |
|------|-------|
| `vertex.rs` | 358 |
| `dag.rs` | 1,504 |
| `finality.rs` | 473 |
| `ordering.rs` | 180 |
| `validator_set.rs` | 254 |
| **Total** | **2,769** |

| System | Approx. Consensus Lines |
|--------|----------------------|
| Narwhal/Tusk | ~15,000 |
| Bullshark | ~20,000 |
| Shoal++ | ~30,000 |
| **UltraDAG** | **~2,800** |

*Line counts for competing protocols are estimates based on their published reference implementations (consensus modules only, excluding networking, storage, and client code). UltraDAG's count includes both production code and inline tests. These comparisons are approximate — different projects draw module boundaries differently.*

### 2.3 What Was Deliberately Omitted

**No separate mempool layer.** UltraDAG bundles transactions directly into vertices, eliminating an entire subsystem (~5,000 lines in Narwhal).

**No leader or anchor selection.** Bullshark and Shoal++ designate "anchor" vertices. UltraDAG replaces this with a single descendant-coverage check. No vertex is special.

**No wave structure.** DAG-Rider organizes rounds into waves of 4. UltraDAG has no waves — every round is identical.

**No reputation system.** Shoal++ includes a ~2,000-line reputation mechanism. UltraDAG handles stall recovery in 8 lines: after 3 consecutive round skips, produce unconditionally.

### 2.4 The Minimalism Claim

**UltraDAG is not optimized for maximum throughput. It is optimized for minimum correct implementation.**

The ~10x reduction in consensus code vs Shoal++ directly reduces the attack surface. A protocol that can be fully described in three sentences can be fully audited by a single engineer in a single day.

---

## 3. System Model

### 3.1 Participants

Let **P** = {p1, p2, ..., pn} be the set of **n** validators (participants). Each validator vi holds an Ed25519 keypair (ski, pki) identified by:

```
addr_i = Blake3(pk_i)
```

We assume the standard BFT fault model: at most **f** validators are Byzantine, where **n >= 3f + 1**.

### 3.2 Network Model

The protocol assumes **partial synchrony**: there exists an unknown Global Stabilization Time (GST) after which all messages between honest validators are delivered within a bounded delay **d**. Before GST, messages may be delayed arbitrarily.

### 3.3 Cryptographic Primitives

| Primitive | Algorithm | Purpose |
|-----------|-----------|---------|
| Digital Signatures | Ed25519 (ed25519-dalek 2.2.0) | Vertex and transaction authentication |
| Hashing | Blake3 | Address derivation, vertex identity, Merkle trees |
| Replay Prevention | NETWORK_ID prefix | Cross-network signature isolation |

---

## 4. Protocol Description

### 4.1 DAG Structure

The core data structure is a directed acyclic graph **G = (V, E)** where each vertex is a tuple:

```rust
v = (block, parents, round, validator, pub_key, signature)
```

**Vertex Identity:**
```
H(v) = Blake3(block_hash || round_LE64 || validator || parent_0 || ... || parent_k)
```

**Signable Bytes:**
```
signable(v) = NETWORK_ID || block_hash || parent_0 || ... || parent_k || round_LE64 || validator
```

### 4.2 Vertex Production (Optimistic Responsiveness)

Each honest validator produces exactly one vertex per round using **optimistic responsiveness**: `tokio::select!` between the round timer (default: 5s) and a quorum notification, producing immediately when quorum is reached.

1. **Wait.** Select between round timer and `round_notify`.
2. **2f+1 round gate.** If |distinct_validators_in_round(r-1)| < ⌈2n/3⌉, skip. After 3 consecutive skips, produce unconditionally.
3. **Active set check.** If not in the active staking set, observe only.
4. **Equivocation check.** If already produced in round r, skip.
5. **Collect parents.** Set parents = all current DAG tips.
6. **Build block.** Include coinbase (transaction fees only) and pending mempool transactions. Block rewards are distributed per-round by the protocol via `distribute_round_rewards()`, not in the coinbase.
7. **Sign and broadcast.** Ed25519-sign and broadcast to all peers.

### 4.3 Vertex Acceptance

A vertex v is accepted into the DAG if and only if: no duplicate H(v) exists, the Ed25519 signature is valid, all parents exist in the DAG, v.round <= current_round + 10, no equivocation is detected, and the validator is not marked Byzantine.

### 4.4 Recursive Parent Fetch

When a vertex fails insertion due to missing parents, the node buffers it (orphan buffer: 1,000 entries / 50 MB max, with a per-peer cap of 100 entries) and sends a `GetParents` request. Each received parent is verified and inserted, recursing for missing grandparents. After any successful insertion, `resolve_orphans()` re-attempts buffered vertices.

**Buffer overflow behavior:** When the buffer is full, the entry with the lowest round (oldest) is evicted to make room for the new vertex. The node does not halt or diverge — it continues operating and requesting missing parents. Safety is preserved because finality requires ⌈2n/3⌉ *honest* descendants, and honest nodes will re-broadcast vertices. A node that temporarily loses orphans may experience delayed finality but cannot reach an inconsistent state, as every vertex is fully re-verified on insertion regardless of source.

---

## 5. Finality

### 5.1 Descendant-Coverage Finality Rule

**Definition -- Descendant Validators:**
```
DV(v, G) = { u.validator : u in descendants(v, G) }
```

**Definition -- Quorum Threshold:**
```
q(n) = ceil(2n/3)
```

**Definition -- Finality:**
```
FINALIZED(v)  <=>  |DV(v, G)| >= ceil(2n/3)  AND  (for all p in v.parents : FINALIZED(p))
```

### 5.2 Incremental Descendant Tracking

Rather than recomputing DV(v, G) via BFS (O(V) per vertex), UltraDAG maintains a precomputed map `descendant_validators: HashMap<Hash, BitVec>` with early-termination BFS on insertion and a `ValidatorIndex` for bidirectional address-to-index mapping. BitVec provides 256x memory reduction vs HashSet<Address> at scale. This gives **O(1) finality lookups**. Benchmark: inserting 10,000 vertices into a synthetic DAG (4 validators, linear chain topology) and checking finality for all vertices — **21ms** with incremental tracking vs. 47,000ms with naive per-vertex BFS, a **2,238x improvement** (measured on Apple M1, single-threaded, `cargo bench`).

### 5.3 Parent Finality Guarantee

A vertex may only be finalized if **all its parents are already finalized**, ensuring all causal history is committed before applying a vertex's state changes.

---

## 6. Safety

### 6.1 No Conflicting Finality

Two equivocating vertices (same validator, same round, different hash) cannot both be finalized. Equivocation detection ensures at most one vertex per validator per round exists in any honest node's DAG.

### 6.2 Quorum Intersection Argument

If vertex v is finalized at node A and a conflicting v' at node B, then:

```
|DV(v, G_A)| + |DV(v', G_B)| >= 2 * ceil(2n/3) > n + f
```

Their intersection must contain at least one honest validator whose DAG would hold both conflicting vertices — triggering equivocation detection, a contradiction.

**Formal Verification:** Machine-checked via TLA+. The TLC model checker exhaustively explored **32.6 million states** (N=4 validators, 1 Byzantine, 2 rounds) and verified six invariants with zero violations: TypeOK, Safety, HonestNoEquivocation, FinalizedParentsConsistency, RoundMonotonicity, and ByzantineBound. Note: at MAX_ROUNDS=2, this covers approximately one finality cycle (finality takes 2-3 rounds). Extending to larger bounds remains future work (see Section 17).

---

## 7. Liveness

If at least ⌈2n/3⌉ validators are honest and connected after GST, the protocol makes progress. A vertex produced in round r accumulates honest descendants in rounds r+1 and r+2, reaching the finality threshold within 2-3 rounds. The stall recovery mechanism (8 lines of Rust) ensures liveness during bootstrap — compare to Shoal++'s ~2,000-line reputation-based recovery.

---

## 8. Equivocation Handling

When a vertex v is submitted but another vertex v' from the same validator in the same round already exists:

1. Equivocation evidence [H(v), H(v')] is stored
2. The validator is **permanently marked as Byzantine**
3. The insertion is rejected
4. Evidence is broadcast to all peers via `EquivocationEvidence` messages
5. All future vertices from this validator are rejected

---

## 9. State Machine

### 9.1 Account-Based Ledger

UltraDAG uses an account-based model with **balance** (1 UDAG = 10^8 satoshis) and **nonce** (replay protection) per address.

### 9.2 Transaction Validation

1. Blake3(pub_key) == from
2. Valid Ed25519 signature over NETWORK_ID || from || to || amount || fee || nonce
3. balance(from) >= amount + fee
4. nonce == current_nonce(from)

### 9.3 Deterministic Ordering

```rust
order(v1, v2) =
  round (ascending)    -> primary key
  H(v) lexicographic   -> tiebreaker
```

Ordering deliberately does **not** use `topo_level` (ancestor count), because `topo_level` is computed locally during insertion and is not part of signed vertex data. If two nodes have different DAG states when inserting, they could compute different `topo_level` values for the same vertex, causing a consensus split. `(round, hash)` is fully deterministic from signed data.

---

## 10. Tokenomics

### 10.1 Supply Parameters

| Parameter | Value |
|-----------|-------|
| Maximum supply | **21,000,000 UDAG** |
| Smallest unit | 1 satoshi = 10^-8 UDAG |
| Initial block reward | 1 UDAG per round |
| Halving interval | Every 10,500,000 finalized rounds (~1.66 years at 5s rounds) |
| Default round time | 5 seconds |

### 10.2 Emission-Only Distribution

**Zero pre-mine. Zero genesis allocations.** Total supply starts at 0. Every UDAG enters circulation through per-round protocol emission:

| Recipient | Share | Mechanism |
|-----------|-------|-----------|
| Validators & Stakers | **75%** | Proportional to effective stake (own + delegated) |
| DAO Treasury | 10% | Governed by Council proposals (TreasurySpend) |
| Council of 21 | 10% | Equal split among seated council members |
| Founder | 5% | Protocol development, earned through emission |

**Council vs. Validators:** The Council of 21 is a **governance body** separate from the validator set. Council members are appointed via `CouncilMembership` proposals and do not require UDAG stake — their authority comes from their seat, not their stake. Validators are the top 100 stakers by effective stake. A person may be both a council member and a validator, but the roles are independent.

**No Pre-Mine:** There are no genesis allocations. No developer pre-mine, no VC funding, no presale. All tokens are distributed through per-round emission starting from round 1. The founder earns 5% of each round's reward on the same timeline as validators. Auditable from block 0.

### 10.3 Validator Staking

| Parameter | Value |
|-----------|-------|
| Minimum stake | 2,000 UDAG |
| Minimum delegation | 100 UDAG |
| Default commission | 10% |
| Unstaking cooldown | 2,016 rounds (~2.8 hours at 5s rounds) |
| Slashing penalty | **50% on equivocation** (cascades to delegators) |
| Reward distribution | Proportional to effective stake (own + delegated) |
| Observer reward | 50% (staked but not in active set) |
| Epoch length | 210,000 rounds (~12 days at 5s rounds) |
| Max active validators | 100 (top stakers by effective stake) |

### 10.4 Emission Model

**Block Reward Formula:**
```
reward(r) = floor(1 * 10^8 / 2^(r / 10500000))
```

Distributed per round by the protocol (not per vertex). 75% to validators/stakers proportional to effective stake, 10% to DAO treasury, 10% to Council of 21, 5% to founder. Coinbase contains only transaction fees. The reward effectively reaches 0 at **halving 27** (~44.8 years), when `floor(10^8 / 2^27) = 0` satoshis due to integer division. The code guards against shift overflow by returning 0 for halvings >= 64. Slashed stake is burned.

---

## 11. Network Protocol

Peers communicate over TCP with 4-byte big-endian length-prefixed postcard (binary) messages (max 4 MB). Postcard encoding provides ~40% smaller wire format compared to the original JSON encoding.

| Message | Direction | Description |
|---------|-----------|-------------|
| `Hello` | Bidirectional | Version, current DAG round, listen port |
| `DagProposal` | Broadcast | New signed DAG vertex |
| `GetDagVertices` | Request | Request vertices from a given round |
| `DagVertices` | Response | Batch of DAG vertices for sync |
| `NewTx` | Broadcast | New transaction for mempool |
| `GetPeers / Peers` | Request/Response | Gossip-based peer discovery |
| `GetParents` | Request | Request specific vertices by hash |
| `ParentVertices` | Response | Requested parent vertices for DAG convergence |
| `EquivocationEvidence` | Broadcast | Two conflicting vertices as proof |
| `CheckpointProposal` | Broadcast | Validator proposes checkpoint |
| `CheckpointSync` | Response | Checkpoint + suffix + state for fast-sync |
| `Ping / Pong` | Keepalive | Connection liveness |

---

## 12. Implementation

### 12.1 Architecture

```
ultradag-node            <- CLI binary: validator loop + HTTP RPC
  +-- ultradag-network   <- TCP P2P: peer discovery, DAG relay, sync
       +-- ultradag-coin <- Core: consensus, state, crypto, persistence
```

### 12.2 Concurrency

Built on Tokio for async I/O. All shared state is protected by `tokio::sync::RwLock` with short lock scopes. Write locks are never held across I/O operations.

### 12.3 Persistence

State is persisted using **redb**, an embedded ACID database with 7 tables (accounts, stakes, delegations, proposals, votes, metadata, active_validators). Redb's transactional writes replace the earlier JSON snapshots and write-ahead log, providing crash-safe persistence without manual fsync or WAL replay.

### 12.4 Checkpointing and Fast-Sync

Every **100 finalized rounds**, validators produce a quorum-signed checkpoint capturing `state_root`, `dag_tip`, and `total_supply`. New nodes fast-sync by requesting the latest checkpoint and inserting only the suffix DAG — reducing sync from O(all history) to O(suffix).

---

## 13. Security Analysis

| Attack Vector | Defense |
|---|---|
| **Equivocation** | One vertex per validator per round; permanent ban + evidence broadcast |
| **Network replay** | NETWORK_ID prefix in all signable bytes |
| **DAG corruption (phantom parents)** | Parent existence check before insertion |
| **Memory exhaustion (future rounds)** | MAX_FUTURE_ROUNDS = 10; vertices beyond rejected |
| **Message flooding DoS** | 4 MB max message size; 10K mempool cap with fee eviction |
| **Nothing-at-stake** | Equivocation detection + permanent ban |
| **Phantom validator inflation** | Staking-based active validator set; quorum derived from staked validators |
| **Orphan buffer exhaustion** | Hard cap: 1,000 entries AND 50 MB byte limit |
| **Sync poisoning** | Every synced vertex verified identically to live proposals |

**Known Limitations**

- **Bounded formal verification** — TLA+ safety verified at MAX_ROUNDS=2; infinite-horizon proof is future work
- **Timer-based rounds** — clock synchronization dependency (mitigated by optimistic responsiveness)
- **Implicit votes only** — descendant coverage, not explicit attestations

**Resolved Optimizations**

- **Finality performance:** O(V^2) to O(1) via incremental descendant tracking with BitVec (2,238x faster)
- **Optimistic responsiveness:** Sub-second finality under normal conditions
- **Epoch-based validator reconfiguration:** Dynamic set transitions every 210,000 rounds (~12 days at 5s rounds)
- **Per-peer rate limiting:** Orphan buffer tracks source peer with per-peer caps (100 entries per peer)
- **Checkpoint broadcasting:** Quorum-signed checkpoints every 100 rounds for fast-sync

---

## 14. Testnet Results

A 5-node Fly.io testnet runs continuously:

| Metric | Value |
|--------|-------|
| Active Validators | 5 |
| Tests Passing | 805+ |
| Avg Round Time | 5.0s |
| UDAG Supply Cap | 21M |

---

## 15. Minimalism vs. Throughput

### 15.1 What Minimalism Costs

```
TPS_max = (max_txs_per_vertex * validators_per_round) / round_duration
```

These figures assume `MAX_TXS_PER_BLOCK = 10,000` transactions per vertex. At ~150-200 bytes per minimal transaction, a full vertex payload is ~1.5-2 MB, which fits within the 4 MB max message size with room for vertex metadata, parent hashes (up to 32), and the Ed25519 signature.

| Round Duration | Theoretical Max TPS (4 validators) |
|---|---|
| 5 seconds | 8,000 |
| 2 seconds | 20,000 |
| 1 second | **40,000** |

These are theoretical upper bounds assuming all validators produce full vertices every round, which requires sustained network bandwidth of ~8-16 MB/s per node.

### 15.2 Why These Tradeoffs Are Acceptable

**Modest per-node transaction volume.** IoT devices generate transactions at rates measured in single digits per second. A sensor reporting every 5 seconds, a smart meter settling micropayments every minute — these fit comfortably within a single vertex per round.

**Code complexity is attack surface.** The ~10x reduction from Shoal++ directly reduces the number of places where a bug could cause a safety violation. For networks where the cost of a consensus bug exceeds the cost of lower throughput, this tradeoff is unambiguously correct.

**Round timing is tunable.** The `--round-ms` flag lets operators choose their position on the latency-throughput curve.

---

## 16. Comparison with Related Work

| Property | PBFT | Tendermint | HotStuff | DAG-Rider | Narwhal/Tusk | Bullshark | Shoal++ | **UltraDAG** |
|---|---|---|---|---|---|---|---|---|
| Leader | Per-view | Round-robin | Rotating | None | None+leader | Anchor | Anchor+rep. | **None** |
| Finality | 3 phases | 2 phases | Pipeline | Wave-based | Separate | 2 rounds | 1 round | **Desc. coverage** |
| Votes | Explicit | Explicit | Threshold | Implicit | Mixed | Implicit | Implicit | **Implicit** |
| Messages | O(n^2) | O(n^2) | O(n) | O(n) | O(n) | O(n) | O(n) | **O(n)** |
| Consensus lines | ~5k | ~10k | ~8k | ~10k | ~15k | ~20k | ~30k | **~2,800** |
| 3-sentence rule | No | No | No | No | No | No | No | **Yes** |
| Separate mempool | No | No | No | No | Yes | Yes | Yes | **No** |
| Waves / anchors | N/A | N/A | N/A | 4-round | N/A | 2-round | Pipelined | **None** |

---

## 17. Future Work

1. **State root proofs** — Merkle proofs for light client verification from checkpoints
2. **Extended formal verification** — liveness checking and larger bounds (N>4, rounds>2)
3. **Data availability separation** — optional Narwhal-style mode for high-throughput deployments
4. **Wire protocol versioning** — forward-compatible upgrades

---

## 18. Conclusion

UltraDAG demonstrates that a complete, working cryptocurrency can be built on a leaderless DAG-BFT consensus protocol with minimal complexity. The entire consensus core — five Rust files — implements DAG construction, BFT finality via descendant coverage, deterministic ordering, validator management, and Ed25519-signed vertices.

The protocol's safety relies on the standard BFT quorum intersection property applied to an implicit voting mechanism where DAG topology replaces explicit vote messages. The system has been validated through **805+ automated tests** (all passing) and a 5-node Fly.io testnet.

**UltraDAG is not the fastest DAG-BFT protocol. It is the simplest correct one.** For networks where auditability, small binary size, and minimal attack surface matter more than maximum throughput — IoT micropayments, embedded systems, resource-constrained validators — this is the right tradeoff.

---

## References

1. Castro, M., & Liskov, B. (1999). *Practical Byzantine Fault Tolerance.* OSDI.
2. Buchman, E. (2016). *Tendermint: Byzantine Fault Tolerance in the Age of Blockchains.*
3. Yin, M., et al. (2019). *HotStuff: BFT Consensus with Linearity and Responsiveness.* PODC.
4. Keidar, I., et al. (2021). *All You Need is DAG.* PODC.
5. Danezis, G., et al. (2022). *Narwhal and Tusk: A DAG-based Mempool and Efficient BFT Consensus.* EuroSys.
6. Spiegelman, A., et al. (2022). *Bullshark: DAG BFT Protocols Made Practical.* CCS.
7. Spiegelman, A., et al. (2024). *Shoal++: High Throughput DAG BFT Can Be Fast!* arXiv.
8. Bernstein, D. J., et al. (2012). *High-speed high-security signatures.* CHES.
9. O'Connor, J. (2019). *BLAKE3: One function, fast everywhere.*
