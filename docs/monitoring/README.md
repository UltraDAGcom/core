# UltraDAG Monitoring with Grafana

This directory contains Grafana dashboard templates for monitoring UltraDAG nodes.

---

## Quick Start

### 1. Install Prometheus

```bash
# Ubuntu/Debian
sudo apt update
sudo apt install prometheus

# macOS
brew install prometheus
```

### 2. Configure Prometheus

Edit `/etc/prometheus/prometheus.yml`:

```yaml
global:
  scrape_interval: 15s
  evaluation_interval: 15s

scrape_configs:
  - job_name: 'ultradag'
    static_configs:
      - targets: ['localhost:10333']
    metrics_path: '/metrics'
    scrape_interval: 15s
```

Start Prometheus:
```bash
sudo systemctl start prometheus
sudo systemctl enable prometheus
```

### 3. Install Grafana

```bash
# Ubuntu/Debian
sudo apt install -y software-properties-common
sudo add-apt-repository "deb https://packages.grafana.com/oss/deb stable main"
wget -q -O - https://packages.grafana.com/gpg.key | sudo apt-key add -
sudo apt update
sudo apt install grafana

# macOS
brew install grafana
```

Start Grafana:
```bash
sudo systemctl start grafana-server
sudo systemctl enable grafana-server
```

Access Grafana at: `http://localhost:3000` (default credentials: admin/admin)

### 4. Add Prometheus Data Source

1. Login to Grafana
2. Go to Configuration → Data Sources
3. Click "Add data source"
4. Select "Prometheus"
5. Set URL to `http://localhost:9090`
6. Click "Save & Test"

### 5. Import Dashboard

1. Go to Dashboards → Import
2. Upload `grafana-dashboard.json`
3. Select Prometheus data source
4. Click "Import"

---

## Dashboard Panels

### Overview Panels (Row 1)

**Node Health Status**
- Shows if node is up or down
- Green = UP, Red = DOWN
- Alert if down for >1 minute

**Finality Lag**
- Current finality lag in rounds
- Green: 0-9, Yellow: 10-99, Red: 100+
- Alert if >10 for 5 minutes

**Peer Connections**
- Number of connected peers
- Green: 3+, Yellow: 1-2, Red: 0
- Alert if 0 for 2 minutes

**Current Round**
- Current DAG round number
- Shows node progress

### DAG Metrics (Row 2)

**Finality Lag Over Time**
- Time series graph of finality lag
- Alert configured for sustained high lag

**DAG Vertex Count**
- Total vertices in DAG
- Finalized vertices count
- Shows consensus progress

### Checkpoint Metrics (Rows 3-5)

**Checkpoint Production**
- Production rate (checkpoints/minute)
- Production duration (milliseconds)

**Checkpoint Co-signing**
- Co-signing rate
- Quorum achievement rate
- Validation failures

**Fast-Sync Operations**
- Sync attempts, successes, failures
- Shows new node onboarding

**Fast-Sync Performance**
- Sync duration
- Download speed

**Checkpoint Storage**
- Persist/load success/failure counts
- Storage reliability

**Checkpoint Health**
- Last checkpoint round
- Checkpoint age (seconds)
- Pending checkpoints
- Alert if checkpoint >10 minutes old

**Checkpoint Pruning**
- Pruning rate
- Disk checkpoint count (should be ~10)

### Network Metrics (Row 6)

**Mempool Size**
- Pending transaction count
- Alert if >5000 transactions

### System Resources (Row 7)

**CPU Usage**
- Process CPU utilization
- Should be <50% average

**Memory Usage**
- Process memory in MB
- Alert if >500 MB for 10 minutes

**Network I/O**
- RX/TX bytes per second
- Network bandwidth usage

---

## Alerts

The dashboard includes pre-configured alerts:

| Alert | Condition | Severity |
|-------|-----------|----------|
| High Finality Lag | >10 rounds for 5 min | Warning |
| Stale Checkpoint | >600 seconds old | Warning |
| High Mempool Size | >5000 txs for 5 min | Warning |
| High Memory Usage | >500 MB for 10 min | Warning |

### Configure Alert Notifications

1. Go to Alerting → Notification channels
2. Add channel (Email, Slack, PagerDuty, etc.)
3. Edit dashboard alerts to use notification channel

**Example Slack Notification:**
```json
{
  "channel": "#ultradag-alerts",
  "username": "Grafana",
  "icon_emoji": ":chart_with_upwards_trend:"
}
```

---

## Metrics Reference

### Node Metrics

