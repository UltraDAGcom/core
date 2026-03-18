# UltraDAG DAG-BFT Safety Proof Sketch

## System Model

- **N validators**, at most **f < N/3** are Byzantine
- **Partial synchrony**: messages between honest nodes are eventually delivered
- Each validator produces at most one vertex per round (enforced by `try_insert()`)
- Vertices are signed with Ed25519; forgery is computationally infeasible

## Definitions

- **Vertex V**: a signed DAG vertex with fields `(round, block, parent_hashes, validator, signature)`
- **V is finalized at node A**: `descendant_validator_count(V) >= ceil(2N/3)` in A's DAG view
- **Ordering function**: `O(V) = (V.round, V.hash())` where hash is blake3 of signed content
- **Conflicting orderings**: two honest nodes apply finalized vertices in different order

## Theorem: Safety

**No two honest nodes can finalize vertices in different orderings.**

### Proof

**Part 1: Ordering is deterministic from vertex content.**

The ordering function `O(V) = (V.round, blake3(V.block || V.round || V.validator || V.parents))` depends solely on the vertex's signed fields. Since:
- `round` is in the vertex (signed)
- `block.hash()` is deterministic from block content (signed)
- blake3 is deterministic

Any two nodes holding the same vertex V compute the same `O(V)`. Therefore, if two nodes have the same set of finalized vertices, they produce identical orderings. **QED for Part 1.**

**Part 2: If vertex V is finalized at honest node A, all honest nodes will eventually finalize V (no permanent exclusion).**

Assume V is finalized at A, meaning >= 2f+1 distinct validators have at least one descendant of V in A's DAG. Since at most f are Byzantine, at least **f+1 honest validators** have descendants of V.

Each honest validator broadcasts its vertices to all peers. Under partial synchrony, these vertices eventually reach all honest nodes. When honest node B receives these vertices:
- B inserts them via `try_insert()` (signature valid, parents exist or are fetched via GetParents)
- B's `descendant_validators` for V is updated via BFS propagation during insert
- Eventually B sees >= 2f+1 descendants of V (the same f+1 honest validators plus potentially others)
- B finalizes V

**It is impossible for V to be finalized at A and permanently excluded at B.** The f+1 honest validators who produced descendants of V will broadcast those vertices, and B will eventually receive them. The reconciliation protocol (GetDagVertices every 50 rounds, GetParents for missing dependencies) ensures delivery even with message loss.

**Quorum intersection**: If V1 is finalized at A (2f+1 descendants) and V2 is finalized at B (2f+1 descendants), the two sets of 2f+1 validators share at least f+1 honest validators (since 2(2f+1) > 3f+1). These f+1 honest validators produced vertices that are descendants of BOTH V1 and V2. Since honest validators propagate all their vertices, both A and B will eventually see descendants of both V1 and V2, finalizing both.

"Conflicting" in this context would mean V1 and V2 occupy the same position in the ordering. But O(V) = (round, hash) — same position means same round AND same hash, which means same vertex (blake3 collision is computationally infeasible). So V1 = V2. No conflict is possible. **QED for Part 2.**

**Part 3: Combined safety.**

From Part 2, every vertex finalized at any honest node is eventually finalized at all honest nodes. From Part 1, finalized vertices are ordered identically. Therefore, at any finalized round R, all honest nodes that have reached R have applied the same vertices in the same order, producing the same state root.

This is verified empirically by the simulation harness (3500+ adversarial proptest scenarios, all checking state root convergence at the same finalized round). **QED.**

## Corollary: No Double Spend

A transaction has a sequential nonce. If tx with nonce=K is included in vertex V1 (finalized) and also in vertex V2 (finalized), `apply_finalized_vertices` processes them in deterministic order. The first application succeeds (nonce matches), the second is skipped (nonce already incremented). All honest nodes skip the same transaction.

## Stuck Parent Threshold Analysis

The stuck parent rule (parents >100 rounds behind `last_finalized_round` are treated as finalized for the parent-finalization prerequisite) does NOT affect safety:

1. The stuck parent rule only relaxes the **parent check** ("are all parents of V finalized?"), not the **finality threshold** ("does V have >= 2f+1 descendants?")
2. A vertex with a stuck parent still requires >= 2f+1 distinct validator descendants to finalize
3. The ordering of finalized vertices is `(round, hash)`, which does not depend on parent finalization status or finalization timing
4. The risk is **liveness** only: an attacker could delay finality of a subtree by creating a vertex that never gets 2f+1 descendants, then waiting for the 100-round threshold

**Timing attack analysis**: An attacker produces vertex X in round R, withholds it, then broadcasts at round R+101 when other nodes treat X's parent as stuck. The vertex X itself still needs 2f+1 descendants. Different nodes may finalize X at different *times*, but the ordering of X relative to other vertices in the same round is deterministic `(round, hash)` — independent of when finality occurs. Governance ticks and unstake completions run at round boundaries within the deterministic sorted order, so finalization timing does not affect their execution. The sort-by-(round, hash) in `apply_finalized_vertices` makes this invariant to when and in what order finality is discovered.

**For the external reviewer**: examine whether the 100-round stuck threshold creates any subtle interaction with Byzantine validators that could manipulate the *set* of finalized vertices (not their ordering) in a way that differs between honest nodes.

## Key Insight: Leaderless Safety

Unlike leader-based BFT protocols (PBFT, Tendermint, HotStuff, Bullshark, Tusk), UltraDAG's ordering is **external to the DAG topology**. The ordering function `(round, hash)` uses only the vertex's own signed fields, not the DAG structure or any leader's decision. This means:

- Disagreements about DAG structure (which parents are known, which vertices are in the local view) do NOT cause ordering disagreements
- No leader election is needed to anchor commits
- No view-change protocol is needed for leader failure recovery

The tradeoff: leader-based protocols can achieve optimistic responsiveness (commit in 1 round in the good case). UltraDAG's leaderless approach requires waiting for descendant counts to accumulate (typically 2-3 rounds for finality). The safety argument is simpler because there's no leader election to reason about.

**For the external reviewer**: the absence of leader election is the most novel aspect. Protocols like Bullshark and Tusk use leaders to anchor commits. Confirm that leaderless finality via descendant counting doesn't introduce edge cases that leader-based protocols avoid. Specifically verify that the quorum intersection argument (2f+1 + 2f+1 > 3f+1 implies f+1 honest overlap) is sufficient for safety without a leader commitment step.

## Comparison to Narwhal/Bullshark

| Property | UltraDAG | Bullshark |
|----------|----------|-----------|
| Commit rule | >= ceil(2N/3) descendant validators | Leader vertex in wave with >= 2f+1 certificates |
| Leader election | None | Per-wave leader rotation |
| Ordering source | (round, hash) — external to DAG | Leader's causal history — internal to DAG |
| Optimistic latency | 2-3 rounds | 1-2 rounds (with good leader) |
| Safety argument | Quorum intersection + deterministic external ordering | Quorum intersection + leader commitment step |
| Formal proof | This sketch (pending review) | Peer-reviewed (ACM CCS 2022) |

## Known Limitations

1. This proof sketch assumes **partial synchrony** — under full asynchrony, liveness is not guaranteed (standard FLP result)
2. The proof is not machine-checked (the TLA+ spec verified invariants over 32.6M states but is a model, not a proof)
3. The stuck parent threshold should be formally analyzed for interaction with Byzantine validators in corner cases (liveness, not safety)
4. The leaderless design has no peer-reviewed publication — this is novel and needs external validation

## WAL Replay (Addressed)

The WAL (`FinalityWal`) is **dead code** — not called from any production path. Startup uses redb + dag.bin + finality.bin. The WAL files exist in the codebase (persistence/wal.rs) but are never imported or called. A corrupt WAL cannot affect production state because WAL replay never executes. This is documented as acceptable risk.

## Recommendation for External Engagement

Frame the review as: "Verify that UltraDAG's leaderless DAG-BFT commit rule provides safety and liveness under partial synchrony with up to f < N/3 Byzantine validators, with specific attention to:
1. The stuck-parent liveness relaxation (100-round threshold)
2. The absence of a leader election mechanism
3. The quorum intersection argument applied to descendant-counting finality
4. Whether the deterministic external ordering (round, hash) is sufficient to replace leader-based commit anchoring"
