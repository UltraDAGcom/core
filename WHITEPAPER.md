# UltraDAG: A Leaderless DAG-BFT Cryptocurrency

**Version 1.0 — March 2026**

---

## Abstract

UltraDAG is a cryptocurrency built on a novel leaderless DAG-BFT consensus protocol. Unlike traditional blockchains where a single leader proposes blocks sequentially, UltraDAG allows all validators to produce cryptographically signed vertices concurrently. These vertices form a directed acyclic graph (DAG) where each vertex references all known tips as parents. Finality is achieved through a descendant-coverage rule: a vertex is considered final when a Byzantine fault tolerant supermajority of validators have built upon it. The protocol requires no leader election, no view changes, and no explicit voting rounds — the DAG structure itself serves as an implicit, persistent vote.

UltraDAG implements an account-based ledger with Bitcoin-inspired tokenomics (21 million supply cap, halving schedule) and Ed25519 cryptography throughout. The system has been verified through 238 automated tests covering BFT safety rules, fault tolerance, cryptographic correctness, and double-spend prevention, and has been validated on a 4-node testnet through 200+ consensus rounds with stable 2-3 round finality latency.

---

## 1. Introduction

### 1.1 Motivation

Traditional Byzantine Fault Tolerant (BFT) consensus protocols — such as PBFT, Tendermint, and HotStuff — operate in a leader-based paradigm. In each round or view, a designated leader proposes a block, and other validators vote on it. This creates three fundamental limitations:

1. **Single point of failure per round.** If the leader is slow, crashed, or Byzantine, the round stalls until a view change occurs.
2. **Sequential throughput.** Only one block is produced per round, regardless of the number of validators.
3. **Protocol complexity.** View change mechanisms add significant complexity and are historically the most bug-prone components of BFT protocols.

DAG-based consensus protocols address these limitations by allowing all validators to produce blocks (vertices) concurrently. Recent protocols such as DAG-Rider, Tusk, Bullshark, and Narwhal have demonstrated that DAG structures can achieve consensus without explicit voting rounds, using the DAG topology itself as an implicit voting mechanism.

### 1.2 Contribution

UltraDAG implements a complete, working cryptocurrency using a custom leaderless DAG-BFT protocol with the following properties:

- **Leaderless vertex production.** All validators produce vertices concurrently every round, with no leader election.
- **Implicit finality via descendant coverage.** A vertex is finalized when ceil(2n/3) distinct validators have at least one descendant of it in the DAG.
- **Parent finality guarantee.** A vertex is only finalized after all its parents are finalized, ensuring correct causal ordering for state application.
- **Single-round vertex propagation.** Vertices are broadcast once, not voted on. The DAG structure accumulates "votes" automatically as subsequent vertices reference prior ones.
- **Equivocation detection with permanent banning.** Validators that produce conflicting vertices in the same round are detected, evidence is broadcast, and they are permanently excluded.

The protocol is implemented in Rust across three crates totaling approximately 5,000 lines of production code, with 238 tests covering all critical safety and liveness properties.

---

## 2. System Model

### 2.1 Participants

Let **V** = {v₁, v₂, ..., vₙ} be the set of **n** validators. Each validator vᵢ holds an Ed25519 keypair (skᵢ, pkᵢ) and is identified by an address:

```
addrᵢ = Blake3(pkᵢ)
```

We assume the standard BFT fault model: at most **f** validators are Byzantine, where:

```
n ≥ 3f + 1
```

Byzantine validators may equivocate (produce conflicting vertices), withhold messages, or send arbitrary data. Honest validators follow the protocol faithfully.

### 2.2 Network Model

The protocol assumes **partial synchrony**: there exists an unknown Global Stabilization Time (GST) after which all messages between honest validators are delivered within a bounded delay **δ**. Before GST, messages may be delayed arbitrarily.

Communication is peer-to-peer over TCP with 4-byte length-prefixed JSON messages. Each connection is split into independent read and write halves for concurrent operation.

### 2.3 Cryptographic Primitives

| Primitive | Algorithm | Purpose |
|-----------|-----------|---------|
| Digital Signatures | Ed25519 (ed25519-dalek 2.2.0) | Vertex and transaction authentication |
| Hashing | Blake3 | Address derivation, vertex identity, Merkle trees |
| Network Replay Prevention | NETWORK_ID prefix | Cross-network signature isolation |

