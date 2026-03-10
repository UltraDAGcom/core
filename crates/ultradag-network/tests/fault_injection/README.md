# Jepsen-Style Fault Injection Testing for UltraDAG

This module provides systematic fault injection capabilities to validate UltraDAG's distributed consensus under adverse conditions.

## Overview

Inspired by [Jepsen](https://jepsen.io/), these tests inject various faults to verify that UltraDAG maintains critical safety and liveness properties:

- **Safety**: No double-spending, finality never reverts, nodes agree on finalized state
- **Liveness**: Progress continues under f < N/3 faults
- **Consistency**: All nodes converge to the same state after faults heal

## Fault Types

### 1. Network Partitions (`network_partition.rs`)

Simulates split-brain scenarios where groups of nodes cannot communicate.

**Scenarios:**
- `SplitBrain`: Divide network into two equal halves
- `IsolateOne(node_id)`: Isolate a single node from the rest
- `MinorityPartition`: Create 1/3 vs 2/3 split
- `CompleteIsolation`: All nodes isolated from each other

**Example:**
```rust
let injector = FaultInjector::new();
injector.partition(vec![vec![0, 1], vec![2, 3]]); // Split into two groups
sleep(Duration::from_secs(10)).await;
injector.heal_partitions(); // Restore connectivity
```

**Expected Behavior:**
- Minority partition (< 2/3) cannot finalize new vertices
- Majority partition (≥ 2/3) continues making progress
- After healing, all nodes converge to the same finalized state

### 2. Clock Skew (`clock_skew.rs`)

Simulates time drift between nodes to test timestamp validation.

**Scenarios:**
- `SingleNodeAhead`: One node's clock is ahead by N seconds
- `SingleNodeBehind`: One node's clock is behind by N seconds
- `GradualDrift`: Nodes drift apart over time
- `RandomSkew`: Each node has random offset

**Example:**
```rust
injector.set_clock_offset(0, 300); // Node 0 is 5 minutes ahead
let node_time = injector.node_time(0); // Get adjusted time
```

**Expected Behavior:**
- Vertices with timestamps >5 minutes in future are rejected
- Moderate skew (±30s) doesn't prevent consensus
- Extreme skew (>1 hour) causes vertex rejection

### 3. Message Chaos (`message_chaos.rs`)

Simulates unreliable networks with delays, reordering, and drops.

**Scenarios:**
- `RandomDelay`: Messages delayed by random amount up to max_ms
- `Reordering`: Messages delivered out of order
- `RandomDrop`: Messages dropped with specified probability
- `ExtremeChao`: Delay + reorder + drops combined

**Example:**
```rust
injector.inject_message_delay(2000); // Up to 2 second delays
injector.enable_message_reordering(true);
let mut chaos = injector.message_chaos.lock().unwrap();
chaos.drop_rate = 0.10; // 10% message drops
```

**Expected Behavior:**
- Consensus works with moderate delays (<5s)
- Reordering doesn't break consensus (orphan resolution handles it)
- Drop rates <33% don't prevent progress (BFT tolerance)

### 4. Crash-Restart (`crash_restart.rs`)

Simulates node crashes and validates state recovery.

**Scenarios:**
- Single node crash during consensus
- Repeated crash-restart cycles
- Simultaneous crashes (< 1/3 of nodes)
- Crash during checkpoint creation

**Example:**
```rust
crash_and_restart(&injector, &mut node, Duration::from_secs(2)).await;
```

**Expected Behavior:**
- Other nodes continue if f < N/3 crashed
- Crashed node can recover from checkpoint + WAL
- No finalized state is lost after recovery

## Invariant Checkers (`invariants.rs`)

Validates critical safety properties:

### Finality Safety
- **FinalityConflict**: Two nodes finalized different vertices at same round
- **FinalityRevert**: A finalized vertex was later reverted

### Supply Consistency
- **SupplyMismatch**: Total supply differs between nodes
- **SupplyAccountingError**: Balance sum doesn't match total supply

### Double-Spend Prevention
- **DoubleSpend**: Transaction spent more than available balance

**Example:**
```rust
let mut checker = InvariantChecker::new();
let violations = checker.check_all(&nodes).await;

if !violations.is_empty() {
    panic!("Invariant violations: {}", checker.report(&violations));
}
```

## Running Tests

### Run all Jepsen tests:
```bash
cargo test --test jepsen_tests -- --ignored --nocapture
```

### Run specific test:
```bash
cargo test --test jepsen_tests test_split_brain_partition -- --ignored --nocapture
```

### Run with logging:
```bash
RUST_LOG=debug cargo test --test jepsen_tests -- --ignored --nocapture
```

## Test Scenarios

### Basic Fault Tests
- `test_split_brain_partition`: Network split into two halves
- `test_minority_partition_liveness`: Minority cannot finalize
- `test_moderate_clock_skew`: ±30 second time drift
- `test_message_delay_resilience`: Up to 2 second delays
- `test_single_node_crash_restart`: One node crashes and recovers

### Combined Fault Tests
- `test_partition_with_clock_skew`: Partition + time drift
- `test_message_chaos_with_crash`: Delays + reordering + crash
- `test_extreme_chaos_scenario`: ALL faults combined

### Extreme Chaos Test

The `test_extreme_chaos_scenario` combines all fault types:

1. **Phase 1**: Network partition + clock skew
2. **Phase 2**: Heal partition, add message delays + reordering
3. **Phase 3**: Crash a node
4. **Phase 4**: Extreme message chaos (2s delays, 15% drops)

**Expected Result**: System survives without critical violations (no finality conflicts or reverts)

## Implementation Notes

### FaultInjector

Central controller for coordinating faults:

```rust
pub struct FaultInjector {
    pub partitions: Arc<Mutex<NetworkPartitions>>,
    pub clock_offsets: Arc<Mutex<HashMap<usize, i64>>>,
    pub message_chaos: Arc<Mutex<MessageChaos>>,
    pub crashed_nodes: Arc<Mutex<HashSet<usize>>>,
}
```

### TestNode

Simplified node state for testing:

```rust
pub struct TestNode {
    pub id: usize,
    pub state: Arc<RwLock<StateEngine>>,
    pub dag: Arc<RwLock<BlockDag>>,
    pub finality: Arc<RwLock<FinalityTracker>>,
    pub validator_address: Address,
}
```

### Integration with Real Network

For production testing, integrate FaultInjector with NodeServer:

1. Check `injector.can_communicate(node_a, node_b)` before sending messages
2. Use `injector.node_time(node_id)` for timestamp generation
3. Apply `message_chaos.calculate_delay()` before message delivery
4. Check `injector.is_crashed(node_id)` before processing

## Future Enhancements

- [ ] **WAL recovery testing**: Verify checkpoint + WAL recovery after crash
- [ ] **Byzantine behavior**: Malicious nodes sending invalid vertices
- [ ] **Asymmetric network**: Different delays per direction (A→B vs B→A)
- [ ] **Gradual failures**: Slowly degrading network conditions
- [ ] **Checkpoint corruption**: Test recovery from corrupted checkpoints
- [ ] **State divergence detection**: Automated detection of state forks
- [ ] **Performance metrics**: Track finality latency under faults
- [ ] **Randomized testing**: Property-based testing with random fault sequences

## References

- [Jepsen: On the perils of network partitions](https://aphyr.com/posts/281-jepsen-on-the-perils-of-network-partitions)
- [Jepsen: Consistency Models](https://jepsen.io/consistency)
- [Testing Distributed Systems](https://asatarin.github.io/testing-distributed-systems/)
- [Narwhal and Tusk: A DAG-based Mempool and Efficient BFT Consensus](https://arxiv.org/abs/2105.11827)

## License

Same as UltraDAG core (see root LICENSE file).
