# UltraDAG — Technical Specification
### A Pure DAG-BFT Cryptocurrency (Formerly TinyDAG)

**Website**: UltraDAG.com
**Repository**: github.com/ultradag/ultradag

## Architecture

Three crates, strict layering:

| Layer | Crate | Purpose |
|-------|-------|---------|
| 0 — Coin | `ultradag-coin` | Ed25519 keys, DAG-BFT consensus, StateEngine (DAG-driven ledger), account-based state |
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
- `ValidatorSet` — tracks known validators, computes BFT quorum threshold (ceil(2n/3)), supports `configured_validators` to fix quorum denominator
- `StateEngine` — Derives account state from finalized DAG vertices (replaces Blockchain)
- `Block` — header + coinbase + transactions (now only exists inside DagVertex)
- `BlockHeader` — version, height, timestamp, prev_hash, merkle_root (no difficulty, no nonce)
- `Address` — 32-byte Blake3 hash of Ed25519 public key
- `SecretKey` — Ed25519 signing key (32-byte seed); `from_bytes()`, `to_bytes()`, `verifying_key()`
- `Signature` — Ed25519 signature (64 bytes), hex-serialized for JSON
- `Transaction` — from, to, amount, fee, nonce (account nonce for replay protection), pub_key, signature

## DAG-BFT Consensus (Pure DAG-Driven Ledger)

**MAJOR REDESIGN**: UltraDAG is now a pure DAG-BFT system where **the DAG IS the ledger**. There is no separate blockchain.

### Core Principles:

- **DAG structure**: each vertex references ALL known tips (multiple parents), forming a DAG
- **Round-based timing**: validators produce one vertex per round **unconditionally** (configurable via `--round-ms`, default 5000ms)
- **Ed25519-signed vertices**: every DAG vertex is signed by the proposing validator; peers verify signatures before accepting
- **BFT finality**: a vertex is finalized when > 2/3 of known validators have at least one descendant of it
- **StateEngine**: derives account balances and nonces from ordered finalized vertices (no separate blockchain)
- **2f+1 gate**: before producing a round-r vertex, the validator checks that at least ceil(2n/3) distinct validators produced vertices in round r-1. If not, it skips the round.
- **Equivocation prevention**: the DAG rejects a second vertex from the same validator in the same round
- **ValidatorSet**: tracks known validators and computes quorum threshold (ceil(2n/3))
- **Configured validators**: `--validators N` CLI arg fixes quorum denominator to prevent phantom validator inflation
- **Deterministic ordering**: finalized vertices are ordered by (round, topological depth, hash) for state application
- **Parallel vertices**: multiple validators produce vertices concurrently in the same round
- **Min validators**: finality requires at least 3 active validators (configurable via `FinalityTracker::new(min)`)
- **No PoW**: round timer replaces proof-of-work as the rate limiter; `tokio::interval` for clean async timing

### Consensus module layout (`ultradag-coin/src/consensus/`):
- `vertex.rs` — `DagVertex`: block + parent_hashes + round + validator + pub_key + signature; `verify_signature()`, `signable_bytes()`
- `dag.rs` — `BlockDag`: DAG data structure with vertices, tips, children, rounds, ancestor/descendant queries, equivocation detection (`has_vertex_from_validator_in_round`), round quorum queries (`distinct_validators_in_round`)
- `finality.rs` — `FinalityTracker`: BFT finality (2/3+ threshold), `find_newly_finalized` batch. Uses `ValidatorSet` internally.
- `validator_set.rs` — `ValidatorSet`: tracks validator addresses, computes `quorum_threshold()` = ceil(2n/3), `has_quorum(count)` check, `configured_validators` field
- `ordering.rs` — `order_vertices()`: deterministic total ordering of finalized vertices

### State module layout (`ultradag-coin/src/state/`):
- `engine.rs` — `StateEngine`: derives account state from finalized DAG vertices (replaces chain/blockchain.rs and chain/state.rs)
  - Tracks balances, nonces, total supply
  - Applies finalized vertices atomically
  - Validates transactions against current state
  - Computes block rewards based on finalized rounds

### Single consensus path (DAG-BFT only):
1. **DAG vertex production**: Validator produces vertex unconditionally every round -> references all DAG tips -> signs with Ed25519
2. **DAG vertex propagation**: `DagProposal` -> verify signature -> equivocation check -> DAG insert -> finality check
3. **State derivation**: Finalized vertices -> ordered by (round, depth, hash) -> applied to StateEngine -> account balances updated

### P2P DAG messages:
- `DagProposal(DagVertex)` — broadcast new signed DAG vertex to peers (signature + equivocation verified on receipt)
- `GetDagVertices { from_round, max_count }` — request vertices by round
- `DagVertices(Vec<DagVertex>)` — response with DAG vertices
- `EquivocationEvidence` — broadcast evidence of Byzantine equivocation

## Coin Tokenomics (Bitcoin model)

- Max supply: 21,000,000 UDAG (1 UDAG = 100,000,000 sats)
- Initial reward: 50 UDAG per block
- Halving: every 210,000 blocks
- Default round time: 5 seconds (configurable via `--round-ms`)
- Ledger: account-based (not UTXO)
- Signatures: Ed25519 (ed25519-dalek). Address = blake3(ed25519_pubkey). Transactions carry pub_key for verification.
- DAG vertices: Ed25519-signed by the proposing validator. Peers reject vertices with invalid signatures or equivocation.

## P2P Protocol

TCP with 4-byte length-prefixed JSON messages (max 4MB):
`Hello`, `HelloAck`, `NewTx`, `DagProposal`, `GetDagVertices`, `DagVertices`, `GetPeers`, `Peers`, `Ping`, `Pong`, `EquivocationEvidence`

