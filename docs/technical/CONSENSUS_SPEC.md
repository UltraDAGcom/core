# UltraDAG Consensus Protocol: Formal Specification

## 1. Overview

UltraDAG implements a leaderless, round-based DAG-BFT consensus protocol. Validators produce cryptographically signed vertices that reference all known DAG tips, forming a directed acyclic graph. Finality is determined by a descendant-coverage rule: a vertex is finalized when a sufficient fraction of the validator set has built upon it. The protocol requires no leader election, no view changes, and no explicit voting messages — the DAG structure itself serves as an implicit vote.

This document describes the protocol as implemented in the `ultradag-coin` crate (`consensus/` module). All claims are grounded in the source code. Where a safety or liveness property cannot be proven from the implementation alone, this is stated explicitly.

## 2. System Model

### 2.1 Participants

Let **V** = {v_1, v_2, ..., v_n} be the set of **n** registered validators. Each validator v_i holds an Ed25519 keypair (sk_i, pk_i) and is identified by an address addr_i = Blake3(pk_i).

We assume the standard BFT fault model: at most **f** validators are Byzantine, where **n >= 3f + 1**. Byzantine validators may equivocate, withhold messages, or send arbitrary data. Honest validators follow the protocol.

### 2.2 Network

The protocol assumes **partial synchrony**: there exists an unknown Global Stabilization Time (GST) after which all messages between honest validators are delivered within a bounded delay delta. Before GST, messages may be delayed arbitrarily.

### 2.3 Validator Set

The validator set is **open and dynamic**: validators are registered when their first vertex is observed. The `ValidatorSet` tracks membership via a `HashSet<Address>` with idempotent registration.

**Quorum threshold** for n registered validators:

```
q(n) = ceil(2n/3) = floor((2n + 2) / 3)
```

The system requires a minimum of `min_validators` (default: 3) before finality is enabled. If n < min_validators, the quorum threshold is defined as infinity (no finality possible).

**Implementation reference:** `validator_set.rs:38-44`

## 3. Data Structures

### 3.1 DagVertex

A vertex is a tuple:

```
V = (block, parents, round, validator, pub_key, signature)
```

where:
- **block**: contains a block header, coinbase transaction, and a list of user transactions
- **parents**: an ordered list of hashes referencing existing vertices in the DAG (all current tips at time of creation)
- **round**: a non-negative integer indicating the logical round
- **validator**: the address of the proposing validator (addr = Blake3(pub_key))
- **pub_key**: the Ed25519 public key of the validator
- **signature**: Ed25519 signature over `signable_bytes(V)`

**Vertex identity.** The hash of a vertex is:

```
H(V) = Blake3(block_hash || round_LE64 || validator || parent_0 || parent_1 || ... || parent_k)
```

where `||` denotes concatenation and `round_LE64` is the round encoded as a little-endian 64-bit integer.

**Implementation reference:** `vertex.rs:75-84`

**Signable bytes.** The data signed by the validator is:

```
signable(V) = NETWORK_ID || block_hash || parent_0 || ... || parent_k || round_LE64 || validator
```

where `NETWORK_ID` is a fixed byte string (`b"ultradag-testnet-v1"`) that prevents cross-network replay attacks.

**Implementation reference:** `vertex.rs:46-56`

### 3.2 BlockDag

The DAG G = (V, E) where:
- V is the set of all accepted vertices
- E = {(u, v) : H(u) in v.parents} — edges from parents to children

The DAG maintains:
- A hash-indexed vertex store
- Forward edges (parent -> children) for descendant traversal
- A tip set: vertices with no children
- Round-indexed vertex lists
- A set of Byzantine validators (detected via equivocation)

**Implementation reference:** `dag.rs:15-30`

### 3.3 Ancestors and Descendants

For a vertex v in the DAG:

```
ancestors(v) = transitive closure of the parent relation from v
descendants(v) = transitive closure of the child relation from v
```

Both are computed via iterative depth-first traversal.

**Implementation reference:** `dag.rs:196-227`

## 4. Vertex Acceptance Rules

A vertex V is accepted into the DAG if and only if **all** of the following hold:

1. **No duplicate hash:** H(V) is not already in the DAG.
2. **Valid signature:** `verify_signature(V)` succeeds — the Ed25519 signature is valid over `signable(V)` using `pub_key`, and `Blake3(pub_key) == validator`.
3. **Parent existence:** Every hash in V.parents either (a) exists in the DAG, or (b) equals the zero hash `[0; 32]` (genesis sentinel).
4. **Round bound:** V.round <= current_round + MAX_FUTURE_ROUNDS (currently 10).
5. **No equivocation:** No other vertex from the same validator in the same round already exists in the DAG.
6. **Not Byzantine:** The validator has not been previously marked as Byzantine.

If rule 5 is violated (equivocation detected), the validator is **permanently marked as Byzantine** and the equivocation evidence (both conflicting vertex hashes) is stored and broadcast to all peers.

**Implementation reference:** `dag.rs:47-143`

## 5. Finality

### 5.1 Finality Rule — Formal Definition

Let G be the current DAG state at some node, let F be its set of finalized vertex hashes, and let q = q(n) be the quorum threshold.

**Definition (Descendant Validators).** For a vertex v in G:

```
DV(v, G) = { u.validator : u in descendants(v, G) }
```

The set of distinct validator addresses that have produced at least one descendant of v.

**Definition (Finalizable).** A vertex v is **finalizable** in state (G, F) if and only if:

1. v is not already in F
2. n >= min_validators (so q != infinity)
3. |DV(v, G)| >= q
4. For every parent p in v.parents: p is in F (the **parent finality guarantee**)

**Definition (Finalization).** The finalization procedure `find_newly_finalized(G, F)` iterates over all non-finalized vertices reachable from the DAG tips (collected into a `BTreeSet` for deterministic ordering), checks conditions 1-4, and adds qualifying vertices to F. The procedure is called in a **multi-pass loop** until no new vertices are finalized, because finalizing a parent may unlock its children.

The output of each pass is sorted in ancestor-first order: if a is an ancestor of b, then a precedes b. Ties between unrelated vertices are broken by hash comparison.

**Implementation reference:** `finality.rs:72-129`

### 5.2 Formal Statement

A vertex v is finalized at a node when:

```
FINALIZED(v) iff |DV(v, G)| >= ceil(2n/3) AND (v.parents = {} OR forall p in v.parents: FINALIZED(p))
```

## 6. Safety Argument

### 6.1 Claim: Agreement

**Claim.** If two honest nodes finalize vertex v, they finalize it in the same position in their respective total orderings.

**Argument sketch (not a proof).** The total ordering of finalized vertices is deterministic given the same DAG and the same finalization set. Two honest nodes that have the same DAG state and the same set of finalized vertices will produce the same ordering (by round, then ancestor count within the finalization set, then hash). The question reduces to: can two honest nodes finalize **different sets**?

### 6.2 Claim: No Conflicting Finality

**Claim.** Two conflicting vertices (same validator, same round, different hash) cannot both be finalized.

**Argument.** Let v and v' be two vertices from the same validator in the same round with H(v) != H(v'). This is equivocation.

The equivocation detection rule (acceptance rule 5) ensures that **at most one** vertex from a given validator in a given round is accepted into any honest node's DAG. When equivocation is detected, the second vertex is rejected and the validator is marked Byzantine; evidence is broadcast to all peers.

Therefore, in any honest node's DAG, at most one of {v, v'} exists. Since finality is evaluated only over the local DAG, and an honest node's DAG contains at most one of the conflicting vertices, only one can ever satisfy the finality rule at any honest node.

### 6.3 Claim: Consistency of Finality Across Honest Nodes

**Claim.** If honest node A finalizes vertex v, then no honest node B can finalize a conflicting state (a different vertex v' in the same "slot").

**Argument sketch using the BFT intersection lemma.**

Suppose vertex v is finalized at node A. Then |DV(v, G_A)| >= ceil(2n/3). This means at least ceil(2n/3) distinct validators have produced descendants of v visible to A.

Now suppose a hypothetical conflicting vertex v' could be finalized at node B. Then |DV(v', G_B)| >= ceil(2n/3).

By the quorum intersection property: any two sets of size >= ceil(2n/3) from a universe of size n must share at least one element when n >= 3f+1. Specifically:

```
|DV(v, G_A)| + |DV(v', G_B)| >= 2 * ceil(2n/3) > n + f
```

