# UltraDAG Emergency Runbook

**Purpose:** Step-by-step procedures for handling critical incidents  
**Audience:** Node operators, validators, core team  
**Last Updated:** March 8, 2026

---

## 🚨 Emergency Contacts

**Core Team:**
- Primary: [Contact info]
- Secondary: [Contact info]

**Validator Coordination:**
- Discord: [Channel]
- Telegram: [Group]

---

## Circuit Breaker Triggered (Exit Code 100)

### **Symptoms**
- Node exits with code 100
- Log shows: "🚨 EMERGENCY CIRCUIT BREAKER"
- Message: "ROLLBACK DETECTED - HALTING NODE"

### **What This Means**
A rollback was detected during runtime. The node attempted to finalize a round number **lower** than a previously finalized round. This should never happen in normal operation.

### **Immediate Actions**

1. **DO NOT RESTART THE NODE**
   - Restarting will not fix the issue
   - May make diagnosis harder

2. **Preserve Logs**
   ```bash
   # Copy all logs immediately
   fly logs -a ultradag-node-X > emergency_logs_$(date +%s).txt
   
   # Copy state files
   cp -r ~/.ultradag/node emergency_state_$(date +%s)
   ```

3. **Check High-Water Mark**
   ```bash
   cat ~/.ultradag/node/high_water_mark.json
   ```
   Note the `max_round` value.

4. **Contact Other Validators**
   - Ask for their current round
   - Ask if they experienced similar issues
   - Compare state hashes

### **Diagnosis**

**Check the logs for:**
```
Last finalized round: X
Current round: Y
Rollback amount: Z rounds
```

**Possible Causes:**

| Cause | Evidence | Action |
|-------|----------|--------|
| Network partition healed | Other validators at different rounds | Coordinate state sync |
| State corruption | Inconsistent state hash | Restore from checkpoint |
| Consensus bug | Multiple validators affected | Emergency patch needed |
| Deployment error | Only your node affected | Restore correct state |

### **Recovery Procedures**

#### **Option 1: Fast-Sync from Network**
```bash
# Delete local state (keeps high-water mark)
rm ~/.ultradag/node/dag.json
rm ~/.ultradag/node/state.json
rm ~/.ultradag/node/finality.json

# Restart node - will sync from peers
ultradag-node --validator <key> --seed <peers>
```

#### **Option 2: Restore from Checkpoint**
```bash
# Find latest checkpoint
ls -lt ~/.ultradag/node/checkpoint_*.json | head -1

# Verify checkpoint round is >= high-water mark
jq '.round' checkpoint_XXXXXXXXXX.json

# If valid, use checkpoint to rebuild state
# (Manual state reconstruction required)
```

#### **Option 3: Reset High-Water Mark (DANGEROUS)**
**Only if you are certain the rollback is spurious:**
```bash
# Backup first
cp ~/.ultradag/node/high_water_mark.json hwm_backup.json

# Edit to lower round (use with extreme caution)
# This bypasses the safety mechanism
```

### **Prevention**
- Ensure all validators run the same software version
- Use deployment safety checks before updates
- Monitor finality lag continuously

---

## State Rollback Detected on Startup

### **Symptoms**
- Node refuses to start
- Log shows: "🚨 STATE ROLLBACK DETECTED - REFUSING TO START"
- Exit code 1

### **What This Means**
The state file on disk has a round number **lower** than the high-water mark. This means you're trying to load old state.

### **Immediate Actions**

1. **Check What State You're Loading**
   ```bash
   # Check DAG state
   jq '.current_round' ~/.ultradag/node/dag.json
   
   # Check high-water mark
   jq '.max_round' ~/.ultradag/node/high_water_mark.json
   ```

2. **Check Network State**
   ```bash
   curl https://ultradag-node-1.fly.dev/status | jq '.dag_round'
   ```

### **Common Causes**

#### **Cause 1: Deployed Old Docker Image**
**Evidence:** Deployment logs show old image hash

