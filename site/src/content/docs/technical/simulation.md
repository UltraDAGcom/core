---
title: "Simulation Harness"
description: "Deterministic consensus simulation for testing DAG-BFT logic with virtual networking and Byzantine fault injection"
order: 4
section: "technical"
---

# Simulation Harness

The `ultradag-sim` crate provides a deterministic consensus simulation for testing DAG-BFT logic without any network I/O. It uses the **real** `BlockDag`, `FinalityTracker`, `StateEngine`, and `Mempool` from `ultradag-coin` — only the network layer is replaced with a virtual network.

---

## Architecture

*Architecture diagram: The SimHarness orchestrates a VirtualNetwork and Invariant Checkers. The VirtualNetwork delivers messages between SimValidator instances (1 through N), each containing a BlockDag, FinalityTracker, StateEngine, and Mempool. A Transaction Generator feeds transactions into each validator's Mempool.*

### Key Design Decisions

- **No TCP, no Tokio, no async**: purely synchronous, deterministic execution
- **Real consensus logic**: uses production `BlockDag`, `FinalityTracker`, `StateEngine`, `Mempool`
- **Virtual network**: message delivery controlled by the harness
- **Deterministic seeding**: all randomness from `ChaCha8Rng` with configurable seed
- **Master invariant**: all honest validators that finalize the same round produce identical `compute_state_root()` output

---

## Components

### VirtualNetwork

Controls message delivery between simulated validators:

| Delivery Mode | Behavior |
|--------------|----------|
| `Perfect` | All messages delivered immediately in order |
| `RandomOrder` | Messages delivered but in random order |
| `Drop { probability }` | Messages dropped with the given probability (0.0-1.0) |
| `Partition { split, heal_after_rounds }` | Messages between groups dropped; validators split at index `split`, healing after N rounds |
| `Lossy { drop_probability }` | Combined reorder + drop at the specified rate |

```rust
let network = VirtualNetwork::new(num_validators, DeliveryPolicy::Lossy { drop_probability: 0.05 }, seed);
```

### SimValidator

A lightweight wrapper around real consensus components:

```rust
struct SimValidator {
    index: usize,
    sk: SecretKey,
    address: Address,
    dag: BlockDag,
    finality: FinalityTracker,
    state: StateEngine,
    mempool: Mempool,
    honest: bool,
    finality_history: Vec<(u64, [u8; 32])>,
}
```

Each `SimValidator` instance uses identical code to a production node, differing only in that messages are delivered through the `VirtualNetwork` instead of TCP.

### ByzantineStrategy

Strategies for Byzantine validators:

| Strategy | Behavior |
|----------|----------|
| `Equivocator` | Produces two different vertices per round (conflicting content) |
| `Withholder` | Produces vertices but withholds them from targeted peers |
| `Crash` | Stops producing entirely |
| `TimestampManipulator` | Produces vertices with manipulated timestamps |
| `RewardGambler` | Attempts to game reward distribution using a puppet address |
| `GovernanceTakeover` | Attempts to take over governance via malicious proposals |
| `DuplicateTxFlooder` | Floods the network with duplicate transactions |
| `FinalityStaller` | Attempts to stall finality progress |
| `SelectiveEquivocator` | Selectively equivocates to target specific rounds |

```rust
let strategy = ByzantineStrategy::Equivocator;
```

### SimHarness

The driver that orchestrates simulation rounds:

1. For each round, each honest validator produces a vertex
2. Byzantine validators execute their strategy
3. Messages are delivered through the VirtualNetwork
4. Each validator processes received vertices (insert, finality check, state apply)
5. Invariants are checked after each round

### Invariant Checkers

Automated checks run after every round:

| Invariant | Description |
|-----------|-------------|
| **State convergence** | All honest validators produce identical `compute_state_root()` for the same finalized round |
| **Supply consistency** | `liquid + staked + delegated + treasury == total_supply` on all validators |
| **Round monotonicity** | Finalized round never decreases |
| **Stake consistency** | `total_staked` and `total_delegated` match across all validators |
| **Governance consistency** | Governance params and proposal IDs match across all validators |
| **Council consistency** | Council member count and set match across all validators |

### Transaction Generator

`TxGen` produces deterministic random transactions for stress testing:

- Transfer transactions with random amounts and recipients
- Stake and delegation transactions
- Governance proposals and votes
- All deterministically seeded from `ChaCha8Rng`

---

## Test Suite

### Base Consensus Tests (sample)