Therefore, DV(v, G_A) and DV(v', G_B) must share at least one **honest** validator. Call this validator h. Validator h has produced a descendant of v (visible to A) and a descendant of v' (visible to B). Since h is honest and descendants reference parents transitively, h's vertex must be a descendant of both v and v'. This means h's DAG contains both v and v'. If v and v' are genuinely conflicting (same validator, same round), this contradicts the equivocation detection rule — h would have rejected one of them.

**Critical limitation.** This argument assumes that "conflicting" means equivocation (same validator, same round). For conflicts at the **transaction level** (e.g., two valid vertices from different validators containing conflicting transactions), the protocol relies on deterministic ordering of finalized vertices to resolve conflicts: the vertex finalized first in the total order wins, and subsequent conflicting transactions fail validation in `StateEngine::apply_vertex`.

**This is not a formal proof.** A complete safety proof would require a formal model of the network, precise definitions of conflict, and consideration of edge cases around dynamic validator sets. The argument above is a sketch that identifies the key structural property (quorum intersection) but does not constitute a machine-checkable proof.

### 6.4 The Dynamic Validator Set Problem

**STATUS: Mitigated for testnet via `configured_validators`. Open concern for mainnet.**

The validator set grows monotonically as new validators are observed. Without mitigation, the quorum threshold q(n) = ceil(2n/3) changes as n increases, creating two problems:

1. **Phantom validator inflation.** If stale validator addresses are registered (from persisted DAG state, sync artifacts, or network partitions), the quorum threshold rises above what active validators can satisfy, causing finality to stall permanently.

2. **Threshold instability.** A vertex that was "almost finalized" can become harder to finalize if new validators join mid-flight. The quorum intersection argument assumes a fixed n; with a changing n, the intersection guarantee may weaken during transitions.

**Testnet fix: `configured_validators`.** The `ValidatorSet` now supports an optional `configured_validators` count (set via `--validators N` CLI flag). When set, the quorum threshold uses this fixed count instead of the dynamically-growing registered count:

```
q = ceil(2 * configured_validators / 3)
```

Dynamic registration still occurs — validators are auto-registered when their vertices appear — but the quorum computation is stable. This prevents phantom validator registrations from inflating the threshold. Verified: 4-node testnet runs 200+ rounds with stable validators=4 and finality_lag=2-3.

**Implementation reference:** `validator_set.rs:19-24`, `validator_set.rs:42-50`

**Production solution.** The correct long-term fix requires **epoch-based validator set reconfiguration**: explicit finality for the old validator set before transitioning to a new set. UltraDAG does not yet implement epochs.

## 7. Liveness Argument

### 7.1 Claim: Progress Under Honest Majority

**Claim.** If at least ceil(2n/3) validators are honest and eventually connected (after GST), the protocol makes progress: new vertices are finalized.

**Argument sketch.**

1. **Vertex production.** Each honest validator produces exactly one vertex per round on a timer. The vertex references all current DAG tips.

2. **Tip coverage.** When an honest validator produces a vertex in round r, it includes all known tips as parents. After GST, honest validators receive each other's vertices within delta time. If the round duration exceeds delta, each honest validator's round-r vertex will reference round-(r-1) vertices from all other honest validators.

3. **Descendant accumulation.** Consider a vertex v produced by an honest validator in round r. In round r+1, all honest validators that received v will produce vertices referencing tips that include v (or descendants of v). After round r+1, v has at least ceil(2n/3) - 1 descendant validators (the honest ones, minus possibly itself). In round r+2, the remaining honest validators have also built on these descendants.

4. **Finality.** Within 2-3 rounds after v is produced (after GST), the set DV(v) will include at least ceil(2n/3) distinct validators, satisfying the finality rule.

**Observed empirically.** In testnet runs with 4 validators and 5-second rounds, finality lag stabilizes at 3 rounds.

### 7.2 The 2f+1 Round Gate

**Mechanism.** Before producing a vertex in round r (for r > 1), a validator checks:

```
|distinct_validators_in_round(r-1)| >= q(n)
```

If this condition is not met, the validator **skips** the round. After `MAX_SKIPS_BEFORE_RECOVERY` (currently 3) consecutive skips, the validator produces unconditionally to break deadlocks.

**Implementation reference:** `validator.rs:49-69`

**Purpose: Liveness, not safety.** The round gate serves two purposes:

1. **Prevents premature advancement.** Without the gate, a single fast validator could advance rounds rapidly while others lag behind, creating a sparse DAG where finality is delayed because vertices lack sufficient descendant diversity.

2. **Coordinates round progression.** By requiring quorum participation in round r-1 before producing in round r, validators advance in approximate lockstep, ensuring dense DAG structure.

**The round gate is NOT necessary for safety.** Removing the gate would not violate the finality rule or the quorum intersection property. However, it would degrade liveness: rounds would desynchronize, the DAG would become sparse, and finality latency would increase.

**The stall recovery mechanism is necessary for liveness.** Without it, if validators start at different times and miss each other's early rounds, the quorum check could permanently stall the network. The unconditional production after 3 skips ensures the network eventually bootstraps even with staggered startup.

## 8. Parent Finality Guarantee

### 8.1 The Rule

A vertex v may only be finalized if **all of its parents are already finalized**:

```
forall p in v.parents: p in F
```

Genesis vertices (with no parents, or parents equal to the zero hash) are exempt.

**Implementation reference:** `finality.rs:95-108`

### 8.2 Why It Is Necessary

The `StateEngine` applies finalized vertices sequentially. Each vertex's transactions and coinbase are applied atomically against the current state. If a vertex v references parent p, then v's transactions may depend on state changes introduced by p's transactions (or by transactions in p's ancestors). Finalizing v before p would mean applying v's transactions against an incomplete state, potentially:

- Accepting transactions that should have been rejected (e.g., spending coins not yet credited by p's coinbase)
- Rejecting transactions that should have been accepted
- Producing non-deterministic state across nodes that finalize in different orders

The parent finality guarantee ensures the following invariant:

**Invariant.** When vertex v is finalized, all state changes from v's causal history (ancestors) have already been committed.

### 8.3 Effect on Liveness

The parent finality guarantee introduces a **multi-pass requirement**: `find_newly_finalized` must be called in a loop because finalizing a parent in pass k may enable its children to be finalized in pass k+1.

**This does not affect liveness under normal operation.** If a vertex v satisfies |DV(v)| >= q, then its parents (which are older and have at least as many descendants) also satisfy this condition. The multi-pass loop converges because:

1. Each pass finalizes at least one new vertex (or the loop terminates).
2. The set of finalizable vertices is finite and monotonically increasing.
3. The parent relation is acyclic (it's a DAG), so there are no circular dependencies.

**Edge case.** If a vertex v has a parent p where |DV(p)| < q but |DV(v)| >= q, then v cannot be finalized until p gains more descendants. This can happen if p is from a minority branch of the DAG. In practice, the all-tips-as-parents construction ensures that most vertices share a common ancestor set, and this edge case is rare.

## 9. Deterministic Ordering

Finalized vertices must be applied to the state machine in a deterministic order that is consistent across all honest nodes. The ordering function is:

```
order(v1, v2) = {
  if v1.round != v2.round:  compare by round (ascending)
  if depth(v1) != depth(v2): compare by ancestor count within the finalization set (ascending)
  otherwise:                 compare by H(v1) vs H(v2) (lexicographic)
}
```

where `depth(v)` = |{u in finalization_set : u in ancestors(v)}|.

**Implementation reference:** `ordering.rs:9-41`

**Correctness.** This ordering is deterministic because:
- Round numbers are integers with a total order.
- Ancestor count is determined by DAG structure (identical across nodes with the same DAG).
- Vertex hashes are deterministic (same inputs produce same Blake3 hash).

**Implementation note.** In the actual runtime (`server.rs`, `validator.rs`), finalized vertices from `find_newly_finalized` are applied directly in the order returned (ancestor-first, then hash). The `order_vertices` function in `ordering.rs` is available but the primary ordering is done within the finality tracker's sort at `finality.rs:118-126`.

## 10. Equivocation Handling

### 10.1 Detection

When `try_insert` is called with a vertex V where another vertex V' from the same validator in the same round already exists (H(V) != H(V')):

