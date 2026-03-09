# Checkpoint Sync Protocol

## Overview

The checkpoint sync protocol enables new nodes to rapidly synchronize with the network by downloading verified state snapshots instead of replaying the entire DAG history. This reduces sync time from hours/days to minutes.

## Key Concepts

### Checkpoint Structure

A checkpoint is a cryptographically signed snapshot of the network state at a specific finalized round:

```rust
pub struct Checkpoint {
    pub round: u64,                           // Finalized round number
    pub state_root: [u8; 32],                 // Merkle root of state snapshot
    pub dag_tip: [u8; 32],                    // Hash of a DAG tip at this round
    pub total_supply: u64,                    // Total coin supply at this round
    pub signatures: Vec<CheckpointSignature>, // BFT signatures from validators
}
```

### Checkpoint Production

**Frequency:** Every `CHECKPOINT_INTERVAL` rounds (currently 100 rounds ≈ 8 minutes at 5s/round)

**Producer:** The validator that finalizes the checkpoint round produces and broadcasts the checkpoint proposal.

**Process:**
1. Validator detects finalization of round `R` where `R % CHECKPOINT_INTERVAL == 0`
2. Computes state root from current state snapshot
3. Signs checkpoint with validator key
4. Stores checkpoint locally
5. Broadcasts `CheckpointProposal` to all peers

### Checkpoint Co-Signing

**Quorum:** Requires signatures from ⌈2n/3⌉ validators (BFT threshold)

**Process:**
1. Peer receives `CheckpointProposal` for round `R`
2. Validates checkpoint:
   - Round `R` is finalized locally
   - State root matches local state
   - Proposer signature is valid
3. If valid, co-signs checkpoint with own validator key
4. Broadcasts `CheckpointSignatureMsg` to all peers
5. Stores in `pending_checkpoints` map

**Quorum Acceptance:**
- When a checkpoint accumulates ⌈2n/3⌉ signatures, it's accepted
- Accepted checkpoint is persisted to disk
- Checkpoint becomes available for fast-sync

### Checkpoint Persistence

**Storage Location:** `{data_dir}/checkpoint_{round}.json`

**Format:** JSON-serialized checkpoint with all signatures

**Retention:** Latest checkpoint only (older checkpoints are deleted)

**Loading:** On startup, node loads the latest checkpoint from disk

## Fast-Sync Protocol

### Overview

Fast-sync allows a new node to bootstrap from a trusted checkpoint instead of syncing from genesis.

### Bootstrap Process

**Step 1: Request Checkpoint**

New node broadcasts `GetCheckpoint` message to all connected peers:

```rust
Message::GetCheckpoint
```

**Step 2: Receive Checkpoint Response**

Peer responds with `CheckpointResponse`:

```rust
Message::CheckpointResponse {
    checkpoint: Checkpoint,           // Signed checkpoint
    suffix_vertices: Vec<DagVertex>,  // Recent DAG vertices (up to 500)
}
```

**Suffix Vertices:** Recent DAG vertices from checkpoint round to current round, enabling the node to catch up to the current tip.

**Size Limit:** Maximum 500 vertices to stay within 4MB message limit.

### Checkpoint Validation

Before accepting a checkpoint, the node validates:

1. **Signature Quorum:** Checkpoint has ⌈2n/3⌉ valid signatures
2. **Validator Set:** All signers are in the known validator set
3. **Signature Validity:** Each signature is cryptographically valid
4. **Round Consistency:** Checkpoint round is a multiple of `CHECKPOINT_INTERVAL`

### State Restoration

**Step 1: Load State Snapshot**

```rust
// Deserialize state from checkpoint
let state_snapshot = checkpoint.state_snapshot;
state_engine.restore_from_snapshot(state_snapshot);
```

**Step 2: Restore DAG**

```rust
// Insert suffix vertices into DAG
for vertex in suffix_vertices {
    dag.insert_vertex(vertex)?;
}
```

**Step 3: Reset Finality Tracker**

