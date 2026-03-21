# UltraDAG Simulator

A comprehensive discrete-event simulator for testing UltraDAG consensus under various network conditions and attack scenarios.

## Overview

The simulator models a network of UltraDAG validators with:
- **Virtual Network**: Configurable delivery policies (perfect, lossy, partition, latency)
- **Byzantine Strategies**: 9 different attack vectors
- **Invariant Checks**: 8 safety properties verified each round
- **Transaction Generation**: Random and scripted transaction injection

## Architecture

```
src/
├── harness.rs      # Simulation orchestration (SimHarness, SimConfig, SimResult)
├── validator.rs    # Simulated validator node (SimValidator)
├── network.rs      # Virtual network with delivery policies (VirtualNetwork)
├── byzantine.rs    # Byzantine attack strategies (ByzantineStrategy)
├── invariants.rs   # Safety property checks
├── properties.rs   # Deep structural property verification
├── txgen.rs        # Transaction generators
└── fuzz.rs         # Property-based testing with proptest
```

## Quick Start

```rust
use ultradag_sim::harness::{SimConfig, SimHarness};
use ultradag_sim::network::DeliveryPolicy;

let config = SimConfig {
    num_honest: 4,
    byzantine: vec![],
    num_rounds: 100,
    delivery_policy: DeliveryPolicy::Perfect,
    seed: 42,
    txs_per_round: 10,
    check_every_round: true,
    scenario: None,
    max_finality_lag: 50,
};

let mut harness = SimHarness::new(&config);
let result = harness.run(&config);

assert!(result.passed, "Simulation failed: {:?}", result.violations);
```

## Configuration

### SimConfig

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `num_honest` | `usize` | Required | Number of honest validators |
| `byzantine` | `Vec<ByzantineStrategy>` | `[]` | Byzantine validator strategies |
| `num_rounds` | `u64` | Required | Number of rounds to simulate |
| `delivery_policy` | `DeliveryPolicy` | Required | Network delivery model |
| `seed` | `u64` | Required | RNG seed for reproducibility |
| `txs_per_round` | `usize` | 0 | Random transactions per round |
| `check_every_round` | `bool` | true | Verify invariants each round |
| `scenario` | `Option<Scenario>` | `None` | Scripted transaction injection |
| `max_finality_lag` | `u64` | 50 | Maximum allowed finality lag |

### Delivery Policies

```rust
// Perfect network: all messages arrive immediately
DeliveryPolicy::Perfect

// Messages arrive in random order
DeliveryPolicy::RandomOrder

// Messages dropped with probability
DeliveryPolicy::Drop { probability: 0.1 }

// Network partition: split validators into two groups
DeliveryPolicy::Partition { split: 2, heal_after_rounds: 50 }

// Combined reorder + drop
DeliveryPolicy::Lossy { drop_probability: 0.05 }

// Variable latency (1-3 rounds typical)
DeliveryPolicy::Latency { base_latency: 1, jitter: 2 }

// Latency + drop combined
DeliveryPolicy::LatencyLossy { base_latency: 1, jitter: 2, drop_probability: 0.1 }
```

### Byzantine Strategies

```rust
// Equivocation: produce two different vertices for same round
ByzantineStrategy::Equivocator

// Withhold from specific validators
ByzantineStrategy::Withholder { targets: vec![0, 1] }

// Crash/offline: don't produce any vertices
ByzantineStrategy::Crash

// Timestamp manipulation
ByzantineStrategy::TimestampManipulator { offset_secs: 300 }

// Adaptive reward manipulation
ByzantineStrategy::RewardGambler { puppet_sk, puppet_address }

// Governance proposal flooding
ByzantineStrategy::GovernanceTakeover

// Duplicate transaction flooding
ByzantineStrategy::DuplicateTxFlooder

// Finality stalling
ByzantineStrategy::FinalityStaller

// Selective equivocation (different vertices to different groups)
ByzantineStrategy::SelectiveEquivocator
```

## Invariants Checked

The simulator verifies these safety properties:

1. **State Convergence**: All honest validators at same finalized round have identical state roots
2. **Supply Invariant**: `liquid + staked + delegated + treasury == total_supply`
3. **Finality Monotonicity**: Finality history has strictly non-decreasing rounds
4. **No Double Finalization**: No round is finalized twice with different roots
5. **Equivocation Detection**: Known equivocators are marked Byzantine
6. **Stake Consistency**: Stake amounts match across validators at same round
7. **Balance Overflow**: No balance exceeds total_supply
8. **Supply Cap**: `total_supply <= MAX_SUPPLY_SATS`

