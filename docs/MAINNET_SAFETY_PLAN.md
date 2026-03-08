# Mainnet Safety Implementation Plan

**Critical Priority:** Prevent state rollbacks before mainnet launch  
**Timeline:** 6 weeks  
**Status:** Planning phase

---

## Executive Summary

The testnet rollback event (round 4047 → 50) revealed a critical vulnerability that would be catastrophic on mainnet. This plan outlines 5 essential safety mechanisms to prevent such events.

**Risk if not implemented:** Total network failure, loss of user funds, irreparable reputation damage.

---

## Implementation Roadmap

### **Week 1: State Monotonicity Check** ⚠️ CRITICAL
**Priority:** HIGHEST  
**Complexity:** Low  
**Impact:** Prevents loading old state files

#### Design

Create a persistent "high-water mark" file that tracks the highest round ever seen:

```rust
// crates/ultradag-coin/src/persistence/monotonicity.rs

use std::path::Path;
use std::fs;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct HighWaterMark {
    /// Highest round number ever finalized
    pub max_round: u64,
    /// Timestamp when this round was reached
    pub timestamp: i64,
    /// Hash of the state at this round (for verification)
    pub state_hash: [u8; 32],
}

impl HighWaterMark {
    /// Load from disk or create new
    pub fn load_or_create(path: &Path) -> Result<Self, Error> {
        if path.exists() {
            let data = fs::read(path)?;
            Ok(serde_json::from_slice(&data)?)
        } else {
            Ok(Self {
                max_round: 0,
                timestamp: 0,
                state_hash: [0; 32],
            })
        }
    }
    
    /// Verify that new_round is >= max_round
    pub fn verify_monotonic(&self, new_round: u64) -> Result<(), Error> {
        if new_round < self.max_round {
            return Err(Error::StateRollbackDetected {
                current: self.max_round,
                attempting: new_round,
            });
        }
        Ok(())
    }
    
    /// Update to new high-water mark
    pub fn update(&mut self, round: u64, state_hash: [u8; 32]) -> Result<(), Error> {
        if round >= self.max_round {
            self.max_round = round;
            self.timestamp = chrono::Utc::now().timestamp();
            self.state_hash = state_hash;
        }
        Ok(())
    }
    
    /// Save to disk atomically
    pub fn save(&self, path: &Path) -> Result<(), Error> {
        let data = serde_json::to_vec_pretty(self)?;
        atomic_write(path, &data)?;
        Ok(())
    }
}
```

#### Integration Points

**In `main.rs` load_state():**
```rust
async fn load_state(server: &NodeServer, data_dir: &Path) {
    let hwm_path = data_dir.join("high_water_mark.json");
    let mut hwm = HighWaterMark::load_or_create(&hwm_path)
        .expect("Failed to load high-water mark");
    
    // Load DAG
    if BlockDag::exists(&dag_path) {
        match BlockDag::load(&dag_path) {
            Ok(dag) => {
                let current_round = dag.current_round();
                
                // CRITICAL: Verify monotonicity
                if let Err(e) = hwm.verify_monotonic(current_round) {
                    error!("🚨 STATE ROLLBACK DETECTED: {}", e);
                    error!("High-water mark: {}", hwm.max_round);
                    error!("Attempting to load: {}", current_round);
                    error!("REFUSING TO START. Manual intervention required.");
                    std::process::exit(1);
                }
                
                info!("✅ Monotonicity check passed: round {}", current_round);
                *server.dag.write().await = dag;
            }
            Err(e) => warn!("Failed to load DAG: {}", e),
        }
    }
    
    // ... rest of state loading
}
```

**In validator loop (after finality):**
```rust
// Update high-water mark after each finalized round
if newly_finalized.len() > 0 {
    let max_fin = newly_finalized.iter().map(|v| v.round).max().unwrap();
    let state_hash = compute_state_hash(&state);
    hwm.update(max_fin, state_hash)?;
    hwm.save(&hwm_path)?;
}
```

#### Testing

- [ ] Test: Load state with old round number → should exit
- [ ] Test: Load state with same round number → should succeed
- [ ] Test: Load state with higher round number → should succeed
- [ ] Test: Corrupt high-water mark file → should handle gracefully
- [ ] Test: Missing high-water mark file → should create new

