# Staking Broadcast Fix — Test Plan

**Issue:** Staking transactions were applied locally only, not broadcast via P2P  
**Fix:** Added `Message::NewTx` broadcast after `apply_stake_tx()` and `apply_unstake_tx()`  
**Files Modified:** `crates/ultradag-node/src/rpc.rs`

---

## Changes Made

### 1. `/stake` Endpoint (lines 483-502)

**Before:**
```rust
let mut state = server.state.write().await;
match state.apply_stake_tx(&tx) {
    Ok(()) => {
        json_response(StatusCode::OK, &serde_json::json!({
            "status": "staked",
            "address": sender.to_hex(),
            "amount": stake_req.amount,
            "amount_udag": stake_req.amount as f64 / 100_000_000.0,
        }))
    }
    Err(e) => error_response(StatusCode::BAD_REQUEST, &e.to_string()),
}
```

**After:**
```rust
let mut state = server.state.write().await;
match state.apply_stake_tx(&tx) {
    Ok(()) => {
        drop(state);
        
        // Broadcast stake transaction to all peers (critical fix)
        let wrapped_tx = Transaction::Stake(tx.clone());
        let msg = Message::NewTx(wrapped_tx);
        server.peers.broadcast(&msg).await;
        
        json_response(StatusCode::OK, &serde_json::json!({
            "status": "staked",
            "address": sender.to_hex(),
            "amount": stake_req.amount,
            "amount_udag": stake_req.amount as f64 / 100_000_000.0,
            "broadcast": "sent to all peers",
        }))
    }
    Err(e) => error_response(StatusCode::BAD_REQUEST, &e.to_string()),
}
```

### 2. `/unstake` Endpoint (lines 544-563)

**Before:**
```rust
let mut state = server.state.write().await;
match state.apply_unstake_tx(&tx, current_round) {
    Ok(()) => {
        let unlock_at = current_round + ultradag_coin::UNSTAKE_COOLDOWN_ROUNDS;
        json_response(StatusCode::OK, &serde_json::json!({
            "status": "unstaking",
            "address": sender.to_hex(),
            "unlock_at_round": unlock_at,
        }))
    }
    Err(e) => error_response(StatusCode::BAD_REQUEST, &e.to_string()),
}
```

**After:**
```rust
let mut state = server.state.write().await;
match state.apply_unstake_tx(&tx, current_round) {
    Ok(()) => {
        let unlock_at = current_round + ultradag_coin::UNSTAKE_COOLDOWN_ROUNDS;
        drop(state);
        
        // Broadcast unstake transaction to all peers
        let wrapped_tx = Transaction::Unstake(tx.clone());
        let msg = Message::NewTx(wrapped_tx);
        server.peers.broadcast(&msg).await;
        
        json_response(StatusCode::OK, &serde_json::json!({
            "status": "unstaking",
            "address": sender.to_hex(),
            "unlock_at_round": unlock_at,
            "broadcast": "sent to all peers",
        }))
    }
    Err(e) => error_response(StatusCode::BAD_REQUEST, &e.to_string()),
}
```

---

## Test Plan

### Prerequisites
1. Build updated code: `cargo build --release`
2. Deploy to Fly.io testnet (all 4 nodes)
3. Wait for nodes to sync and reach steady state

### Test 1: Basic Stake Propagation

**Objective:** Verify stake transaction broadcasts to all nodes

**Steps:**
1. Get initial stake state from all nodes:
   ```bash
   curl http://node1:8080/stake/<ADDRESS>
   curl http://node2:8080/stake/<ADDRESS>
   curl http://node3:8080/stake/<ADDRESS>
   curl http://node4:8080/stake/<ADDRESS>
   ```
   Expected: All show `staked: 0`, `active: false`

2. Submit stake transaction to node 1:
   ```bash
   curl -X POST http://node1:8080/stake \
     -H "Content-Type: application/json" \
     -d '{"secret_key": "<KEY>", "amount": 1000000000000}'
   ```
   Expected: Response includes `"broadcast": "sent to all peers"`

3. Wait 5-10 seconds for propagation and finalization

4. Check stake state on all nodes again:
   ```bash
   curl http://node1:8080/stake/<ADDRESS>
   curl http://node2:8080/stake/<ADDRESS>
   curl http://node3:8080/stake/<ADDRESS>
   curl http://node4:8080/stake/<ADDRESS>
   ```
   **Expected:** All nodes show:
   - `staked: 1000000000000` (10,000 UDAG)
   - `active: true`
   - Consistent across all 4 nodes

### Test 2: Cross-Node Stake Submission

**Objective:** Verify broadcast works regardless of submission node

**Steps:**
1. Submit stake from different address to node 3:
   ```bash
   curl -X POST http://node3:8080/stake \
     -H "Content-Type: application/json" \
     -d '{"secret_key": "<KEY2>", "amount": 1000000000000}'
   ```

2. Wait 5-10 seconds

3. Verify all nodes see the new stake:
   ```bash
   for i in 1 2 3 4; do
     echo "Node $i:"
     curl http://node$i:8080/stake/<ADDRESS2>
   done
   ```
   **Expected:** Consistent stake amount across all nodes

### Test 3: Unstake Propagation

**Objective:** Verify unstake transactions also broadcast correctly

