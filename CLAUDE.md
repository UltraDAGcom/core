# UltraDAG — Technical Specification
### A Pure DAG-BFT Cryptocurrency (Formerly TinyDAG)

**Website**: UltraDAG.com
**Repository**: github.com/ultradag/core

## Architecture

Three crates, strict layering:

| Layer | Crate | Purpose |
|-------|-------|---------|
| 0 — Coin | `ultradag-coin` | Ed25519 keys, DAG-BFT consensus, StateEngine (DAG-driven ledger), staking, account-based state |
| 1 — Network | `ultradag-network` | TCP P2P: peer discovery, DAG vertex relay, state synchronization |
| 2 — Node | `ultradag-node` | Full node binary (round-based validator + networking + HTTP RPC) |

## Workspace Layout

```
crates/
  ultradag-coin/src/       # address/ block/ block_producer/ consensus/ state/ tx/ constants.rs error.rs
  ultradag-network/src/    # protocol/ peer/ node/
  ultradag-node/src/       # main.rs validator.rs rpc.rs bin/loadtest.rs
site/
  index.html              # Landing page
  wallet.html             # Web wallet (connects to node RPC: keygen, balance, send, DAG explorer)
  whitepaper.html         # Whitepaper page
loadtest.sh               # Load testing script
throughput_test.py        # Python throughput tester
WHITEPAPER.md             # Full whitepaper
CONSENSUS_SPEC.md         # Formal consensus specification
```

## Conventions

- **mod.rs only re-exports** — no logic inside mod.rs files
- **One concern per file**, small files (<200 lines)
- **Deeply structured directories** — sub-sub-sub folders over flat layouts
- **Inline unit tests** — `#[cfg(test)] mod tests` in each module

## Key Types

- `DagVertex` — block + parent_hashes + round + validator + pub_key + Ed25519 signature
- `BlockDag` — DAG of DagVertex entries, tracks tips/children/rounds, equivocation detection, round quorum queries
- `FinalityTracker` — BFT finality: vertex finalized when 2/3+ validators have descendants. Uses `ValidatorSet` internally.
- `ValidatorSet` — tracks known validators, computes BFT quorum threshold (ceil(2n/3)), supports `configured_validators` and permissioned allowlist
- `StateEngine` — Derives account state from finalized DAG vertices, manages staking/unstaking/slashing
- `StakeAccount` — tracks staked amount and unstake cooldown per address
- `Block` — header + coinbase + transactions (now only exists inside DagVertex)
- `BlockHeader` — version, height, timestamp, prev_hash, merkle_root (no difficulty, no nonce)
- `Address` — 32-byte Blake3 hash of Ed25519 public key
- `SecretKey` — Ed25519 signing key (32-byte seed); `from_bytes()`, `to_bytes()`, `verifying_key()`
- `Signature` — Ed25519 signature (64 bytes), hex-serialized for JSON
- `Transaction` — from, to, amount, fee, nonce (account nonce for replay protection), pub_key, signature
- `StakeTx` — from, amount, nonce, pub_key, signature — locks UDAG as validator stake
- `UnstakeTx` — from, nonce, pub_key, signature — begins unstake cooldown

## DAG-BFT Consensus (Pure DAG-Driven Ledger)

**MAJOR REDESIGN**: UltraDAG is now a pure DAG-BFT system where **the DAG IS the ledger**. There is no separate blockchain.

### Core Principles:

- **DAG structure**: each vertex references ALL known tips (multiple parents), forming a DAG
- **Optimistic responsiveness**: validators produce a vertex immediately when 2f+1 vertices from the previous round are seen. Round timer (`--round-ms`, default 5000ms) is the fallback.
- **Ed25519-signed vertices**: every DAG vertex is signed by the proposing validator; peers verify signatures before accepting
- **BFT finality**: a vertex is finalized when > 2/3 of known validators have at least one descendant of it (O(1) via incremental descendant tracking)
- **StateEngine**: derives account balances and nonces from ordered finalized vertices (no separate blockchain)
- **2f+1 gate**: before producing a round-r vertex, the validator checks that at least ceil(2n/3) distinct validators produced vertices in round r-1. If not, it skips the round.
- **Equivocation prevention**: the DAG rejects a second vertex from the same validator in the same round
- **ValidatorSet**: tracks known validators and computes quorum threshold (ceil(2n/3))
- **Permissioned validator allowlist**: `--validator-key FILE` loads trusted validator addresses; only listed validators count toward quorum/finality
- **Configured validators**: `--validators N` CLI arg fixes quorum denominator to prevent phantom validator inflation
- **Deterministic ordering**: finalized vertices are ordered by (round, topological depth, hash) for state application
- **Parallel vertices**: multiple validators produce vertices concurrently in the same round
- **Min validators**: finality requires at least 3 active validators (configurable via `FinalityTracker::new(min)`)
- **No PoW**: round timer replaces proof-of-work as the rate limiter; `tokio::interval` for clean async timing

