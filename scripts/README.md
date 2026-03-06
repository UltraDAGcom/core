# UltraDAG Scripts

Operational scripts for building, running, and managing UltraDAG nodes.

## Quick Start

```bash
git clone https://github.com/ultradag/ultradag.git
cd ultradag
./scripts/install.sh
./scripts/testnet-local.sh   # local 4-node testnet
# or
./scripts/node.sh --seed mainnet-seed.ultradag.com:9333  # join testnet
```

## Scripts

### Tier 1 — Core Operations

| Script | Description |
|--------|-------------|
| `install.sh` | Check for Rust, build the node binary from source |
| `node.sh` | Start a persistent validator node (background, with PID file) |
| `testnet-local.sh` | Spin up a 4-node local testnet (Ctrl+C to stop) |
| `stop.sh` | Stop a running node or all testnet nodes cleanly |

### Tier 2 — Testnet Operations

| Script | Description |
|--------|-------------|
| `testnet-join.sh` | Join an existing testnet as a validator |
| `status.sh` | Show formatted node status (round, finality, peers, supply) |
| `reset.sh` | Wipe node data and start fresh (requires confirmation) |

### Tier 3 — Utilities

| Script | Description |
|--------|-------------|
| `keygen.sh` | Generate a new keypair via RPC and save to file |
| `loadtest.sh` | Submit transactions and measure throughput |
| `logs.sh` | Tail node log with color formatting |

## Common Workflows

### Local Development

```bash
# Build and start a 4-node testnet
./scripts/install.sh
./scripts/testnet-local.sh

# In another terminal, check status
./scripts/status.sh

# Generate a keypair
./scripts/keygen.sh --output mykey.json

# Stop the testnet
# Press Ctrl+C in the testnet terminal, or:
./scripts/stop.sh --all
```

### Join a Remote Testnet

```bash
./scripts/install.sh
./scripts/testnet-join.sh --seed 1.2.3.4:9333
```

### Run a Persistent Node

```bash
./scripts/node.sh --port 9333 --data-dir ~/.ultradag/validator1
./scripts/status.sh
./scripts/logs.sh
./scripts/stop.sh --data-dir ~/.ultradag/validator1
```

### Load Testing

```bash
# Start testnet, generate keys, fund sender via faucet
curl -X POST http://127.0.0.1:10333/faucet \
  -H "Content-Type: application/json" \
  -d '{"address":"<sender_address>","amount":1000000000}'

# Run load test
./scripts/loadtest.sh --from-secret <secret> --to <address> --txs 500
```

## Requirements

- **Rust** (installed automatically by `install.sh`)
- **curl** (for RPC scripts)
- **jq** or **python3** (for JSON parsing in `status.sh`)
- Works on **Linux** and **macOS**

All scripts support `--help` for detailed usage information.