Split read/write connections — PeerReader for recv loop, PeerWriter (Arc<Mutex>) for broadcast.

**DAG sync**: on connect, nodes exchange `Hello` with current DAG round. If a peer is ahead, request `GetDagVertices`.
**DAG vertex handling**: `DagProposal` verifies Ed25519 signature, rejects equivocation (duplicate validator+round), inserts into DAG (short lock scope), registers validator, checks finality, applies finalized vertices to StateEngine, rebroadcasts.
**Transaction propagation**: `NewTx` broadcasts transactions to mempool across all peers.

## HTTP RPC (ultradag-node)

Default port: P2P port + 1000 (e.g., P2P 9333 -> RPC 10333).

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/status` | GET | Last finalized round, peers, mempool, supply, accounts, DAG vertices/round/tips, finalized/validator counts |
| `/balance/:address` | GET | Balance (sats + UDAG), nonce for an address |
| `/round/:round` | GET | All vertices in a round: hash, validator, reward, tx count, parent count |
| `/tx` | POST | Submit transaction: `{from_secret, to, amount, fee}`. Validates balance and nonce. |
| `/mempool` | GET | List pending transactions (top 100 by fee) |
| `/keygen` | GET | Generate new keypair (secret_key + address) |
| `/faucet/:address` | POST | Testnet faucet: sends 100 UDAG from first validator |

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

# 4-node local testnet
./scripts/testnet-local.sh

# RPC examples
curl http://127.0.0.1:10333/status
curl http://127.0.0.1:10333/balance/<address>
curl http://127.0.0.1:10333/keygen
curl -X POST http://127.0.0.1:10333/tx -H "Content-Type: application/json" \
  -d '{"from_secret":"...","to":"...","amount":1000000000,"fee":100000}'

# Tests
cargo test --workspace
```

## Tests

238 tests across workspace:
- `ultradag-coin`: 109 unit tests + 107 integration tests
- `ultradag-network`: 22 tests
- Integration test files:
  - `bft_rules.rs` — 12 tests proving all 5 BFT rules
  - `multi_validator_progression.rs` — 3 tests proving consensus progression
  - `fault_tolerance.rs` — 5 tests proving Byzantine fault tolerance
  - `state_correctness.rs` — 3 tests proving state determinism
  - `crypto_correctness.rs` — 14 tests proving cryptographic correctness
  - `double_spend_prevention.rs` — 12 tests proving double-spend prevention
  - `equivocation_gossip.rs` — 2 tests for Byzantine evidence broadcast
  - `phantom_validator.rs` — 2 tests for configured validator quorum fix
  - `state_persistence.rs` — 2 tests for state save/load
  - Plus: address, vertex, finality, ordering, dag_structure, dag_bft_finality, parent_finality tests

## Key Design Decisions

### Pure DAG-Driven Ledger
- **DAG IS the ledger**: No separate blockchain. StateEngine derives all account state from finalized DAG vertices.
- **Unconditional vertex production**: Validators produce one vertex per round unconditionally (no chain tip competition).
- **StateEngine**: Replaces Blockchain and ChainState. Applies finalized vertices atomically, tracks balances/nonces, validates transactions.

### DAG-BFT Consensus
- **2f+1 gate**: validators skip a round if they haven't seen quorum (ceil(2n/3)) distinct validator vertices from the previous round.
- **Equivocation prevention**: both the local validator and the P2P handler reject duplicate vertices from the same validator in the same round.
- **ValidatorSet**: proper struct with membership tracking and quorum threshold computation. Supports `configured_validators` to fix quorum denominator.
- **Ed25519-signed vertices**: every DAG vertex carries the validator's public key and Ed25519 signature. Peers verify before accepting.
- **Deterministic ordering**: finalized vertices ordered by (round, topological depth, hash) before state application.
- **Parent finality guarantee**: vertices only finalized after all parents finalized.
- **Equivocation evidence gossip**: Byzantine validators detected and evidence broadcast network-wide.

### Phantom Validator Fix
- **Problem**: Stale addresses from persistence or sync inflate quorum beyond what active validators can satisfy, causing finality stalls.
- **Solution**: `--validators N` CLI arg sets `configured_validators` on ValidatorSet. Quorum uses fixed N instead of dynamic registered count.
- **5 register_validator() call sites**: DagProposal handler, DagVertices sync, resolve_orphans, validator loop, startup DAG rebuild.

### Security Protections
- **NETWORK_ID prefix**: All signable bytes include `b"ultradag-testnet-v1"` for replay prevention.
- **Phantom parent rejection**: Parent existence check before DAG insertion.
- **Future round limit**: Reject vertices >10 rounds ahead (MAX_FUTURE_ROUNDS=10).
- **Deterministic finality**: BTreeSet instead of HashSet for iteration order.
- **Message size limit**: 4MB maximum before deserialization.
- **Mempool limit**: 10,000 transactions with fee-based eviction.

### State Persistence
- JSON serialization for BlockDag, FinalityTracker, StateEngine, Mempool.
- Save/load/exists methods for all components.
- Nodes survive restarts without data loss.

## Testnet Verified

4-node local testnet confirmed stable through 200+ rounds:

| Round | Validators | Finalized | Lag | Supply |
|-------|-----------|-----------|-----|--------|
| 50 | 4 | 48 | 2 | 2400 UDAG |
| 100 | 4 | 98 | 2 | 4900 UDAG |
| 150 | 4 | 147 | 3 | 7350 UDAG |
| 200 | 4 | 198 | 2 | 9900 UDAG |