All vertex signatures include a fixed network identifier prefix (`b"ultradag-testnet-v1"`) in the signed data to prevent cross-network replay attacks.

---

## 3. Protocol Description

### 3.1 DAG Structure

The core data structure is a directed acyclic graph **G = (V, E)** where:

- **V** is the set of all accepted vertices
- **E** = {(u, v) : H(u) ∈ v.parents} — directed edges from parents to children

Each vertex is a tuple:

```
v = (block, parents, round, validator, pub_key, signature)
```

where:
- **block** contains a block header, coinbase transaction, and user transactions
- **parents** is an ordered list of vertex hashes (all DAG tips at time of creation)
- **round** is a non-negative integer indicating the logical round number
- **validator** is the Blake3 hash of the proposing validator's public key
- **pub_key** is the Ed25519 public key
- **signature** is an Ed25519 signature over the vertex's signable bytes

**Vertex Identity.** The hash of a vertex is computed as:

```
H(v) = Blake3(block_hash ‖ round_LE64 ‖ validator ‖ parent₀ ‖ parent₁ ‖ ... ‖ parentₖ)
```

This ensures that two vertices with the same block content but different rounds, validators, or parent sets produce distinct hashes.

**Signable Bytes.** The data authenticated by the validator's signature is:

```
signable(v) = NETWORK_ID ‖ block_hash ‖ parent₀ ‖ ... ‖ parentₖ ‖ round_LE64 ‖ validator
```

### 3.2 Vertex Production

Each honest validator produces exactly one vertex per round on a configurable timer (default: 5 seconds). The production procedure is:

1. **Determine round number.** Set `r = current_dag_round + 1`.
2. **2f+1 round gate.** If r > 1 and `|distinct_validators_in_round(r-1)| < ceil(2n/3)`, skip this round. After 3 consecutive skips, produce unconditionally (stall recovery).
3. **Equivocation check.** If this validator already produced a vertex in round r, skip.
4. **Collect parents.** Set parents = all current DAG tips.
5. **Build block.** Include coinbase reward and pending mempool transactions.
6. **Sign and broadcast.** Sign the vertex with Ed25519 and broadcast to all peers.

The 2f+1 round gate is a **liveness mechanism**, not a safety mechanism. It coordinates round progression among validators so the DAG remains dense. Without it, a fast validator could advance many rounds ahead, creating a sparse DAG where finality is delayed. The stall recovery mechanism (unconditional production after 3 skips) ensures the network bootstraps even with staggered validator startup.

### 3.3 Vertex Acceptance

A vertex v is accepted into the DAG if and only if **all** of the following hold:

1. **No duplicate:** H(v) is not already in the DAG
2. **Valid signature:** Ed25519 signature verification succeeds, and Blake3(pub_key) == validator
3. **Parent existence:** Every parent hash either exists in the DAG or equals the genesis sentinel `[0; 32]`
4. **Round bound:** v.round ≤ current_round + 10 (prevents memory exhaustion via future-round flooding)
5. **No equivocation:** No other vertex from the same validator in the same round exists in the DAG
6. **Not Byzantine:** The validator has not been marked Byzantine via prior equivocation detection

If rule 5 is violated, the validator is **permanently marked as Byzantine**, equivocation evidence is stored, and an `EquivocationEvidence` message is broadcast to all peers.

---

## 4. Finality

### 4.1 Descendant-Coverage Finality Rule

UltraDAG's finality mechanism is based on **descendant coverage**: a vertex is finalized when enough of the validator set has "built upon" it by producing descendant vertices.

**Definition (Descendant Validators).** For a vertex v in DAG state G:

```
DV(v, G) = { u.validator : u ∈ descendants(v, G) }
```

The set of distinct validator addresses that have produced at least one descendant of v.

**Definition (Quorum Threshold).**

```
q(n) = ⌈2n/3⌉ = ⌊(2n + 2) / 3⌋
```

When a `configured_validators` count is set (e.g., via `--validators N`), the threshold uses that fixed count instead of the dynamically registered count. This prevents phantom validator registrations from inflating the threshold.

**Definition (Finality).** A vertex v is **finalized** in state (G, F) if and only if:

1. v ∉ F (not already finalized)
2. n ≥ min_validators (default 3), so q ≠ ∞
3. |DV(v, G)| ≥ q(n)
4. ∀p ∈ v.parents : p ∈ F (**parent finality guarantee**)

Formally:

```
FINALIZED(v) ⟺ |DV(v, G)| ≥ ⌈2n/3⌉ ∧ (v.parents = ∅ ∨ ∀p ∈ v.parents : FINALIZED(p))
```

### 4.2 Multi-Pass Finalization

The finalization procedure `find_newly_finalized(G, F)` must be called in a **loop** because finalizing a parent in pass k may enable its children to be finalized in pass k+1. Each pass:

1. Collects all non-finalized vertices reachable from DAG tips
2. Collects them into a `BTreeSet` for deterministic iteration order
3. Checks conditions 1-4 for each candidate
4. Adds qualifying vertices to F
5. Sorts output in ancestor-first order (ancestors before descendants; ties broken by hash)

The loop terminates because:
- Each pass finalizes at least one new vertex (or the loop exits)
- The set of finalizable vertices is finite and monotonically increasing
- The parent relation is acyclic (it's a DAG), so there are no circular dependencies

### 4.3 Why Parent Finality Is Necessary

The `StateEngine` applies finalized vertices sequentially. Each vertex's transactions are applied atomically against the current account state. If vertex v references parent p, then v's transactions may depend on state changes introduced by p's transactions. Finalizing v before p would mean applying transactions against an incomplete state, potentially:

- Accepting transactions that should fail (spending coins not yet credited by p's coinbase)
- Rejecting transactions that should succeed
- Producing different state across nodes that finalize in different orders

The parent finality guarantee ensures: **when vertex v is finalized, all state changes from v's causal history have already been committed.**

---

## 5. Safety

### 5.1 Agreement

**Claim.** If two honest nodes finalize vertex v, they finalize it in the same position in their respective total orderings.

**Argument.** The total ordering of finalized vertices is deterministic given the same DAG and the same finalization set — it depends only on round numbers, ancestor counts, and vertex hashes, all of which are deterministic. Two honest nodes with the same DAG and finalization set produce identical orderings. The question reduces to whether two honest nodes can finalize different sets.

### 5.2 No Conflicting Finality

**Claim.** Two equivocating vertices (same validator, same round, different hash) cannot both be finalized.

**Argument.** The equivocation detection rule (acceptance rule 5) ensures that **at most one** vertex from a given validator in a given round exists in any honest node's DAG. When equivocation is detected, the second vertex is rejected and the validator is permanently banned. Evidence is broadcast to all peers.

Therefore, at any honest node, at most one of two conflicting vertices exists. Since finality is evaluated only over the local DAG, only one can ever satisfy the finality rule.

### 5.3 Consistency Across Nodes (Quorum Intersection)

**Claim.** If honest node A finalizes vertex v, then no honest node B can finalize a state inconsistent with v's inclusion.

**Argument (sketch).** If v is finalized at node A, then |DV(v, G_A)| ≥ ⌈2n/3⌉. If a hypothetical conflicting vertex v' were finalized at node B, then |DV(v', G_B)| ≥ ⌈2n/3⌉. By the quorum intersection property:

```
|DV(v, G_A)| + |DV(v', G_B)| ≥ 2 · ⌈2n/3⌉ > n + f
```

Therefore DV(v, G_A) ∩ DV(v', G_B) must contain at least one **honest** validator h. Validator h has descendants of both v and v', meaning h's DAG contains both. If v and v' are equivocating (same validator, same round), this contradicts the equivocation detection rule — h would have rejected one.

For **transaction-level conflicts** (valid vertices from different validators containing conflicting transactions), safety is ensured by deterministic ordering: the vertex finalized first in the total order wins, and subsequent conflicting transactions fail state validation.

**Limitation.** This is an argument sketch, not a formal proof. A complete safety proof would require formal modeling of the network, precise conflict definitions, and analysis of dynamic validator set transitions.

---

## 6. Liveness

### 6.1 Progress Under Honest Majority

**Claim.** If at least ⌈2n/3⌉ validators are honest and connected (after GST), the protocol makes progress.

**Argument.**

1. **Vertex production.** Each honest validator produces one vertex per round on a timer.
2. **Tip coverage.** After GST, honest validators receive each other's vertices within δ time. If the round duration exceeds δ, each validator's round-r vertex references round-(r-1) vertices from all honest validators.
3. **Descendant accumulation.** A vertex v produced in round r accumulates honest descendants in rounds r+1 and r+2. After 2-3 rounds, |DV(v)| ≥ ⌈2n/3⌉.
4. **Finality.** The finality rule is satisfied within 2-3 rounds of production.

**Observed empirically.** In testnet with 4 validators and 2-5 second rounds, finality lag stabilizes at 2-3 rounds through 200+ rounds of operation.

### 6.2 Stall Recovery

The protocol includes a stall recovery mechanism: after 3 consecutive round skips (due to the 2f+1 gate), a validator produces unconditionally. This prevents permanent deadlocks during network bootstrap or after partitions heal.

---

## 7. Equivocation Handling

### 7.1 Detection

When a vertex v is submitted for insertion but another vertex v' from the same validator in the same round already exists (H(v) ≠ H(v')):