**Fix:**
```bash
# Deploy correct image
./scripts/deploy_fly.sh

# Or manually
fly deploy --app ultradag-node-X --image <correct-image>
```

#### **Cause 2: Restored from Old Backup**
**Evidence:** State file timestamp is old

**Fix:**
```bash
# Delete old state, fast-sync from network
rm ~/.ultradag/node/dag.json
rm ~/.ultradag/node/state.json
rm ~/.ultradag/node/finality.json

# Keep high-water mark - it's correct
# Restart will sync from peers
```

#### **Cause 3: Volume Rollback (Fly.io)**
**Evidence:** All state files have old timestamps

**Fix:**
```bash
# Check Fly.io volume snapshots
fly volumes list -a ultradag-node-X

# Restore from correct snapshot or start fresh
```

### **Recovery**
Fast-sync is the safest option:
```bash
# Keep high-water mark, delete state
rm ~/.ultradag/node/dag.json
rm ~/.ultradag/node/state.json
rm ~/.ultradag/node/finality.json
rm ~/.ultradag/node/mempool.json

# Restart - will sync from network
ultradag-node --validator <key> --seed <peers>
```

---

## Network-Wide Rollback

### **Symptoms**
- Multiple validators report rollback
- Network has split into groups at different rounds
- Finality has stopped

### **What This Means**
Critical consensus failure. Requires coordinated recovery.

### **Immediate Actions**

1. **HALT ALL VALIDATORS**
   ```bash
   # Stop all nodes immediately
   fly scale count 0 -a ultradag-node-1
   fly scale count 0 -a ultradag-node-2
   fly scale count 0 -a ultradag-node-3
   fly scale count 0 -a ultradag-node-4
   ```

2. **Emergency Coordination Call**
   - Gather all validator operators
   - Share logs and state
   - Determine canonical state

3. **Identify Fork Point**
   ```bash
   # Compare state hashes at each round
   for round in {100..200}; do
     curl https://node1/round/$round | jq '.[] | .hash'
   done
   ```

### **Recovery Strategy**

**Option A: Rollback to Last Common Checkpoint**
1. Find last checkpoint all validators agree on
2. All validators restore from that checkpoint
3. Restart network from that point

**Option B: Canonical Chain Selection**
1. Identify chain with most stake
2. All validators sync to that chain
3. Discard minority chain

**Option C: Emergency Patch**
1. If caused by consensus bug
2. Deploy fix to all validators
3. Coordinate restart

### **Post-Recovery**
- Root cause analysis
- Update consensus rules if needed
- Improve monitoring
- Update this runbook

---

## Deployment Failure

### **Symptoms**
- Pre-deploy check fails
- Deployment aborted

### **Common Failures**

#### **"State is too old"**
```
❌ ERROR: Local state is 1500 rounds behind network!
```

**Fix:**
```bash
# Sync state before deploying
# Or deploy with fresh state
rm -rf ~/.ultradag/node/*.json
./scripts/deploy_fly.sh
```

#### **"Tests failed"**
```
❌ Tests failed
```

**Fix:**
```bash
# Run tests locally to diagnose
cargo test --workspace

# Fix failing tests
# Commit fixes
# Re-run deployment
```

#### **"Network unreachable"**
```
⚠️  Could not reach network
```

**Fix:**
```bash
# Check network status
curl https://ultradag-node-1.fly.dev/status

# If down, check Fly.io status
fly status -a ultradag-node-1

# Deploy anyway if testnet (use caution on mainnet)
```

---

## Monitoring Alerts

### **High Finality Lag**
**Alert:** Finality lag > 10 rounds

**Actions:**
1. Check if validator is producing vertices
2. Check peer connectivity
3. Check for network partition
4. Restart if necessary

### **Supply Divergence**
**Alert:** Nodes report different total_supply

