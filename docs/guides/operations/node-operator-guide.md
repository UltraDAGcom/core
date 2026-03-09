# UltraDAG Node Operator Guide

**Version:** 1.0  
**Last Updated:** March 2026  
**Target Audience:** System administrators, DevOps engineers, node operators

---

## Table of Contents

1. [Overview](#overview)
2. [System Requirements](#system-requirements)
3. [Installation](#installation)
4. [Configuration](#configuration)
5. [Running a Node](#running-a-node)
6. [Monitoring & Observability](#monitoring--observability)
7. [Maintenance](#maintenance)
8. [Backup & Recovery](#backup--recovery)
9. [Security](#security)
10. [Troubleshooting](#troubleshooting)
11. [Performance Tuning](#performance-tuning)

---

## Overview

This guide covers everything needed to deploy, operate, and maintain an UltraDAG node in production. Whether you're running a validator or a full node for RPC access, this guide provides the operational knowledge required for reliable operation.

**Node Types:**
- **Validator Node** - Participates in consensus, produces vertices, earns rewards
- **Full Node** - Syncs DAG and state, provides RPC access, does not validate

---

## System Requirements

### Minimum Requirements (Full Node)

| Component | Specification |
|-----------|--------------|
| CPU | 2 cores (x86_64 or ARM64) |
| RAM | 2 GB |
| Disk | 20 GB SSD |
| Network | 10 Mbps symmetric |
| OS | Linux (Ubuntu 22.04+, Debian 11+) |

### Recommended Requirements (Validator Node)

| Component | Specification |
|-----------|--------------|
| CPU | 4 cores (x86_64) |
| RAM | 4 GB |
| Disk | 50 GB NVMe SSD |
| Network | 100 Mbps symmetric, <50ms latency to peers |
| OS | Linux (Ubuntu 22.04 LTS) |

### Network Requirements

**Ports:**
- `9333` - P2P network (TCP, must be publicly accessible)
- `10333` - RPC API (TCP, restrict to trusted networks)

**Firewall Rules:**
```bash
# Allow P2P
sudo ufw allow 9333/tcp

# Allow RPC (restrict to specific IPs in production)
sudo ufw allow from 10.0.0.0/8 to any port 10333
```

**DNS:**
- Optional but recommended for validators
- A record pointing to your node's public IP
- Helps peers discover and connect to your node

---

## Installation

### Option 1: Pre-built Binary (Recommended)

```bash
# Download latest release
wget https://github.com/UltraDAGcom/core/releases/download/v1.0.0/ultradag-node-linux-amd64

# Make executable
chmod +x ultradag-node-linux-amd64

# Move to system path
sudo mv ultradag-node-linux-amd64 /usr/local/bin/ultradag-node

# Verify installation
ultradag-node --version
```

### Option 2: Build from Source

**Prerequisites:**
```bash
# Install Rust (1.75+)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# Install build dependencies
sudo apt update
sudo apt install -y build-essential pkg-config libssl-dev
```

**Build:**
```bash
# Clone repository
git clone https://github.com/UltraDAGcom/core.git
cd core

# Build release binary
cargo build --release --bin ultradag-node

# Binary location
ls -lh target/release/ultradag-node

# Install to system
sudo cp target/release/ultradag-node /usr/local/bin/
```

### Option 3: Docker

```bash
# Pull image
docker pull ultradag/node:latest

# Run container
docker run -d \
  --name ultradag-node \
  -p 9333:9333 \
  -p 10333:10333 \
  -v /var/lib/ultradag:/data \
  ultradag/node:latest \
  --data-dir /data \
  --listen 0.0.0.0:9333 \
  --rpc-addr 0.0.0.0:10333
```

---

## Configuration

### Command-Line Flags

```bash
ultradag-node [OPTIONS]
```

**Essential Flags:**

| Flag | Description | Default |
|------|-------------|---------|
| `--data-dir <PATH>` | Data directory for state/DAG | `./ultradag-data` |
| `--listen <ADDR:PORT>` | P2P listen address | `0.0.0.0:9333` |
| `--rpc-addr <ADDR:PORT>` | RPC server address | `127.0.0.1:10333` |
| `--bootstrap <ADDR>` | Bootstrap peer address | None |
| `--validators <N>` | Fixed validator count | Auto-detect |
| `--round-ms <MS>` | Round duration in milliseconds | `5000` |

**Validator-Specific Flags:**

| Flag | Description | Default |
|------|-------------|---------|
| `--validator` | Enable validator mode | `false` |
| `--secret-key <HEX>` | Validator secret key (64 hex chars) | None |

**Advanced Flags:**

| Flag | Description | Default |
|------|-------------|---------|
| `--max-peers <N>` | Maximum peer connections | `8` |
| `--checkpoint-interval <N>` | Checkpoint every N rounds | `100` |
| `--log-level <LEVEL>` | Log level (error/warn/info/debug) | `info` |

### Configuration File

Create `/etc/ultradag/config.toml`:

```toml
[network]
listen_addr = "0.0.0.0:9333"
rpc_addr = "0.0.0.0:10333"
max_peers = 8
bootstrap_peers = [
    "node1.ultradag.io:9333",
    "node2.ultradag.io:9333",
    "node3.ultradag.io:9333"
]

[consensus]
round_duration_ms = 5000
validators = 4
checkpoint_interval = 100

[storage]
data_dir = "/var/lib/ultradag"

[logging]
level = "info"
```

Load config file:
```bash
ultradag-node --config /etc/ultradag/config.toml
```

### Environment Variables

```bash
export ULTRADAG_DATA_DIR=/var/lib/ultradag
export ULTRADAG_LISTEN_ADDR=0.0.0.0:9333
export ULTRADAG_RPC_ADDR=0.0.0.0:10333
export ULTRADAG_LOG_LEVEL=info
```

---

## Running a Node

### Full Node (Non-Validator)

**Manual Start:**
```bash
ultradag-node \
  --data-dir /var/lib/ultradag \
  --listen 0.0.0.0:9333 \
  --rpc-addr 0.0.0.0:10333 \
  --bootstrap node1.ultradag.io:9333 \
  --validators 4
```

**Systemd Service:**

Create `/etc/systemd/system/ultradag.service`:

```ini
[Unit]
Description=UltraDAG Full Node
After=network.target
Wants=network-online.target

[Service]
Type=simple
User=ultradag
Group=ultradag
WorkingDirectory=/var/lib/ultradag
ExecStart=/usr/local/bin/ultradag-node \
  --data-dir /var/lib/ultradag \
  --listen 0.0.0.0:9333 \
  --rpc-addr 0.0.0.0:10333 \
  --bootstrap node1.ultradag.io:9333 \
  --validators 4 \
  --log-level info

Restart=on-failure
RestartSec=10
LimitNOFILE=65535

StandardOutput=journal
StandardError=journal
SyslogIdentifier=ultradag

[Install]
WantedBy=multi-user.target
```

**Enable and start:**
```bash
# Create user
sudo useradd -r -s /bin/false ultradag
sudo mkdir -p /var/lib/ultradag
sudo chown ultradag:ultradag /var/lib/ultradag

# Enable service
sudo systemctl daemon-reload
sudo systemctl enable ultradag
sudo systemctl start ultradag

# Check status
sudo systemctl status ultradag
sudo journalctl -u ultradag -f
```

### Validator Node

**Generate Validator Keys:**
```bash
# Generate keypair (save output securely!)
curl http://localhost:10333/keygen

# Output:
# {
#   "secret_key": "abc123...",
#   "public_key": "def456...",
#   "address": "789xyz..."
# }
```

**⚠️ Security Warning:** Store the secret key securely. Anyone with access to this key can produce vertices as your validator.

**Start Validator:**
```bash
ultradag-node \
  --data-dir /var/lib/ultradag \
  --listen 0.0.0.0:9333 \
  --rpc-addr 127.0.0.1:10333 \
  --bootstrap node1.ultradag.io:9333 \
  --validators 4 \
  --validator \
  --secret-key abc123def456... \
  --round-ms 5000
```

**Systemd Service for Validator:**

```ini
[Unit]
Description=UltraDAG Validator Node
After=network.target
Wants=network-online.target

[Service]
Type=simple
User=ultradag
Group=ultradag
WorkingDirectory=/var/lib/ultradag

# Load secret key from environment file
EnvironmentFile=/etc/ultradag/validator.env

ExecStart=/usr/local/bin/ultradag-node \
  --data-dir /var/lib/ultradag \
  --listen 0.0.0.0:9333 \
  --rpc-addr 127.0.0.1:10333 \
  --bootstrap node1.ultradag.io:9333 \
  --validators 4 \
  --validator \
  --secret-key ${VALIDATOR_SECRET_KEY} \
  --round-ms 5000

Restart=on-failure
RestartSec=10
LimitNOFILE=65535

StandardOutput=journal
StandardError=journal
SyslogIdentifier=ultradag-validator

[Install]
WantedBy=multi-user.target
```

Create `/etc/ultradag/validator.env`:
```bash
VALIDATOR_SECRET_KEY=abc123def456...
```

Secure the environment file:
```bash
sudo chmod 600 /etc/ultradag/validator.env
sudo chown ultradag:ultradag /etc/ultradag/validator.env
```

---

## Monitoring & Observability

### Health Checks

**Simple Health Check:**
```bash
curl http://localhost:10333/health
# {"status":"ok"}
```

**Detailed Diagnostics:**
```bash
curl http://localhost:10333/health/detailed | jq .
```

**Automated Health Monitoring:**
```bash
#!/bin/bash
# /usr/local/bin/check-ultradag-health.sh

HEALTH=$(curl -s http://localhost:10333/health/detailed)
STATUS=$(echo $HEALTH | jq -r '.status')

if [ "$STATUS" != "healthy" ]; then
    echo "ALERT: Node status is $STATUS"
    echo $HEALTH | jq .
    # Send alert (email, Slack, PagerDuty, etc.)
fi
```

Add to crontab:
```bash
*/5 * * * * /usr/local/bin/check-ultradag-health.sh
```

### Prometheus Metrics

**Scrape Configuration:**

Add to `prometheus.yml`:
```yaml
scrape_configs:
  - job_name: 'ultradag'
    static_configs:
      - targets: ['localhost:10333']
    metrics_path: '/metrics'
    scrape_interval: 15s
```

**Key Metrics to Monitor:**

| Metric | Description | Alert Threshold |
|--------|-------------|-----------------|
| `finality_lag` | Rounds behind current | >10 (warning), >100 (critical) |
| `peer_count` | Connected peers | <1 (critical) |
| `checkpoint_age_seconds` | Time since last checkpoint | >600 (warning) |
| `checkpoint_persist_failures` | Failed checkpoint writes | >0 (warning) |
| `mempool_transaction_count` | Pending transactions | >5000 (warning) |

### Grafana Dashboard

**Import Dashboard:**

1. Open Grafana
2. Import dashboard from `docs/monitoring/grafana-dashboard.json`
3. Configure Prometheus data source

**Key Panels:**
- Node health status
- Finality lag over time
- Peer connections
- Checkpoint production rate
- Memory and CPU usage
- Network I/O

### Log Monitoring

**View Logs:**
```bash
# Systemd
sudo journalctl -u ultradag -f

# Docker
docker logs -f ultradag-node

# File-based
tail -f /var/log/ultradag/node.log
```

**Log Levels:**
- `ERROR` - Critical issues requiring immediate attention
- `WARN` - Potential issues, degraded performance
- `INFO` - Normal operational messages
- `DEBUG` - Detailed debugging information

**Important Log Patterns:**

```bash
# Finality progress
grep "finalized" /var/log/ultradag/node.log

# Checkpoint production
grep "checkpoint" /var/log/ultradag/node.log

# Peer connections
grep "peer" /var/log/ultradag/node.log

# Errors
grep "ERROR" /var/log/ultradag/node.log
```

---

## Maintenance

### Regular Tasks

**Daily:**
- Check health status
- Monitor finality lag
- Verify peer connections
- Review error logs

**Weekly:**
- Check disk usage
- Review performance metrics
- Update monitoring dashboards
- Test backup restoration

**Monthly:**
- Update node software
- Review security patches
- Audit access logs
- Test disaster recovery procedures

### Software Updates

**Check for Updates:**
```bash
# GitHub releases
curl -s https://api.github.com/repos/UltraDAGcom/core/releases/latest | jq -r '.tag_name'
```

**Update Procedure:**

1. **Download new binary:**
```bash
wget https://github.com/UltraDAGcom/core/releases/download/v1.1.0/ultradag-node-linux-amd64
chmod +x ultradag-node-linux-amd64
```

2. **Backup current binary:**
```bash
sudo cp /usr/local/bin/ultradag-node /usr/local/bin/ultradag-node.backup
```

3. **Stop node:**
```bash
sudo systemctl stop ultradag
```

4. **Replace binary:**
```bash
sudo mv ultradag-node-linux-amd64 /usr/local/bin/ultradag-node
```

5. **Start node:**
```bash
sudo systemctl start ultradag
```

6. **Verify:**
```bash
ultradag-node --version
sudo journalctl -u ultradag -n 50
curl http://localhost:10333/health/detailed | jq .
```

**Rollback if needed:**
```bash
sudo systemctl stop ultradag
sudo cp /usr/local/bin/ultradag-node.backup /usr/local/bin/ultradag-node
sudo systemctl start ultradag
```

### Disk Space Management

**Check Disk Usage:**
```bash
du -sh /var/lib/ultradag/*
```

**Expected Sizes:**
- DAG state: ~100 MB per 10,000 rounds
- Checkpoints: ~20 MB (10 checkpoints × 2 MB each)
- State snapshots: ~5 MB
- Logs: Varies (rotate regularly)

**Automatic Cleanup:**

Checkpoint pruning is automatic (keeps 10 most recent). For logs:

```bash
# Logrotate configuration
sudo tee /etc/logrotate.d/ultradag <<EOF
/var/log/ultradag/*.log {
    daily
    rotate 7
    compress
    delaycompress
    missingok
    notifempty
    create 0640 ultradag ultradag
    postrotate
        systemctl reload ultradag > /dev/null 2>&1 || true
    endscript
}
EOF
```

---

## Backup & Recovery

### What to Backup

**Critical Data:**
1. **Validator Secret Key** - `/etc/ultradag/validator.env`
2. **Configuration** - `/etc/ultradag/config.toml`

**Optional Data (can be re-synced):**
3. DAG state - `/var/lib/ultradag/dag.json`
4. State engine - `/var/lib/ultradag/state.json`
5. Checkpoints - `/var/lib/ultradag/checkpoint_*.json`

### Backup Procedures

**Manual Backup:**
```bash
#!/bin/bash
# /usr/local/bin/backup-ultradag.sh

BACKUP_DIR=/backup/ultradag/$(date +%Y%m%d-%H%M%S)
mkdir -p $BACKUP_DIR

# Stop node (optional, for consistency)
sudo systemctl stop ultradag

# Backup critical files
sudo cp /etc/ultradag/validator.env $BACKUP_DIR/
sudo cp /etc/ultradag/config.toml $BACKUP_DIR/
sudo cp -r /var/lib/ultradag $BACKUP_DIR/

# Restart node
sudo systemctl start ultradag

# Compress backup
tar -czf $BACKUP_DIR.tar.gz -C /backup/ultradag $(basename $BACKUP_DIR)
rm -rf $BACKUP_DIR

echo "Backup complete: $BACKUP_DIR.tar.gz"
```

**Automated Backup (Cron):**
```bash
# Daily backup at 2 AM
0 2 * * * /usr/local/bin/backup-ultradag.sh
```

### Recovery Procedures

**Restore from Backup:**
```bash
# Stop node
sudo systemctl stop ultradag

# Extract backup
tar -xzf /backup/ultradag/20260310-020000.tar.gz -C /tmp/

# Restore files
sudo cp /tmp/20260310-020000/validator.env /etc/ultradag/
sudo cp /tmp/20260310-020000/config.toml /etc/ultradag/
sudo cp -r /tmp/20260310-020000/ultradag/* /var/lib/ultradag/

# Fix permissions
sudo chown -R ultradag:ultradag /var/lib/ultradag

# Start node
sudo systemctl start ultradag
```

**Fast-Sync from Checkpoint:**

If you don't have a backup, use fast-sync to bootstrap from the network:

```bash
# Start node normally
# It will automatically request the latest checkpoint from peers
# Sync completes in 30-120 seconds

sudo systemctl start ultradag
sudo journalctl -u ultradag -f | grep "fast-sync"
```

---

## Security

### Secret Key Management

**Best Practices:**
1. Generate keys offline on air-gapped machine
2. Store secret key in encrypted vault (HashiCorp Vault, AWS Secrets Manager)
3. Never commit secret keys to version control
4. Use environment files with restricted permissions (600)
5. Rotate keys periodically (requires unstaking and re-staking)

**Key Rotation:**
```bash
# 1. Generate new keypair
NEW_KEY=$(curl http://localhost:10333/keygen)

# 2. Unstake with old key
# (wait for cooldown period: 2,016 rounds)

# 3. Update validator.env with new secret key
sudo nano /etc/ultradag/validator.env

# 4. Restart node
sudo systemctl restart ultradag

# 5. Stake with new key
```

### Network Security

**Firewall Configuration:**
```bash
# Default deny
sudo ufw default deny incoming
sudo ufw default allow outgoing

# Allow SSH (change port if needed)
sudo ufw allow 22/tcp

# Allow P2P
sudo ufw allow 9333/tcp

# Allow RPC only from specific IPs
sudo ufw allow from 10.0.0.0/8 to any port 10333

# Enable firewall
sudo ufw enable
```

**Reverse Proxy (nginx):**

```nginx
# /etc/nginx/sites-available/ultradag-rpc

upstream ultradag_rpc {
    server 127.0.0.1:10333;
}

server {
    listen 443 ssl http2;
    server_name rpc.yourdomain.com;

    ssl_certificate /etc/letsencrypt/live/yourdomain.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/yourdomain.com/privkey.pem;

    # Rate limiting
    limit_req_zone $binary_remote_addr zone=rpc_limit:10m rate=10r/s;
    limit_req zone=rpc_limit burst=20 nodelay;

    location / {
        proxy_pass http://ultradag_rpc;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    }
}
```

### Access Control

**RPC API Security:**
- Bind RPC to localhost (`127.0.0.1:10333`) by default
- Use reverse proxy with authentication for external access
- Implement IP whitelisting
- Use TLS/SSL for encrypted communication
- Monitor access logs for suspicious activity

**SSH Hardening:**
```bash
# Disable password authentication
sudo sed -i 's/#PasswordAuthentication yes/PasswordAuthentication no/' /etc/ssh/sshd_config

# Use SSH keys only
# Disable root login
sudo sed -i 's/PermitRootLogin yes/PermitRootLogin no/' /etc/ssh/sshd_config

# Restart SSH
sudo systemctl restart sshd
```

---

## Troubleshooting

### Node Won't Start

**Check logs:**
```bash
sudo journalctl -u ultradag -n 100
```

**Common Issues:**

1. **Port already in use:**
```
Error: Address already in use (os error 98)
```
Solution: Check for existing process, change port, or kill conflicting process.

2. **Permission denied:**
```
Error: Permission denied (os error 13)
```
Solution: Check file permissions, run as correct user.

3. **Missing data directory:**
```
Error: No such file or directory
```
Solution: Create data directory with correct permissions.

### Finality Lag High

**Symptoms:** Finality lag >10 rounds

**Diagnosis:**
```bash
curl http://localhost:10333/health/detailed | jq '.components.finality'
```

**Causes:**
1. **Network partition** - Check peer connections
2. **Slow peers** - Check network latency
3. **Insufficient validators** - Verify validator count
4. **Clock drift** - Synchronize system time

**Solutions:**
```bash
# Check peers
curl http://localhost:10333/peers | jq .

# Sync system time
sudo timedatectl set-ntp true
sudo systemctl restart systemd-timesyncd

# Restart node
sudo systemctl restart ultradag
```

### No Peer Connections

**Symptoms:** `peer_count: 0`

**Diagnosis:**
```bash
curl http://localhost:10333/peers | jq .
sudo netstat -tulpn | grep 9333
```

**Causes:**
1. **Firewall blocking** - Check UFW/iptables rules
2. **Wrong bootstrap peers** - Verify bootstrap addresses
3. **Network issues** - Check internet connectivity

**Solutions:**
```bash
# Check firewall
sudo ufw status

# Test connectivity to bootstrap peer
nc -zv node1.ultradag.io 9333

# Check node logs for connection attempts
sudo journalctl -u ultradag | grep "peer"
```

### High Memory Usage

**Symptoms:** Memory usage >500 MB

**Diagnosis:**
```bash
ps aux | grep ultradag-node
```

**Causes:**
1. **Large DAG** - Normal for long-running nodes
2. **Memory leak** - Check for increasing usage over time
3. **Orphan buffer full** - Check logs for orphan messages

**Solutions:**
```bash
# Restart node (clears memory)
sudo systemctl restart ultradag

# Monitor memory over time
watch -n 5 'ps aux | grep ultradag-node'

# If persistent, report issue on GitHub
```

### Checkpoint Sync Failing

**Symptoms:** `fast_sync_failures_total` increasing

**Diagnosis:**
```bash
curl http://localhost:10333/metrics/json | jq '.fast_sync'
```

**Causes:**
1. **No peers with checkpoints** - Wait for peers to connect
2. **Corrupted checkpoint** - Peers have invalid checkpoints
3. **Network issues** - Slow or unreliable connection

**Solutions:**
```bash
# Clear local state and re-sync
sudo systemctl stop ultradag
sudo rm -rf /var/lib/ultradag/*
sudo systemctl start ultradag

# Monitor sync progress
sudo journalctl -u ultradag -f | grep "checkpoint"
```

---

## Performance Tuning

### System Limits

**File Descriptors:**
```bash
# /etc/security/limits.conf
ultradag soft nofile 65535
ultradag hard nofile 65535
```

**Kernel Parameters:**
```bash
# /etc/sysctl.conf
net.core.somaxconn = 1024
net.ipv4.tcp_max_syn_backlog = 2048
net.ipv4.ip_local_port_range = 10000 65535
```

Apply:
```bash
sudo sysctl -p
```

### Round Duration Tuning

**Faster rounds (lower latency, higher throughput):**
```bash
--round-ms 2000  # 2 second rounds
```

**Slower rounds (more stable, lower resource usage):**
```bash
--round-ms 10000  # 10 second rounds
```

**Recommendation:** Start with 5000ms, adjust based on network conditions.

### Database Optimization

UltraDAG uses JSON persistence. For better performance:

1. **Use SSD/NVMe storage**
2. **Enable filesystem caching**
3. **Regular checkpoint pruning** (automatic)

---

## Additional Resources

- **Whitepaper:** [docs/reference/specifications/whitepaper.md](../../reference/specifications/whitepaper.md)
- **RPC API Reference:** [docs/reference/api/rpc-endpoints.md](../../reference/api/rpc-endpoints.md)
- **Operations Runbook:** [docs/operations/RUNBOOK.md](../../operations/RUNBOOK.md)
- **GitHub Issues:** https://github.com/UltraDAGcom/core/issues
- **Community Discord:** https://discord.gg/ultradag

---

## Support

For operational support:
- **GitHub Discussions:** https://github.com/UltraDAGcom/core/discussions
- **Emergency Contact:** ops@ultradag.io
- **Security Issues:** security@ultradag.io (PGP key available)

---

**Last Updated:** March 10, 2026  
**Document Version:** 1.0  
**Maintainer:** UltraDAG Core Team
