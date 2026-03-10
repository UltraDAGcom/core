# UltraDAG Formal Verification

## What Was Verified

The TLA+ specification (`UltraDAGConsensus.tla`) models UltraDAG's DAG-BFT consensus protocol. TLC (the TLA+ model checker) exhaustively explored all reachable states to verify that the protocol's safety and structural invariants hold under all possible executions, including Byzantine behavior.

### Invariants Checked

1. **TypeOK** — All state variables have correct types and value ranges throughout execution.

2. **Safety** — No two finalized vertices from the same honest validator in the same round can have different content. This is the core consensus safety property: once a vertex is finalized, no conflicting vertex from the same validator can also be finalized.

3. **HonestNoEquivocation** — Honest validators never produce two vertices in the same round. This models the equivocation check in `dag.rs:try_insert()`.

4. **FinalizedParentsConsistency** — If a vertex is finalized, all of its parents are also finalized. This ensures the finalized set forms a coherent causal history with no gaps.

5. **RoundMonotonicity** — The global round counter never decreases. Rounds advance forward only.

6. **ByzantineBound** — The number of Byzantine validators never exceeds the configured maximum (f, where N >= 3f+1).

### Model Configurations

**Run 1: 3 validators, 1 Byzantine**
- VALIDATORS = {v1, v2, v3}, MAX_ROUNDS = 2, MAX_BYZANTINE = 1
- Quorum threshold: ceil(2*3/3) = 2
- States generated: 326,000 | Distinct: 160,000
- Time: ~2 seconds
- Result: **No errors found**

**Run 2: 4 validators, 1 Byzantine**
- VALIDATORS = {v1, v2, v3, v4}, MAX_ROUNDS = 2, MAX_BYZANTINE = 1
- Quorum threshold: ceil(2*4/3) = 3
- States generated: 32,600,000 | Distinct: 13,400,000
- Time: ~50 seconds
- Result: **No errors found**

### What the Spec Models

The TLA+ specification covers:
- **Vertex production** — Each validator produces one vertex per round referencing parents from the previous round.
- **Parent referencing** — Vertices reference at most one vertex per validator from the previous round (modeling equivocation filtering).
- **BFT finality** — A vertex is finalized when ceil(2N/3) distinct validators have descendants of it.
- **Byzantine behavior** — Byzantine validators can equivocate (produce multiple vertices per round) or remain silent.
- **2f+1 gate** — Validators require quorum from the previous round before producing.

### What the Spec Does NOT Model

The specification deliberately excludes (to keep state space tractable):
- Staking, unstaking, slashing, and epoch transitions
- State engine, balances, and transactions
- Network transport, message delays, and partitions
- Checkpoints, pruning, and persistence
- Governance proposals and voting

These components are covered by the 557 integration and unit tests in the Rust codebase.

## Honest Limitations

### Bounded Model Checking

TLC performs exhaustive exploration within configured bounds. Our verification used MAX_ROUNDS=2, meaning:

- All executions up to 2 rounds with 3-4 validators were checked exhaustively.
- Bugs that only manifest at round 3+ would not be caught by these runs.
- The 32.6M state space at N=4 confirms meaningful coverage, but it is not infinite-horizon proof.

Increasing to MAX_ROUNDS=3 with N=4 would likely produce billions of states, requiring significantly more time and disk. MAX_ROUNDS=4 is likely infeasible without symmetry reduction or abstraction.

### Liveness Not Model-Checked

The specification includes a `Liveness` temporal property (every honest vertex is eventually finalized under weak fairness). However, liveness checking was deferred because:

1. TLC's liveness checking requires exploring the full state graph and checking for fair cycles, which is significantly more expensive than invariant checking.
2. At N=4 with 32.6M states, liveness checking would require substantially more memory and time.
3. The invariant results already confirm the critical safety properties. Liveness is important but less urgent — a liveness bug means the system stalls (detectable and recoverable), while a safety bug means conflicting finalization (catastrophic and potentially undetectable).

Liveness verification is planned for a future run with optimized bounds or a dedicated verification environment.

### Abstraction Gap

The TLA+ spec is a manual abstraction of the Rust implementation, not mechanically extracted. There is inherently a gap between the spec and the code. The spec was derived from:
- `crates/ultradag-coin/src/consensus/dag.rs`
- `crates/ultradag-coin/src/consensus/finality.rs`
- `crates/ultradag-coin/src/consensus/validator_set.rs`

The 557 Rust tests (including Jepsen-style fault injection) provide complementary coverage of the actual implementation.

## Results Summary

| Property | N=3,f=1 | N=4,f=1 | Status |
|----------|---------|---------|--------|
| TypeOK | Pass | Pass | Verified |
| Safety | Pass | Pass | Verified |
| HonestNoEquivocation | Pass | Pass | Verified |
| FinalizedParentsConsistency | Pass | Pass | Verified |
| RoundMonotonicity | Pass | Pass | Verified |
| ByzantineBound | Pass | Pass | Verified |
| Liveness | — | — | Deferred |

**Total states explored: 32,926,000 across both runs. Zero violations found.**

## Files

- `UltraDAGConsensus.tla` — TLA+ specification
- `UltraDAGConsensus.cfg` — TLC model checker configuration
- `tlc-results-invariants.txt` — Raw TLC output summary
