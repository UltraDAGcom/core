---
title: "Quick Start"
description: "Get an UltraDAG node running in under 5 minutes — build from source, run a local testnet, and make your first RPC calls."
order: 1
section: "getting-started"
---

# Quick Start

Get an UltraDAG node running in under 5 minutes. This guide covers building from source, running a single node, spinning up a local testnet, and making your first RPC calls.

---

## Download Pre-Built Binary

The fastest way to get started. Pre-built binaries are available for Linux and macOS.

### Linux (x86_64)

```bash
curl -L https://github.com/UltraDAGcom/core/releases/download/latest/ultradag-node-linux-x86_64.tar.gz | tar xz
chmod +x ultradag-node-linux-x86_64
./ultradag-node-linux-x86_64 --port 9333
```

### macOS (Apple Silicon)

```bash
curl -L https://github.com/UltraDAGcom/core/releases/download/latest/ultradag-node-macos-arm64.tar.gz | tar xz
chmod +x ultradag-node-macos-arm64
./ultradag-node-macos-arm64 --port 9333
```

### macOS (Intel)

```bash
curl -L https://github.com/UltraDAGcom/core/releases/download/latest/ultradag-node-macos-x86_64.tar.gz | tar xz
chmod +x ultradag-node-macos-x86_64
./ultradag-node-macos-x86_64 --port 9333
```

To validate (produce blocks and earn UDAG):

```bash
./ultradag-node-linux-x86_64 --port 9333 --validate
```

<div class="callout callout-tip"><div class="callout-title">No download needed? Build from source</div>If you prefer to compile yourself or need a different target, see the build instructions below.</div>

---

## Prerequisites (for building from source)

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

<div class="callout callout-tip"><div class="callout-title">No Rust? Use Docker</div>If you prefer not to install Rust, see the <a href="/docs/getting-started/docker">Docker Guide</a> for container-based deployment.</div>

---

## Build from Source

Clone the repository and build the node binary:

```bash
git clone https://github.com/UltraDAGcom/core.git ultradag
cd ultradag
cargo build --release -p ultradag-node
```

The binary is at `target/release/ultradag-node` (~2.9 MB stripped on macOS arm64, ~3.5 MB on Linux aarch64 — depends on your host). You can copy it anywhere:

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

<div class="callout callout-note"><div class="callout-title">Solo mode</div>A single validator will finalize immediately since it constitutes 100% of the validator set. This is useful for local development but not representative of production finality timing.</div>

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
cargo run --release -p ultradag-node -- --port 9333 --validate --validators 4 --testnet &
cargo run --release -p ultradag-node -- --port 9334 --validate --validators 4 --seed 127.0.0.1:9333 --testnet &
cargo run --release -p ultradag-node -- --port 9335 --validate --validators 4 --seed 127.0.0.1:9333 --testnet &
cargo run --release -p ultradag-node -- --port 9336 --validate --validators 4 --seed 127.0.0.1:9333 --testnet &

echo "Testnet running. RPC ports: 10333, 10334, 10335, 10336"
echo "Press Ctrl+C to stop all nodes"
wait
```

Make it executable and run:

```bash
chmod +x run-local-testnet.sh
./run-local-testnet.sh
```

<div class="callout callout-info"><div class="callout-title">Seed peers</div>The <code>--seed</code> flag specifies peer addresses (<code>host:port</code>) for the node to connect to on startup. The first node starts without <code>--seed</code> and subsequent nodes connect to it.</div>

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
  "dag_round": 42,
  "last_finalized_round": 40,
  "peers": 3,
  "total_supply": 1050042000000000
}
```

### Generate a Keypair

```bash
curl http://localhost:10333/keygen
```

```json
{
  "address": "e7f8a9b0c1d2...",
  "secret_key": "9f8e7d6c5b4a..."
}
```

<div class="callout callout-warning"><div class="callout-title">Testnet only</div>The <code>/keygen</code> endpoint returns private keys in plaintext. This is a testnet convenience — on mainnet, generate keys client-side using an SDK.</div>

### Get Testnet UDAG from Faucet

```bash
curl -X POST http://localhost:10333/faucet -H "Content-Type: application/json" -d '{"address":"e7f8a9b0c1d2...","amount":10000000000}'
```

```json
{
  "tx_hash": "abc123...",
  "amount": 10000000000,
  "message": "Sent 100 UDAG to e7f8a9b0c1d2..."
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
    "secret_key": "9f8e7d6c5b4a...",
    "to": "1a2b3c4d5e6f...",
    "amount": 50000000000,
    "fee": 10000
  }'
```

```json
{
  "tx_hash": "def456...",
  "status": "accepted"
}
```

<div class="callout callout-note"><div class="callout-title">Testnet signing</div>The <code>/tx</code> endpoint accepts <code>secret_key</code> for testnet convenience. For mainnet, sign transactions client-side and submit via <code>/tx/submit</code>. See <a href="/docs/api/transactions">Transaction Format</a>.</div>

---

## Run the Test Suite

Verify everything works:

```bash
cargo test --workspace
```

This runs all 836 tests across the crates, including consensus simulation, state engine invariants, and P2P protocol tests.

---

## Next Steps

| Goal | Guide |
|------|-------|
| Deploy with Docker | [Docker Guide](/docs/getting-started/docker) |
| Become a validator | [Run a Validator](/docs/getting-started/validator) |
| Integrate via API | [RPC Endpoints](/docs/api/rpc) |
| Use an SDK | [SDKs](/docs/api/sdks) |
| Understand the consensus | [DAG-BFT Consensus](/docs/architecture/consensus) |