### Consensus module layout (`ultradag-coin/src/consensus/`):
- `vertex.rs` — `DagVertex`: block + parent_hashes + round + validator + pub_key + signature; `verify_signature()`, `signable_bytes()`
- `dag.rs` — `BlockDag`: DAG data structure with vertices, tips, children, rounds, ancestor/descendant queries, equivocation detection, incremental `descendant_validators` tracking (updated on insert via BFS with early termination)
- `finality.rs` — `FinalityTracker`: BFT finality (2/3+ threshold), O(1) `check_finality` via precomputed counts, `find_newly_finalized` with forward propagation through children. Uses `ValidatorSet` internally.
- `epoch.rs` — `sync_epoch_validators()`: synchronizes FinalityTracker with StateEngine's active validator set at epoch boundaries
- `validator_set.rs` — `ValidatorSet`: tracks validator addresses, computes `quorum_threshold()` = ceil(2n/3), `has_quorum(count)` check, `configured_validators` field, permissioned allowlist with `set_allowed_validators()`
- `ordering.rs` — `order_vertices()`: deterministic total ordering of finalized vertices

### State module layout (`ultradag-coin/src/state/`):
- `engine.rs` — `StateEngine`: derives account state from finalized DAG vertices
  - Tracks balances, nonces, total supply, stake accounts
  - Applies finalized vertices atomically with supply invariant check
  - Validates transactions against current state
  - Stake-proportional block rewards when staking is active; equal-split fallback pre-staking
  - Staking: `apply_stake_tx()`, `apply_unstake_tx()`, `process_unstake_completions()`, `slash()`
  - Supply invariant: `sum(liquid balances) + sum(staked) == total_supply`

### Single consensus path (DAG-BFT only):
1. **DAG vertex production**: Validator produces vertex every round -> references all DAG tips -> signs with Ed25519
2. **DAG vertex propagation**: `DagProposal` -> verify signature -> equivocation check -> DAG insert -> finality check
3. **State derivation**: Finalized vertices -> ordered by (round, depth, hash) -> applied to StateEngine -> account balances updated

### P2P DAG messages:
- `DagProposal(DagVertex)` — broadcast new signed DAG vertex to peers (signature + equivocation verified on receipt)
- `GetDagVertices { from_round, max_count }` — request vertices by round
- `DagVertices(Vec<DagVertex>)` — response with DAG vertices
- `EquivocationEvidence` — broadcast evidence of Byzantine equivocation
- `GetParents { hashes: Vec<[u8; 32]> }` — request specific vertices by hash (for resolving missing parents)
- `ParentVertices { vertices: Vec<DagVertex> }` — response with requested parent vertices

### Recursive Parent Fetch (DAG Sync Convergence):
When a vertex fails insertion due to missing parents, the node:
1. Buffers the vertex in the orphan buffer (capped at 1000 entries / 50MB)
2. Sends `GetParents` with the missing parent hashes (capped at 32 per request)
3. Peer responds with `ParentVertices` containing the requested vertices
4. Node inserts received parents (with signature verification), recursively requests still-missing grandparents
5. After any successful insert, `resolve_orphans()` attempts to flush buffered orphans
6. Stall-recovery: if finality lags >10 rounds behind DAG round, validator broadcasts `GetDagVertices` to trigger re-sync
- `DagInsertError::MissingParents(Vec<[u8; 32]>)` — returned by `try_insert()` when parent hashes are not in the DAG

## Tokenomics

