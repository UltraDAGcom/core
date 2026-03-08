# Run a Validator on UltraDAG Testnet

This guide walks you through joining the UltraDAG testnet as a validator. Running a validator helps secure the network, earns you testnet rewards, and gives you hands-on experience with DAG-BFT consensus.

## Table of Contents

- [Requirements](#requirements)
- [Quick Start](#quick-start)
- [Building from Source](#building-from-source)
- [Configuration](#configuration)
- [Deployment Options](#deployment-options)
- [Staking](#staking)
- [Monitoring](#monitoring)
- [Troubleshooting](#troubleshooting)

---

## Requirements

### Hardware

UltraDAG is designed to run on minimal hardware:

- **CPU:** 1 core (2 cores recommended)
- **RAM:** 512 MB minimum, 1 GB recommended
- **Disk:** 10 GB (grows ~1 GB per million rounds)
- **Network:** 5 Mbps up/down, stable connection

**This runs on a $5/month VPS.** No specialized hardware required.

### Software

- **OS:** Linux (Ubuntu 22.04 recommended), macOS, or Windows (WSL2)
- **Rust:** 1.75+ (`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`)
- **Git:** For cloning the repository

### Network

- **Open ports:**
  - `9333` (TCP) — P2P consensus
  - `10333` (TCP) — RPC API (optional, for monitoring)

- **Firewall rules:**
  ```bash
  # Ubuntu/Debian
  sudo ufw allow 9333/tcp
  sudo ufw allow 10333/tcp
  ```

---

## Quick Start

### 1. Clone and Build

```bash
git clone https://github.com/ultradag/core.git ultradag
cd ultradag
cargo build --release --bin ultradag-node
```

Build time: ~5-10 minutes on a modern machine.

### 2. Generate a Validator Keypair

```bash
curl https://ultradag-node-1.fly.dev/keygen
```

Save the output:
```json
{
  "secret_key": "a1b2c3d4...",
  "address": "e5f6g7h8..."
}
```

**Store the secret key securely.** This is your validator identity.

### 3. Fund Your Validator

You need at least 10,000 UDAG to become a validator.

Request testnet tokens:
```bash
curl -X POST https://ultradag-node-1.fly.dev/faucet \
  -H "Content-Type: application/json" \
  -d '{"address": "YOUR_ADDRESS_HERE"}'
```

The faucet gives 100 UDAG per request. Repeat 100 times or ask in Telegram @ultra_dag for a larger allocation.

### 4. Stake Your Tokens

```bash
curl -X POST https://ultradag-node-1.fly.dev/stake \
  -H "Content-Type: application/json" \
  -d '{
    "from": "YOUR_ADDRESS",
    "amount": 1000000000000,
    "secret_key": "YOUR_SECRET_KEY"
  }'
```

Amount is in sats (1 UDAG = 100,000,000 sats). This stakes 10,000 UDAG.

### 5. Run Your Node

```bash
./target/release/ultradag-node \
  --port 9333 \
  --rpc-port 10333 \
  --data-dir ./data \
  --validators 21 \
  --seed ultradag-node-1.fly.dev:9333 \
  --seed ultradag-node-2.fly.dev:9333
```

Your node will:
1. Connect to seed nodes
2. Download the latest checkpoint
3. Sync to the current round
4. Start participating in consensus

**You're now a validator!** (Active in the next epoch, ~6 days)

---

## Building from Source

### Clone the Repository

```bash
git clone https://github.com/ultradag/core.git ultradag
cd ultradag
```

### Install Rust

If you don't have Rust installed:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

Verify installation:
```bash
rustc --version  # Should show 1.75+
```

### Build the Node

```bash
cargo build --release --bin ultradag-node
```

The binary will be at `./target/release/ultradag-node`.

### Run Tests (Optional)

Verify everything works:

```bash
cargo test --release
```

All 235+ tests should pass.

---

## Configuration

### Command-Line Arguments

```bash
ultradag-node [OPTIONS]

OPTIONS:
  --port <PORT>              P2P port [default: 9333]
  --rpc-port <RPC_PORT>      RPC API port [default: 10333]
  --data-dir <DATA_DIR>      Data directory [default: ./data]
  --validators <N>           Expected validator count [default: 21]
  --seed <ADDR>              Seed peer address (can specify multiple)
  --no-bootstrap             Don't connect to seed peers (for first node)
  --clean-state              Delete existing state and start fresh
```

### Environment Variables

Alternatively, use environment variables:

```bash
export PORT=9333
export RPC_PORT=10333
export DATA_DIR=/var/lib/ultradag
export VALIDATORS=21
export SEED="ultradag-node-1.fly.dev:9333 ultradag-node-2.fly.dev:9333"

./target/release/ultradag-node
```

### Seed Peers

Current testnet seed peers:

```
ultradag-node-1.fly.dev:9333
ultradag-node-2.fly.dev:9333
ultradag-node-3.fly.dev:9333
ultradag-node-4.fly.dev:9333
```

Specify multiple seeds for redundancy:

```bash
--seed ultradag-node-1.fly.dev:9333 \
--seed ultradag-node-2.fly.dev:9333
```

### Data Directory

The `--data-dir` stores:
- `state.json` — StateEngine snapshot
- `dag.json` — Recent DAG vertices
- `monotonicity.json` — Round monotonicity tracker

**Backup strategy:**
- State is checkpointed every 1,000 rounds
- If data is lost, node will fast-sync from peers
- No need for manual backups (testnet)

---

## Deployment Options

### Option 1: Bare Metal / VPS

**Recommended for:** Production validators, maximum control.

#### Ubuntu 22.04 Setup

```bash
# Update system
sudo apt update && sudo apt upgrade -y

# Install dependencies
sudo apt install -y build-essential pkg-config libssl-dev

# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# Clone and build
git clone https://github.com/ultradag/core.git ultradag
cd ultradag
cargo build --release --bin ultradag-node

# Create data directory
sudo mkdir -p /var/lib/ultradag
sudo chown $USER:$USER /var/lib/ultradag

# Create systemd service
sudo tee /etc/systemd/system/ultradag.service > /dev/null <<EOF
[Unit]
Description=UltraDAG Validator Node
After=network.target

[Service]
Type=simple
User=$USER
WorkingDirectory=$HOME/ultradag
ExecStart=$HOME/ultradag/target/release/ultradag-node \
  --port 9333 \
  --rpc-port 10333 \
  --data-dir /var/lib/ultradag \
  --validators 21 \
  --seed ultradag-node-1.fly.dev:9333 \
  --seed ultradag-node-2.fly.dev:9333
Restart=always
RestartSec=10

[Install]
WantedBy=multi-user.target
EOF

# Start service
sudo systemctl daemon-reload
sudo systemctl enable ultradag
sudo systemctl start ultradag

# Check status
sudo systemctl status ultradag
```

#### View Logs

```bash
sudo journalctl -u ultradag -f
```

### Option 2: Fly.io

**Recommended for:** Quick deployment, global distribution.

#### Prerequisites

```bash
# Install flyctl
curl -L https://fly.io/install.sh | sh

# Login
flyctl auth login
```

#### Create fly.toml

```toml
app = "my-ultradag-validator"
primary_region = "ams"  # Choose your region

[build]

[env]
  RUST_LOG = "info"
  PORT = "9333"
  RPC_PORT = "10333"
  DATA_DIR = "/data"
  VALIDATORS = "21"
  SEED = "ultradag-node-1.fly.dev:9333 ultradag-node-2.fly.dev:9333"

[mounts]
  source = "ultradag_data"
  destination = "/data"

[http_service]
  internal_port = 10333
  force_https = true
  auto_stop_machines = false
  auto_start_machines = true
  min_machines_running = 1

[[services]]
  internal_port = 9333
  protocol = "tcp"
  auto_stop_machines = false
  auto_start_machines = true
  min_machines_running = 1

  [[services.ports]]
    port = 9333
```

#### Deploy

```bash
# Create app
flyctl apps create my-ultradag-validator

# Create volume
flyctl volumes create ultradag_data --size 10 --region ams

# Deploy
flyctl deploy

# Check status
flyctl status
flyctl logs
```

### Option 3: Docker

**Recommended for:** Development, testing.

#### Dockerfile

```dockerfile
FROM rust:1.75 as builder
WORKDIR /build
COPY . .
RUN cargo build --release --bin ultradag-node

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /build/target/release/ultradag-node /usr/local/bin/
EXPOSE 9333 10333
CMD ["ultradag-node"]
```

#### Build and Run

```bash
# Build image
docker build -t ultradag-node .

# Run container
docker run -d \
  --name ultradag \
  -p 9333:9333 \
  -p 10333:10333 \
  -v ultradag-data:/data \
  -e PORT=9333 \
  -e RPC_PORT=10333 \
  -e DATA_DIR=/data \
  -e VALIDATORS=21 \
  -e SEED="ultradag-node-1.fly.dev:9333 ultradag-node-2.fly.dev:9333" \
  ultradag-node

# View logs
docker logs -f ultradag
```

---

## Staking

### Check Your Balance

```bash
curl https://ultradag-node-1.fly.dev/balance/YOUR_ADDRESS
```

### Stake Tokens

Minimum stake: 10,000 UDAG (1,000,000,000,000 sats)

```bash
curl -X POST https://ultradag-node-1.fly.dev/stake \
  -H "Content-Type: application/json" \
  -d '{
    "from": "YOUR_ADDRESS",
    "amount": 1000000000000,
    "secret_key": "YOUR_SECRET_KEY"
  }'
```

### Check Stake Status

```bash
curl https://ultradag-node-1.fly.dev/stake/YOUR_ADDRESS
```

Response:
```json
{
  "address": "...",
  "staked": 1000000000000,
  "staked_udag": 10000.0,
  "is_active_validator": false,
  "unlock_at_round": null,
  "current_round": 123456
}
```

### Becoming an Active Validator

1. **Stake at least 10,000 UDAG**
2. **Wait for the next epoch** (~6 days maximum)
3. **Be in the top 21 by stake amount**

Check active validators:
```bash
curl https://ultradag-node-1.fly.dev/status | jq .active_validators
```

### Unstaking

Unstaking has a cooldown period of 2,016 rounds (~1.4 hours):

```bash
curl -X POST https://ultradag-node-1.fly.dev/unstake \
  -H "Content-Type: application/json" \
  -d '{
    "from": "YOUR_ADDRESS",
    "secret_key": "YOUR_SECRET_KEY"
  }'
```

After cooldown, your stake returns to liquid balance automatically.

---

## Monitoring

### RPC Endpoints

Your node exposes an HTTP RPC API on port 10333:

#### Node Status

```bash
curl http://localhost:10333/status
```

Response:
```json
{
  "last_finalized_round": 123456,
  "finality_lag": 2,
  "total_supply": 10500000000000000,
  "active_validators": ["addr1...", "addr2...", ...],
  "peer_count": 12
}
```

#### Balance

```bash
curl http://localhost:10333/balance/ADDRESS
```

#### Mempool

```bash
curl http://localhost:10333/mempool
```

### Metrics to Watch

**Finality lag:** Should stay at 1-2 rounds. If it grows, your node may be falling behind.

**Peer count:** Should be 8-20. If it's 0-2, you may have network issues.

**Last finalized round:** Should increment every ~2.5 seconds.

### Logging

Set log level with `RUST_LOG` environment variable:

```bash
export RUST_LOG=info  # Options: error, warn, info, debug, trace
```

**Systemd:**
```bash
sudo journalctl -u ultradag -f
```

**Docker:**
```bash
docker logs -f ultradag
```

### Dashboard

Open the web dashboard at:
```
http://YOUR_IP:10333/dashboard.html
```

Features:
- Real-time DAG visualization
- Balance and transaction history
- Staking interface
- Governance proposals

---

## Troubleshooting

### Node Won't Start

**Error:** `Address already in use`

**Solution:** Another process is using port 9333 or 10333.

```bash
# Find process using port
sudo lsof -i :9333

# Kill it
sudo kill -9 PID
```

### Node Won't Sync

**Error:** `No peers connected`

**Solution:** Check firewall, verify seed peers are reachable.

```bash
# Test connectivity
nc -zv ultradag-node-1.fly.dev 9333

# Check firewall
sudo ufw status
```

### High Finality Lag

**Symptom:** `finality_lag` growing beyond 10 rounds.

**Causes:**
- Slow network connection
- Insufficient CPU
- Node falling behind

**Solution:**
```bash
# Check network latency
ping ultradag-node-1.fly.dev

# Check CPU usage
top

# Restart node to fast-sync
sudo systemctl restart ultradag
```

### Not Becoming a Validator

**Symptom:** Staked 10,000+ UDAG but not in active set.

**Causes:**
- Not yet reached next epoch boundary
- Not in top 21 by stake amount

**Solution:**
```bash
# Check current epoch
curl http://localhost:10333/status | jq .current_epoch

# Check your rank
curl http://localhost:10333/stake/YOUR_ADDRESS

# Wait for next epoch or stake more
```

### Data Corruption

**Symptom:** Node crashes on startup with state errors.

**Solution:** Delete state and fast-sync from peers.

```bash
# Stop node
sudo systemctl stop ultradag

# Delete state
rm -rf /var/lib/ultradag/*

# Start node (will fast-sync)
sudo systemctl start ultradag
```

---

## Security Best Practices

### Protect Your Secret Key

- **Never share your secret key**
- Store it in a password manager
- Use environment variables, not command-line arguments
- Consider hardware wallet integration (future)

### Firewall Configuration

Only expose necessary ports:

```bash
# Allow P2P
sudo ufw allow 9333/tcp

# Allow RPC only from trusted IPs
sudo ufw allow from YOUR_IP to any port 10333

# Enable firewall
sudo ufw enable
```

### Regular Updates

```bash
# Update code
cd ~/ultradag
git pull
cargo build --release --bin ultradag-node

# Restart node
sudo systemctl restart ultradag
```

### Monitoring Alerts

Set up alerts for:
- Node downtime (no finalized rounds for 60 seconds)
- High finality lag (>10 rounds)
- Low peer count (<3 peers)

---

## Validator Economics

### Block Rewards

Validators earn rewards for producing blocks:

- **Base reward:** Decreases by 50% every 210,000 rounds (halving)
- **Initial reward:** 50 UDAG per block
- **Observer reward:** Validators ranked 22-100 earn 20% of block rewards

### Expected Returns

**Assumptions:**
- 21 active validators
- 10,000 UDAG staked per validator
- 2.5-second rounds

**Calculation:**
- Blocks per day: 21 validators × 34,560 rounds/day = 725,760 blocks/day
- Rewards per day: 725,760 × 50 UDAG = 36,288,000 UDAG/day
- Your share: 36,288,000 / 21 = 1,728,000 UDAG/day
- Annual return: ~63,000% APY

**Reality check:** These are testnet tokens with no monetary value. Mainnet economics will be different.

### Slashing

Validators can be slashed for:
- **Equivocation:** Creating two blocks for the same round
- **Penalty:** 50% of stake burned, immediate removal from active set

**How to avoid:** Run only one validator instance per keypair.

---

## Community

- **Telegram:** @ultra_dag — Ask questions, share monitoring results
- **GitHub:** https://github.com/ultradag/core — Report issues, contribute code
- **Testnet Status:** https://testnet.ultradag.com — Live network stats

---

## Next Steps

1. **Run your validator** for at least 24 hours
2. **Monitor finality lag** — should stay at 1-2 rounds
3. **Join Telegram** — Introduce yourself at @ultra_dag
4. **Participate in governance** — Vote on proposals
5. **Report issues** — Help improve the network

Welcome to the UltraDAG validator community!
