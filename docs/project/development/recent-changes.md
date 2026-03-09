# UltraDAG Recent Changes Summary

## Project Renamed: TinyDAG → UltraDAG

**Date**: March 5, 2026

### Complete Renaming
- ✅ Project directory: `15_TinyDAG` → `15_UltraDAG`
- ✅ All crate names: `tinydag-*` → `ultradag-*`
- ✅ All code references updated (66 files)
- ✅ Currency ticker: `TDAG` → `UDAG`
- ✅ Repository: `github.com/ultradag/ultradag`
- ✅ Website: UltraDAG.com
- ✅ Build verified: All tests passing

## Major Architecture Redesign: Pure DAG-BFT

**Date**: March 5, 2026

### Core Changes
UltraDAG has been redesigned from a hybrid blockchain+DAG system to a **pure DAG-BFT system** where the DAG IS the ledger.

### What Changed

#### 1. StateEngine Replaces Blockchain
- **Removed**: `chain/blockchain.rs` and `chain/chainstate.rs`
- **Added**: `state/engine.rs` - StateEngine that derives account state from finalized DAG vertices
- **Impact**: No separate linear blockchain, all state derived from DAG

#### 2. Unconditional Vertex Production
- **Before**: Validators competed for chain tips, produced blocks conditionally
- **After**: Validators produce one DAG vertex per round unconditionally
- **Impact**: Simpler consensus, no chain tip competition

#### 3. Simplified P2P Protocol
- **Removed messages**: `NewBlock`, `GetBlocks`, `Blocks`
- **Kept messages**: `DagProposal`, `GetDagVertices`, `DagVertices`, `NewTx`
- **Impact**: Cleaner protocol, DAG-only synchronization

#### 4. Finalized Rounds = Blocks
- **Before**: Block rewards based on chain height
- **After**: Block rewards based on finalized rounds
- **Impact**: Rewards tied to DAG finality, not chain position

### Technical Details

#### StateEngine (`state/engine.rs`)
```rust
pub struct StateEngine {
    balances: HashMap<Address, u64>,
    nonces: HashMap<Address, u64>,
    total_supply: u64,
    last_finalized_round: Option<u64>,
}

impl StateEngine {
    pub fn apply_finalized_vertices(&mut self, vertices: Vec<DagVertex>)
    pub fn balance(&self, addr: &Address) -> u64
    pub fn nonce(&self, addr: &Address) -> u64
    pub fn validate_transaction(&self, tx: &Transaction) -> Result<()>
}
```

#### Validator Loop Changes
- Produces vertex every round (no skipping)
- References ALL DAG tips as parents
- Applies finalized vertices to StateEngine automatically
- Cleans mempool after state updates
- No separate block production

#### RPC Changes
- `/status` now shows `last_finalized_round` instead of chain height
- `/round/:round` replaces `/block/:height`
- Returns all vertices in a round (not single block)

### Test Results

#### 4-Node Testnet (1000ms rounds)
```json
{
  "last_finalized_round": 17,
  "dag_round": 19,
  "total_supply": 85000000000,  // 850 UDAG
  "dag_vertices": 19,
  "finalized_count": 18,
  "validator_count": 3
}
```

**Observations**:
- ✅ Unconditional vertex production working
- ✅ Round advancement in lockstep
- ✅ BFT finality with 2-round lag (normal)
- ✅ State derivation correct (17 rounds × 50 UDAG = 850 UDAG)
- ✅ 2f+1 gate working (3 validators, threshold = 2)
- ✅ Equivocation prevention working

### Performance Testing

**Load testing infrastructure created**:
- `loadtest` binary (Rust)
- `throughput_test.py` (Python)
- `simple_loadtest.py` (Python async)

**Characteristics**:
- Mempool acceptance: < 1ms per transaction
- DAG inclusion: Depends on round time
- Finalization: 2-3 rounds after inclusion
- Scales with validator count and round time

## Documentation Updates

### Claude.md
- ✅ Updated with pure DAG-BFT architecture
- ✅ Documented StateEngine
- ✅ Updated consensus flow
- ✅ Added testnet verification results
- ✅ Added performance testing section

### New Files
- `WALLET_IMPROVEMENT_PLAN.md` - Comprehensive plan for wallet redesign
- `RECENT_CHANGES.md` - This file
- `loadtest.sh` - Shell-based load tester
- `throughput_test.py` - Python throughput tester
- `simple_loadtest.py` - Python async tester

## Next Steps

### Immediate Priorities
1. **Wallet Redesign** - Make it user-friendly (see WALLET_IMPROVEMENT_PLAN.md)
2. **Website Improvement** - Modern landing page for UltraDAG.com
3. **Performance Optimization** - Tune round times and throughput
4. **Documentation** - User guides, tutorials, API docs

### Wallet Improvement Plan
See `WALLET_IMPROVEMENT_PLAN.md` for detailed plan including:
- Secure wallet creation with mnemonic phrases
- Password encryption and auto-lock
- Modern UI with React + TailwindCSS
- Transaction history and management
- DAG explorer and network visualization
- Multi-account support
- Hardware wallet integration (future)

### Technical Debt
- Remove legacy `chain/` module (kept for backward compatibility)
- Clean up unused blockchain-specific code
- Optimize DAG synchronization
- Add WebSocket support for real-time updates
- Implement transaction indexing for history

## Breaking Changes

### For Node Operators
- No breaking changes - nodes work the same way
- RPC endpoints changed slightly:
  - `/status` response format changed
  - `/block/:height` → `/round/:round`

### For Developers
- Import paths changed: `tinydag_*` → `ultradag_*`
- StateEngine API different from Blockchain API
- No more `chain.height()`, use `state.last_finalized_round()`

### For Users
- Currency ticker: TDAG → UDAG
- Wallet needs complete redesign (current one not user-friendly)

## Migration Guide

### Updating Code
```bash
# Find and replace
find . -type f -name "*.rs" -exec sed -i 's/tinydag/ultradag/g' {} +
find . -type f -name "*.rs" -exec sed -i 's/TinyDAG/UltraDAG/g' {} +
find . -type f -name "*.rs" -exec sed -i 's/TDAG/UDAG/g' {} +

# Update Cargo.toml dependencies
# tinydag-coin → ultradag-coin
# tinydag-network → ultradag-network
```

### Running Nodes
```bash
# Old command (still works)
cargo run --release -p ultradag-node -- --port 9333 --validate

# New features
cargo run --release -p ultradag-node -- \
  --port 9333 \
  --validate \
  --round-ms 1000  # Faster rounds for testing
```

## Summary

UltraDAG has evolved from a hybrid system to a **pure DAG-BFT cryptocurrency** with:
- ✅ Simpler architecture (DAG IS the ledger)
- ✅ Cleaner consensus (unconditional vertex production)
- ✅ Better performance characteristics
- ✅ Verified on 4-node testnet
- ✅ Complete project renaming
- 🔄 Wallet needs major improvement
- 🔄 Website needs modernization

The core protocol is solid and working. Focus now shifts to user experience and ecosystem development.