### Supply
- Max supply: 21,000,000 UDAG (1 UDAG = 100,000,000 sats)
- Initial block reward: 50 UDAG per round (total emission, split among validators)
- Halving: every 210,000 rounds
- Default round time: 5 seconds (configurable via `--round-ms`)

### Genesis Allocations
- **Faucet reserve**: 1,000,000 UDAG (testnet only) — `SecretKey::from_bytes([0xFA; 32])`
- **Developer allocation**: 1,050,000 UDAG (5% of max supply) — `SecretKey::from_bytes([0xDE; 32])`
- Both credited in `StateEngine::new_with_genesis()`

### Emission Model (Stake-Proportional)
- **With staking active**: each validator's reward = `block_reward(height) × (own_stake / total_stake)`
- **Pre-staking fallback**: each vertex gets full `block_reward(height)` (backward compatible)
- `create_block()` takes `validator_reward` parameter; validator computes its share before block production
- Remainder from integer division is implicitly burned (sum of rewards <= block_reward)
- Supply cap enforced: reward capped at `MAX_SUPPLY_SATS - total_supply`

### Staking & Validator Cap
- **Minimum stake**: `MIN_STAKE_SATS` = 10,000 UDAG (updated from 1,000)
- **StakeTx**: locks UDAG from liquid balance into stake account
- **UnstakeTx**: begins cooldown period (`UNSTAKE_COOLDOWN_ROUNDS` = 2,016 rounds ≈ 1 week)
- **Max active validators**: `MAX_ACTIVE_VALIDATORS` = 21 (odd number for clean BFT quorum: ceil(2×21/3) = 14)
- **Epoch-based validator set**: recalculated every `EPOCH_LENGTH_ROUNDS` = 210,000 rounds (~1 year at 5s rounds)
  - `epoch_of(round)` = round / 210,000
  - `is_epoch_boundary(round)` = round % 210,000 == 0
  - Top 21 stakers by amount become active validators; set frozen between epoch boundaries
  - `recalculate_active_set()` sorts by (stake desc, address asc) for determinism, then truncates to 21
- **Observer rewards**: staked but not in active set earn 20% of normal reward (`OBSERVER_REWARD_PERCENT` = 20)
  - Observer reward = `block_reward(h) × (own_stake / total_stake) × 20 / 100`
- **Slashing**: 50% stake burn on equivocation (slashed amount removed from total_supply)
  - **Slash policy**: slash immediately removes from active validator set if stake drops below `MIN_STAKE_SATS`. Security trumps epoch stability — Byzantine actors should not continue earning rewards.
- **Stale epoch recovery**: on `StateEngine::load()`, if persisted `current_epoch` doesn't match `epoch_of(last_finalized_round)`, active set is recalculated
- Ed25519 signatures on all staking transactions with NETWORK_ID prefix

### Cryptography
- Signatures: Ed25519 (ed25519-dalek). Address = blake3(ed25519_pubkey). Transactions carry pub_key for verification.
- DAG vertices: Ed25519-signed by the proposing validator. Peers reject vertices with invalid signatures or equivocation.

## P2P Protocol

TCP with 4-byte length-prefixed JSON messages (max 4MB):
`Hello`, `HelloAck`, `NewTx`, `DagProposal`, `GetDagVertices`, `DagVertices`, `GetParents`, `ParentVertices`, `GetPeers`, `Peers`, `Ping`, `Pong`, `EquivocationEvidence`

Split read/write connections — PeerReader for recv loop, PeerWriter (Arc<Mutex>) for broadcast.

**DAG sync**: on connect, nodes exchange `Hello` with current DAG round. If a peer is ahead, request `GetDagVertices`.
**DAG vertex handling**: `DagProposal` verifies Ed25519 signature, rejects equivocation (duplicate validator+round), inserts into DAG (short lock scope), registers validator, checks finality, applies finalized vertices to StateEngine, rebroadcasts.
**Transaction propagation**: `NewTx` broadcasts transactions to mempool across all peers.

## HTTP RPC (ultradag-node)

