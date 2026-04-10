---
title: "Node Operator Guide"
description: "Installing, configuring, and operating an UltraDAG node in production"
order: 1
section: "operations"
---

# Node Operator Guide

This guide covers installing, configuring, and operating an UltraDAG node in production. For a quick setup, see the [Quick Start](/docs/getting-started/quickstart).

---

## Installation

### Pre-Built Binary

Download the latest release binary from GitHub:

```bash
curl -L -o ultradag-node.tar.gz \
  https://github.com/UltraDAGcom/core/releases/download/latest/ultradag-node-linux-x86_64.tar.gz
tar -xzf ultradag-node.tar.gz
chmod +x ultradag-node-linux-x86_64
mv ultradag-node-linux-x86_64 /usr/local/bin/ultradag-node
```

The binary is under 4 MB (3.5 MB stripped on Linux aarch64 at v0.9; 2.87 MB on macOS arm64) and has zero runtime dependencies beyond standard glibc.

### Build from Source

```bash
git clone https://github.com/UltraDAGcom/core.git
cd core
cargo build --release -p ultradag-node
cp target/release/ultradag-node /usr/local/bin/
```

Requires Rust 1.75+ and a working C compiler (for ed25519-dalek).

### Docker

```bash
docker pull ghcr.io/ultradagcom/ultradag-node:latest
```

See the [Docker Guide](/docs/getting-started/docker) for complete container deployment instructions.

---

## Configuration

### CLI Flags

All configuration is done through CLI flags:

| Flag | Default | Description |
|------|---------|-------------|
| `--port` | `9333` | P2P listening port |
| `--rpc-port` | P2P + 1000 | HTTP RPC port |
| `--seed` | (bootstrap) | Seed peer addresses (`host:port`, repeatable) |
| `--validate` | `true` | Enable validator mode (set to `false` for observer) |
| `--round-ms` | `5000` | Round duration in milliseconds |
| `--validators` | auto | Expected validator count (fixes quorum threshold) |
| `--validator-key` | none | Path to allowlist file (one address per line) |
| `--data-dir` | `~/.ultradag/node` | Data persistence directory |
| `--no-bootstrap` | `false` | Disable auto-connection to bootstrap nodes |
| `--pruning-depth` | `1000` | Rounds to keep before pruning |
| `--archive` | `false` | Disable pruning (keep full history) |
| `--skip-fast-sync` | `false` | Skip checkpoint fast-sync on startup |
| `--pkey` | none | Validator private key (64-char hex) |
| `--auto-stake` | none | Auto-stake N UDAG after startup and sync |
| `--testnet` | `true` | Enable testnet mode (exposes convenience endpoints) |

### Key Priority

When loading the validator identity:

1. `--pkey` flag (highest priority)
2. `validator.key` file in data directory
3. Auto-generated new keypair (lowest priority)

---

## Running Modes

### Validator

Produces DAG vertices and participates in consensus:

```bash
ultradag-node --port 9333 --validate --pkey YOUR_KEY_HEX
```

Requirements: minimum 2,000 UDAG staked (the `MIN_STAKE_SATS` protocol constant), stable network connectivity.

### Observer

Follows the chain without producing vertices. No staking required:

```bash
ultradag-node --port 9333
```

Observers sync the DAG, maintain state, and serve RPC queries.

### Archive

Full history mode. Retains all DAG vertices without pruning:

```bash
ultradag-node --port 9333 --archive
```

Useful for block explorers and historical analysis. Requires more storage.

---

## State Persistence

### Data Directory Structure

| File | Format | Purpose |
|------|--------|---------|
| `dag.bin` | postcard binary | DAG vertices, tips, rounds |
| `finality.bin` | postcard binary | Finality tracker state |
| `state.redb` | redb ACID database | Accounts, stakes, governance |
| `mempool.json` | postcard binary | Pending transactions |
| `validator.key` | hex text | Validator private key |
| `checkpoints/` | directory | Checkpoint snapshots |

### Persistence Triggers

State is saved:

- Every 10 rounds during normal operation
- On graceful shutdown (SIGTERM/SIGINT)
- Atomically via temp file + rename (crash-safe)

---

## Startup Sequence

