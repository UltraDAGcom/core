---
title: Monitoring
---

# Monitoring

UltraDAG provides built-in monitoring endpoints for health checks, Prometheus metrics, and JSON diagnostics. This guide covers setting up monitoring for production nodes.

---

## Health Endpoints

### Simple Health Check

```bash
curl http://localhost:10333/health
```

Returns HTTP 200 with `{"status": "ok"}` if the node is running. Suitable for Kubernetes liveness probes and simple uptime monitors.

### Detailed Health Check

```bash
curl http://localhost:10333/health/detailed
```

Returns component-level diagnostics:

```json
{
  "status": "healthy",
  "components": {
    "dag": {
      "status": "healthy",
      "round": 15420,
      "vertices": 77100
    },
    "finality": {
      "status": "healthy",
      "finalized_round": 15418,
      "lag": 2
    },
    "state": {
      "status": "healthy",
      "accounts": 142,
      "total_supply": 1050154200000000
    },
    "mempool": {
      "status": "healthy",
      "size": 7,
      "capacity": 10000
    },
    "network": {
      "status": "healthy",
      "peers": 4
    },
    "checkpoints": {
      "status": "healthy",
      "latest_round": 15400
    }
  }
}
```

**Status levels:**

| Status | Meaning |
|--------|---------|
| `healthy` | Component operating normally |
| `warning` | Component functional but degraded |
| `unhealthy` | Component has issues requiring attention |
| `degraded` | Component partially operational |

---

## Prometheus Metrics

### Endpoint

```bash
curl http://localhost:10333/metrics
```

Returns metrics in Prometheus exposition format:

```
# HELP ultradag_dag_round Current DAG round
# TYPE ultradag_dag_round gauge
ultradag_dag_round 15420

# HELP ultradag_finalized_round Last finalized round
# TYPE ultradag_finalized_round gauge
ultradag_finalized_round 15418

# HELP ultradag_finality_lag Rounds between DAG tip and last finalized
# TYPE ultradag_finality_lag gauge
ultradag_finality_lag 2

# HELP ultradag_peer_count Connected peers
# TYPE ultradag_peer_count gauge
ultradag_peer_count 4

# HELP ultradag_mempool_size Pending transactions in mempool
# TYPE ultradag_mempool_size gauge
ultradag_mempool_size 7

# HELP ultradag_total_supply Total UDAG supply in sats
# TYPE ultradag_total_supply gauge
ultradag_total_supply 1050154200000000

# HELP ultradag_total_staked Total staked UDAG in sats
# TYPE ultradag_total_staked gauge
ultradag_total_staked 50000000000000

# HELP ultradag_checkpoint_latest Latest checkpoint round
# TYPE ultradag_checkpoint_latest gauge
ultradag_checkpoint_latest 15400

# HELP ultradag_checkpoint_production_total Checkpoints produced
# TYPE ultradag_checkpoint_production_total counter
ultradag_checkpoint_production_total 154

# HELP ultradag_checkpoint_quorum_total Checkpoints reaching quorum
# TYPE ultradag_checkpoint_quorum_total counter
ultradag_checkpoint_quorum_total 152
```

### JSON Metrics

```bash
curl http://localhost:10333/metrics/json
```

Returns the same data as JSON, suitable for custom dashboards:

```json
{
  "dag_round": 15420,
  "finalized_round": 15418,
  "finality_lag": 2,
  "peer_count": 4,
  "mempool_size": 7,
  "total_supply": 1050154200000000,
  "total_staked": 50000000000000,
  "active_validators": 5,
  "checkpoint_latest": 15400,
  "uptime_seconds": 86400
}
```

---

## Key Metrics

### Critical Metrics

| Metric | Healthy | Warning | Critical |
|--------|---------|---------|----------|
| `finality_lag` | <= 3 | 4-10 | > 10 |
| `peer_count` | >= 3 | 2 | 0-1 |
| `mempool_size` | < 5000 | 5000-8000 | > 8000 |

