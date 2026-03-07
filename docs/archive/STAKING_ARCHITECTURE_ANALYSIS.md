# Staking Architecture Analysis — Root Cause & Solution

**Date:** March 7, 2026  
**Issue:** Stake/unstake transactions not propagating across network  
**Status:** Architectural limitation identified — requires design decision

---

## Problem Statement

When a stake transaction is submitted via the `/stake` RPC endpoint on one node, other nodes in the network never see the stake. This causes inconsistent validator state:

- **Node 1** (submission node): `staked=10,000 UDAG`, `active=True`
- **Nodes 2-4** (other nodes): `staked=0`, `active=False`

---

## Root Cause Analysis

### Current Architecture

The UltraDAG codebase has **three separate transaction types**:

1. **`Transaction`** (transfers) — in `tx/transaction.rs`
   - Included in `Block.transactions: Vec<Transaction>`
   - Broadcast via `Message::NewTx(Transaction)`
   - Stored in mempool before inclusion in DAG vertices
   - **Propagates correctly** ✅

2. **`StakeTx`** — in `tx/stake.rs`
   - **NOT** included in `Block` structure
   - **NO** P2P broadcast mechanism
   - Applied directly to `StateEngine` via RPC
   - **Does not propagate** ❌

3. **`UnstakeTx`** — in `tx/stake.rs`
   - **NOT** included in `Block` structure
   - **NO** P2P broadcast mechanism
   - Applied directly to `StateEngine` via RPC
   - **Does not propagate** ❌

### Why Simple Broadcast Doesn't Work

Initial attempt to fix by broadcasting stake transactions failed because:

```rust
// This doesn't compile:
let msg = Message::NewTx(Transaction::Stake(tx));  // ❌ Transaction is a struct, not an enum
server.peers.broadcast(&msg).await;  // ❌ broadcast() requires 2 args: (msg, exclude)
```

**Fundamental issues:**
1. `Transaction` is a **struct** for transfers only, not an enum that can hold different transaction types
2. `Message::NewTx` only accepts `Transaction` (transfers), not `StakeTx` or `UnstakeTx`
3. `Block.transactions` only holds `Vec<Transaction>`, not stake/unstake transactions
4. No mempool for stake/unstake transactions
5. No mechanism to include stake/unstake in DAG vertices

---

## Architectural Options

### Option 1: Unified Transaction Enum (Recommended)

**Change `Transaction` from struct to enum:**

```rust
// In tx/transaction.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Transaction {
    Transfer(TransferTx),
    Stake(StakeTx),
    Unstake(UnstakeTx),
}

// Rename current Transaction to TransferTx
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferTx {
    pub from: Address,
    pub to: Address,
    pub amount: u64,
    pub fee: u64,
    pub nonce: u64,
    pub pub_key: [u8; 32],
    pub signature: Signature,
}
```

**Propagation flow:**
1. Submit stake via `/stake` RPC
2. Apply locally to state
3. Wrap in `Transaction::Stake(tx)`
4. Broadcast via `Message::NewTx(Transaction::Stake(...))`
5. Other nodes receive, add to mempool
6. Validators include in next DAG vertex
7. Finalized → all nodes apply to state

**Pros:**
- ✅ Clean, unified transaction model
- ✅ Stake/unstake propagate like regular transactions
- ✅ Mempool handles all transaction types
- ✅ Consistent with blockchain best practices

**Cons:**
- ⚠️ Large refactor (affects `Block`, `StateEngine`, mempool, RPC handlers)
- ⚠️ Breaking change to serialization format
- ⚠️ Requires migration for existing nodes

**Estimated effort:** 4-6 hours

---

### Option 2: Separate Stake Messages

**Add new P2P message types:**

```rust
// In protocol/message.rs
pub enum Message {
    // ... existing variants ...
    NewStakeTx(StakeTx),
    NewUnstakeTx(UnstakeTx),
}
```

**Propagation flow:**
1. Submit stake via `/stake` RPC
2. Apply locally to state
3. Broadcast via `Message::NewStakeTx(tx)`
4. Other nodes receive, apply to state immediately
5. No mempool, no vertex inclusion needed

**Pros:**
- ✅ Smaller code change
- ✅ No refactor of existing Transaction/Block structure
- ✅ Faster to implement

**Cons:**
- ❌ Stake transactions bypass consensus (applied immediately, not finalized)
- ❌ No ordering guarantees vs regular transactions
- ❌ Potential for state divergence if messages arrive out of order
- ❌ Not included in DAG vertices → no historical record
- ❌ Checkpoints won't include stake state transitions

**Estimated effort:** 2-3 hours

**Risk:** Medium-High (state consistency issues)

---

### Option 3: Validator-Only Staking (Current Behavior)

**Keep stake/unstake as local-only operations:**

- Validators must be configured identically on all nodes (via `--validator-key` allowlist)
- Stake operations performed manually on each node
- No dynamic validator set changes via transactions

**Pros:**
- ✅ No code changes needed
- ✅ Simple, predictable validator set
- ✅ Works for permissioned networks

**Cons:**
- ❌ No dynamic staking
- ❌ Manual coordination required
- ❌ Not suitable for public networks
- ❌ Validator changes require redeployment

**Estimated effort:** 0 hours (document current behavior)

---

### Option 4: Stake via Regular Transfers (Workaround)

**Use special transfer transactions to designated staking address:**

```rust
// Stake: send to STAKE_ADDRESS with special amount encoding
let stake_tx = Transaction {
    from: user_addr,
    to: STAKE_ADDRESS,  // 0x5374616b65... ("Stake" in hex)
    amount: stake_amount,
    fee: 0,
    // ...
};

// StateEngine recognizes transfers to STAKE_ADDRESS as stake operations
```

