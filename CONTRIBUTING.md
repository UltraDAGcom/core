# Contributing to UltraDAG

UltraDAG is a community-first DAG-BFT blockchain. This project is designed to be genuinely accessible to contributors — the codebase is clean, well-tested, and structured for clarity. If you're reading this, you're already part of the community.

## Table of Contents

- [Quick Start](#quick-start)
- [Running a Local 4-Node Testnet](#running-a-local-4-node-testnet)
- [Codebase Architecture](#codebase-architecture)
- [Development Workflow](#development-workflow)
- [Testing](#testing)
- [Code Style](#code-style)
- [Pull Request Process](#pull-request-process)
- [Good First Issues](#good-first-issues)

---

## Quick Start

**Prerequisites:**
- Rust 1.75+ (`rustup update`)
- Git

**Clone and build:**

```bash
git clone https://github.com/ultradag/core.git ultradag
cd ultradag
cargo build --release
```

**Run tests:**

```bash
cargo test --release
```

If all tests pass, you're ready to contribute.

---

## Running a Local 4-Node Testnet

The fastest way to understand UltraDAG is to run a local testnet. This takes under 10 minutes.

### Step 1: Build the node

```bash
cargo build --release --bin ultradag-node
```

### Step 2: Create data directories

```bash
mkdir -p data/node{1,2,3,4}
```

### Step 3: Start 4 nodes in separate terminals

**Terminal 1 (Node 1 - Bootstrap):**
```bash
./target/release/ultradag-node \
  --port 9001 \
  --rpc-port 10001 \
  --data-dir data/node1 \
  --validators 4 \
  --no-bootstrap
```

**Terminal 2 (Node 2):**
```bash
./target/release/ultradag-node \
  --port 9002 \
  --rpc-port 10002 \
  --data-dir data/node2 \
  --validators 4 \
  --seed 127.0.0.1:9001
```

**Terminal 3 (Node 3):**
```bash
./target/release/ultradag-node \
  --port 9003 \
  --rpc-port 10003 \
  --data-dir data/node3 \
  --validators 4 \
  --seed 127.0.0.1:9001
```

**Terminal 4 (Node 4):**
```bash
./target/release/ultradag-node \
  --port 9004 \
  --rpc-port 10004 \
  --data-dir data/node4 \
  --validators 4 \
  --seed 127.0.0.1:9001
```

### Step 4: Verify consensus

Within 10-15 seconds, all nodes should start producing blocks. Check any node's status:

```bash
curl http://localhost:10001/status
```

You should see:
- `last_finalized_round` incrementing every ~2.5 seconds
- `finality_lag` staying at 1-2 rounds
- `active_validators` showing 4 addresses

**What you're seeing:** Four nodes running DAG-BFT consensus with optimistic responsiveness. Blocks finalize in 1-2 rounds (2.5-5 seconds) with no leader election overhead.

### Step 5: Send a transaction

Generate a keypair:
```bash
curl http://localhost:10001/keygen
```

Fund it from the faucet:
```bash
curl -X POST http://localhost:10001/faucet \
  -H "Content-Type: application/json" \
  -d '{"address": "YOUR_ADDRESS_HERE"}'
```

Check balance:
```bash
curl http://localhost:10001/balance/YOUR_ADDRESS_HERE
```

Send a transfer:
```bash
curl -X POST http://localhost:10001/tx \
  -H "Content-Type: application/json" \
  -d '{
    "from": "YOUR_ADDRESS",
    "to": "RECIPIENT_ADDRESS",
    "amount": 100000000,
    "fee": 10000,
    "secret_key": "YOUR_SECRET_KEY"
  }'
```

The transaction will be included in the next finalized vertex (typically within 5 seconds).

---

## Codebase Architecture

UltraDAG is structured as three Rust crates with clear separation of concerns:

### `crates/ultradag-coin/`
**The state machine and consensus logic.**

Key modules:
- `consensus/` — DAG-BFT implementation, finality tracking, epoch management
- `state/` — StateEngine (account balances, staking, governance)
- `tx/` — Transaction types (Transfer, Stake, Unstake, CreateProposal, Vote)
- `governance/` — On-chain governance system
- `block.rs` — Block and DAG vertex structures
- `constants.rs` — Network parameters (supply, epochs, fees, governance)

This crate is pure logic — no networking, no I/O. It's designed to be testable in isolation.

### `crates/ultradag-network/`
**P2P networking and message protocol.**

Key modules:
- `protocol/` — Message encoding/decoding (blocks, transactions, checkpoints)
- `peer/` — Peer connection management, registry, reader/writer split
- `server.rs` — Main network event loop

This crate handles all TCP connections, message serialization, and peer discovery. It's agnostic to the consensus logic.

### `crates/ultradag-node/`
**The binary that ties everything together.**

Key modules:
- `main.rs` — CLI argument parsing, node initialization
- `rpc.rs` — HTTP RPC server (status, transactions, staking, governance)
- `rate_limit.rs` — Connection and endpoint rate limiting

This is the entry point. It wires the state engine to the network layer and exposes the RPC API.

### Data Flow

```
User → RPC (ultradag-node) → Mempool (ultradag-coin)
                            ↓
Network (ultradag-network) → DAG (ultradag-coin) → StateEngine (ultradag-coin)
                            ↓
                    Finalized Vertices → State Transitions
```

1. Transactions arrive via RPC or P2P gossip
2. Mempool validates and stores them
3. Block producer includes them in new vertices
4. DAG-BFT consensus finalizes vertices
5. StateEngine applies finalized transactions in order

---

## Development Workflow

### Branch Strategy

- `main` — stable, always passes tests
- `dev` — active development
- Feature branches: `feature/your-feature-name`

### Making Changes

1. **Fork the repo** (or create a branch if you have write access)
2. **Create a feature branch:**
   ```bash
   git checkout -b feature/improve-rpc-errors
   ```
3. **Make your changes** — keep commits focused and atomic
4. **Run tests:**
   ```bash
   cargo test --release
   ```
5. **Run clippy:**
   ```bash
   cargo clippy --all-targets --all-features
   ```
6. **Format code:**
   ```bash
   cargo fmt
   ```
7. **Commit with a clear message:**
   ```bash
   git commit -m "rpc: improve error messages for invalid addresses"
   ```

---

## Testing

UltraDAG has extensive test coverage. All PRs must pass existing tests and add new tests for new functionality.

### Running Tests

**All tests:**
```bash
cargo test --release
```

**Specific test file:**
```bash
cargo test --release --test staking
```

**Specific test:**
```bash
cargo test --release test_stake_and_unstake_flow
```

**With output:**
```bash
cargo test --release -- --nocapture
```

### Test Organization

- `crates/ultradag-coin/src/*/tests.rs` — Unit tests (in-module)
- `crates/ultradag-coin/tests/*.rs` — Integration tests
  - `staking.rs` — Stake/unstake lifecycle
  - `checkpoint.rs` — Checkpoint creation and validation
  - `adversarial.rs` — Byzantine fault scenarios
  - `recovery.rs` — State persistence and recovery
  - `pruning.rs` — DAG pruning and memory management

### Writing Tests

UltraDAG tests follow a clear pattern:

```rust
#[test]
fn test_descriptive_name() {
    // Setup
    let mut state = StateEngine::new_with_genesis();
    let sk = SecretKey::generate();
    
    // Action
    let result = state.some_operation(&sk.address());
    
    // Assert
    assert!(result.is_ok());
    assert_eq!(state.balance(&sk.address()), expected_balance);
}
```

**Guidelines:**
- Test one thing per test
- Use descriptive names: `test_unstake_fails_during_cooldown`
- Test both success and failure cases
- Use `assert_eq!` with helpful messages
- Avoid `unwrap()` — use `assert!(result.is_ok())`

---

## Code Style

UltraDAG follows standard Rust conventions with a few project-specific guidelines:

### General

- **Format with `cargo fmt`** before committing
- **No warnings** — fix all clippy warnings
- **Prefer explicit over clever** — code is read more than written
- **Comments explain why, not what** — the code shows what

### Naming

- Types: `PascalCase` (e.g., `StateEngine`, `DagVertex`)
- Functions: `snake_case` (e.g., `apply_vertex`, `verify_signature`)
- Constants: `SCREAMING_SNAKE_CASE` (e.g., `MIN_FEE_SATS`, `EPOCH_LENGTH_ROUNDS`)
- Modules: `snake_case` (e.g., `consensus`, `state`)

### Error Handling

- Use `Result<T, CoinError>` for fallible operations
- Define specific error variants in `error.rs`
- Avoid `unwrap()` in production code
- Use `?` operator for error propagation

### Documentation

- Public APIs require doc comments: `/// Returns the account balance in sats.`
- Complex algorithms get a module-level doc comment explaining the approach
- Link to papers or RFCs when implementing known algorithms

### Example

```rust
/// Apply a finalized DAG vertex to the state engine.
/// 
/// This processes all transactions in the vertex, updates account balances,
/// handles stake/unstake operations, and recalculates the active validator
/// set at epoch boundaries.
/// 
/// Returns an error if any transaction is invalid (bad signature, insufficient
/// balance, invalid nonce, etc.). The state is not modified on error.
pub fn apply_vertex(&mut self, vertex: &DagVertex) -> Result<(), CoinError> {
    // Implementation...
}
```

---

## Pull Request Process

### Before Submitting

1. **Tests pass:** `cargo test --release`
2. **No clippy warnings:** `cargo clippy --all-targets`
3. **Code is formatted:** `cargo fmt`
4. **Commit messages are clear** — explain what and why

### PR Template

When you open a PR, include:

**What:** Brief description of the change

**Why:** What problem does this solve? Link to issue if applicable.

**Testing:** How did you test this? New tests added?

**Breaking changes:** Does this change any public APIs?

### Example PR Description

```
## Add governance proposal expiration

**What:** Proposals now automatically transition to `Expired` status 
if voting period ends without reaching quorum.

**Why:** Prevents stale proposals from cluttering the governance UI. 
Closes #42.

**Testing:** Added `test_proposal_expires_without_quorum` integration 
test. All existing tests pass.

**Breaking changes:** None — this is a new state transition.
```

### Review Process

- Maintainers will review within 48 hours
- Address review comments with new commits (don't force-push during review)
- Once approved, a maintainer will merge

---

## Good First Issues

New to the codebase? Start here. These issues are well-scoped, achievable, and don't require deep system knowledge:

### Documentation
- **Improve RPC error messages** — Many endpoints return generic "invalid input". Add specific error messages explaining what's wrong.
- **Add JSDoc comments to dashboard.js** — The frontend functions lack documentation.
- **Document the checkpoint system** — Write a `docs/checkpoints.md` explaining how checkpoints work.

### Features
- **Add `/health` endpoint** — Simple endpoint returning `{"status": "ok"}` for load balancers.
- **Add round time chart to dashboard** — Show average round time over last 100 rounds.
- **Improve faucet rate limiting** — Currently per-IP. Add per-address rate limiting.

### Testing
- **Add governance integration tests** — Test proposal creation, voting, and status transitions.
- **Test DAG pruning edge cases** — What happens when pruning during epoch boundary?
- **Add benchmarks** — Measure transaction throughput, finality latency.

### Refactoring
- **Extract magic numbers to constants** — Some hardcoded values should be named constants.
- **Reduce code duplication in RPC handlers** — Several endpoints have similar validation logic.

**How to claim an issue:**
1. Comment on the issue: "I'd like to work on this"
2. A maintainer will assign it to you
3. Ask questions in the issue thread if you get stuck

---

## Community

- **Discord:** [Join here](#) — Ask questions, discuss proposals, share testnet results
- **GitHub Discussions:** For longer-form technical discussions
- **Testnet:** https://testnet.ultradag.com — Run a validator, test governance

---

## Questions?

If something in this guide is unclear, that's a bug in the documentation. Please:
1. Open an issue with the "documentation" label
2. Ask in Discord #development channel
3. Submit a PR improving this guide

UltraDAG is built by people like you. Welcome to the community.