**Actions:**
1. **CRITICAL** - This indicates state corruption
2. Halt affected nodes immediately
3. Compare state hashes
4. Identify divergence point
5. Coordinate recovery

### **Round Rollback Detected**
**Alert:** Monitoring shows round decreased

**Actions:**
1. Check all nodes immediately
2. Preserve logs and state
3. Follow "Network-Wide Rollback" procedure

---

## Testing Procedures

### **Test Circuit Breaker (Testnet Only)**

**DO NOT RUN ON MAINNET**

```bash
# 1. Note current round
ROUND=$(curl https://ultradag-node-1.fly.dev/status | jq '.dag_round')

# 2. Stop node
fly scale count 0 -a ultradag-node-1

# 3. Manually edit high-water mark to future round
jq '.max_round = 99999' ~/.ultradag/node/high_water_mark.json > tmp.json
mv tmp.json ~/.ultradag/node/high_water_mark.json

# 4. Restart node
fly scale count 1 -a ultradag-node-1

# 5. Observe circuit breaker trigger
fly logs -a ultradag-node-1

# Expected: Node exits with code 100
# Expected: Log shows "EMERGENCY CIRCUIT BREAKER"

# 6. Restore correct high-water mark
jq '.max_round = '$ROUND ~/.ultradag/node/high_water_mark.json > tmp.json
mv tmp.json ~/.ultradag/node/high_water_mark.json

# 7. Restart normally
fly scale count 1 -a ultradag-node-1
```

### **Test Monotonicity Check (Testnet Only)**

```bash
# 1. Stop node
fly scale count 0 -a ultradag-node-1

# 2. Replace state with old state
# (Requires having saved old state file)
cp old_dag.json ~/.ultradag/node/dag.json

# 3. Attempt to start
fly scale count 1 -a ultradag-node-1

# Expected: Node refuses to start
# Expected: Log shows "STATE ROLLBACK DETECTED"

# 4. Restore correct state
rm ~/.ultradag/node/dag.json
fly scale count 1 -a ultradag-node-1
```

---

## Escalation Matrix

| Severity | Response Time | Who to Contact |
|----------|---------------|----------------|
| **P0 - Network Down** | Immediate | All validators + core team |
| **P1 - Rollback Detected** | < 15 min | All validators |
| **P2 - High Finality Lag** | < 1 hour | On-call validator |
| **P3 - Deployment Issue** | < 4 hours | DevOps team |

---

## Appendix: Useful Commands

### **Check Node Health**
```bash
# Status
curl https://ultradag-node-X.fly.dev/status | jq

# Specific round
curl https://ultradag-node-X.fly.dev/round/1234 | jq

# Logs
fly logs -a ultradag-node-X

# SSH into node
fly ssh console -a ultradag-node-X
```

### **State Inspection**
```bash
# High-water mark
jq '.' ~/.ultradag/node/high_water_mark.json

# DAG state
jq '.current_round' ~/.ultradag/node/dag.json

# State engine
jq '.total_supply, .last_finalized_round' ~/.ultradag/node/state.json
```

### **Emergency Backup**
```bash
# Backup all state
tar -czf emergency_backup_$(date +%s).tar.gz ~/.ultradag/node/

# Upload to safe location
# (Add your backup destination)
```

---

## Post-Incident Review Template

**Date:**  
**Incident:** [Brief description]  
**Duration:** [How long network was affected]  
**Impact:** [Users/validators affected]

**Timeline:**
- [Time] - Incident detected
- [Time] - Team notified
- [Time] - Root cause identified
- [Time] - Fix deployed
- [Time] - Network recovered

**Root Cause:**  
[Detailed explanation]

**Resolution:**  
[What was done to fix it]

**Prevention:**  
[What will prevent this in future]

**Action Items:**
- [ ] Update monitoring
- [ ] Update runbook
- [ ] Deploy fixes
- [ ] Train team

---

**Remember:** When in doubt, halt the network and coordinate. Better to be down for an hour than corrupted forever.