### Supply Monitoring

| Metric | Description |
|--------|-------------|
| `total_supply` | Total UDAG minted (should never exceed 21M) |
| `total_staked` | Total UDAG locked in stakes |

!!! warning "Supply invariant"
    If `liquid + staked + delegated + treasury != total_supply`, the node will exit with code 101. This should never happen in normal operation.

---

## Grafana Dashboard Setup

### Prometheus Configuration

Add UltraDAG to your `prometheus.yml`:

```yaml title="prometheus.yml"
scrape_configs:
  - job_name: 'ultradag'
    scrape_interval: 15s
    static_configs:
      - targets:
        - 'ultradag-node-1:10333'
        - 'ultradag-node-2:10333'
        - 'ultradag-node-3:10333'
    metrics_path: '/metrics'
```

### Dashboard Panels

Recommended Grafana dashboard layout:

**Row 1: Overview**

- Finality lag (gauge, threshold colors)
- Peer count (gauge)
- DAG round (counter)
- Mempool size (gauge)

**Row 2: Economics**

- Total supply over time (graph)
- Total staked over time (graph)
- Active validators (gauge)

**Row 3: Checkpoints**

- Checkpoints produced (counter)
- Checkpoint quorum rate (percentage)
- Fast-sync operations (counter)

**Row 4: System**

- Node uptime (counter)
- Memory usage (gauge, if available from host metrics)

---

## Alert Thresholds

### Recommended Alerts

```yaml title="alert_rules.yml"
groups:
  - name: ultradag
    rules:
      - alert: HighFinalityLag
        expr: ultradag_finality_lag > 10
        for: 5m
        labels:
          severity: critical
        annotations:
          summary: "Finality lag is {{ $value }} rounds"

      - alert: LowPeerCount
        expr: ultradag_peer_count < 2
        for: 2m
        labels:
          severity: warning
        annotations:
          summary: "Only {{ $value }} peers connected"

      - alert: MempoolNearFull
        expr: ultradag_mempool_size > 8000
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "Mempool at {{ $value }}/10000"

      - alert: CheckpointStale
        expr: (ultradag_dag_round - ultradag_checkpoint_latest) > 500
        for: 10m
        labels:
          severity: warning
        annotations:
          summary: "No checkpoint in {{ $value }} rounds"
```

---

## Circuit Breaker Behavior

UltraDAG has two fatal exit conditions that trigger graceful shutdown:

| Exit Code | Condition | Meaning |
|-----------|-----------|---------|
| **100** | Finality rollback detected | A previously finalized round was un-finalized. Should never happen in normal operation. Indicates a critical consensus bug. |
| **101** | Supply invariant violated | `liquid + staked + delegated + treasury != total_supply`. Indicates a critical state engine bug. |

In both cases, the node:

1. Signals all components to stop
2. Saves all state to disk
3. Exits with the appropriate code

!!! danger "Do not auto-restart on code 100/101"
    These exit codes indicate a critical bug. Auto-restarting will reproduce the same error. Investigate the logs, report the issue, and wait for a fix before restarting.

Configure your process manager to distinguish these:

```ini title="systemd example"
[Service]
Restart=on-failure
# Don't restart on exit codes 100 or 101
RestartPreventExitStatus=100 101
```

---

## Log Levels

Control verbosity with `RUST_LOG`:

| Level | Use Case |
|-------|----------|
| `error` | Production (errors only) |
| `warn` | Production (errors + warnings) |
| `info` | Normal operation (recommended) |
| `debug` | Troubleshooting specific issues |
| `trace` | Deep debugging (very verbose, may affect performance) |

Fine-grained control:

```bash
RUST_LOG=ultradag_coin=debug,ultradag_network=info,ultradag_node=info
```

---

## Next Steps

- [Node Operator Guide](node-guide.md) — installation and configuration
- [Validator Handbook](validator-handbook.md) — validator-specific operations
- [CLI Reference](cli.md) — all configuration options