**Pros:**
- ✅ Works with existing infrastructure
- ✅ Propagates via normal transaction path
- ✅ Included in DAG vertices
- ✅ No protocol changes

**Cons:**
- ❌ Hacky, non-obvious design
- ❌ Requires special-case logic in StateEngine
- ❌ Unstaking more complex (how to encode unlock time?)
- ❌ Less type-safe than dedicated transaction types

**Estimated effort:** 3-4 hours

---

## Recommendation

**Implement Option 1: Unified Transaction Enum**

### Rationale

1. **Correctness:** Stake transactions should go through consensus like all state changes
2. **Consistency:** All transactions treated uniformly by mempool, DAG, and finality
3. **Auditability:** Stake operations recorded in DAG vertices for historical analysis
4. **Checkpointing:** Stake state transitions included in checkpoint snapshots
5. **Future-proof:** Extensible to other transaction types (governance, contracts, etc.)

### Implementation Plan

#### Phase 1: Type Refactor (2 hours)
1. Create `TransferTx` struct (rename current `Transaction`)
2. Convert `Transaction` to enum with variants: `Transfer`, `Stake`, `Unstake`
3. Update `Block.transactions` to `Vec<Transaction>` (now enum)
4. Add impl methods: `hash()`, `verify_signature()`, `nonce()`, `from()` on enum

#### Phase 2: Mempool & State (1.5 hours)
5. Update mempool to handle all transaction variants
6. Update `StateEngine::apply_finalized_vertex()` to match on transaction type
7. Remove direct `apply_stake_tx()` / `apply_unstake_tx()` calls from RPC

#### Phase 3: RPC & Broadcast (1 hour)
8. Update `/stake` endpoint to create `Transaction::Stake`, add to mempool, broadcast
9. Update `/unstake` endpoint to create `Transaction::Unstake`, add to mempool, broadcast
10. Update `/tx` endpoint to handle enum variants

#### Phase 4: Testing (1.5 hours)
11. Unit tests for enum variants
12. Integration test: stake on node 1 → verify on all nodes
13. Test nonce ordering: transfer → stake → transfer sequence
14. Test equivocation: duplicate stake transactions

---

## Migration Strategy

### For Testnet (Current Deployment)

**Option A: Clean slate**
- Redeploy with `CLEAN_STATE=true`
- No migration needed (no production data)

**Option B: Manual sync**
- Document current stake state on each node
- Redeploy with new code
- Resubmit stake transactions via `/tx` endpoint (they'll propagate correctly)

### For Future Mainnet

- Checkpoint before upgrade
- Migrate stake state to new format
- Replay stake transactions as `Transaction::Stake` variants

---

## Immediate Action (Testnet)

### Short-term Workaround

Until Option 1 is implemented, use the `/tx` endpoint to manually broadcast stake transactions:

```bash
# Instead of:
curl -X POST http://node1:8080/stake -d '{"secret_key": "...", "amount": 1000000000000}'

# Use (requires adding stake tx to regular tx path - not currently supported):
# This won't work with current code, but documents the intended flow
```

**Current limitation:** Even this workaround doesn't work because `Transaction` struct can't hold stake data.

### Recommended Path Forward

1. **Document current behavior** in testnet README
2. **Implement Option 1** (unified enum) — 6 hours of focused work
3. **Redeploy testnet** with `CLEAN_STATE=true`
4. **Re-run full test suite** from STAKING_FIX_TEST_PLAN.md
5. **Verify propagation** across all 4 nodes

---

## Code Locations

**Files requiring changes for Option 1:**

```
crates/ultradag-coin/src/tx/
  ├── transaction.rs        # Convert to enum, add TransferTx struct
  └── stake.rs              # Already has StakeTx, UnstakeTx

crates/ultradag-coin/src/block/
  └── block.rs              # Block.transactions already Vec<Transaction>

crates/ultradag-coin/src/state/
  └── engine.rs             # Update apply_finalized_vertex() to match enum

crates/ultradag-node/src/
  ├── rpc.rs                # Update /stake, /unstake, /tx endpoints
  ├── mempool.rs            # Handle enum variants (if separate file)
  └── validator.rs          # Vertex production includes all tx types

crates/ultradag-network/src/protocol/
  └── message.rs            # Message::NewTx already uses Transaction
```

---

## Testing Checklist (Post-Implementation)

- [ ] Stake transaction propagates to all nodes
- [ ] Unstake transaction propagates to all nodes
- [ ] Nonce ordering preserved: transfer → stake → transfer
- [ ] Duplicate stake rejected (same nonce)
- [ ] Stake appears in mempool before finalization
- [ ] Stake included in DAG vertex
- [ ] Finalized stake updates state on all nodes
- [ ] Validator count consistent across nodes
- [ ] Total staked amount consistent across nodes
- [ ] Checkpoint includes stake state transitions

---

## Conclusion

The staking broadcast issue is **not a simple bug** but an **architectural limitation** of the current design. Stake and unstake transactions are fundamentally different types that don't fit into the existing `Transaction` struct or `Block` structure.

**The correct solution is Option 1:** Refactor `Transaction` into an enum that can hold all transaction types. This ensures stake operations go through consensus, propagate correctly, and maintain consistency across the network.

**Estimated total effort:** 6 hours of focused development + testing

**Risk level:** Low (well-understood refactor, comprehensive test coverage possible)

**Recommendation:** Implement Option 1 before moving to longer testnet runs or external testing.
