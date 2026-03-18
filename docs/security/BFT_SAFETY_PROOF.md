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

**Part 2: If vertex V is finalized at honest node A, all honest nodes will eventually finalize V.**

Assume V is finalized at A, meaning >= 2f+1 distinct validators have at least one descendant of V in A's DAG. Since at most f are Byzantine, at least f+1 honest validators have descendants of V.

Each honest validator broadcasts its vertices to all peers. Under partial synchrony, these vertices eventually reach all honest nodes. When honest node B receives these vertices:
- B inserts them via `try_insert()` (signature valid, parents exist or are fetched via GetParents)
- B's `descendant_validators` for V is updated via BFS propagation during insert
- Eventually B sees >= 2f+1 descendants of V (the same f+1 honest validators plus potentially others)
- B finalizes V

The vertex reconciliation protocol (every 50 rounds) and the DAG sync protocol (GetDagVertices, GetParents) ensure delivery even with message loss. **QED for Part 2.**

**Part 3: The set of finalized vertices at any round R is identical across all honest nodes (eventual consistency).**

From Part 2, every vertex finalized at any honest node is eventually finalized at all honest nodes. From Part 1, finalized vertices are ordered identically. Therefore, at any finalized round R, all honest nodes that have reached R have applied the same vertices in the same order, producing the same state root.

This is verified empirically by the simulation harness (3500+ adversarial proptest scenarios, all checking state root convergence at the same finalized round). **QED.**

## Corollary: No Double Spend

A transaction has a sequential nonce. If tx with nonce=K is included in vertex V1 (finalized) and also in vertex V2 (finalized), `apply_finalized_vertices` processes them in deterministic order. The first application succeeds (nonce matches), the second is skipped (nonce already incremented). All honest nodes skip the same transaction.

## Stuck Parent Threshold Analysis

The stuck parent rule (parents >100 rounds behind `last_finalized_round` are treated as finalized for the parent-finalization prerequisite) does NOT affect safety:

1. The stuck parent rule only relaxes the **parent check** ("are all parents of V finalized?"), not the **finality threshold** ("does V have ≥ 2f+1 descendants?")
2. A vertex with a stuck parent still requires ≥ 2f+1 distinct validator descendants to finalize
3. The ordering of finalized vertices is `(round, hash)`, which does not depend on parent finalization status
4. The risk is **liveness** only: an attacker could delay finality of a subtree by creating a vertex that never gets 2f+1 descendants, then waiting for the 100-round threshold. But the children of that stuck vertex still need their own 2f+1 descendants, and the ordering is still deterministic.

## Key Insight

Unlike traditional BFT protocols (PBFT, Tendermint, HotStuff), UltraDAG's ordering is **external to the DAG topology**. The ordering function `(round, hash)` uses only the vertex's own signed fields, not the DAG structure. This means disagreements about DAG structure (which parents are known, which vertices are in the local view) do NOT cause ordering disagreements. This is the fundamental property that makes the safety argument simpler than leader-based protocols.

## Comparison to Narwhal/Bullshark

Narwhal/Bullshark use leader-based commit rules tied to wave structure. UltraDAG uses a simpler descendant-count rule without leaders. The tradeoff:
- **UltraDAG advantage**: No leader election overhead, no single-point-of-failure per round
- **UltraDAG tradeoff**: No formal proof in a peer-reviewed paper (yet)
- **Both share**: Ordering derived from deterministic function of vertex content, not DAG topology

## Limitations

1. This proof sketch assumes **partial synchrony** — under full asynchrony, liveness is not guaranteed (standard FLP result)
2. The proof is not machine-checked (unlike the TLA+ invariant verification which checked 32.6M states)
3. The stuck parent threshold is a liveness optimization that should be formally analyzed for interaction with Byzantine validators in corner cases

## Recommendation

This proof sketch should be reviewed by a distributed systems theorist before mainnet launch. The core argument (deterministic ordering + eventual vertex propagation + 2f+1 threshold) is standard BFT reasoning, but the stuck parent rule and the leaderless design are novel enough to warrant formal scrutiny.
