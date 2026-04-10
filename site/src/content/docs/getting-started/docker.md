---
title: "Docker Guide"
description: "Run UltraDAG nodes using Docker for easy deployment and reproducible environments."
order: 3
section: "getting-started"
---

# Docker Guide

Run UltraDAG nodes using Docker for easy deployment and reproducible environments.

---

## Single Node

Pull and run a single node:

```bash
docker run -d \
  --name ultradag \
  -p 9333:9333 \
  -p 10333:10333 \
  -v ultradag-data:/data \
  ghcr.io/ultradagcom/ultradag-node:latest \
  --port 9333 \
  --validate \
  --data-dir /data \
  --testnet
```

This exposes:

- **Port 9333**: P2P protocol
- **Port 10333**: RPC API

Verify the node is running:

```bash
curl http://localhost:10333/status
```

---

## 4-Node Local Testnet

Use Docker Compose to run a complete local testnet:

```yaml
version: "3.8"

services:
  node1:
    image: ghcr.io/ultradagcom/ultradag-node:latest
    command: >
      --port 9333
      --validate
      --validators 4
      --no-bootstrap
      --data-dir /data
      --testnet
    ports:
      - "9333:9333"
      - "10333:10333"
    volumes:
      - node1-data:/data
    environment:
      - RUST_LOG=info

  node2:
    image: ghcr.io/ultradagcom/ultradag-node:latest
    command: >
      --port 9333
      --validate
      --validators 4
      --seed node1:9333
      --data-dir /data
      --testnet
    ports:
      - "9334:9333"
      - "10334:10333"
    volumes:
      - node2-data:/data
    environment:
      - RUST_LOG=info

  node3:
    image: ghcr.io/ultradagcom/ultradag-node:latest
    command: >
      --port 9333
      --validate
      --validators 4
      --seed node1:9333
      --data-dir /data
      --testnet
    ports:
      - "9335:9333"
      - "10335:10333"
    volumes:
      - node3-data:/data
    environment:
      - RUST_LOG=info

  node4:
    image: ghcr.io/ultradagcom/ultradag-node:latest
    command: >
      --port 9333
      --validate
      --validators 4
      --seed node1:9333
      --data-dir /data
      --testnet
    ports:
      - "9336:9333"
      - "10336:10333"
    volumes:
      - node4-data:/data
    environment:
      - RUST_LOG=info

volumes:
  node1-data:
  node2-data:
  node3-data:
  node4-data:
```

Start the network:

```bash
docker compose up -d
```

Check all nodes:

```bash
for port in 10333 10334 10335 10336; do
  echo "--- Node on port $port ---"
  curl -s http://localhost:$port/status | jq .round
done
```

Stop and clean up:

```bash
docker compose down -v   # -v removes volumes (fresh state)
```

---

## Environment Variables

Configure node behavior through environment variables:

| Variable | Default | Description |
|----------|---------|-------------|
| `RUST_LOG` | `info` | Log level (`trace`, `debug`, `info`, `warn`, `error`) |
| `PORT` | `9333` | P2P listening port |
| `RPC_PORT` | P2P + 1000 | RPC server port |
| `DATA_DIR` | `./data` | State persistence directory |
| `VALIDATORS` | `1` | Expected validator count (for deterministic genesis) |
| `SEED` | (none) | Seed peer address (`host:port`) to connect to on startup |
| `CLEAN_STATE` | `false` | Delete existing state on startup |

<div class="callout callout-tip"><div class="callout-title">Log levels</div>Use <code>RUST_LOG=ultradag_coin=debug,ultradag_network=info</code> for fine-grained control. The <code>trace</code> level is very verbose and should only be used for debugging specific issues.</div>

---

## Volume Mounting for Persistence

The node stores all state in the data directory. Mount a volume to persist across container restarts:

```bash
docker run -d \
  --name ultradag \
  -v /path/on/host:/data \
  ghcr.io/ultradagcom/ultradag-node:latest \
  --data-dir /data \
  --validate \
  --testnet
```

The data directory contains:

| File | Purpose |
|------|---------|
| `dag.bin` | Serialized DAG vertices |
| `finality.bin` | Finality tracker state |
| `state.redb` | Account balances, stakes, governance (ACID database) |
| `mempool.json` | Pending transactions |
| `checkpoint_*.bin` | Checkpoint snapshots for fast-sync (flat files) |

<div class="callout callout-warning"><div class="callout-title">Backup before upgrades</div>Always back up your data directory before upgrading the node binary. While UltraDAG supports in-place upgrades, having a backup protects against unforeseen issues.</div>

---

## Building the Docker Image

Build locally from the repository:

```bash
git clone https://github.com/UltraDAGcom/core.git
cd core
docker build -t ultradag-node .
```

The Dockerfile downloads a pre-built binary from GitHub Releases (built by CI on every push to `main`):

```dockerfile
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates curl && rm -rf /var/lib/apt/lists/*

# Download pre-built binary from GitHub Releases
RUN curl -L -o /usr/local/bin/ultradag-node \
    https://github.com/UltraDAGcom/core/releases/download/latest/ultradag-node && \
    chmod +x /usr/local/bin/ultradag-node

ENTRYPOINT ["ultradag-node"]
```

The resulting image is minimal — it contains only the ~2.9 MB stripped binary plus base system libraries. Deployment takes ~60 seconds per node (vs 15+ minutes with source compilation).

---

## Production Deployment

For production Docker deployments:

### Resource Limits

```yaml
services:
  ultradag:
    image: ghcr.io/ultradagcom/ultradag-node:latest
    deploy:
      resources:
        limits:
          cpus: "1.0"
          memory: 512M
        reservations:
          cpus: "0.25"
          memory: 128M
    restart: unless-stopped
```

### Health Checks

```yaml
services:
  ultradag:
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:10333/health"]
      interval: 30s
      timeout: 5s
      retries: 3
      start_period: 10s
```

### Networking

For validators, ensure the P2P port is reachable from the internet:

```yaml
services:
  ultradag:
    ports:
      - "9333:9333"    # P2P - must be publicly accessible
      - "127.0.0.1:10333:10333"  # RPC - bind to localhost only
```

<div class="callout callout-warning"><div class="callout-title">RPC security</div>Never expose the RPC port to the public internet without authentication. The testnet RPC includes endpoints like <code>/keygen</code> and <code>/faucet</code> that should not be publicly accessible in production.</div>

---

## Troubleshooting

### Container exits immediately

Check logs:

```bash
docker logs ultradag
```

Common causes:

- Port already in use — change the host port mapping
- Corrupted state — set `CLEAN_STATE=true` to start fresh
- Missing data directory permissions — ensure the volume is writable

### Node not finding peers

If running a local testnet, ensure all containers are on the same Docker network:

```bash
docker network create ultradag-net
docker run --network ultradag-net ...
```

### High memory usage

UltraDAG is designed to run within 128-512 MB. If memory grows beyond this:

1. Check `RUST_LOG` — `trace` level can cause memory growth from log buffering
2. Ensure pruning is not disabled (use `--archive` to disable pruning; note that `--pruning-depth 0` is rejected)
3. Check for stuck checkpoint sync with `/health/detailed`

---

## Next Steps

- [Run a Validator](/docs/getting-started/validator) — stake UDAG and earn rewards
- [Node Operator Guide](/docs/operations/node-guide) — detailed operational guidance
- [Monitoring](/docs/operations/monitoring) — set up Prometheus and Grafana