#### Deliverables

- `crates/ultradag-coin/src/persistence/monotonicity.rs`
- Integration in `main.rs`
- Unit tests
- Integration test

---

### **Week 2: Peer State Verification** ⚠️ CRITICAL
**Priority:** HIGH  
**Complexity:** Medium  
**Impact:** Prevents network-wide rollbacks

#### Design

Query peers on startup to verify local state is not stale:

```rust
// crates/ultradag-network/src/sync/startup_verification.rs

pub struct StartupVerifier {
    min_peers_to_query: usize,
    max_round_lag_allowed: u64,
}

impl StartupVerifier {
    pub async fn verify_state(
        &self,
        my_round: u64,
        peers: &[PeerConnection],
    ) -> Result<(), Error> {
        // Query at least N peers for their current round
        let peer_rounds = self.query_peer_rounds(peers).await?;
        
        if peer_rounds.len() < self.min_peers_to_query {
            warn!("Only {} peers responded, minimum is {}", 
                  peer_rounds.len(), self.min_peers_to_query);
            // Continue anyway but log warning
        }
        
        // Find max peer round
        let max_peer_round = peer_rounds.iter().max().copied().unwrap_or(0);
        
        // Check if we're too far behind
        if my_round + self.max_round_lag_allowed < max_peer_round {
            return Err(Error::StateTooStale {
                my_round,
                network_round: max_peer_round,
                lag: max_peer_round - my_round,
            });
        }
        
        info!("✅ Peer verification passed: my_round={}, max_peer={}", 
              my_round, max_peer_round);
        Ok(())
    }
    
    async fn query_peer_rounds(&self, peers: &[PeerConnection]) -> Result<Vec<u64>, Error> {
        let mut rounds = Vec::new();
        
        for peer in peers.iter().take(10) {  // Query up to 10 peers
            match peer.request_status().await {
                Ok(status) => {
                    rounds.push(status.dag_round);
                }
                Err(e) => {
                    warn!("Failed to query peer {}: {}", peer.addr(), e);
                }
            }
        }
        
        Ok(rounds)
    }
}
```

#### Integration

**In `main.rs` after loading state:**
```rust
// After state is loaded, verify against peers
if !args.skip_peer_verification {
    info!("Verifying state against network peers...");
    
    // Connect to bootstrap nodes first
    for seed in &args.seed {
        server.connect_to(seed).await?;
    }
    
    // Wait for peer connections
    tokio::time::sleep(Duration::from_secs(5)).await;
    
    let verifier = StartupVerifier {
        min_peers_to_query: 2,
        max_round_lag_allowed: 100,  // Allow up to 100 rounds behind
    };
    
    let my_round = server.dag.read().await.current_round();
    let peers = server.peers.get_all_peers().await;
    
    if let Err(e) = verifier.verify_state(my_round, &peers).await {
        error!("🚨 PEER VERIFICATION FAILED: {}", e);
        error!("Your state may be stale. Consider fast-sync or manual recovery.");
        
        if !args.force_start {
            error!("Refusing to start. Use --force-start to override.");
            std::process::exit(1);
        }
    }
}
```

#### Testing

- [ ] Test: Start with current state → should pass
- [ ] Test: Start with state 50 rounds behind → should pass
- [ ] Test: Start with state 200 rounds behind → should fail
- [ ] Test: No peers available → should warn but continue
- [ ] Test: Peers return conflicting rounds → should use max

#### Deliverables

- `crates/ultradag-network/src/sync/startup_verification.rs`
- CLI flag `--skip-peer-verification` for testing
- CLI flag `--force-start` for emergency override
- Integration in `main.rs`
- Tests

---

### **Week 3: Immutable State Archives** ⚠️ IMPORTANT
**Priority:** MEDIUM  
**Complexity:** Medium  
**Impact:** Prevents accidental state overwrites

#### Design

Change state persistence to write-once files with automatic cleanup:

```rust
// crates/ultradag-coin/src/persistence/archive.rs

pub struct StateArchive {
    base_dir: PathBuf,
    keep_last_n: usize,
}

impl StateArchive {
    /// Save state to immutable archive
    pub fn save_state(&self, state: &StateEngine, round: u64) -> Result<(), Error> {
        // Create filename with round number
        let filename = format!("state_{:010}.json", round);
        let path = self.base_dir.join(filename);
        
        // CRITICAL: Never overwrite existing state files
        if path.exists() {
            return Err(Error::StateAlreadyExists {
                round,
                path: path.clone(),
            });
        }
        
        // Write atomically
        let data = serde_json::to_vec_pretty(&state.snapshot())?;
        atomic_write(&path, &data)?;
        
        info!("📦 Archived state at round {}", round);
        
        // Cleanup old states
        self.cleanup_old_states(round)?;
        
        Ok(())
    }
    
    /// Keep only the last N state files
    fn cleanup_old_states(&self, current_round: u64) -> Result<(), Error> {
        let cutoff_round = current_round.saturating_sub(self.keep_last_n as u64);
        
        // List all state files
        let entries = fs::read_dir(&self.base_dir)?;
        
        for entry in entries {
            let entry = entry?;
            let filename = entry.file_name();
            
            // Parse round number from filename
            if let Some(round) = parse_round_from_filename(&filename) {
                if round < cutoff_round {
                    fs::remove_file(entry.path())?;
                    info!("🗑️  Deleted old state archive: round {}", round);
                }
            }
        }
        
        Ok(())
    }
    
    /// Load most recent state
    pub fn load_latest_state(&self) -> Result<(StateEngine, u64), Error> {
        // Find highest round number
        let entries = fs::read_dir(&self.base_dir)?;
        let mut max_round = 0;
        let mut max_path = None;
        
        for entry in entries {
            let entry = entry?;
            if let Some(round) = parse_round_from_filename(&entry.file_name()) {
                if round > max_round {
                    max_round = round;
                    max_path = Some(entry.path());
                }
            }
        }
        
        if let Some(path) = max_path {
            let data = fs::read(&path)?;
            let snapshot: StateSnapshot = serde_json::from_slice(&data)?;
            let state = StateEngine::from_snapshot(snapshot)?;
            Ok((state, max_round))
        } else {
            Err(Error::NoStateFound)
        }
    }
}

fn parse_round_from_filename(filename: &OsStr) -> Option<u64> {
    let s = filename.to_str()?;
    if s.starts_with("state_") && s.ends_with(".json") {
        let round_str = &s[6..s.len()-5];  // Extract number
        round_str.parse().ok()
    } else {
        None
    }
}
```

#### Migration Strategy

1. **Phase 1:** Add archive system alongside current persistence
2. **Phase 2:** Switch to archive for new saves
3. **Phase 3:** Migrate old state files to archive format
4. **Phase 4:** Remove old persistence code

#### Configuration

```rust
// In Args
#[arg(long, default_value = "10")]
keep_state_archives: usize,

#[arg(long)]
disable_state_archives: bool,  // For testing only
```

#### Testing

- [ ] Test: Save state twice with same round → should error
- [ ] Test: Save 20 states, verify only last 10 kept
- [ ] Test: Load latest state from archive
- [ ] Test: Corrupt one archive file → should skip and load next
- [ ] Test: No archive files → should start fresh

#### Deliverables

- `crates/ultradag-coin/src/persistence/archive.rs`
- Migration guide
- Integration in `main.rs`
- Tests

---

### **Week 4: Deployment Safety Checks** ⚠️ CRITICAL
**Priority:** HIGH  
**Complexity:** Low  
**Impact:** Prevents bad deployments

#### Design

Create pre-deployment validation script:

```bash
#!/bin/bash
# scripts/pre_deploy_check.sh
# Run this before EVERY deployment to mainnet

set -e  # Exit on any error

echo "🔍 UltraDAG Pre-Deployment Safety Check"
echo "========================================"

# 1. Check if this is mainnet
if [ "$NETWORK" != "mainnet" ]; then
    echo "⚠️  Not mainnet deployment, skipping some checks"
fi

# 2. Verify binary exists
if [ ! -f "target/release/ultradag-node" ]; then
    echo "❌ Binary not found. Run: cargo build --release"
    exit 1
fi
echo "✅ Binary found"

# 3. Check binary size (should be <5MB)
SIZE=$(stat -f%z target/release/ultradag-node 2>/dev/null || stat -c%s target/release/ultradag-node)
SIZE_MB=$((SIZE / 1024 / 1024))
if [ $SIZE_MB -gt 5 ]; then
    echo "⚠️  Binary size is ${SIZE_MB}MB (larger than expected)"
fi
echo "✅ Binary size: ${SIZE_MB}MB"

# 4. Run all tests
echo "🧪 Running test suite..."
cargo test --release --workspace 2>&1 | grep "test result"
if [ ${PIPESTATUS[0]} -ne 0 ]; then
    echo "❌ Tests failed"
    exit 1
fi
echo "✅ All tests passed"

# 5. Query live network for current round
if [ -n "$NETWORK_URL" ]; then
    echo "🌐 Querying live network..."
    NETWORK_ROUND=$(curl -s "$NETWORK_URL/status" | jq -r '.dag_round')
    echo "Network is at round: $NETWORK_ROUND"
    
    # 6. Check if we have state files
    if [ -d "$DATA_DIR" ]; then
        # Find latest state file
        LATEST_STATE=$(ls -t "$DATA_DIR"/state_*.json 2>/dev/null | head -1)
        if [ -n "$LATEST_STATE" ]; then
            # Extract round from filename
            STATE_ROUND=$(echo "$LATEST_STATE" | grep -oP 'state_\K[0-9]+')
            echo "Local state is at round: $STATE_ROUND"
            
            # 7. Verify state is not too old
            LAG=$((NETWORK_ROUND - STATE_ROUND))
            if [ $LAG -gt 1000 ]; then
                echo "❌ ERROR: Local state is $LAG rounds behind network!"
                echo "This would cause a rollback. Aborting deployment."
                exit 1
            fi
            echo "✅ State lag is acceptable: $LAG rounds"
        else
            echo "⚠️  No state files found (fresh start)"
        fi
    fi
fi

# 8. Check for uncommitted changes
if [ -n "$(git status --porcelain)" ]; then
    echo "⚠️  Uncommitted changes detected:"
    git status --short
    read -p "Continue anyway? (y/N) " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        exit 1
    fi
fi

# 9. Verify deployment target
echo ""
echo "Deployment target: $DEPLOY_TARGET"
echo "Network: $NETWORK"
echo ""
read -p "Proceed with deployment? (y/N) " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    echo "Deployment cancelled"
    exit 1
fi

echo ""
echo "✅ All safety checks passed"
echo "Safe to deploy"
```

#### Fly.io Integration

```bash
# scripts/deploy_fly.sh

#!/bin/bash
set -e

# Set environment
export NETWORK="mainnet"
export NETWORK_URL="https://ultradag-node-1.fly.dev"
export DATA_DIR="/root/.ultradag/node"
export DEPLOY_TARGET="fly.io"

# Run safety checks
./scripts/pre_deploy_check.sh

# Deploy
fly deploy --app ultradag-node-1
fly deploy --app ultradag-node-2
fly deploy --app ultradag-node-3
fly deploy --app ultradag-node-4

echo "✅ Deployment complete"
```

#### Testing

- [ ] Test: Deploy with old state → should abort
- [ ] Test: Deploy with current state → should succeed
- [ ] Test: Deploy with failing tests → should abort
- [ ] Test: Deploy with uncommitted changes → should warn

#### Deliverables

- `scripts/pre_deploy_check.sh`
- `scripts/deploy_fly.sh`
- Documentation

---

### **Week 5: Emergency Circuit Breaker** ⚠️ CRITICAL
**Priority:** HIGHEST  
**Complexity:** Low  
**Impact:** Halts network on rollback detection

#### Design

Add runtime rollback detection that halts the node:

```rust
// crates/ultradag-coin/src/safety/circuit_breaker.rs

use std::sync::atomic::{AtomicU64, Ordering};
use tracing::{error, warn};

pub struct CircuitBreaker {
    /// Last finalized round seen
    last_finalized: AtomicU64,
    /// Whether circuit breaker is enabled
    enabled: bool,
}

impl CircuitBreaker {
    pub fn new(enabled: bool) -> Self {
        Self {
            last_finalized: AtomicU64::new(0),
            enabled,
        }
    }
    
    /// Check if round is moving forward
    /// HALTS THE PROCESS if rollback detected
    pub fn check_finality(&self, current_round: u64) {
        if !self.enabled {
            return;
        }
        
        let last = self.last_finalized.load(Ordering::SeqCst);
        
        if current_round < last {
            // CRITICAL: ROLLBACK DETECTED
            error!("╔═══════════════════════════════════════╗");
            error!("║  🚨 EMERGENCY CIRCUIT BREAKER 🚨     ║");
            error!("║  ROLLBACK DETECTED - HALTING NODE    ║");
            error!("╚═══════════════════════════════════════╝");
            error!("");
            error!("Last finalized round: {}", last);
            error!("Current round: {}", current_round);
            error!("Rollback amount: {} rounds", last - current_round);
            error!("");
            error!("This indicates a critical consensus failure.");
            error!("The node is halting to prevent state corruption.");
            error!("");
            error!("MANUAL INTERVENTION REQUIRED:");
            error!("1. Check all validator logs");
            error!("2. Verify network state with other operators");
            error!("3. Determine root cause");
            error!("4. Coordinate recovery plan");
            error!("");
            error!("DO NOT RESTART without understanding the cause.");
            
            // HALT THE PROCESS
            std::process::exit(100);  // Exit code 100 = circuit breaker triggered
        }
        
        // Update last finalized
        self.last_finalized.store(current_round, Ordering::SeqCst);
    }
    
    /// Check if round is advancing too slowly (possible stall)
    pub fn check_liveness(&self, current_round: u64, max_lag: u64) {
        if !self.enabled {
            return;
        }
        
        let last = self.last_finalized.load(Ordering::SeqCst);
        
        if last > 0 && current_round == last {
            // No progress - this is checked elsewhere
            return;
        }
        
        // Check for large gaps (possible network partition)
        if current_round > last + max_lag {
            warn!("⚠️  Large finality gap detected: {} rounds", current_round - last);
            warn!("Possible network partition or synchronization issue");
        }
    }
}
```

#### Integration

**In validator loop:**
```rust
// After finality check
let newly_finalized = finality.find_newly_finalized(&dag);

for vertex in &newly_finalized {
    // CRITICAL: Check circuit breaker
    circuit_breaker.check_finality(vertex.round);
    
    // Apply to state
    state.apply_vertex(vertex)?;
}
```

**In main.rs:**
```rust
let circuit_breaker = Arc::new(CircuitBreaker::new(
    !args.disable_circuit_breaker  // Enabled by default
));
```

#### Configuration

```rust
#[arg(long)]
disable_circuit_breaker: bool,  // For testing ONLY
```

#### Testing

- [ ] Test: Finalize rounds 1, 2, 3 → should succeed
- [ ] Test: Finalize rounds 1, 2, 1 → should halt
- [ ] Test: Large gap (1 → 1000) → should warn
- [ ] Test: Circuit breaker disabled → should not halt

#### Deliverables

- `crates/ultradag-coin/src/safety/circuit_breaker.rs`
- Integration in validator loop
- Tests
- Emergency runbook

---

### **Week 6: Testing & Documentation** ⚠️ CRITICAL
**Priority:** HIGHEST  
**Complexity:** Medium  
**Impact:** Validates all safety mechanisms

#### Integration Tests

```rust
// tests/rollback_prevention.rs

#[tokio::test]
async fn test_state_monotonicity_prevents_rollback() {
    // 1. Start node, finalize to round 100
    // 2. Stop node
    // 3. Replace state file with round 50
    // 4. Attempt to restart
    // Expected: Node refuses to start
}

#[tokio::test]
async fn test_peer_verification_detects_stale_state() {
    // 1. Start 3 nodes, sync to round 100
    // 2. Stop node 1
    // 3. Other nodes advance to round 200
    // 4. Attempt to restart node 1
    // Expected: Node detects it's behind and warns
}

#[tokio::test]
async fn test_immutable_archives_prevent_overwrite() {
    // 1. Save state at round 100
    // 2. Attempt to save state at round 100 again
    // Expected: Error, file already exists
}

#[tokio::test]
async fn test_circuit_breaker_halts_on_rollback() {
    // 1. Finalize rounds 1, 2, 3
    // 2. Attempt to finalize round 2 again
    // Expected: Process exits with code 100
}
```

#### Documentation