Default port: P2P port + 1000 (e.g., P2P 9333 -> RPC 10333).

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/status` | GET | Last finalized round, peers, mempool, supply, accounts, DAG vertices/round/tips, finalized/validator counts, total_staked, active_stakers |
| `/balance/:address` | GET | Balance (sats + UDAG), nonce for an address |
| `/round/:round` | GET | All vertices in a round: hash, validator, reward, tx count, parent count |
| `/tx` | POST | Submit transaction: `{from_secret, to, amount, fee}`. Validates balance and nonce. |
| `/mempool` | GET | List pending transactions (top 100 by fee) |
| `/keygen` | GET | Generate new keypair (secret_key + address) |
| `/faucet` | POST | Testnet faucet: `{address, amount}`. Creates real signed tx from deterministic faucet keypair. |
| `/peers` | GET | Connected peers, bootstrap node status |
| `/stake` | POST | Stake UDAG: `{secret_key, amount}`. Locks funds as validator stake. |
| `/unstake` | POST | Begin unstake: `{secret_key}`. Starts cooldown period. |
| `/stake/:address` | GET | Stake info: staked amount, unlock_at_round, is_active_validator |
| `/validators` | GET | List of active validators with stake amounts |

All responses are JSON with CORS headers for browser wallet access.

## Commands

```bash
# Run validator node
cargo run --release -p ultradag-node -- --port 9333 --validate
cargo run --release -p ultradag-node -- --port 9334 --seed 127.0.0.1:9333 --validate

# Custom round duration (default 5000ms)
cargo run --release -p ultradag-node -- --port 9333 --validate --round-ms 3000

# Fixed validator count (prevents phantom inflation)
cargo run --release -p ultradag-node -- --port 9333 --validate --validators 4

# Permissioned validator set
cargo run --release -p ultradag-node -- --port 9333 --validate --validator-key testnet-validators.txt

# 4-node local testnet
./scripts/testnet-local.sh

# RPC examples
curl http://127.0.0.1:10333/status
curl http://127.0.0.1:10333/balance/<address>
curl http://127.0.0.1:10333/keygen
curl http://127.0.0.1:10333/validators
curl http://127.0.0.1:10333/stake/<address>
curl -X POST http://127.0.0.1:10333/tx -H "Content-Type: application/json" \
  -d '{"from_secret":"...","to":"...","amount":1000000000,"fee":100000}'
curl -X POST http://127.0.0.1:10333/stake -H "Content-Type: application/json" \
  -d '{"secret_key":"...","amount":100000000000}'

