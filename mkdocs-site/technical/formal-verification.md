---
title: Formal Verification
---

# Formal Verification

UltraDAG's consensus protocol is formally specified in TLA+ and verified using the TLC model checker. This page describes the specification, invariants, and verification results.

---

## Overview

| Property | Value |
|----------|-------|
| Specification file | `formal/UltraDAGConsensus.tla` |
| Configuration | `formal/UltraDAGConsensus.cfg` |
| States explored | 32,900,000+ |
| Distinct states | 13,400,000+ |
| Invariants verified | 6 |
| Violations found | 0 |

---

## TLA+ Specification

The specification models UltraDAG's DAG-BFT consensus, derived directly from the Rust implementation. It captures vertex production, parent referencing, BFT finality, and Byzantine behavior.

### State Variables

| Variable | Type | Description |
|----------|------|-------------|
| `round` | Nat | Current global round counter |
| `vertices` | Set of records | All produced vertices |
| `finalized` | Set of vertex IDs | Vertices that have achieved BFT finality |
| `byzantine` | Set of validator IDs | Validators behaving arbitrarily |
| `active` | Set of validator IDs | Currently active validators |
| `nextId` | Nat | Monotonic vertex ID counter |

### Actions

#### ProduceVertex(v, r)

An honest validator produces a vertex:

- References parents from round `r-1`
- Enforces equivocation prevention (one vertex per validator per round)
- Checks the 2f+1 gate (quorum from previous round required)

#### FinalizeVertex(vtx)

A vertex is finalized when:

- At least `ceil(2N/3)` distinct validators have descendants of `vtx`
- All parents of `vtx` are already finalized (parent finality guarantee)

#### ByzantineAction(v, r)

A Byzantine validator can:

- **Equivocate**: produce multiple different vertices in the same round
- **Withhold**: stay silent and produce nothing

#### AdvanceRound

The system advances to the next round after validators have produced vertices.

---

## Verified Invariants

### 1. Safety

**Statement**: No two finalized vertices from the same honest validator in the same round have different content.

$$
\forall v_1, v_2 \in \text{finalized}: v_1.\text{author} = v_2.\text{author} \land v_1.\text{round} = v_2.\text{round} \implies v_1 = v_2
$$

This is the fundamental BFT safety property. It guarantees that finalized history is consistent across all honest nodes.

### 2. HonestNoEquivocation

**Statement**: Honest validators never produce two vertices in the same round.

This verifies that the equivocation prevention mechanism in the validator loop works correctly.

### 3. FinalizedParentsConsistency

**Statement**: All parents of a finalized vertex are also finalized.

$$
\forall v \in \text{finalized}, \forall p \in v.\text{parents}: p \in \text{finalized}
$$

This ensures that finality propagates cleanly through the DAG without orphaned references.

### 4. TypeOK

**Statement**: All state variables have the correct types (sets contain the right kinds of elements, counters are non-negative, etc.).

### 5. RoundMonotonicity

**Statement**: Round numbers never decrease. Once the system advances to round `r`, it never produces vertices for rounds less than `r`.

### 6. ByzantineBound

**Statement**: The number of Byzantine validators never exceeds the assumed fault threshold.

---

## Model Checking Results

### Configuration: N=3, f=1, MAX_ROUNDS=2

| Metric | Value |
|--------|-------|
| States generated | 326,000 |
| Distinct states | 160,000 |
| Time | ~2 seconds |
| Result | No errors |

### Configuration: N=4, f=1, MAX_ROUNDS=2

| Metric | Value |
|--------|-------|
| States generated | 32,600,000 |
| Distinct states | 13,400,000 |
| Time | ~50 seconds |
| Result | No errors |

**Total: 32.9 million states explored, zero invariant violations.**

---

## Running the Model Checker

### Prerequisites

Install the TLA+ tools (TLC model checker):

```bash
# Download tla2tools.jar
curl -L -o tla2tools.jar \
  https://github.com/tlaplus/tlaplus/releases/latest/download/tla2tools.jar
```

### Run Verification

```bash
cd formal/
java -jar tla2tools.jar \
  -config UltraDAGConsensus.cfg \
  UltraDAGConsensus.tla
```

### Expected Output

```
TLC2 Version 2.xx ...
Model checking completed. No error has been found.
  Evaluating invariant Safety ... ok
  Evaluating invariant HonestNoEquivocation ... ok
  Evaluating invariant FinalizedParentsConsistency ... ok
  Evaluating invariant TypeOK ... ok
  Evaluating invariant RoundMonotonicity ... ok
  Evaluating invariant ByzantineBound ... ok
32926000 states generated, 13562000 distinct states found.
```

---

## Limitations

### Bounded Model Checking

The model is checked at `MAX_ROUNDS=2`. Bugs that only manifest at round 3 or higher would not be caught. Increasing `MAX_ROUNDS` causes exponential state space growth:

| MAX_ROUNDS | Estimated States (N=4) |
|------------|----------------------|
| 1 | ~50,000 |
| 2 | ~32,900,000 |
| 3 | ~10,000,000,000+ (infeasible) |

### Liveness Not Verified

The specification includes a `Liveness` temporal property (the system eventually makes progress), but it is not yet model-checked due to TLC resource requirements for liveness checking at this state space size. Safety is verified; liveness is deferred.

### Abstraction Gap

The TLA+ specification abstracts away:

- Network transport details (TCP, Noise encryption)
- Serialization format specifics
- Clock synchronization
- Memory management and persistence

These aspects are tested through the 977+ core test suite, 80+ simulation tests, and 14 Jepsen fault injection tests rather than formal verification.

---

## Relationship to Implementation

The TLA+ specification is kept in sync with the Rust implementation through:

1. **Derived specification**: the TLA+ model was written by reading the Rust code, not the other way around
2. **Matching invariants**: the safety invariants in TLA+ correspond to runtime checks in the Rust code (supply invariant, finality checks)
3. **Simulation bridge**: the `ultradag-sim` crate uses the same `BlockDag`, `FinalityTracker`, and `StateEngine` as production, providing a middle ground between formal verification and runtime testing

---

## Files

| File | Purpose |
|------|---------|
| `formal/UltraDAGConsensus.tla` | TLA+ specification |
| `formal/UltraDAGConsensus.cfg` | TLC model checker configuration |
| `formal/VERIFICATION.md` | Detailed results and methodology |
| `formal/tlc-results-invariants.txt` | Raw TLC output summary |

---

## Next Steps

- [DAG-BFT Consensus](../architecture/consensus.md) — the protocol being verified
- [Simulation Harness](simulation.md) — deterministic testing with real code
- [Security Model](../security/model.md) — complete security architecture
