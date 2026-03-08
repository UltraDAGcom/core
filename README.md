# UltraDAG

[![Bug Bounty](https://img.shields.io/badge/Bug%20Bounty-500k%20UDAG-success)](./security/bug-bounty/PROGRAM.md)
[![Security Policy](https://img.shields.io/badge/Security-Policy-blue)](./security/POLICY.md)
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

**[View Full Program →](./security/bug-bounty/PROGRAM.md)** | **[Quick Start Guide →](./security/bug-bounty/GUIDE.md)**

All rewards tracked publicly and convertible to mainnet tokens at launch.

## What is UltraDAG

UltraDAG is a DAG-BFT consensus protocol designed for permissioned networks and IoT applications. It uses a directed acyclic graph (DAG) structure instead of a linear blockchain, enabling parallel block production by multiple validators. The protocol achieves Byzantine fault tolerance with deterministic finality—once a vertex is finalized, it cannot be reverted. There is no proof of work.

## How It Works

In UltraDAG, each validator produces one vertex per round. Each vertex references all known tips from the previous round, forming a DAG structure where multiple validators produce blocks concurrently. A vertex achieves finality when 2/3+ of validators have produced at least one descendant of it. This provides immediate, deterministic finality without waiting for confirmations.

**Round duration:** Design target is 30 seconds, but configurable via `--round-ms` flag. Testnet currently runs at 5 seconds for faster testing.

Transaction ordering is deterministic: finalized vertices are sorted by (round, topological depth, hash) before applying to state. This ensures all nodes derive identical account balances from the same set of finalized vertices.

## Tokenomics

- **Max supply**: 21,000,000 UDAG (hard cap enforced in state engine)
- **Halving**: every 210,000 rounds (~2.5 months at 30s design target, ~12 days at 5s testnet)
- **Initial block reward**: 50 UDAG per round (total emission per round, split among validators)
- **Developer allocation**: 1,050,000 UDAG (5%) allocated at genesis
  - Funds protocol development. No VC funding. No presale.
  - Deterministic testnet address (see `constants.rs` for seed)
  - Visible and auditable from round 0
- **Faucet reserve**: 1,000,000 UDAG at genesis (testnet only)
- **Validator rewards**: Proportional to stake when staking is active
  - Pre-staking fallback: each validator receives full block reward
  - Post-staking: total round reward split proportionally by stake
- **Minimum stake**: 10,000 UDAG to become validator
- **Unstaking cooldown**: 2,016 rounds (~1 week)
- **Slashing**: 50% stake burn on equivocation

## Running a Node

```bash
# Run a validator node (RPC on port 10333)
cargo run --release -p ultradag-node -- --port 9333 --validate

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

**POST /unstake** — Begin unstake cooldown (~1 week)
- **Cooldown:** 2,016 rounds before funds are returned
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

**394 tests passing** (all pass, none ignored)

- ultradag-coin: 116 unit + 241 integration tests
- ultradag-network: 25 unit + 12 integration tests

Test coverage includes: consensus safety, Byzantine fault tolerance, cryptographic correctness, double-spend prevention, staking lifecycle, supply invariants, state persistence, crash recovery, checkpoint production, fast-sync, equivocation evidence retention.

## Architecture

**3 crates, strict layering:**

| Crate | Purpose |
|-------|--------|
| `ultradag-coin` | Ed25519 keys, DAG-BFT consensus, StateEngine, staking, account-based state |
| `ultradag-network` | TCP P2P: peer discovery, DAG vertex relay, state synchronization |
| `ultradag-node` | Full node binary: validator + networking + HTTP RPC |

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

MIT OR Apache-2.0
