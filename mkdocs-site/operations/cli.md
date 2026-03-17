---
title: CLI Reference
---

# CLI Reference

Complete reference for all `ultradag-node` command-line flags and environment variables.

---

## Usage

```bash
ultradag-node [FLAGS] [OPTIONS]
```

---

## Flags

### Network

| Flag | Default | Description |
|------|---------|-------------|
| `--port <PORT>` | `9333` | P2P listening port. Other nodes connect to this port for DAG sync and vertex gossip. |
| `--rpc-port <PORT>` | P2P + 1000 | HTTP RPC server port. Serves the JSON API for wallets, explorers, and monitoring. |
| `--seed <ADDR>` | (none) | Seed peer address in `host:port` format. Can be specified multiple times. |
| `--no-bootstrap` | `false` | Disable automatic connection to hardcoded testnet bootstrap nodes. Use for isolated local testnets. |

### Validator

| Flag | Default | Description |
|------|---------|-------------|
| `--validate` | `true` | Enable validator mode. The node will produce DAG vertices each round. Set to `false` for observer mode. |
| `--pkey <HEX>` | (none) | Validator private key as a 64-character hex string. Takes priority over key file. |
| `--validator-key <FILE>` | (none) | Path to a file containing trusted validator addresses (one per line). Only listed addresses count toward quorum and finality. |
| `--validators <N>` | auto | Fix the expected validator count for quorum calculation. Prevents phantom validator inflation. Must be >= 1. |
| `--auto-stake <UDAG>` | (none) | Automatically submit a stake transaction after startup and sync. Waits 20 seconds, checks balance and existing stake before proceeding. |
| `--round-ms <MS>` | `5000` | Round duration in milliseconds. The round timer is the fallback when the optimistic 2f+1 gate is not met. Must be >= 1. |

### Storage

| Flag | Default | Description |
|------|---------|-------------|
| `--data-dir <PATH>` | `~/.ultradag/node` | Directory for state persistence (DAG, finality, state.redb, mempool, checkpoints). |
| `--pruning-depth <N>` | `1000` | Number of finalized rounds to retain before pruning. Lower values save memory at the cost of sync flexibility. Must be >= 1. |
| `--archive` | `false` | Disable pruning entirely. Keeps full DAG history. Overrides `--pruning-depth`. |

### Mode

| Flag | Default | Description |
|------|---------|-------------|
| `--testnet` | `true` | Enable testnet mode. Exposes convenience endpoints (`/tx`, `/stake`, `/keygen`, `/faucet`, etc.) that accept private keys. Omit this flag for mainnet mode (only /tx/submit accepted). |
| `--skip-fast-sync` | `false` | Skip checkpoint-based fast-sync on startup. The node will use only local state. Useful for debugging or when fast-sync is failing. |

---

## Environment Variables

Environment variables can be used as an alternative to CLI flags, primarily for Docker deployments:

| Variable | Maps To | Description |
|----------|---------|-------------|
| `PORT` | `--port` | P2P listening port |
| `RPC_PORT` | `--rpc-port` | RPC server port |
| `DATA_DIR` | `--data-dir` | Data persistence directory |
| `VALIDATORS` | `--validators` | Expected validator count |
| `SEED` | `--seed` | Seed peer address |
| `CLEAN_STATE` | (special) | If `true`, delete all state on startup |
| `RUST_LOG` | (logging) | Log level filter |

!!! note "Precedence"
    CLI flags take precedence over environment variables. Environment variables are primarily used in Docker and Fly.io deployments via the entrypoint script.

---

## Examples

### Local Development (Single Node)

```bash
ultradag-node --port 9333 --validate --testnet
```

### Local 4-Node Testnet

```bash
# Terminal 1 (first node, no seeds needed)
ultradag-node --port 9333 --validate --validators 4 --no-bootstrap

# Terminal 2
ultradag-node --port 9334 --validate --validators 4 \
  --seed 127.0.0.1:9333 --no-bootstrap

# Terminal 3
ultradag-node --port 9335 --validate --validators 4 \
  --seed 127.0.0.1:9333 --no-bootstrap

# Terminal 4
ultradag-node --port 9336 --validate --validators 4 \
  --seed 127.0.0.1:9333 --no-bootstrap
```

### Join Public Testnet as Observer

```bash
ultradag-node --port 9333 --testnet
```

### Join Public Testnet as Validator

```bash
ultradag-node --port 9333 --validate \
  --pkey YOUR_SECRET_KEY_HEX \
  --auto-stake 10000 \
  --testnet
```

### Production Validator

```bash
ultradag-node --port 9333 --validate \
  --pkey YOUR_SECRET_KEY_HEX \
  --data-dir /var/lib/ultradag \
  --testnet false
```

### Archive Node

```bash
ultradag-node --port 9333 --archive \
  --data-dir /var/lib/ultradag-archive
```

### Custom Round Duration

```bash
ultradag-node --port 9333 --validate --round-ms 3000
```

### Permissioned Network

```bash
# Create allowlist
cat > validators.txt << EOF
a1b2c3d4e5f6...
d4e5f6a7b8c9...
7a8b9c0d1e2f...
EOF

# Run with allowlist
ultradag-node --port 9333 --validate \
  --validator-key validators.txt \
  --validators 3 \
  --no-bootstrap
```

---

## Validation Rules

The following values are rejected at startup:

| Flag | Rejected Values | Reason |
|------|----------------|--------|
| `--validators` | `0` | Division by zero in quorum calculation |
| `--round-ms` | `0` | Tight spin loop |
| `--pruning-depth` | `0` | Prunes everything immediately |
| `--pkey` | Non-hex, wrong length | Invalid Ed25519 key |

---

## Next Steps

- [Node Operator Guide](node-guide.md) — deployment and operations
- [Docker Guide](../getting-started/docker.md) — container deployment
- [Validator Handbook](validator-handbook.md) — validator-specific guidance