1. Parse CLI arguments and validate
2. Load or generate validator keypair
3. Load persisted state from data directory (DAG, finality, redb, mempool)
4. Verify genesis checkpoint hash
5. Apply validator allowlist (if `--validator-key` specified)
6. Start P2P listener
7. Connect to seed peers or bootstrap nodes
8. Attempt checkpoint fast-sync (unless `--skip-fast-sync`)
9. Auto-stake (if `--auto-stake` specified, waits 20s for sync)
10. Start RPC server
11. Start validator loop (if `--validate`)
12. Install graceful shutdown handler

---

## Graceful Shutdown

The node handles SIGTERM and SIGINT signals:

1. Stops the validator loop (no more vertex production)
2. Saves DAG state to `dag.bin`
3. Saves finality tracker to `finality.bin`
4. Saves state engine to `state.redb`
5. Saves mempool to `mempool.json`
6. Exits with code 0

<div class="callout callout-warning"><div class="callout-title">Do not use SIGKILL</div>Always use <code>SIGTERM</code> or <code>SIGINT</code> for graceful shutdown. <code>SIGKILL</code> (kill -9) prevents state saving and may require a longer sync on restart.</div>

---

## Backup and Restore

### Backup

Stop the node and copy the data directory:

```bash
systemctl stop ultradag
cp -r ~/.ultradag/node ~/.ultradag/node-backup-$(date +%Y%m%d)
systemctl start ultradag
```

### Restore

Replace the data directory with a backup:

```bash
systemctl stop ultradag
rm -rf ~/.ultradag/node
cp -r ~/.ultradag/node-backup-20260317 ~/.ultradag/node
systemctl start ultradag
```

The node will resume from the backed-up state and sync any missed vertices from peers.

---

## Upgrading

Binary upgrades follow a simple swap-and-restart pattern:

```bash
# Download new binary
curl -L -o /tmp/ultradag-node.tar.gz \
  https://github.com/UltraDAGcom/core/releases/download/latest/ultradag-node-linux-x86_64.tar.gz
tar -xzf /tmp/ultradag-node.tar.gz -C /tmp/
chmod +x /tmp/ultradag-node-linux-x86_64

# Swap and restart
systemctl stop ultradag
cp /tmp/ultradag-node-linux-x86_64 /usr/local/bin/ultradag-node
systemctl start ultradag
```

<div class="callout callout-tip"><div class="callout-title">Backup first</div>Always back up your data directory before upgrading. While UltraDAG supports in-place upgrades, keeping a backup protects against unforeseen issues.</div>

---

## Troubleshooting

### Node won't start

- **Port in use**: check with `ss -tlnp | grep 9333`
- **Corrupted state**: try `--skip-fast-sync` or delete data directory for fresh sync
- **Permission denied**: verify data directory is writable

### Finality stuck

- Check peer count: `curl http://localhost:10333/peers`
- Check finality lag: `curl http://localhost:10333/health/detailed`
- Verify validators are producing: `curl http://localhost:10333/status`

### No peers connecting

- Ensure P2P port is reachable (check firewall rules)
- Try explicit seeds: `--seed known-peer:9333`
- Check bootstrap connectivity: `curl https://ultradag-node-1.fly.dev/status`

### High memory usage

- Verify pruning is enabled (default: 1000 rounds)
- Check `RUST_LOG` level (trace can cause memory growth)
- Monitor with `/health/detailed` endpoint

### Transaction pending forever

- Verify nonce matches: `curl http://localhost:10333/balance/ADDRESS`
- Check mempool: `curl http://localhost:10333/mempool`
- Ensure sufficient fee (>= 10,000 sats for transfers)

---

## systemd Service File

For production deployments:

```ini
[Unit]
Description=UltraDAG Node
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=ultradag
ExecStart=/usr/local/bin/ultradag-node \
  --port 9333 \
  --validate \
  --data-dir /var/lib/ultradag \
  --pkey YOUR_VALIDATOR_KEY_HEX
Restart=on-failure
RestartSec=10
LimitNOFILE=65536

[Install]
WantedBy=multi-user.target
```

---

## Next Steps

- [Validator Handbook](/docs/operations/validator-handbook) -- detailed validator operations
- [Monitoring](/docs/operations/monitoring) -- set up metrics and alerts
- [CLI Reference](/docs/operations/cli) -- all flags and environment variables