**Create `docs/ROLLBACK_PREVENTION.md`:**
- Overview of all 5 mechanisms
- How they work together
- Configuration options
- Testing procedures
- Emergency procedures

**Create `docs/EMERGENCY_RUNBOOK.md`:**
- What to do if circuit breaker triggers
- How to diagnose rollback cause
- Recovery procedures
- Coordination with other validators

**Update `README.md`:**
- Add safety features section
- Link to rollback prevention docs

#### Deliverables

- Integration test suite
- `docs/ROLLBACK_PREVENTION.md`
- `docs/EMERGENCY_RUNBOOK.md`
- Updated `README.md`

---

## Timeline Summary

| Week | Feature | Status | Blocker |
|------|---------|--------|---------|
| 1 | State Monotonicity Check | ⚠️ Not Started | None |
| 2 | Peer State Verification | ⚠️ Not Started | Week 1 |
| 3 | Immutable State Archives | ⚠️ Not Started | None |
| 4 | Deployment Safety Checks | ⚠️ Not Started | None |
| 5 | Emergency Circuit Breaker | ⚠️ Not Started | Week 1 |
| 6 | Testing & Documentation | ⚠️ Not Started | Weeks 1-5 |

**Total Duration:** 6 weeks  
**Parallel Work:** Weeks 1, 3, 4 can be done in parallel  
**Critical Path:** Week 1 → Week 2 → Week 5 → Week 6

---

## Success Criteria

### **Must Pass Before Mainnet:**

- [ ] All 5 safety mechanisms implemented
- [ ] All integration tests passing
- [ ] Manual rollback test successful (testnet)
- [ ] Deployment safety checks validated
- [ ] Emergency runbook reviewed by team
- [ ] Circuit breaker tested in production-like environment

### **Validation Tests:**

1. **Rollback Prevention Test:**
   - Manually trigger rollback on testnet
   - Verify all mechanisms activate correctly
   - Verify node refuses to start or halts appropriately

2. **Deployment Safety Test:**
   - Attempt to deploy with old state
   - Verify pre-deploy check catches it
   - Verify deployment is aborted

3. **Network Partition Test:**
   - Partition testnet into 2 groups
   - Verify circuit breaker detects divergence
   - Verify safe recovery after partition heals

---

## Risk Assessment

### **If Not Implemented:**

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Mainnet rollback | HIGH | CATASTROPHIC | Implement all 5 mechanisms |
| Loss of user funds | HIGH | CATASTROPHIC | Implement all 5 mechanisms |
| Reputation damage | CERTAIN | SEVERE | Implement all 5 mechanisms |
| Legal liability | MEDIUM | SEVERE | Implement all 5 mechanisms |

### **If Implemented:**

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| False positive halt | LOW | MEDIUM | Manual override flags |
| Deployment complexity | MEDIUM | LOW | Automation scripts |
| Performance impact | LOW | LOW | Minimal overhead |

---

## Resource Requirements

### **Development Time:**
- Senior developer: 6 weeks full-time
- OR 2 developers: 3 weeks each

### **Testing Time:**
- QA: 2 weeks
- Testnet validation: 1 week

### **Total Time to Mainnet:**
- Optimistic: 6 weeks
- Realistic: 8 weeks
- Conservative: 10 weeks

---

## Next Steps

1. **Immediate (This Week):**
   - Review and approve this plan
   - Assign developer(s)
   - Set up tracking (GitHub issues/project board)

2. **Week 1:**
   - Begin state monotonicity implementation
   - Begin immutable archives implementation (parallel)
   - Begin deployment scripts (parallel)

3. **Week 2:**
   - Complete state monotonicity
   - Begin peer verification
   - Continue archives and scripts

4. **Ongoing:**
   - Daily standup on progress
   - Weekly demo of completed features
   - Continuous testing on testnet

---

## Conclusion

**The testnet rollback was a gift** - it revealed a critical vulnerability before mainnet.

**These 5 mechanisms are non-negotiable** for mainnet launch. Without them, a rollback is not a question of "if" but "when".

**Timeline is aggressive but achievable** with focused effort.

**Success means:** A mainnet that can never rollback, protecting user funds and network integrity.

---

**Status:** Ready for implementation  
**Next Action:** Assign developer and begin Week 1 tasks  
**Target Mainnet Date:** 6-10 weeks from start