| Test | Configuration | Rounds | Validates |
|------|--------------|--------|-----------|
| 4-validator perfect | Perfect delivery | 100 | Basic consensus convergence |
| 4-validator with transactions | Perfect + 20 tx/round | 200 | Tx processing under consensus |
| Single validator | 1 validator | 50 | Solo finality works |
| Random message reorder | RandomOrder delivery | 200 | Order-independent convergence |
| 100-seed sweep | Perfect, 100 different seeds | 50 each | Determinism across seeds |
| 2-2 partition heal | Partition for 100 rounds, heal | 200 | Partition recovery |
| Equivocator detection | 1 Byzantine/4 | 100 | Equivocation detected + supply correct |
| 21-validator stress | 5% loss, 50 tx/round | 1000 | Large-scale convergence |
| Mixed Byzantine (2/7) | 2 Byzantine, 5 honest | 200 | BFT tolerance |
| Late-joiner convergence | 1 node joins at round 50 | 200 | Late join converges |
| Governance with reorder | RandomOrder + governance | 200 | tick_governance deterministic under reorder |

### Scenario Tests (sample)

| Scenario | Description | Rounds |
|----------|-------------|--------|
| StakingLifecycle | Stake, earn rewards, set commission, unstake | 500 |
| DelegationRewards | Delegate, earn split rewards, undelegate | 300 |
| GovernanceParameterChange | Propose, vote, execute ParameterChange | 200 |
| CrossFeature | Stake + delegate + governance + equivocation simultaneously | 500 |
| EpochTransition | Force active set recalculation | 250 |
| StakeWithReorder | Staking under random message reordering | 300 |
| DelegationWithLoss | Delegation under 5% message loss | 400 |
| GovernanceStress | Multiple proposals + votes under adversarial conditions | 300 |

**Total: 80+ simulation tests**, all passing, all deterministic.

---

## Master Invariant

The simulation's primary correctness check:

<div class="callout callout-note"><div class="callout-title">Master Invariant</div>All honest validators that finalize the same round <strong>must</strong> produce identical <code>compute_state_root()</code> output.</div>

This invariant has been verified under:

- Normal operation (perfect delivery)
- Random message reordering
- Message loss (5%)
- Network partitions with healing
- Equivocation with slashing
- Staking, delegation, and commission splits
- Governance parameter change execution
- Epoch transitions with validator set changes
- Combined adverse conditions

---

## Determinism

All simulation tests are fully deterministic:

1. Each test has a seed value (u64)
2. `ChaCha8Rng::seed_from_u64(seed)` initializes all randomness
3. Message delivery order, transaction generation, and Byzantine behavior all derive from the seeded RNG
4. Running the same test with the same seed always produces the same result

The 100-seed sweep test verifies this by running 100 different seeds and confirming all converge correctly.

---

## Running the Tests

```bash
# Run all simulation tests
cargo test -p ultradag-sim

# Run a specific test
cargo test -p ultradag-sim -- test_4_validator_perfect

# Run with output
cargo test -p ultradag-sim -- --nocapture
```

---

## Adding New Tests

To add a new simulation test:

1. Define the scenario configuration (validators, rounds, delivery mode, byzantine strategies)
2. Optionally configure transaction generation
3. Run the harness
4. The master invariant is checked automatically after each round

```rust
#[test]
fn test_my_scenario() {
    let config = SimConfig {
        validators: 4,
        byzantine: vec![(2, ByzantineStrategy::Withholder)],
        rounds: 200,
        delivery: DeliveryPolicy::Lossy(0.03),
        seed: 42,
        tx_per_round: 10,
    };
    let result = SimHarness::run(config);
    assert!(result.all_invariants_passed());
}
```

---

## Relationship to Other Testing

| Layer | What It Tests | How |
|-------|--------------|-----|
| **Unit tests** | Individual functions and types | `#[cfg(test)]` inline |
| **Integration tests** | Cross-module interactions | `tests/` directory |
| **Simulation** (this) | Full consensus with virtual network | `ultradag-sim` crate |
| **Jepsen tests** | Consensus under fault injection | Real `BlockDag` + fault injector |
| **Testnet** | Full stack including real TCP | 5-node Fly.io deployment |

The simulation harness fills the gap between unit/integration tests (too narrow) and testnet (too slow, non-deterministic) by providing fast, deterministic, full-consensus testing.

---

## Next Steps

- [DAG-BFT Consensus](/docs/architecture/consensus) — the protocol being simulated
- [Formal Verification](/docs/technical/formal-verification) — TLA+ proof of safety
- [Audit Reports](/docs/security/audits) — test coverage details