```rust
// Reset finality to checkpoint round
finality_tracker.reset_to_checkpoint(checkpoint.round);

// Register all validators from state
for validator in state.active_validators() {
    finality_tracker.register_validator(validator);
}
```

**Step 4: Mark Sync Complete**

```rust
sync_complete.store(true, Ordering::SeqCst);
```

## Message Flow Diagrams

### Checkpoint Production Flow

```
Validator A (Proposer)
    |
    | 1. Finalize round 200 (checkpoint interval)
    |
    | 2. Compute state_root from state snapshot
    |
    | 3. Sign checkpoint
    |
    | 4. Store locally: checkpoint_200.json
    |
    | 5. Broadcast CheckpointProposal
    |
    v
All Peers
    |
    | 6. Validate checkpoint
    |
    | 7. Co-sign if valid
    |
    | 8. Broadcast CheckpointSignatureMsg
    |
    v
All Validators
    |
    | 9. Collect signatures in pending_checkpoints
    |
    | 10. When quorum reached (3/4 signatures):
    |     - Accept checkpoint
    |     - Persist to disk
    |     - Clean up old checkpoints
```

### Fast-Sync Flow

```
New Node                          Peer Node
    |                                 |
    | 1. GetCheckpoint                |
    |-------------------------------->|
    |                                 |
    |                                 | 2. Load latest checkpoint
    |                                 |
    |                                 | 3. Collect suffix vertices
    |                                 |
    | 4. CheckpointResponse            |
    |<--------------------------------|
    |    (checkpoint + 500 vertices)  |
    |                                 |
    | 5. Validate checkpoint          |
    |    - Check quorum               |
    |    - Verify signatures          |
    |                                 |
    | 6. Restore state from snapshot  |
    |                                 |
    | 7. Insert suffix vertices       |
    |                                 |
    | 8. Reset finality tracker       |
    |                                 |
    | 9. Mark sync complete           |
    |                                 |
    | 10. Begin normal operation      |
```

## Security Considerations

### Trust Model

**Bootstrap Trust:** New nodes trust the checkpoint if it has ⌈2n/3⌉ valid signatures from known validators.

**Validator Set:** The initial validator set must be known (hardcoded or configured) to verify checkpoint signatures.

**BFT Guarantee:** With ⌈2n/3⌉ signatures, at most ⌊n/3⌋ validators can be Byzantine, ensuring safety.

### Attack Vectors

**1. Fake Checkpoint Attack**

**Attack:** Malicious peer sends a fake checkpoint with forged signatures.

**Defense:**
- Signature verification ensures only valid validator signatures are accepted
- Quorum requirement prevents single malicious validator from creating fake checkpoint
- State root verification ensures consistency

**2. Stale Checkpoint Attack**

**Attack:** Malicious peer sends an old checkpoint to prevent node from syncing to current state.

**Defense:**
- Node requests checkpoints from multiple peers
- Accepts the checkpoint with the highest round number
- Suffix vertices allow catching up to current tip

**3. Checkpoint Withholding**

**Attack:** Malicious validators refuse to co-sign checkpoints, preventing checkpoint creation.

**Defense:**
- Only ⌈2n/3⌉ signatures required (BFT threshold)
- Up to ⌊n/3⌋ validators can be offline/malicious without blocking checkpoints
- Checkpoint production is automatic and decentralized

**4. State Root Manipulation**

**Attack:** Malicious validator signs checkpoint with incorrect state root.

**Defense:**
- Each validator independently computes state root from local state
- Validators only sign if state root matches their local computation
- Quorum ensures majority of honest validators agree on state root

## Performance Characteristics

### Checkpoint Size

**State Snapshot:** ~1-10 MB (depends on number of accounts and proposals)

**Suffix Vertices:** ~500 vertices × 2 KB = ~1 MB

**Total:** ~2-11 MB per checkpoint

### Sync Time Comparison

**Full Sync (from genesis):**
- 100,000 rounds × 5s = ~6 days of history
- Download and verify all vertices
- Replay all transactions
- **Estimated time:** 2-4 hours

