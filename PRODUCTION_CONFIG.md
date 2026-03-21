# UltraDAG Production Configuration Guide

This document provides comprehensive configuration options for running UltraDAG nodes in production environments.

## Table of Contents

1. [Node Configuration](#node-configuration)
2. [Performance Tuning](#performance-tuning)
3. [Security Hardening](#security-hardening)
4. [Monitoring & Alerting](#monitoring--alerting)
5. [Backup & Recovery](#backup--recovery)

---

## Node Configuration

### Command-Line Flags

```bash
# Production validator node
ultradag-node \
  --port 9333 \
  --rpc-port 10333 \
  --validate \
  --round-ms 5000 \
  --validators 21 \
  --validator-key /secure/path/validators.txt \
  --data-dir /var/lib/ultradag \
  --pruning-depth 1000 \
  --seed ultradag-node-1.fly.dev:9333 \
  --seed ultradag-node-2.fly.dev:9333 \
  --testnet false
```

### Flag Reference

| Flag | Default | Production Value | Description |
|------|---------|------------------|-------------|
| `--port` | 9333 | 9333 | P2P listening port |
| `--rpc-port` | 10333 | 10333 | HTTP RPC port |
| `--validate` | true | true | Enable block production |
| `--round-ms` | 5000 | 5000 | Round duration (milliseconds) |
| `--validators` | None | 21 | Expected validator count |
| `--validator-key` | None | Required | Path to validator allowlist |
| `--data-dir` | ~/.ultradag | /var/lib/ultradag | Data directory |
| `--pruning-depth` | 1000 | 1000 | Rounds to retain |
| `--archive` | false | false | Disable pruning (explorers only) |
| `--skip-fast-sync` | false | false | Skip checkpoint sync |
| `--pkey` | None | Secure path | Validator private key |
| `--auto-stake` | None | 10000 | Auto-stake amount (UDAG) |
| `--testnet` | true | false | Mainnet mode |
| `--no-bootstrap` | false | false | Connect to public seeds |

### Environment Variables

```bash
# Mainnet: Required
export ULTRADAG_DEV_KEY="<64-char-hex-private-key>"

# Optional tuning
export ULTRADAG_LOG_LEVEL="info"  # trace, debug, info, warn, error
export ULTRADAG_METRICS_PORT="9090"  # Prometheus metrics port
export ULTRADAG_MAX_CONNECTIONS="100"  # Max P2P connections
```

---

## Performance Tuning

### System Requirements

| Component | Minimum | Recommended | Production |
|-----------|---------|-------------|------------|
| CPU | 4 cores | 8 cores | 16+ cores |
| RAM | 8 GB | 16 GB | 32+ GB |
| Disk | 100 GB SSD | 500 GB NVMe | 1+ TB NVMe |
| Network | 100 Mbps | 1 Gbps | 10 Gbps |

### Kernel Tuning (Linux)

```bash
# /etc/sysctl.d/99-ultradag.conf

# Increase file descriptor limit
fs.file-max = 2097152

# Increase network buffers
net.core.rmem_max = 16777216
net.core.wmem_max = 16777216
net.ipv4.tcp_rmem = 4096 87380 16777216
net.ipv4.tcp_wmem = 4096 65536 16777216

# Increase connection queue
net.core.somaxconn = 65535
net.ipv4.tcp_max_syn_backlog = 65535

# Enable TCP fast open
net.ipv4.tcp_fastopen = 3

# Apply settings
sudo sysctl -p /etc/sysctl.d/99-ultradag.conf
```

### ulimits Configuration

```bash
# /etc/security/limits.d/ultradag.conf

ultradag soft nofile 1048576
ultradag hard nofile 1048576
ultradag soft nproc 65535
ultradag hard nproc 65535
ultradag soft memlock unlimited
ultradag hard memlock unlimited
```

### Rust Runtime Tuning

```bash
# Optimize for production
export RUST_BACKTRACE=0
export RUST_LOG=ultradag=info
export MALLOC_CONF="thp:always,metadata_thp:always"
```

---

## Security Hardening

### Firewall Configuration

```bash
# UFW (Ubuntu)
sudo ufw default deny incoming
sudo ufw default allow outgoing

# Allow P2P
sudo ufw allow 9333/tcp comment "UltraDAG P2P"

# Allow RPC (restrict to trusted IPs)
sudo ufw allow from 10.0.0.0/8 to any port 10333 proto tcp comment "UltraDAG RPC internal"

# Allow SSH (restrict to admin IPs)
sudo ufw allow from <admin-ip> to any port 22 proto tcp comment "SSH admin"

sudo ufw enable
```

### Systemd Service

```ini
# /etc/systemd/system/ultradag.service

[Unit]
Description=UltraDAG Node
After=network.target
Wants=network-online.target

[Service]
Type=simple
User=ultradag
Group=ultradag
ExecStart=/usr/local/bin/ultradag-node \
  --port 9333 \
  --validate \
  --data-dir /var/lib/ultradag \
  --validator-key /etc/ultradag/validators.txt

# Security hardening
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=true
PrivateTmp=true
ReadWritePaths=/var/lib/ultradag
CapabilityBoundingSet=CAP_NET_BIND_SERVICE
RestrictAddressFamilies=AF_INET AF_INET6 AF_UNIX
RestrictNamespaces=true
RestrictRealtime=true
RestrictSUIDSGID=true
MemoryDenyWriteExecute=true
LockPersonality=true

# Resource limits
LimitNOFILE=1048576
LimitNPROC=65535

# Restart policy
Restart=always
RestartSec=10

# Environment
Environment="RUST_LOG=ultradag=info"
Environment="ULTRADAG_DEV_KEY="

[Install]
WantedBy=multi-user.target
```

### Key Management

```bash
# Generate key offline (air-gapped machine)
ultradag-node --generate-key > /dev/null
# Copy the 64-char hex key securely

# Store in hardware wallet (recommended)
# - Ledger Nano X
# - Trezor Model T

# Or use encrypted storage
echo "<private-key>" | age -R <recipient-public-key> > key.age
```

---

## Monitoring & Alerting

### Prometheus Metrics

```prometheus
# Available at http://localhost:10333/metrics

# Consensus
ultradag_current_round       # Current DAG round
ultradag_vertex_count        # Total vertices
ultradag_pruning_floor       # Pruning floor round
ultradag_finality_lag        # Rounds behind finality

# Validators
ultradag_validator_count     # Registered validators
ultradag_active_validators   # Active validators

# State
ultradag_total_supply        # Total supply (sats)
ultradag_account_count       # Account count
ultradag_total_staked        # Total staked (sats)

# Network
ultradag_mempool_size        # Transactions in mempool
ultradag_peer_count          # Connected peers
ultradag_banned_ips          # Banned IP addresses
```

### Grafana Dashboard

Import dashboard ID: `TODO` (create and publish)

### Alerting Rules

```yaml
# prometheus/alerts.yml

groups:
  - name: ultradag
    rules:
      # Critical: Finality stalled
      - alert: UltraDAGFinalityStalled
        expr: ultradag_finality_lag > 100
        for: 5m
        labels:
          severity: critical
        annotations:
          summary: "UltraDAG finality stalled"
          description: "Finality lag is {{ $value }} rounds for 5+ minutes"

      # Warning: Low peer count
      - alert: UltraDAGLowPeers
        expr: ultradag_peer_count < 4
        for: 10m
        labels:
          severity: warning
        annotations:
          summary: "UltraDAG low peer count"
          description: "Only {{ $value }} peers connected"

      # Warning: High mempool
      - alert: UltraDAGMempoolFull
        expr: ultradag_mempool_size > 9000
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "UltraDAG mempool near capacity"
          description: "Mempool is {{ $value }}% full"

      # Critical: Supply invariant
      - alert: UltraDAGSupplyInvariant
        expr: changes(ultradag_total_supply[5m]) > 100
        for: 1m
        labels:
          severity: critical
        annotations:
          summary: "UltraDAG supply changing rapidly"
          description: "Supply changed significantly in 5 minutes"
```

### Log Aggregation

```bash
# Install Loki + Promtail
# Configure promtail to scrape:
# - /var/log/ultradag/*.log
# - journalctl -u ultradag
```

---

## Backup & Recovery

### Automated Backups

```bash
#!/bin/bash
# /usr/local/bin/ultradag-backup.sh

set -e

DATA_DIR="/var/lib/ultradag"
BACKUP_DIR="/backup/ultradag"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)

# Create backup directory
mkdir -p "$BACKUP_DIR"

# Stop node gracefully
systemctl stop ultradag

# Create compressed backup
tar -czf "$BACKUP_DIR/ultradag_$TIMESTAMP.tar.gz" \
  -C "$DATA_DIR" \
  dag.bin \
  finality.bin \
  state.redb \
  validator.key

# Verify backup integrity
tar -tzf "$BACKUP_DIR/ultradag_$TIMESTAMP.tar.gz" > /dev/null

# Restart node
systemctl start ultradag

# Cleanup old backups (keep 7 days)
find "$BACKUP_DIR" -name "ultradag_*.tar.gz" -mtime +7 -delete

echo "Backup completed: ultradag_$TIMESTAMP.tar.gz"
```

### Cron Schedule

```bash
# /etc/cron.d/ultradag-backup
0 3 * * * root /usr/local/bin/ultradag-backup.sh >> /var/log/ultradag-backup.log 2>&1
```

### Disaster Recovery

1. **Node Failure**
   ```bash
   # Restore from backup
   systemctl stop ultradag
   tar -xzf /backup/ultradag/ultradag_YYYYMMDD_HHMMSS.tar.gz -C /var/lib/ultradag
   systemctl start ultradag
   ```

2. **State Corruption**
   ```bash
   # Fast-sync from checkpoint
   systemctl stop ultradag
   rm /var/lib/ultradag/{dag.bin,finality.bin,state.redb}
   systemctl start ultradag
   # Node will fast-sync from network checkpoints
   ```

3. **Key Loss**
   ```bash
   # Restore from hardware wallet backup
   # Or use recovery phrase if configured
   # Contact other validators for checkpoint verification
   ```

---

## Production Checklist

### Pre-Launch

- [ ] Generate keys offline
- [ ] Configure firewall rules
- [ ] Set up monitoring (Prometheus + Grafana)
- [ ] Configure alerting (PagerDuty/Slack)
- [ ] Test backup/restore procedure
- [ ] Document runbooks
- [ ] Configure log aggregation
- [ ] Set up metrics dashboard
- [ ] Test failover procedure
- [ ] Verify network connectivity

### Post-Launch

- [ ] Monitor finality lag (<10 rounds)
- [ ] Monitor peer count (>4 peers)
- [ ] Monitor mempool size (<90% capacity)
- [ ] Review logs daily
- [ ] Verify backups completing
- [ ] Test restore procedure monthly
- [ ] Rotate keys annually
- [ ] Update software quarterly

---

## Support

- **Documentation**: https://ultradag.com/docs
- **GitHub**: https://github.com/UltraDAGcom/core
- **Discord**: https://discord.gg/ultradag
- **Email**: security@ultradag.com