## Scenarios

Pre-scripted transaction injection scenarios:

```rust
// Staking lifecycle: stake → earn rewards → unstake → cooldown
Scenario::StakingLifecycle

// Delegation: delegate to validators → earn rewards minus commission
Scenario::DelegationRewards

// Governance: create proposal → vote → execute parameter change
Scenario::GovernanceParameterChange

// Full cross-feature: all features simultaneously
Scenario::CrossFeature

// Epoch boundary with stakers
Scenario::EpochTransition
```

## Running Tests

```bash
# Run all simulator tests
cargo test --package ultradag-sim --release

# Run specific test
cargo test --package ultradag-sim --test basic --release

# Run with output
cargo test --package ultradag-sim --test long_running -- --nocapture

# Run fuzz tests
cargo test --package ultradag-sim --test fuzz_adversarial --release
```

## Test Suites

| Test File | Purpose | Rounds |
|-----------|---------|--------|
| `basic.rs` | Basic consensus scenarios | 100-200 |
| `determinism.rs` | Verify deterministic execution | 50-100 |
| `delegation.rs` | Delegation lifecycle | 500 |
| `governance.rs` | Governance operations | 100 |
| `staking.rs` | Staking lifecycle | 500 |
| `equivocation.rs` | Equivocation detection | 100-200 |
| `finality_attack.rs` | Finality stalling attempts | 200 |
| `fuzz_adversarial.rs` | Property-based Byzantine testing | 1000 |
| `partition.rs` | Network partition scenarios | 200 |
| `long_running.rs` | Stability under extended load | 10,000 |
| `stress.rs` | High-load scenarios | 1000 |

## Simulation Results

```rust
pub struct SimResult {
    pub passed: bool,
    pub rounds_completed: u64,
    pub seed: u64,
    pub violations: Vec<String>,
    pub final_state_roots: Vec<(usize, [u8; 32])>,
    pub final_finalized_rounds: Vec<(usize, u64)>,
    pub total_messages_sent: u64,
    pub total_messages_dropped: u64,
    pub equivocations_detected: usize,
    pub total_txs_applied: u64,
}
```

## Determinism

The simulator is **fully deterministic** when using the same seed:
- All validators use round-based timestamps (not system time)
- RNG is seeded with configurable `seed` parameter
- Same seed = same results every time

```rust
// These two runs produce identical results
let config1 = SimConfig { seed: 42, .. };
let config2 = SimConfig { seed: 42, .. };
```

## Limitations

The simulator does NOT model:
- OS-level resource exhaustion (memory, disk, file descriptors)
- Real network latency distributions (uses simplified model)
- Disk I/O failures or corruption
- Clock skew between nodes (uses simulated time)
- CPU contention or scheduling delays

**Use simulator for:** Consensus logic verification, invariant checking, attack scenario testing.

**Use testnet for:** Real network conditions, performance benchmarking, operational testing.

## Adding New Tests

1. Create test file in `crates/ultradag-sim/tests/`
2. Configure `SimConfig` with desired parameters
3. Run simulation and assert on `SimResult`

```rust
use ultradag_sim::harness::{SimConfig, SimHarness};
use ultradag_sim::network::DeliveryPolicy;

#[test]
fn my_new_test() {
    let config = SimConfig {
        num_honest: 4,
        byzantine: vec![],
        num_rounds: 100,
        delivery_policy: DeliveryPolicy::Perfect,
        seed: 12345,
        txs_per_round: 10,
        check_every_round: true,
        scenario: None,
        max_finality_lag: 50,
    };
    
    let mut harness = SimHarness::new(&config);
    let result = harness.run(&config);
    
    assert!(result.passed, "Test failed: {:?}", result.violations);
}
```

## Debugging Failures

When a test fails:

1. Check `result.violations` for specific invariant violations
2. Use `--nocapture` to see detailed output
3. Reduce `num_rounds` to isolate the failure point
4. Try different seeds to check if failure is deterministic

```bash
cargo test --package ultradag-sim --test my_test -- --nocapture
```

## Performance

Typical simulation speeds (release mode):
- 4 validators, 100 rounds: ~100ms
- 4 validators, 1000 rounds: ~500ms
- 21 validators, 1000 rounds: ~2s
- 4 validators, 10,000 rounds: ~5s

Run with `--release` for best performance.

## License

MIT License - see LICENSE file for details.