**Fast-Sync (from checkpoint):**
- Download checkpoint (~10 MB)
- Verify signatures
- Restore state snapshot
- Insert suffix vertices
- **Estimated time:** 30-60 seconds

**Speedup:** ~100-200x faster

### Network Bandwidth

**Checkpoint Production:** ~10 MB broadcast every 100 rounds (8 minutes)

**Checkpoint Co-Signing:** 4 validators × 96 bytes = ~384 bytes per checkpoint

**Fast-Sync:** ~10 MB download per new node

## Implementation Details

### Checkpoint Storage

**File Format:**
```json
{
  "round": 200,
  "state_root": "a1b2c3...",
  "dag_tip": "d4e5f6...",
  "total_supply": 21000000000000000,
  "signatures": [
    {
      "validator": "0x1234...",
      "pub_key": "abcd...",
      "signature": "ef01..."
    }
  ]
}
```

**Cleanup Policy:**
- Keep only the latest checkpoint
- Delete checkpoints older than current - 1000 rounds
- Prevents disk bloat

### Pending Checkpoints

**Data Structure:**
```rust
pending_checkpoints: Arc<RwLock<HashMap<u64, Checkpoint>>>
```

**Eviction Policy:**
- Maximum 10 pending checkpoints
- Evict oldest when limit exceeded
- Prevents memory exhaustion

### Checkpoint Interval

**Current Value:** 100 rounds

**Rationale:**
- Frequent enough for fast bootstrap (8 minutes of history to replay)
- Infrequent enough to minimize overhead (~10 MB every 8 minutes)
- Aligned with pruning horizon (1000 rounds)

**Tuning:** Can be adjusted based on network conditions and storage constraints.

## Error Handling

### Checkpoint Production Failures

**State Root Computation Error:**
- Log error
- Skip checkpoint production for this round
- Next checkpoint will be produced at next interval

**Signature Failure:**
- Log error
- Skip checkpoint production
- Does not block consensus

**Persistence Failure:**
- Log error
- Checkpoint still broadcast (peers can persist)
- Retry on next checkpoint

### Checkpoint Sync Failures

**No Peers Respond:**
- Retry with exponential backoff (3 attempts, 10s between)
- Fall back to full sync from genesis if all retries fail

**Invalid Checkpoint Received:**
- Reject checkpoint
- Request from different peer
- Log malicious peer for potential ban

**State Restoration Failure:**
- Log error
- Clear partial state
- Retry fast-sync or fall back to full sync

## Monitoring and Metrics

### Key Metrics

**Checkpoint Production:**
- `checkpoint_produced_total` - Total checkpoints produced
- `checkpoint_production_duration_ms` - Time to produce checkpoint
- `checkpoint_size_bytes` - Size of checkpoint file

**Checkpoint Co-Signing:**
- `checkpoint_cosigned_total` - Total checkpoints co-signed
- `checkpoint_quorum_reached_total` - Checkpoints that reached quorum
- `checkpoint_signatures_collected` - Signatures per checkpoint

**Fast-Sync:**
- `fast_sync_attempts_total` - Total fast-sync attempts
- `fast_sync_success_total` - Successful fast-syncs
- `fast_sync_duration_ms` - Time to complete fast-sync
- `fast_sync_bytes_downloaded` - Bytes downloaded during sync

### Health Checks

**Checkpoint Lag:**
- Alert if no checkpoint produced in last 200 rounds
- Indicates checkpoint production failure

**Signature Participation:**
- Alert if validator hasn't co-signed in last 10 checkpoints
- Indicates validator offline or malicious

**Fast-Sync Availability:**
- Alert if no valid checkpoint available for new nodes
- Indicates checkpoint storage or network issues

## Future Enhancements

### Incremental Checkpoints

**Concept:** Store state diffs instead of full snapshots

**Benefits:**
- Smaller checkpoint size
- Faster checkpoint production
- Lower storage requirements

**Implementation:** Use Merkle tree diffs to compute minimal state changes

### Checkpoint Compression

**Concept:** Compress checkpoint data before storage/transmission

