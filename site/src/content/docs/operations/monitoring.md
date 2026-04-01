---
title: "Monitoring"
description: "Health checks, Prometheus metrics, Grafana dashboards, and alerting for UltraDAG nodes"
order: 3
section: "operations"
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

Returns checkpoint metrics in Prometheus exposition format. The `/metrics` endpoint currently exports checkpoint-related metrics:

```
# HELP checkpoint_produced_total Total number of checkpoints produced by this node
# TYPE checkpoint_produced_total counter
checkpoint_produced_total 154

# HELP checkpoint_production_duration_ms Duration of last checkpoint production in milliseconds
# TYPE checkpoint_production_duration_ms gauge
checkpoint_production_duration_ms 42

# HELP checkpoint_size_bytes Size of last checkpoint in bytes
# TYPE checkpoint_size_bytes gauge
checkpoint_size_bytes 5000

# HELP checkpoint_cosigned_total Total number of checkpoints co-signed by this node
# TYPE checkpoint_cosigned_total counter
checkpoint_cosigned_total 300

# HELP checkpoint_quorum_reached_total Total number of checkpoints that reached quorum
# TYPE checkpoint_quorum_reached_total counter
checkpoint_quorum_reached_total 152

# HELP checkpoint_validation_failures_total Total number of checkpoint validation failures
# TYPE checkpoint_validation_failures_total counter
checkpoint_validation_failures_total 0

# HELP fast_sync_success_total Total number of successful fast-syncs
# TYPE fast_sync_success_total counter
fast_sync_success_total 1

# HELP checkpoint_persist_success_total Total number of successful checkpoint persists
# TYPE checkpoint_persist_success_total counter
checkpoint_persist_success_total 154
```

<div class="callout callout-note"><div class="callout-title">DAG and state metrics</div>For DAG round, finality lag, peer count, mempool size, and supply metrics, use the <code>/status</code> endpoint (JSON) or <code>/health/detailed</code> endpoint. These are not currently exported in Prometheus format but can be scraped via a custom exporter wrapping <code>/status</code>.</div>

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

<div class="callout callout-warning"><div class="callout-title">Supply invariant</div>If <code>liquid + staked + delegated + treasury != total_supply</code>, the node will exit with code 101. This should never happen in normal operation.</div>

---

## Grafana Dashboard Setup

### Prometheus Configuration

Add UltraDAG to your `prometheus.yml`:

```yaml
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

<div class="callout callout-note"><div class="callout-title">Custom exporter required</div>The alert rules below assume a custom Prometheus exporter that scrapes the <code>/status</code> JSON endpoint and re-exports the values as Prometheus gauges with an <code>ultradag_</code> prefix. The built-in <code>/metrics</code> endpoint only exports checkpoint metrics.</div>

```yaml
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

<div class="callout callout-danger"><div class="callout-title">Do not auto-restart on code 100/101</div>These exit codes indicate a critical bug. Auto-restarting will reproduce the same error. Investigate the logs, report the issue, and wait for a fix before restarting.</div>

Configure your process manager to distinguish these:

```ini
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

- [Node Operator Guide](/docs/operations/node-guide) -- installation and configuration
- [Validator Handbook](/docs/operations/validator-handbook) -- validator-specific operations
- [CLI Reference](/docs/operations/cli) -- all configuration options