**Steps:**
1. Submit unstake transaction to node 2:
   ```bash
   curl -X POST http://node2:8080/unstake \
     -H "Content-Type: application/json" \
     -d '{"secret_key": "<KEY>"}'
   ```
   Expected: Response includes `"broadcast": "sent to all peers"`

2. Wait 5-10 seconds

3. Check stake state on all nodes:
   ```bash
   for i in 1 2 3 4; do
     echo "Node $i:"
     curl http://node$i:8080/stake/<ADDRESS>
   done
   ```
   **Expected:** All nodes show:
   - `staked: 0`
   - `active: false`
   - `unlock_at_round: <current_round + 2016>`

### Test 4: Validator Set Consistency

**Objective:** Verify validator count updates consistently

**Steps:**
1. Get validator count from all nodes:
   ```bash
   for i in 1 2 3 4; do
     echo "Node $i:"
     curl http://node$i:8080/status | jq '.validator_count, .active_stakers'
   done
   ```

2. Submit stake from new address to node 4:
   ```bash
   curl -X POST http://node4:8080/stake \
     -H "Content-Type: application/json" \
     -d '{"secret_key": "<KEY3>", "amount": 1000000000000}'
   ```

3. Wait for next epoch boundary (or sufficient rounds)

4. Check validator count again on all nodes:
   ```bash
   for i in 1 2 3 4; do
     echo "Node $i:"
     curl http://node$i:8080/status | jq '.validator_count, .active_stakers, .total_staked'
   done
   ```
   **Expected:** All nodes report same validator count and total staked

### Test 5: Mempool Visibility

**Objective:** Verify stake transactions appear in mempool before finalization

**Steps:**
1. Monitor mempool on node 2:
   ```bash
   watch -n 1 'curl -s http://node2:8080/mempool | jq'
   ```

2. In another terminal, submit stake to node 1:
   ```bash
   curl -X POST http://node1:8080/stake \
     -H "Content-Type: application/json" \
     -d '{"secret_key": "<KEY4>", "amount": 1000000000000}'
   ```

3. Observe mempool on node 2

   **Expected:**
   - Stake transaction appears in node 2's mempool within 1 second
   - Transaction type shows as "Stake"
   - Transaction disappears after finalization (~3-9 seconds)

### Test 6: Balance Deduction Consistency

**Objective:** Verify liquid balance decreases consistently across nodes

**Steps:**
1. Check balance on all nodes before stake:
   ```bash
   for i in 1 2 3 4; do
     echo "Node $i:"
     curl http://node$i:8080/balance/<ADDRESS>
   done
   ```

2. Submit stake transaction:
   ```bash
   curl -X POST http://node1:8080/stake \
     -H "Content-Type: application/json" \
     -d '{"secret_key": "<KEY>", "amount": 1000000000000}'
   ```

3. Wait 10 seconds for finalization

4. Check balance again on all nodes:
   ```bash
   for i in 1 2 3 4; do
     echo "Node $i:"
     curl http://node$i:8080/balance/<ADDRESS>
   done
   ```
   **Expected:**
   - All nodes show balance decreased by exactly 1000000000000 sats
   - Nonce incremented by 1 on all nodes
   - Perfect consistency across network

---

## Success Criteria

✅ **Pass:** All tests show consistent state across all 4 nodes  
✅ **Pass:** Stake/unstake transactions propagate within 1 second  
✅ **Pass:** State updates finalize within 3-9 seconds  
✅ **Pass:** No node shows different staked amounts or active status  
✅ **Pass:** Validator count updates consistently  
✅ **Pass:** Mempool shows transactions before finalization  

❌ **Fail:** Any node shows different stake state  
❌ **Fail:** Transactions don't appear in remote node mempools  
❌ **Fail:** State divergence after finalization  

---

## Rollback Plan

If tests fail:
1. Check logs for broadcast errors: `fly logs -a ultradag-node-X`
2. Verify P2P connectivity: `curl http://nodeX:8080/peers`
3. If broadcast logic is broken, revert commit and investigate
4. If P2P is down, check Fly.io network configuration

---

## Deployment Commands

```bash
# Build release binary
cargo build --release

# Deploy to all nodes (from project root)
fly deploy --config fly-node1.toml
fly deploy --config fly-node2.toml
fly deploy --config fly-node3.toml
fly deploy --config fly-node4.toml

# Wait for all nodes to restart and sync
sleep 30

# Run test suite
./scripts/test_staking_propagation.sh
```

---

## Expected Log Output

After fix, logs should show:

**Node 1 (submitting node):**
```
[INFO] RPC: Stake transaction applied locally
[INFO] P2P: Broadcasting NewTx(Stake) to 3 peers
[INFO] Mempool: Added Stake tx to local mempool
```

**Node 2, 3, 4 (receiving nodes):**
```
[INFO] P2P: Received NewTx(Stake) from peer
[INFO] Mempool: Added Stake tx to mempool
[INFO] State: Applied Stake tx from finalized vertex
```

---

## Notes

- The fix adds `drop(state)` before broadcast to release the write lock
- Response JSON now includes `"broadcast": "sent to all peers"` for confirmation
- This matches the pattern used by `/tx` endpoint for regular transfers
- Both `/stake` and `/unstake` endpoints now broadcast consistently