# Tests
cargo test --workspace
```

## Tests

**370 tests passing** (all pass, none ignored):

Run `cargo test --workspace --release` to verify:
```
test result: ok. 370 passed; 0 failed; 0 ignored
```

### Test Breakdown by Crate:
- **ultradag-coin**: 116 unit tests + 218 integration tests
- **ultradag-network**: 21 unit tests + 12 integration tests

### Integration Test Files (ultradag-coin/tests/):
- `adversarial.rs` — 32 tests: consensus safety, Byzantine validators, tx edge cases, multi-validator scenarios, optimistic responsiveness, epoch transitions, descendant tracking, finality regression
- `staking.rs` — 27 tests: stake/unstake lifecycle, proportional rewards, slashing, supply invariants, epoch boundaries, validator cap, observer rewards, slash policy, stale epoch recovery
- `edge_cases.rs` — 22 tests: coinbase validation, supply exhaustion, orphan handling, faucet depletion, dev allocation
- `bft_rules.rs` — 12 tests: proving all 5 BFT consensus rules
- `crypto_correctness.rs` — 14 tests: Ed25519 signatures, address derivation, replay protection
- `double_spend_prevention.rs` — 12 tests: nonce enforcement, balance validation
- `dag_bft_finality.rs` — 8 tests: finality threshold, equivocation, deterministic ordering
- `dag_structure.rs` — 13 tests: DAG topology, tips tracking, causal history, incremental descendant tracking
- `dag_sync.rs` — 6 tests: recursive parent fetch, orphan resolution, DAG convergence after partition
- `epoch_transition.rs` — 5 tests: epoch boundary recalculation, active set sync, validator cap
- `fault_tolerance.rs` — 5 tests: Byzantine fault tolerance, network resilience
- `finality.rs` — 8 tests: finality horizon, quorum thresholds, ancestor propagation
- `multi_validator_progression.rs` — 3 tests: multi-validator consensus progression
- `ordering.rs` — 7 tests: deterministic vertex ordering
- `parent_finality_guarantee.rs` — 2 tests: parent-before-child finality
- `parent_finality_simple.rs` — 1 test: basic parent finality
- `performance.rs` — 2 tests: finality performance at 1K (< 50ms) and 10K vertices (< 500ms)
- `phantom_validator.rs` — 2 tests: phantom validator handling
- `recovery.rs` — 2 tests: crash recovery, coinbase reward sum verification
- `state_correctness.rs` — 3 tests: state determinism
- `state_persistence.rs` — 5 tests: state save/load
- `vertex.rs` — 7 tests: vertex structure, signatures
- `equivocation_gossip.rs` — 2 tests: equivocation evidence propagation

## Key Design Decisions

### Pure DAG-Driven Ledger
- **DAG IS the ledger**: No separate blockchain. StateEngine derives all account state from finalized DAG vertices.
- **Unconditional vertex production**: Validators produce one vertex per round unconditionally (no chain tip competition).
- **StateEngine**: Replaces Blockchain and ChainState. Applies finalized vertices atomically, tracks balances/nonces/stakes, validates transactions.

### DAG-BFT Consensus
- **Optimistic responsiveness**: validators produce immediately when 2f+1 vertices from previous round are available via `tokio::select!` on `round_notify`. Timer is fallback.
- **2f+1 gate**: validators skip a round if they haven't seen quorum (ceil(2n/3)) distinct validator vertices from the previous round.
- **Incremental descendant tracking**: `descendant_validators: HashMap<[u8;32], HashSet<Address>>` updated on each DAG insert via BFS upward with early termination. Finality checks are O(1).
- **Forward propagation finality**: `find_newly_finalized` seeds from candidate vertices, then propagates through children. Single-pass, no full DAG re-scan.
- **Equivocation prevention**: both the local validator and the P2P handler reject duplicate vertices from the same validator in the same round.
- **ValidatorSet**: proper struct with membership tracking and quorum threshold computation. Supports `configured_validators`, permissioned allowlist.
- **Ed25519-signed vertices**: every DAG vertex carries the validator's public key and Ed25519 signature. Peers verify before accepting.
- **Deterministic ordering**: finalized vertices ordered by (round, hash) before state application.
- **Parent finality guarantee**: vertices only finalized after all parents finalized.
- **Equivocation evidence gossip**: Byzantine validators detected and evidence broadcast network-wide.

### Permissioned Validator Set
- **Problem**: Validator count drifts when external nodes connect and register as validators.
- **Solution**: `--validator-key FILE` loads allowlist of trusted validator addresses.
- **Behavior**: Only listed validators count toward quorum/finality. Others can connect, sync, submit transactions (observers).
- **Purge on set**: `set_allowed_validators()` removes already-registered non-allowed validators.
- **Ordering**: Allowlist loaded BEFORE DAG validator rebuild on startup.

### Staking Economics
- **Stake-proportional rewards**: When staking is active, each validator's reward = `block_reward × (own_stake / total_stake)`
- **Pre-staking fallback**: Before any stake exists, each vertex gets full `block_reward(height)` (backward compatible)
- **Validator cap**: Top 21 stakers by amount are active validators; rest are observers earning 20% of normal reward
- **Dynamic epoch transitions**: Validator set recalculated every 210,000 rounds; `sync_epoch_validators()` updates FinalityTracker's allowlist and configured count at each boundary. Old set finalizes boundary vertex before new set takes over.
- **Minimum stake**: 10,000 UDAG to become eligible for active validator set
- **Unstake cooldown**: 2,016 rounds (~1 week) prevents stake-and-run attacks
- **Slashing**: 50% stake burn on equivocation, reduces total_supply (deflationary). Immediately removes from active set if stake drops below minimum.
- **Supply invariant**: `sum(liquid balances) + sum(staked amounts) == total_supply` checked in debug builds
- **Stale epoch recovery**: State loading detects epoch mismatch and recalculates active set

### Security Protections
- **NETWORK_ID prefix**: All signable bytes include `b"ultradag-testnet-v1"` for replay prevention.
- **Phantom parent rejection**: Parent existence check before DAG insertion.
- **Future round limit**: Reject vertices >10 rounds ahead (MAX_FUTURE_ROUNDS=10).
- **Timestamp validation**: Reject vertices with timestamps >5 minutes in future.
- **Coinbase validation**: Verify coinbase amount = validator_reward + total_fees.
- **Supply invariant**: Debug assertion that sum(liquid + staked) == total_supply.
- **Deterministic finality**: BTreeSet instead of HashSet for iteration order.
- **Message size limit**: 4MB maximum before deserialization.
- **Mempool limit**: 10,000 transactions with fee-based eviction.

### State Persistence
- JSON serialization for BlockDag, FinalityTracker, StateEngine (including stake_accounts, active_validator_set, current_epoch), Mempool.
- Save/load/exists methods for all components.
- Nodes survive restarts without data loss.
- `#[serde(default)]` on stake_accounts, active_validator_set, current_epoch for backward compatibility.
- Stale epoch detection on load: recalculates active set if persisted epoch doesn't match actual round.