| Metric | Description | Type |
|--------|-------------|------|
| `up` | Node up/down status | Gauge |
| `ultradag_current_round` | Current DAG round | Gauge |
| `ultradag_finalized_round` | Last finalized round | Gauge |
| `ultradag_finality_lag` | Rounds behind current | Gauge |
| `ultradag_vertex_count` | Total vertices | Counter |
| `ultradag_peer_count` | Connected peers | Gauge |
| `ultradag_mempool_transaction_count` | Pending txs | Gauge |

### Checkpoint Metrics

| Metric | Description | Type |
|--------|-------------|------|
| `checkpoint_produced_total` | Checkpoints produced | Counter |
| `checkpoint_production_duration_ms` | Production time | Gauge |
| `checkpoints_cosigned_total` | Checkpoints co-signed | Counter |
| `checkpoint_quorum_reached_total` | Quorums reached | Counter |
| `fast_sync_attempts_total` | Fast-sync attempts | Counter |
| `fast_sync_success_total` | Successful syncs | Counter |
| `fast_sync_duration_ms` | Sync duration | Gauge |
| `checkpoint_persist_success` | Persist successes | Counter |
| `checkpoint_persist_failures` | Persist failures | Counter |
| `checkpoint_last_round` | Last checkpoint round | Gauge |
| `checkpoint_age_seconds` | Checkpoint age | Gauge |
| `checkpoints_pruned_total` | Pruned checkpoints | Counter |
| `checkpoint_disk_count` | Checkpoints on disk | Gauge |

### System Metrics

| Metric | Description | Type |
|--------|-------------|------|
| `process_cpu_seconds_total` | CPU time | Counter |
| `process_resident_memory_bytes` | Memory usage | Gauge |
| `node_network_receive_bytes_total` | Network RX | Counter |
| `node_network_transmit_bytes_total` | Network TX | Counter |

---

## Customization

### Add Custom Panel

1. Click "Add panel" in dashboard
2. Select visualization type
3. Configure query:
   ```promql
   rate(checkpoint_produced_total[5m])
   ```
4. Set thresholds and alerts
5. Save dashboard

### Modify Refresh Rate

1. Dashboard settings (gear icon)
2. Change "Refresh" value (default: 10s)
3. Options: 5s, 10s, 30s, 1m, 5m

### Export Dashboard

1. Dashboard settings → JSON Model
2. Copy JSON
3. Save to file
4. Share with team

---

## Troubleshooting

### No Data Showing

**Check Prometheus is scraping:**
```bash
curl http://localhost:9090/api/v1/targets
```

**Check UltraDAG metrics endpoint:**
```bash
curl http://localhost:10333/metrics
```

**Check Prometheus logs:**
```bash
sudo journalctl -u prometheus -f
```

### Alerts Not Firing

1. Check alert conditions in panel edit
2. Verify notification channel configured
3. Check Grafana alert history
4. Review Prometheus alert rules

### High Resource Usage

**Reduce scrape frequency:**
```yaml
scrape_interval: 30s  # Instead of 15s
```

**Reduce retention:**
```yaml
storage:
  tsdb:
    retention.time: 7d  # Instead of 15d
```

---

## Production Recommendations

### High Availability

**Multiple Prometheus Instances:**
```yaml
# prometheus-1.yml
global:
  external_labels:
    replica: 1

# prometheus-2.yml
global:
  external_labels:
    replica: 2
```

**Grafana HA:**
- Use shared database (MySQL/PostgreSQL)
- Load balancer in front
- Shared storage for dashboards

### Long-term Storage

**Use Thanos or Cortex:**
```bash
# Thanos sidecar
thanos sidecar \
  --prometheus.url=http://localhost:9090 \
  --objstore.config-file=bucket.yml
```

### Security

**Enable Authentication:**
```ini
# /etc/grafana/grafana.ini
[auth]
disable_login_form = false

[security]
admin_user = admin
admin_password = <strong-password>
```

**Use HTTPS:**
```ini
[server]
protocol = https
cert_file = /etc/grafana/cert.pem
cert_key = /etc/grafana/key.pem
```

---

## Additional Resources

- [Prometheus Documentation](https://prometheus.io/docs/)
- [Grafana Documentation](https://grafana.com/docs/)
- [PromQL Guide](https://prometheus.io/docs/prometheus/latest/querying/basics/)
- [UltraDAG Metrics Reference](../reference/api/rpc-endpoints.md#metrics--monitoring)

---

**Last Updated:** March 10, 2026  
**Dashboard Version:** 1.0  
**Maintainer:** UltraDAG Core Team