1. The equivocation evidence (H(V), H(V')) is stored.
2. The validator is permanently marked as Byzantine.
3. The insertion is rejected with `DagInsertError::Equivocation`.
4. All future vertices from this validator are rejected.

**Implementation reference:** `dag.rs:101-143`

### 10.2 Evidence Propagation

Equivocation evidence is broadcast to all peers via `EquivocationEvidence` messages containing both conflicting vertices. Receiving nodes independently verify the evidence (same validator, same round, different hash) before marking the validator as Byzantine.

### 10.3 Limitation

Equivocation detection is **local to each node's view**. If a Byzantine validator sends vertex V to one set of nodes and vertex V' to a disjoint set, the equivocation is not detected until some node receives both. The protocol relies on honest nodes forwarding evidence to eventually propagate detection network-wide, but there is no guaranteed detection bound.

## 11. Known Limitations

### 11.1 Limitations Compared to Production BFT Protocols

1. **No formal safety proof.** The safety argument in Section 6 is a sketch based on quorum intersection. It has not been mechanically verified. Production protocols (e.g., HotStuff, Tendermint) come with formal proofs.

2. **No epochs or validator set reconfiguration.** The validator set grows monotonically. There is no mechanism to remove validators, rotate validator sets, or handle the quorum threshold changes that occur when new validators join. Production protocols use epoch-based reconfiguration with explicit finality for the old set before transitioning.

3. **No view change or leader recovery.** Since the protocol is leaderless, there is no view change mechanism. This simplifies the protocol but means there is no mechanism to skip a slow or crashed leader — because there are no leaders. Liveness depends on the round gate and stall recovery instead.

4. **No accountability beyond equivocation.** Byzantine behavior is only detected in the form of equivocation (two vertices in the same round). Other Byzantine behaviors — such as selectively withholding vertices, including invalid transactions, or referencing stale tips — are not explicitly detected or punished.

5. **No slashing or staking.** There is no economic penalty for Byzantine behavior. Detected equivocators are banned from the DAG, but there is no stake to slash.

6. **Timer-based rounds, not message-driven.** Round advancement is based on wall-clock timers (`tokio::interval`), not on receiving messages. This simplifies the protocol but creates a dependency on approximate clock synchronization. If validators' clocks drift significantly relative to the round duration, the DAG may become sparse.

7. **No optimistic responsiveness.** The protocol does not advance faster when all validators are honest and the network is fast. The round timer is the pacing mechanism regardless of conditions. Production protocols like HotStuff achieve "optimistic responsiveness" — advancing at network speed when conditions are favorable.

8. **The finality rule counts validator descendants, not distinct validator attestations to a specific value.** In the current design, a validator v "votes for" vertex u simply by having any descendant of u in the DAG. This is an implicit vote. If validator v produces a vertex that references a tip which is a descendant of u, then v counts toward u's finality — even if v was unaware of u. This is weaker than explicit voting because it does not distinguish between intentional endorsement and incidental graph connectivity.

### 11.2 Finality Latency

The finality rule requires ceil(2n/3) distinct validators to have descendants of a vertex. In the best case (all validators honest, synchronous network, all-tips-as-parents), this takes 2 rounds: the vertex is produced in round r, ceil(2n/3) validators produce children in round r+1, and finality is detected. In practice, 3 rounds of latency is observed in testnet.

### 11.3 State Divergence Window

Between the time a vertex is produced and the time it is finalized, the state is not committed. Transactions in unfinalized vertices may be reordered or dropped if the vertex is never finalized (e.g., if the validator is marked Byzantine). The `StateEngine` only applies finalized vertices.

### 11.4 Quadratic Descendant Computation

The finality check computes `descendants(v)` for each candidate vertex, which traverses the DAG. For a DAG with V vertices and E edges, each descendant computation is O(V + E). The `find_newly_finalized` function performs this for every non-finalized vertex reachable from tips, making the finality check O(C * (V + E)) where C is the number of candidates. This is acceptable for small validator sets and moderate DAG sizes but does not scale to thousands of validators or millions of vertices without optimization (e.g., incremental descendant tracking).

## 12. Summary of Protocol Properties

| Property | Status | Mechanism |
|----------|--------|-----------|
| **Agreement** | Argued (not proven) | Deterministic ordering + quorum intersection |
| **Validity** | Implemented | Only signed, non-equivocating vertices are accepted |
| **Termination** | Argued (not proven) | Round timer + stall recovery + quorum gate |
| **Equivocation resistance** | Implemented | One vertex per validator per round, permanent ban |
| **Finality** | Implemented | ceil(2n/3) descendant validator coverage |
| **Deterministic state** | Implemented | Total order on finalized vertices, atomic application |
| **Formal proof** | **Not available** | Safety argument is a sketch, not a proof |
| **Dynamic validator sets** | **Mitigated (testnet)** | `configured_validators` fixes threshold; epochs needed for mainnet |
| **Accountability** | Partial | Only equivocation is detected and punished |