## Faucet System

The faucet creates real signed transactions that propagate through DAG consensus (not local-only state mutations).

- **Deterministic keypair**: `SecretKey::from_bytes([0xFA; 32])` — same on every node
- **Genesis pre-fund**: 1,000,000 UDAG via `StateEngine::new_with_genesis()`
- **Endpoint**: `POST /faucet` with `{address, amount}` — creates signed tx, inserts in mempool, broadcasts via NewTx
- **Constants**: `FAUCET_SEED`, `FAUCET_PREFUND_SATS`, `faucet_keypair()` in `constants.rs`

## Developer Allocation

- **5% of max supply**: 1,050,000 UDAG allocated at genesis
- **Deterministic keypair**: `SecretKey::from_bytes([0xDE; 32])` — auditable from block 0
- **Constants**: `DEV_ALLOCATION_SATS`, `DEV_ADDRESS_SEED`, `dev_address()` in `constants.rs`
- Credited in `StateEngine::new_with_genesis()` alongside faucet pre-fund

## Public Bootstrap Nodes

Hardcoded in `crates/ultradag-network/src/bootstrap.rs`:
```
206.51.242.223:9333  — ultradag-node-1 (ams, dedicated IPv4)
137.66.57.226:9333   — ultradag-node-2 (ams, dedicated IPv4)
169.155.54.169:9333  — ultradag-node-3 (ams, dedicated IPv4)
169.155.55.151:9333  — ultradag-node-4 (ams, dedicated IPv4)
```

New nodes auto-connect when no `--seed` is provided. Use `--no-bootstrap` for local/private networks.
Exponential backoff retry (2, 4, 8, 16, 32 seconds) for bootstrap connections.

## Fly.io Testnet Infrastructure

- **4 nodes** in ams region on Fly.io
- **Dedicated IPv4** for each node ($2/mo) — required for raw TCP (shared IPv4 only works for HTTP)
- **TCP service**: port 9333 exposed via `[[services]]` block in fly.toml
- **RPC**: HTTPS via `https://ultradag-node-{1,2,3,4}.fly.dev/`
- **Env vars**: RUST_LOG, PORT, RPC_PORT, DATA_DIR, VALIDATORS, SEED, NO_BOOTSTRAP, CLEAN_STATE
- **CLEAN_STATE=true**: Removes persisted state on startup (one-time use for fresh resets)
- **Docker entrypoint**: `scripts/docker-entrypoint.sh` handles all env vars
- **Permissioned validators**: `testnet-validators.txt` copied to `/etc/ultradag/validators.txt` via Dockerfile

### Deployment Commands
```bash
# Deploy (uses FLY_API_TOKEN env var)
(export FLY_API_TOKEN=...; fly deploy -a ultradag-node-N --remote-only)

# SSH into node
(export FLY_API_TOKEN=...; fly ssh console -a ultradag-node-N -C "command")

# Fresh reset: set CLEAN_STATE=true in fly.toml, deploy, then remove it
```

## Testnet Verified

4-node Fly.io testnet confirmed stable through 1800+ rounds with permissioned validator set:

| Metric | Value |
|--------|-------|
| Last finalized round | 1863 |
| Validator count | 4 (stable with permissioned set) |
| DAG vertices | 1955 |
| Finality lag | 2 rounds |
| Peers | 13 |
| Supply | 1,095,050 UDAG |

