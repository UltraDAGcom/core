# UltraDAG

[![Bug Bounty](https://img.shields.io/badge/Bug%20Bounty-500k%20UDAG-success)](./docs/security/bug-bounty/PROGRAM.md)
[![Security Policy](https://img.shields.io/badge/Security-Policy-blue)](./docs/security/POLICY.md)
[![Testnet](https://img.shields.io/badge/Testnet-Live-green)](https://ultradag-node-1.fly.dev/status)
[![License](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-informational)](./LICENSE)

A DAG-BFT cryptocurrency for permissioned networks and IoT applications. Built in Rust.

**Ed25519 signatures. DAG-BFT consensus. Blake3 hashing. 21M max supply. Validator staking.**

## 🔒 Security & Bug Bounty

**Active Bug Bounty Program:** We're offering up to **50,000 UDAG** for critical vulnerabilities!

- 🔴 **Critical:** 10,000 - 50,000 UDAG
- 🟠 **High:** 5,000 - 10,000 UDAG  
- 🟡 **Medium:** 1,000 - 5,000 UDAG
- 🟢 **Low:** 100 - 1,000 UDAG

**[View Full Program →](./docs/security/bug-bounty/PROGRAM.md)** | **[Quick Start Guide →](./docs/security/bug-bounty/GUIDE.md)**

All rewards tracked publicly and convertible to mainnet tokens at launch.

## What is UltraDAG

UltraDAG is a DAG-BFT consensus protocol designed for permissioned networks and IoT applications. It uses a directed acyclic graph (DAG) structure instead of a linear blockchain, enabling parallel block production by multiple validators. The protocol achieves Byzantine fault tolerance with deterministic finality—once a vertex is finalized, it cannot be reverted. There is no proof of work.

## How It Works

In UltraDAG, each validator produces one vertex per round. Each vertex references all known tips from the previous round, forming a DAG structure where multiple validators produce blocks concurrently. A vertex achieves finality when 2/3+ of validators have produced at least one descendant of it. This provides immediate, deterministic finality without waiting for confirmations.

**Round duration:** Design target is 30 seconds, but configurable via `--round-ms` flag. Testnet currently runs at 5 seconds for faster testing.

Transaction ordering is deterministic: finalized vertices are sorted by (round, topological depth, hash) before applying to state. This ensures all nodes derive identical account balances from the same set of finalized vertices.

## Tokenomics

- **Max supply**: 21,000,000 UDAG (hard cap enforced in state engine)
- **Emission-only**: Zero genesis pre-mine. All tokens distributed through per-round emission.
- **Halving**: every 10,500,000 rounds (~1.66 years at 5s rounds)
- **Initial block reward**: 1 UDAG per round total, split by protocol:
  - 75% validators/stakers (proportional to effective stake)
  - 10% DAO treasury (council-controlled via TreasurySpend proposals)
  - 10% Council of 21 (equal split among seated members)
  - 5% founder (earned through emission, starts at 0)
- **Faucet reserve**: 1,000,000 UDAG at genesis (testnet only, excluded from mainnet)
- **Validator rewards**: Proportional to stake when staking is active
  - Pre-staking fallback: 1 UDAG/round split equally among configured validators
  - Post-staking: validator pool split proportionally by effective stake + delegations
- **Minimum stake**: 10,000 UDAG to become validator
- **Unstaking cooldown**: 2,016 rounds (~2.8 hours at 5s rounds)
- **Slashing**: 50% stake burn on equivocation (governable 10-100%)

## Running a Node

### Docker (Recommended)

**Easiest way to run on Linux, macOS, or Windows:**

```bash
# Single node
docker run -p 9333:9333 -p 10333:10333 \
  ghcr.io/ultradagcom/core:latest --port 9333 --validate

# 4-node local network
docker-compose up -d
```

**[Full Docker Guide →](./docs/getting-started/docker-guide.md)**

### Running a Validator

The fastest way to get a validator running on testnet:

```bash
# Step 1: Generate a keypair
curl https://ultradag-node-1.fly.dev/keygen

# Step 2: Get testnet UDAG (100,000 UDAG for staking)
curl -X POST https://ultradag-node-1.fly.dev/faucet \
  -H "Content-Type: application/json" \
  -d '{"address":"<your-address>","amount":10000000000000}'

# Step 3: Run your validator node (auto-stakes 10,000 UDAG on startup)
cargo run --release -p ultradag-node -- \
  --port 9333 --validate \
  --pkey <your-secret-key> \
  --auto-stake 10000
```

The `--pkey` flag lets you bring your own Ed25519 private key (64-char hex) instead of auto-generating one. The `--auto-stake` flag automatically submits a stake transaction after the node syncs with the network.

### From Source

```bash
# Run a validator node (RPC on port 10333)
cargo run --release -p ultradag-node -- --port 9333 --validate

# Bring your own key
cargo run --release -p ultradag-node -- --port 9333 --validate --pkey <hex-secret-key>

# Connect a second validator
cargo run --release -p ultradag-node -- --port 9334 --seed 127.0.0.1:9333 --validate

# Custom round duration (default 5000ms)
cargo run --release -p ultradag-node -- --port 9335 --seed 127.0.0.1:9333 --validate --round-ms 3000

# Fixed validator count (prevents phantom inflation)
cargo run --release -p ultradag-node -- --port 9333 --validate --validators 4
```

## API Reference

HTTP RPC runs on P2P port + 1000 (e.g., P2P 9333 → RPC 10333).

### Core Endpoints

**GET /status** — Node status
```json
{
  "last_finalized_round": 1863,
  "dag_round": 1865,
  "total_supply": 109505000000000,
  "total_staked": 4000000000000,
  "active_stakers": 4,
  "peers": 13,
  "mempool_size": 0
}
```

**GET /balance/:address** — Account balance and nonce
```json
{
  "balance_sats": 5000000000,
  "balance_udag": 50.0,
  "nonce": 0
}
```

**POST /tx** — Submit transaction
```json
{
  "from_secret": "<64-char-hex>",
  "to": "<64-char-hex>",
  "amount": 1000000000,
  "fee": 100000
}
```

**GET /keygen** — Generate new keypair
```json
{
  "secret_key": "<64-char-hex>",
  "address": "<64-char-hex>"
}
```

**POST /faucet** — Testnet faucet (testnet only)
```json
{
  "address": "<64-char-hex>",
  "amount": 1000000000
}
```

### Staking Endpoints

**POST /stake** — Lock UDAG as validator stake
- **Minimum stake:** 10,000 UDAG (1,000,000,000,000 sats)
- **Request:**
```json
{
  "secret_key": "<64-char-hex>",
  "amount": 1000000000000
}
```
- **Response:** Transaction hash and confirmation
- **Note:** Stake becomes active at the next epoch boundary (every 210,000 rounds)

**POST /unstake** — Begin unstake cooldown (~2.8 hours at 5s rounds)
- **Cooldown:** 2,016 rounds before funds are returned (~2.8 hours at 5s testnet, ~16.8 hours at 30s design target)
- **Request:**
```json
{
  "secret_key": "<64-char-hex>"
}
```
- **Response:** Transaction hash and unlock round
- **Note:** Validator is removed from active set immediately; funds return after cooldown

**GET /stake/:address** — Query stake status for an address
- **Response:**
```json
{
  "staked": 1000000000000,
  "unlock_at_round": null,
  "is_active_validator": true
}
```
- **Fields:**
  - `staked`: Current staked amount in sats
  - `unlock_at_round`: Round when unstaked funds will be available (null if not unstaking)
  - `is_active_validator`: Whether address is in the active validator set

**GET /validators** — List all active validators and their stakes
- **Response:**
```json
[
  {
    "address": "a1b2c3...",
    "stake": 1000000000000
  }
]
```
- **Note:** Validators are sorted by stake amount (descending). Max 21 active validators.

### DAG Endpoints

**GET /round/:round** — Vertices in a specific round
**GET /mempool** — Pending transactions (top 100 by fee)
**GET /peers** — Connected peers

## Test Suite

```bash
cargo test --workspace --release
```

**757 tests passing** (all pass, 14 ignored jepsen long-running tests)

- ultradag-coin: 146 unit + 399 integration tests
- ultradag-network: 25 unit + 12 integration tests + 49 fault injection tests
- ultradag-sdk: 2 doc tests

Test coverage includes: consensus safety, Byzantine fault tolerance, cryptographic correctness, double-spend prevention, staking lifecycle, supply invariants, state persistence, crash recovery, checkpoint production, fast-sync, equivocation evidence retention.

## Architecture

**3 crates, strict layering:**

| Crate | Purpose |
|-------|--------|
| `ultradag-coin` | Ed25519 keys, DAG-BFT consensus, StateEngine, staking, account-based state |
| `ultradag-network` | TCP P2P: peer discovery, DAG vertex relay, state synchronization |
| `ultradag-node` | Full node binary: validator + networking + HTTP RPC |

## Formal Verification

The consensus protocol has a [TLA+ formal specification](./formal/UltraDAGConsensus.tla) verified by the TLC model checker. Six invariants were checked exhaustively across 32.6 million states with zero violations:

- **Safety** — No conflicting finalized vertices from the same validator in the same round
- **HonestNoEquivocation** — Honest validators never equivocate
- **FinalizedParentsConsistency** — All parents of finalized vertices are also finalized
- **TypeOK**, **RoundMonotonicity**, **ByzantineBound** — Structural invariants

Verified at N=4 validators, 1 Byzantine, 2 rounds. See [formal/VERIFICATION.md](./formal/VERIFICATION.md) for full results and limitations.

## Known Limitations

- **Pre-staking emission**: Total emission scales with validator count until first validator stakes. After staking activates, emission is fixed per round.
- **No per-peer rate limiting**: Acceptable for current permissioned validator set.
- **Round synchronization**: Nodes produce at independent round numbers, preventing in-round quorum. Finality happens via descendant accumulation across rounds.
- **Vertex ordering**: O(V²) for the ordering step (not finality). Acceptable for <=21 validators.

## Cryptography

- **Signing**: Ed25519 via `ed25519-dalek`
- **Addresses**: `blake3(ed25519_public_key)` — 32 bytes
- **Transactions**: carry sender's public key for on-chain verification
- **DAG vertices**: Ed25519-signed by the proposing validator
- **Block hashing**: Blake3
- **Replay protection**: NETWORK_ID prefix on all signable bytes

## License

Business Source License 1.1 (BISL) - see [LICENSE](./LICENSE) file.

**Summary:**
- Free to use for operating validators, building wallets/explorers, and developing applications on UltraDAG
- Cannot be used to launch competing blockchain networks
- Converts to MIT License on March 10, 2030