1. Equivocation evidence `[H(v), H(v')]` is stored
2. The validator is permanently marked as Byzantine
3. The insertion is rejected
4. All future vertices from this validator are rejected

### 7.2 Evidence Propagation

Equivocation evidence is broadcast to all peers via `EquivocationEvidence` messages containing both conflicting vertices. Receiving nodes independently verify:
- Both vertices are from the same validator
- Both vertices are in the same round
- They have different hashes

If valid, the receiving node marks the validator as Byzantine.

### 7.3 Limitation

Equivocation detection is local to each node's view. If a Byzantine validator sends vertex v to one set of nodes and v' to a disjoint set, detection requires some honest node to receive both. The protocol relies on evidence propagation to eventually achieve network-wide detection, but there is no guaranteed detection bound before GST.

---

## 8. State Machine

### 8.1 Account-Based Ledger

UltraDAG uses an account-based model (similar to Ethereum) rather than UTXO:

| Field | Description |
|-------|-------------|
| **balance** | Account balance in satoshis (1 UDAG = 10⁸ sats) |
| **nonce** | Transaction counter for replay protection |

### 8.2 Transaction Format

```
Transaction = {
    from:      Address,        // Sender (Blake3 of public key)
    to:        Address,        // Recipient
    amount:    u64,            // Transfer amount in satoshis
    fee:       u64,            // Fee paid to block proposer
    nonce:     u64,            // Must equal sender's current nonce
    pub_key:   [u8; 32],       // Ed25519 public key
    signature: [u8; 64],       // Ed25519 signature over signable bytes
}
```

Transaction validation requires:
1. `Blake3(pub_key) == from`
2. Valid Ed25519 signature over `NETWORK_ID ‖ from ‖ to ‖ amount ‖ fee ‖ nonce`
3. `balance(from) ≥ amount + fee`
4. `nonce == current_nonce(from)`

### 8.3 State Derivation

The `StateEngine` derives all account state from finalized DAG vertices:

1. Finalized vertices are collected via multi-pass finalization
2. Vertices are ordered deterministically: (round ascending, ancestor count ascending, hash lexicographic)
3. Each vertex is applied atomically via snapshot-then-commit:
   - Credit coinbase reward (block_reward + fees) to proposer
   - Cap block reward if total_supply would exceed MAX_SUPPLY_SATS
   - Validate and apply each transaction (signature, balance, nonce)
   - If any transaction fails, the entire vertex application is rolled back

### 8.4 Deterministic Ordering

All honest nodes must apply finalized vertices in the same order to produce identical state. The ordering function:

```
order(v₁, v₂) =
  if v₁.round ≠ v₂.round:     compare by round (ascending)
  if depth(v₁) ≠ depth(v₂):   compare by ancestor count (ascending)
  otherwise:                    compare by H(v₁) vs H(v₂) (lexicographic)
```

This ordering is deterministic because all inputs (round, ancestor count, hash) are computed from DAG structure and are identical across nodes with the same DAG.

---

## 9. Tokenomics

UltraDAG follows a Bitcoin-inspired token emission model:

| Parameter | Value |
|-----------|-------|
| Maximum supply | 21,000,000 UDAG |
| Smallest unit | 1 satoshi = 10⁻⁸ UDAG |
| Initial block reward | 50 UDAG |
| Halving interval | Every 210,000 finalized rounds |
| Default round time | 5 seconds |
| Estimated halving period | ~12.2 days at 5s rounds |
| Supply cap enforcement | Reward capped when approaching MAX_SUPPLY |

The block reward for height h is:

```
reward(h) = ⌊50 × 10⁸ / 2^(h / 210000)⌋
```

with reward = 0 after 64 halvings. Each finalized vertex's coinbase transaction credits the proposing validator with `reward(height) + sum(fees)`.

**Supply cap enforcement.** If `total_supply + reward > MAX_SUPPLY_SATS`, the reward is reduced to `MAX_SUPPLY_SATS - total_supply`. This guarantees the 21 million supply cap is never exceeded.

---

## 10. Network Protocol

### 10.1 Transport

Peers communicate over TCP with a simple framing protocol:
- 4-byte big-endian length prefix
- JSON-encoded message body
- Maximum message size: 4 MB (enforced before deserialization)

Each TCP connection is split into independent PeerReader and PeerWriter halves, allowing concurrent message reception and transmission without lock contention.

### 10.2 Message Types

| Message | Direction | Description |
|---------|-----------|-------------|
| `Hello` | Bidirectional | Version, current DAG round, listen port |
| `HelloAck` | Response | Version, current DAG round |
| `DagProposal` | Broadcast | New signed DAG vertex |
| `GetDagVertices` | Request | Request vertices from a given round |
| `DagVertices` | Response | Batch of DAG vertices |
| `NewTx` | Broadcast | New transaction for mempool |
| `GetPeers` | Request | Request known peer addresses |
| `Peers` | Response | List of known peer addresses |
| `EquivocationEvidence` | Broadcast | Two conflicting vertices as proof of equivocation |
| `Ping` / `Pong` | Keepalive | Connection liveness check |

### 10.3 Peer Discovery

UltraDAG uses a gossip-based peer discovery mechanism:

1. On connection, nodes exchange `Hello` messages including their listen port
2. After handshake, nodes request each other's peer lists via `GetPeers`
3. Learned peer addresses are added to the known set
4. Nodes attempt to connect to learned peers (up to MAX_PEERS=8)
5. Periodic peer exchange occurs every 30 seconds

Duplicate connections are prevented by tracking canonical listen addresses. When a node receives a `Hello` with a listen port, it registers the peer's `ip:listen_port` as connected, preventing `try_connect_peer` from creating redundant connections.

### 10.4 DAG Synchronization

When a node connects to a peer that is ahead (higher DAG round), it requests missing vertices:

```
GetDagVertices { from_round: our_round + 1, max_count: 100 }
```

The receiving node responds with all vertices in the requested round range. The requesting node verifies signatures, checks equivocation, and inserts valid vertices into its DAG. Vertices with missing parents are buffered as orphans (up to 1000 entries / 50MB) and retried when new vertices arrive.

---

## 11. Implementation

### 11.1 Architecture

```
┌─────────────────────────────┐
│      ultradag-node          │  CLI binary: validator loop + RPC
│  main.rs, validator.rs,     │
│  rpc.rs                     │
├─────────────────────────────┤
│      ultradag-network       │  TCP P2P: peer discovery, DAG relay
│  node/server.rs             │
│  peer/{registry,connection} │
│  protocol/{message}         │
├─────────────────────────────┤
│      ultradag-coin          │  Core: consensus, state, crypto
│  consensus/{dag,finality,   │
│   vertex,validator_set,     │
│   ordering}                 │
│  state/engine.rs            │
│  address/, block/, tx/      │
│  persistence.rs             │
└─────────────────────────────┘
```

### 11.2 Concurrency Model

The node is built on Tokio for asynchronous I/O:

- **Validator loop**: `tokio::interval` fires every round; produces and broadcasts vertices
- **Listener**: accepts incoming TCP connections, spawns per-peer handlers
- **Peer handlers**: each connection runs an async recv loop in a spawned task
- **RPC server**: Hyper HTTP server for wallet and monitoring access

All shared state (DAG, finality tracker, state engine, mempool) is protected by `tokio::sync::RwLock` with short lock scopes. Write locks are never held across I/O operations.