## Performance Roadmap

### ✅ Finality Algorithm Optimization (P2 — COMPLETED)
**Before:** Descendant traversal recomputed from scratch each call (O(V²) complexity).
- 1,000 vertices: 421ms
- 10,000 vertices: 47 seconds

**After:** Incremental descendant validator tracking with O(1) lookups.
- 1,000 vertices: **1ms** (421x faster)
- 10,000 vertices: **21ms** (2,238x faster)

**Implementation:**
- Added `descendant_validators: HashMap<[u8; 32], HashSet<Address>>` to track which validators have descendants of each vertex
- Updated incrementally during `insert()` via BFS through ancestors
- Rebuilt during `load()` for persistence compatibility
- `descendant_validator_count(hash)` is now O(1) HashMap lookup
- `find_newly_finalized()` uses single-pass iteration instead of per-tip ancestor traversal

**Impact:** Production-ready finality performance. No protocol change required.

### ✅ DAG Pruning (P1 — COMPLETED)
**Before:** DAG grows unbounded. All vertices kept in memory forever.

**After:** Automatic pruning of vertices older than `PRUNING_HORIZON` (1000 rounds = ~7 days at 10s rounds).

**Implementation:**
- Added `pruning_floor: u64` to track earliest round still in memory
- `prune_old_rounds(last_finalized_round)` removes vertices from rounds < (last_finalized_round - 1000)
- Integrated into `FinalityTracker` - automatically tracks `last_finalized_round`
- Persistence: `pruning_floor` saved/loaded in DAG snapshots
- Safe: Only prunes deeply finalized vertices (1000 rounds behind finality frontier)

**Memory savings:** 80-90% reduction after steady state (keeps only last 1000 rounds + unfinalized tips)

**Sync protocol:** New nodes sync from pruned state via snapshots + recent suffix
- `pruning_floor()` indicates earliest available round
- Nodes joining after pruning fetch from checkpoint + recent DAG
- Full history available from archive nodes (optional deployment)

**Current status:** Basic pruning implemented and tested. All 340 tests passing.

**Next steps for production hardening:**
1. **Checkpoint broadcasting** - Broadcast pruning checkpoints to peers for verification
2. **State root proofs** - Add Merkle proofs to checkpoints for light client verification  
3. **Equivocation evidence retention** - Keep Byzantine evidence in separate persistent store
4. **Tunable pruning depth** - Make `PRUNING_HORIZON` configurable (currently hardcoded to 1000)
5. **Archive node mode** - Optional flag to disable pruning for full history nodes
6. **Sync optimization** - Efficient range queries for checkpoint + suffix sync

**Design principles (conservative & safe):**
- Never prune unfinalized vertices or their causal ancestors
- Only prune vertices deeply behind finality frontier (1000 rounds buffer)
- Deterministic: all nodes agree on what gets pruned based on finalized depth
- Auditable: pruning_floor tracked in persistent state
- Preserves safety: no risk of state divergence or re-orgs

**Trade-offs:**
- Memory: Huge savings (80-90% reduction in steady state)
- Sync: New nodes fetch from checkpoint (faster than full history)
- Cost: O(V) scan during prune (amortized, runs infrequently)
- Light clients: Can verify from checkpoint onward (with state proofs)

**Status:** Production-ready for testnet. Checkpoint broadcasting and state proofs recommended before mainnet.

### Known Performance Limitations (Non-Critical)

#### Vertex Ordering O(V²) Complexity
**Location:** `crates/ultradag-coin/src/consensus/ordering.rs`

**Current behavior:**
- `order_vertices()` sorts finalized vertices by (round, topological_depth, hash)
- `count_ancestors_in_set()` calls `dag.ancestors(hash)` for every vertex during comparison
- When finalizing N vertices: O(N²) worst case (N vertices × N ancestor traversals)
- Example: 500 finalized vertices = potentially 500 full DAG traversals

**Performance impact:**
- Small/medium DAGs (<5-10K vertices): acceptable
- Low finalization rate (typical 2-3 rounds): minimal impact
- Contributes to overall finality overhead but not the primary bottleneck

**Future optimization (P3 - non-urgent):**
1. **Memoization:** Cache ancestor counts during sort (simple HashMap)
2. **Pre-computation:** Assign topological levels during finality collection
3. **Incremental tracking:** Similar to descendant validator tracking

