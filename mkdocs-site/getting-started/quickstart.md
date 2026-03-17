---
title: Quick Start
---

# Quick Start

Get an UltraDAG node running in under 5 minutes. This guide covers building from source, running a single node, spinning up a local testnet, and making your first RPC calls.

---

## Prerequisites

You need a working Rust toolchain (1.75+):

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup update stable
```

Verify your installation:

```bash
rustc --version   # 1.75.0 or newer
cargo --version
```

!!! tip "No Rust? Use Docker"
    If you prefer not to install Rust, see the [Docker Guide](docker.md) for container-based deployment.

---

## Build from Source

Clone the repository and build the node binary:

```bash
git clone https://github.com/UltraDAGcom/core.git ultradag
cd ultradag
cargo build --release -p ultradag-node
```

The binary is at `target/release/ultradag-node` (< 2 MB). You can copy it anywhere:

```bash
cp target/release/ultradag-node /usr/local/bin/
```

---

## Run a Single Node

Start a standalone validator node:

```bash
cargo run --release -p ultradag-node -- --port 9333 --validate
```

This will:

1. Generate a fresh Ed25519 keypair
2. Start the P2P listener on port `9333`
3. Start the RPC server on port `10333` (P2P port + 1000)
4. Begin producing DAG vertices as a solo validator

!!! note "Solo mode"
    A single validator will finalize immediately since it constitutes 100% of the validator set. This is useful for local development but not representative of production finality timing.

You should see output like:

```
[INFO] UltraDAG node starting on port 9333
[INFO] RPC server listening on 0.0.0.0:10333
[INFO] Validator mode enabled
[INFO] Generated keypair: address=a1b2c3...
[INFO] Round 1: produced vertex abc123...
[INFO] Round 1: finalized 1 vertices
```

---

## Run a Local Testnet

For a more realistic setup, run a 4-node local testnet:

```bash
#!/bin/bash
# run-local-testnet.sh

# Start 4 validator nodes
for i in 1 2 3 4; do
  PORT=$((9332 + i))
  cargo run --release -p ultradag-node -- \
    --port $PORT \
    --validate \
    --validators 4 \
    --seed $i \
    --testnet &
done

echo "Testnet running. RPC ports: 10333, 10334, 10335, 10336"
echo "Press Ctrl+C to stop all nodes"
wait
```

Make it executable and run:

```bash
chmod +x run-local-testnet.sh
./run-local-testnet.sh
```

!!! info "Deterministic seeds"
    The `--seed` flag generates deterministic keypairs for testing. Seed 1-4 will always produce the same addresses, making test scripts reproducible.

---

## Connect to the Public Testnet

To join the live 5-node testnet on Fly.io:

```bash
cargo run --release -p ultradag-node -- \
  --port 9333 \
  --testnet
```

The node will automatically discover and connect to bootstrap nodes. Omit `--validate` to run as an observer (no staking required).

---

## Basic RPC Calls

With a node running (RPC on port 10333 by default), try these commands:

### Check Node Status

```bash
curl http://localhost:10333/status
```

```json
{
  "round": 42,
  "finalized_round": 40,
  "peers": 3,
  "validator": true,
  "address": "a1b2c3d4e5f6...",
  "total_supply": 1050042000000000,
  "version": "0.1.0"
}
```

### Generate a Keypair

```bash
curl http://localhost:10333/keygen
```

```json
{
  "address": "e7f8a9b0c1d2...",
  "public_key": "3a4b5c6d7e8f...",
  "private_key": "9f8e7d6c5b4a..."
}
```

!!! warning "Testnet only"
    The `/keygen` endpoint returns private keys in plaintext. This is a testnet convenience — on mainnet, generate keys client-side using an [SDK](../api/sdks.md).

### Get Testnet UDAG from Faucet

```bash
curl -X POST http://localhost:10333/faucet -H "Content-Type: application/json" -d '{"address":"e7f8a9b0c1d2...","amount":100000000}'
```

```json
{
  "tx_hash": "abc123...",
  "amount": 100000000000,
  "message": "Sent 1000 UDAG to e7f8a9b0c1d2..."
}
```

### Check Balance

```bash
curl http://localhost:10333/balance/e7f8a9b0c1d2...
```

```json
{
  "address": "e7f8a9b0c1d2...",
  "balance": 100000000000,
  "nonce": 0,
  "staked": 0,
  "delegated": 0
}
```

### Send a Transaction

```bash
curl -X POST http://localhost:10333/tx \
  -H "Content-Type: application/json" \
  -d '{
    "from": "e7f8a9b0c1d2...",
    "to": "1a2b3c4d5e6f...",
    "amount": 50000000000,
    "private_key": "9f8e7d6c5b4a..."
  }'
```

```json
{
  "tx_hash": "def456...",
  "status": "accepted"
}
```

!!! note "Testnet signing"
    The `/tx` endpoint accepts `private_key` for testnet convenience. For mainnet, sign transactions client-side and submit via `/tx/submit`. See [Transaction Format](../api/transactions.md).

---

## Run the Test Suite

Verify everything works:

```bash
cargo test --workspace
```

This runs all 977 tests across the 5 crates, including consensus simulation, state engine invariants, and P2P protocol tests.

---

## Next Steps

| Goal | Guide |
|------|-------|
| Deploy with Docker | [Docker Guide](docker.md) |
| Become a validator | [Run a Validator](validator.md) |
| Integrate via API | [RPC Endpoints](../api/rpc.md) |
| Use an SDK | [SDKs](../api/sdks.md) |
| Understand the consensus | [DAG-BFT Consensus](../architecture/consensus.md) |