### 11.3 Persistence

All node state is periodically saved to disk:

- **BlockDag**: complete DAG structure (vertices, children, tips, rounds, Byzantine validators, evidence)
- **FinalityTracker**: finalized vertex hashes
- **StateEngine**: account balances, nonces, total supply, last finalized round
- **Mempool**: pending transactions

Writes use atomic file operations (write to `.tmp`, then rename) to prevent corruption on crash. State is saved every 10 rounds and on graceful shutdown (SIGTERM).

### 11.4 Configured Validator Count

For testnet deployment, the `--validators N` flag fixes the quorum threshold at `ceil(2N/3)` regardless of how many validators are dynamically registered. This prevents a class of bugs where phantom validator registrations (from stale persistence, sync artifacts, or network partitions) inflate the quorum beyond what active validators can satisfy.

Dynamic registration still occurs — validators are auto-registered when their vertices appear — but the quorum computation uses the configured count. This is the correct testnet solution; the production solution requires epoch-based validator set management.

---

## 12. Security Analysis

### 12.1 Verified Properties

The following properties have been verified through comprehensive testing (238 tests, all using real Ed25519 cryptography — no mocks):

| Property | Tests | Status |
|----------|-------|--------|
| Equivocation prevention | 12 | Verified |
| 2f+1 reference gate | 3 | Verified |
| Signature verification (all tampering) | 14 | Verified |
| Finality threshold correctness | 9 | Verified |
| Deterministic ordering | 3 | Verified |
| Crash fault tolerance (f=1) | 5 | Verified |
| Byzantine equivocation detection | 2 | Verified |
| Nonce enforcement (replay prevention) | 12 | Verified |
| Balance enforcement | 12 | Verified |
| DAG concurrent double-spend | 2 | Verified |
| Parent finality guarantee | 3 | Verified |
| Supply cap enforcement | 2 | Verified |
| Phantom validator resilience | 2 | Verified |
| Address derivation (Blake3) | 14 | Verified |
| Persistence correctness | 5 | Verified |

### 12.2 Attack Resistance

| Attack | Defense |
|--------|---------|
| **Equivocation** | One vertex per validator per round; permanent ban + evidence broadcast |
| **Network replay** | NETWORK_ID prefix in all signable bytes |
| **DAG corruption (phantom parents)** | Parent existence check before insertion |
| **Memory exhaustion (future rounds)** | MAX_FUTURE_ROUNDS = 10 |
| **Message flooding DoS** | 4MB max message size, 10K mempool cap with fee eviction |
| **Nothing-at-stake** | Equivocation detection + permanent ban |
| **Long-range attacks** | Irreversible finality (once finalized, always finalized) |
| **Non-deterministic finality** | BTreeSet for candidate iteration; deterministic hash ordering |
| **Phantom validator inflation** | Configured validator count fixes quorum threshold |

### 12.3 Known Limitations

1. **No formal safety proof.** The safety argument is based on quorum intersection but has not been mechanically verified.
2. **No epoch-based reconfiguration.** The validator set is fixed at startup (for testnet) or grows monotonically. Safe validator set transitions require epoch boundaries.
3. **No slashing.** Byzantine validators are banned but face no economic penalty.
4. **Timer-based rounds.** Round progression depends on approximate clock synchronization. Significant clock drift degrades DAG density.
5. **No optimistic responsiveness.** The protocol does not advance faster when all validators are honest and the network is fast.
6. **Implicit votes only.** Finality is determined by descendant coverage, which does not distinguish between intentional endorsement and incidental graph connectivity.
7. **Quadratic finality computation.** Descendant traversal for finality checking is O(V + E) per candidate vertex, which does not scale to thousands of validators without optimization.

---

## 13. Testnet Results

A 4-node local testnet was run for 200+ rounds with the following results:

| Metric | Value |
|--------|-------|
| Validators | 4 (stable throughout) |
| Round duration | 2-5 seconds |
| Finality latency | 2-3 rounds |
| Peers per node | 3 (full mesh) |
| Total supply at round 204 | 10,200 UDAG |
| Finality stalls | 0 |
| Phantom validator incidents | 0 (with configured validator fix) |

Detailed checkpoint data:

| Round | Validators | Finalized | Lag | Supply |
|-------|-----------|-----------|-----|--------|
| 51    | 4         | 49        | 2   | ~2,450 UDAG |
| 102   | 4         | 100       | 2   | ~5,000 UDAG |
| 150   | 4         | 148       | 2   | ~7,400 UDAG |
| 203   | 4         | 201       | 3   | ~10,050 UDAG |

---

## 14. Comparison with Related Work

| Protocol | Leader | Finality | Votes | Complexity |
|----------|--------|----------|-------|------------|
| **PBFT** | Per-view leader | 3 phases | Explicit | O(n²) messages |
| **Tendermint** | Round-robin leader | 2 phases | Explicit | O(n²) messages |
| **HotStuff** | Rotating leader | Pipeline | Explicit (threshold) | O(n) messages |
| **DAG-Rider** | No leader | Wave-based | Implicit (DAG) | O(n) per vertex |
| **Narwhal/Tusk** | No leader (DAG) + leader (ordering) | Separate | Mixed | O(n) amortized |
| **Bullshark** | Anchor-based | 2 rounds | Implicit (DAG) | O(n) amortized |
| **UltraDAG** | No leader | Descendant coverage | Implicit (DAG) | O(n) per vertex |

UltraDAG is most similar to DAG-Rider in its leaderless design but uses a simpler finality rule (descendant coverage vs. wave-based common coins). Unlike Narwhal/Tusk and Bullshark, UltraDAG does not separate data availability from consensus — each vertex carries its own transactions. This simplifies the architecture at the cost of potentially lower throughput for large transaction volumes.

---

## 15. Future Work

1. **Epoch-based validator set reconfiguration.** Safe transitions between validator sets with explicit finality for the old set before the new set activates.
2. **Per-peer rate limiting.** Defense against message flooding from individual peers.
3. **Incremental descendant tracking.** Replace O(V+E) per-vertex descendant computation with incremental updates to reduce finality check overhead.
4. **Formal verification.** Machine-checkable safety proof using a model checker or proof assistant.
5. **Data availability separation.** Separate transaction data dissemination from consensus ordering (Narwhal-style) for higher throughput.
6. **Optimistic responsiveness.** Advance rounds at network speed when all validators are honest, falling back to timer-based rounds under adversarial conditions.
7. **Slashing mechanism.** Economic penalties for equivocation, requiring a staking mechanism.
8. **Wire protocol versioning.** Version field in message framing for forward-compatible protocol upgrades.

---

## 16. Conclusion

UltraDAG demonstrates that a complete, working cryptocurrency can be built on a leaderless DAG-BFT consensus protocol with minimal complexity. The descendant-coverage finality rule provides an intuitive and implementable path to BFT finality without leader election, view changes, or explicit voting rounds. The parent finality guarantee ensures correct state derivation, and the configured validator count mechanism provides practical stability for testnet deployment.

The protocol's safety relies on the standard BFT quorum intersection property — the same foundation used by PBFT, Tendermint, and HotStuff — applied to an implicit voting mechanism where DAG topology replaces explicit vote messages. While a formal safety proof remains future work, the system has been thoroughly tested with 238 automated tests covering all critical BFT properties and validated on a multi-node testnet through 200+ consensus rounds.

---

## References

1. Castro, M., & Liskov, B. (1999). Practical Byzantine Fault Tolerance. OSDI.
2. Buchman, E. (2016). Tendermint: Byzantine Fault Tolerance in the Age of Blockchains. M.Sc. Thesis.
3. Yin, M., et al. (2019). HotStuff: BFT Consensus with Linearity and Responsiveness. PODC.
4. Keidar, I., et al. (2021). All You Need is DAG. PODC.
5. Danezis, G., et al. (2022). Narwhal and Tusk: A DAG-based Mempool and Efficient BFT Consensus. EuroSys.
6. Spiegelman, A., et al. (2022). Bullshark: DAG BFT Protocols Made Practical. CCS.
7. Bernstein, D. J., et al. (2012). High-speed high-security signatures. CHES.
8. O'Connor, J. (2019). BLAKE3: One function, fast everywhere. Specification.

---

*UltraDAG is open source software. The protocol specification and implementation are available at github.com/ultradag/ultradag.*