**Why deferred:**
- Not a bottleneck for IoT-scale workloads (target use case)
- Finality check optimization (P2) was the critical path - now complete
- DAG pruning (P1) is higher priority for mainnet readiness
- Easy to optimize later without protocol changes

**Estimated effort:** 1-2 days when needed.

#### Equivocation Check O(vertices_in_round)
**Location:** `crates/ultradag-coin/src/consensus/dag.rs:try_insert()`

**Current behavior:**
- Scans all vertices in the same round to detect equivocation
- With many validators and dense rounds: O(validators_per_round) per insertion

**Future optimization (P3):**
- Add secondary index: `HashMap<(Address, Round), Hash>`
- Makes equivocation check O(1)
- Costs ~32 bytes per vertex in memory

**Status:** Acceptable for current validator counts (4-21). Can optimize if needed.

## Mainnet Launch Checklist

**CRITICAL — Must complete before mainnet:**

### Security
- [ ] **Replace DEV_ADDRESS_SEED** — Generate offline keypair, store in hardware wallet, NEVER commit private key
- [ ] **Remove faucet** — Delete faucet_keypair(), faucet_credit(), and /faucet endpoint entirely
- [ ] **Security audit** — External audit of consensus, state, and cryptographic implementations
- [ ] **Penetration testing** — Network-level attacks, eclipse attacks, DDoS resilience
- [ ] **Formal verification** — Machine-checkable safety proof (or document why deferred)

### Protocol
- [ ] **Change NETWORK_ID** — Update from `ultradag-testnet-v1` to `ultradag-mainnet-v1`
- [ ] **Verify genesis parameters** — Confirm MAX_SUPPLY_SATS, INITIAL_REWARD_SATS, HALVING_INTERVAL
- [ ] **Verify staking parameters** — Confirm MIN_STAKE_SATS, UNSTAKE_COOLDOWN_ROUNDS, MAX_ACTIVE_VALIDATORS
- [ ] **DAG pruning** — Implement before launch to prevent unbounded growth (P1 requirement)
- [ ] **Snapshot mechanism** — Allow new nodes to sync from recent state without full history

### Testing
- [ ] **Extended testnet run** — Minimum 1 month continuous operation with 21 validators
- [ ] **Chaos testing** — Network partitions, validator crashes, Byzantine behavior simulation
- [ ] **Load testing** — Sustained high transaction volume, mempool saturation
- [ ] **Upgrade testing** — Binary upgrade without consensus failure
- [ ] **All tests passing** — 337 automated tests, 0 failures, 0 ignored (except documented performance tests)

### Documentation
- [ ] **Remove testnet warnings** — Update all references from testnet to mainnet
- [ ] **Mainnet deployment guide** — Production-grade setup, monitoring, backup procedures
- [ ] **Validator handbook** — Staking guide, slashing conditions, reward calculations
- [ ] **API stability guarantees** — Version RPC endpoints, document breaking changes policy
- [ ] **Incident response plan** — Emergency contacts, rollback procedures, communication channels

### Infrastructure
- [ ] **Bootstrap nodes** — Deploy and harden 3+ geographically distributed bootstrap nodes
- [ ] **Block explorer** — Public dashboard for mainnet transparency
- [ ] **Monitoring** — Prometheus/Grafana for validator health, finality lag, network metrics
- [ ] **Backup strategy** — Automated state snapshots, disaster recovery plan

### Legal & Compliance
- [ ] **Legal review** — Regulatory compliance for target jurisdictions
- [ ] **Terms of service** — Clear disclaimers, no investment advice
- [ ] **Privacy policy** — GDPR/CCPA compliance if applicable
- [ ] **Trademark** — Protect UltraDAG name and logo

### Launch Coordination
- [ ] **Genesis ceremony** — Transparent, auditable genesis block creation
- [ ] **Validator onboarding** — Pre-launch validator registration and testing
- [ ] **Communication plan** — Announce launch date, migration from testnet
- [ ] **Emergency pause mechanism** — Circuit breaker for critical bugs (remove after stability proven)

**DO NOT LAUNCH MAINNET until ALL items are complete and verified.**