**Benefits:**
- Reduced bandwidth usage
- Faster sync times
- Lower storage costs

**Implementation:** Use zstd or similar compression algorithm

### Checkpoint Sharding

**Concept:** Split large checkpoints into multiple shards

**Benefits:**
- Parallel download from multiple peers
- Resilience to peer failures
- Better load distribution

**Implementation:** Merkle tree sharding with proof verification

### Historical Checkpoint Archive

**Concept:** Maintain archive of historical checkpoints for audit/analysis

**Benefits:**
- Historical state queries
- Audit trail for compliance
- Research and analytics

**Implementation:** Optional archive node role with extended storage

## References

- **BFT Consensus:** Castro & Liskov, "Practical Byzantine Fault Tolerance" (1999)
- **State Snapshots:** Ethereum's Warp Sync, Cosmos SDK State Sync
- **Merkle Proofs:** Merkle, "A Digital Signature Based on a Conventional Encryption Function" (1987)
- **Checkpoint Security:** Buterin, "Casper the Friendly Finality Gadget" (2017)

## Appendix: Code Examples

### Producing a Checkpoint

```rust
// In validator loop, after finalization
let last_finalized = finality.read().await.last_finalized_round();

if last_finalized > 0 && last_finalized % CHECKPOINT_INTERVAL == 0 {
    // Compute state root
    let state_snapshot = state.read().await.snapshot();
    let state_root = compute_state_root(&state_snapshot);
    
    // Get DAG tip
    let dag_tip = dag.read().await.tips().first().copied().unwrap_or([0u8; 32]);
    
    // Create checkpoint
    let mut checkpoint = Checkpoint {
        round: last_finalized,
        state_root,
        dag_tip,
        total_supply: state.read().await.total_supply(),
        signatures: vec![],
    };
    
    // Sign checkpoint
    let sig = CheckpointSignature {
        validator: validator_addr,
        pub_key: sk.verifying_key().to_bytes(),
        signature: sk.sign(&checkpoint.signable_bytes()),
    };
    checkpoint.signatures.push(sig);
    
    // Persist locally
    save_checkpoint(&data_dir, &checkpoint)?;
    
    // Store in pending for co-signature collection
    pending_checkpoints.write().await.insert(last_finalized, checkpoint.clone());
    
    // Broadcast to peers
    peers.broadcast(&Message::CheckpointProposal(checkpoint), "").await;
}
```

### Fast-Sync Request

```rust
// On startup, if no local state
if !has_local_state() {
    for attempt in 0..3 {
        // Request checkpoint from peers
        peers.broadcast(&Message::GetCheckpoint, "").await;
        
        // Wait for response (with timeout)
        tokio::time::sleep(Duration::from_secs(10)).await;
        
        // Check if checkpoint received
        if let Some(checkpoint) = received_checkpoint.lock().await.take() {
            // Validate checkpoint
            if validate_checkpoint(&checkpoint, &validator_set) {
                // Restore state
                restore_from_checkpoint(checkpoint, state, dag, finality).await?;
                sync_complete.store(true, Ordering::SeqCst);
                break;
            }
        }
    }
}
```

### Checkpoint Validation

```rust
fn validate_checkpoint(checkpoint: &Checkpoint, validator_set: &ValidatorSet) -> bool {
    // Check round is checkpoint interval
    if checkpoint.round % CHECKPOINT_INTERVAL != 0 {
        return false;
    }
    
    // Verify signatures
    let mut valid_sigs = 0;
    for sig in &checkpoint.signatures {
        // Check signer is in validator set
        if !validator_set.contains(&sig.validator) {
            continue;
        }
        
        // Verify signature
        if let Ok(vk) = VerifyingKey::from_bytes(&sig.pub_key) {
            if vk.verify_strict(&checkpoint.signable_bytes(), &sig.signature).is_ok() {
                valid_sigs += 1;
            }
        }
    }
    
    // Check quorum (⌈2n/3⌉)
    let quorum = validator_set.quorum_threshold();
    valid_sigs >= quorum
}
```
