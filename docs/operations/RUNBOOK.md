# UltraDAG Operations Runbook

**Version:** 1.0  
**Last Updated:** March 10, 2026  
**Maintainer:** UltraDAG Core Team

---

## Table of Contents

1. [Emergency Contacts](#emergency-contacts)
2. [Quick Reference](#quick-reference)
3. [Emergency Procedures](#emergency-procedures)
4. [Troubleshooting Guide](#troubleshooting-guide)
5. [Recovery Procedures](#recovery-procedures)
6. [Monitoring & Alerting](#monitoring--alerting)
7. [Deployment Procedures](#deployment-procedures)
8. [Rollback Procedures](#rollback-procedures)
9. [Performance Tuning](#performance-tuning)
10. [Security Incidents](#security-incidents)

---

## Emergency Contacts

### On-Call Rotation
- **Primary:** [Contact Info]
- **Secondary:** [Contact Info]
- **Escalation:** [Contact Info]

### Communication Channels
- **Slack:** #ultradag-incidents
- **Email:** incidents@ultradag.com
- **Phone:** [Emergency Hotline]

### External Resources
- **Cloud Provider Support:** [Contact Info]
- **Security Team:** [Contact Info]
- **Legal/Compliance:** [Contact Info]

---

## Quick Reference

### Critical Endpoints
```
Health Check:     GET https://node.ultradag.com/health
Detailed Health:  GET https://node.ultradag.com/health/detailed
Metrics:          GET https://node.ultradag.com/metrics
Status:           GET https://node.ultradag.com/status
```

### Critical Commands
```bash
# Check node health
curl https://node.ultradag.com/health/detailed | jq .

# View logs (last 100 lines)
docker logs ultradag-node --tail 100

# Restart node
docker restart ultradag-node

# Check disk usage
df -h

# Check memory usage
free -h

# View running processes
ps aux | grep ultradag
```

### Key Metrics to Monitor
- **Finality Lag:** Should be ≤ 10 rounds (warning if >10, critical if >100)
- **Peer Count:** Should be ≥ 3 (warning if <3, critical if 0)
- **Checkpoint Age:** Should be < 600 seconds (warning if >600)
- **Memory Usage:** Should be < 500MB (warning if >500MB)
- **Disk Usage:** Should be < 80% (warning if >80%, critical if >90%)

---

## Emergency Procedures

### 🚨 CRITICAL: Network Partition Detected

**Symptoms:**
- Peer count drops to 0
- No new vertices being produced
- Finality lag increasing rapidly

**Immediate Actions:**
1. **Check network connectivity**
   ```bash
   ping 8.8.8.8
   curl https://www.google.com
   ```

2. **Check peer connections**
   ```bash
   curl http://localhost:10333/peers | jq .
   ```

3. **Verify bootstrap nodes are reachable**
   ```bash
   # Test each bootstrap node
   nc -zv ultradag-node-1.fly.dev 9333
   nc -zv ultradag-node-2.fly.dev 9333
   nc -zv ultradag-node-3.fly.dev 9333
   ```

4. **Restart node if no peers after 5 minutes**
   ```bash
   docker restart ultradag-node
   # Wait 60 seconds
   curl http://localhost:10333/health/detailed | jq .components.network
   ```

5. **If still no peers, check firewall rules**
   ```bash
   # Ensure port 9333 (P2P) is open
   sudo ufw status
   sudo iptables -L
   ```

**Escalation:** If no peers after restart, contact on-call engineer immediately.

---

### 🚨 CRITICAL: High Finality Lag (>100 rounds)

**Symptoms:**
- Finality lag > 100 rounds
- Transactions not finalizing
- Health status shows "unhealthy"

**Root Causes:**
- Insufficient validator participation (< 2/3 online)
- Network partition
- Byzantine validator behavior
- Performance degradation

**Immediate Actions:**
1. **Check validator count and participation**
   ```bash
   curl http://localhost:10333/health/detailed | jq .components.finality
   ```

2. **Verify this node is producing vertices**
   ```bash
   # Check logs for "Produced vertex" messages
   docker logs ultradag-node --tail 50 | grep "Produced vertex"
   ```

3. **Check peer connectivity**
   ```bash
   curl http://localhost:10333/peers | jq .
   ```

4. **Monitor for 5 minutes**
   - If finality lag decreases: Continue monitoring
   - If finality lag stable or increasing: Proceed to step 5

5. **Check for Byzantine behavior**
   ```bash
   # Look for equivocation warnings
   docker logs ultradag-node | grep -i "equivocation\|byzantine\|invalid"
   ```

6. **Verify system resources**
   ```bash
   top -b -n 1 | head -20
   df -h
   free -h
   ```

**Recovery:**
- If < 2/3 validators online: Contact other validator operators
- If network partition: Follow network partition procedure
- If resource exhaustion: Follow performance degradation procedure

**Escalation:** If finality lag > 200 rounds for > 10 minutes, escalate immediately.

---

### 🚨 CRITICAL: Node Crash / Unresponsive

**Symptoms:**
- Health endpoint returns 503 or times out
- No response from RPC endpoints
- Process not running

**Immediate Actions:**
1. **Check if process is running**
   ```bash
   docker ps | grep ultradag-node
   # OR for binary deployment
   ps aux | grep ultradag-node
   ```

2. **Check recent logs for crash reason**
   ```bash
   docker logs ultradag-node --tail 200
   # Look for panic, segfault, OOM killer
   ```

3. **Check system resources**
   ```bash
   # Check for OOM killer
   dmesg | grep -i "out of memory\|oom"
   
   # Check disk space
   df -h
   
   # Check memory
   free -h
   ```

4. **Restart the node**
   ```bash
   # Docker deployment
   docker restart ultradag-node
   
   # Binary deployment
   sudo systemctl restart ultradag-node
   ```

5. **Verify recovery**
   ```bash
   # Wait 30 seconds, then check health
   sleep 30
   curl http://localhost:10333/health/detailed | jq .
   ```

6. **Monitor for stability**
   ```bash
   # Watch logs for errors
   docker logs -f ultradag-node
   ```

**Post-Recovery:**
- Document crash reason
- Check if data corruption occurred
- Review resource limits
- Consider increasing memory/disk if OOM

**Escalation:** If node crashes repeatedly (>3 times in 1 hour), escalate immediately.

---

### ⚠️ WARNING: High Memory Usage

**Symptoms:**
- Memory usage > 500MB
- Swap usage increasing
- Performance degradation

**Immediate Actions:**
1. **Check current memory usage**
   ```bash
   free -h
   docker stats ultradag-node --no-stream
   ```

2. **Check for memory leaks**
   ```bash
   # Monitor memory over time
   watch -n 5 'docker stats ultradag-node --no-stream'
   ```

3. **Check DAG size**
   ```bash
   curl http://localhost:10333/health/detailed | jq .components.dag
   ```

4. **Verify pruning is working**
   ```bash
   # Check pruning floor is advancing
   curl http://localhost:10333/status | jq .dag.pruning_floor
   ```

5. **If memory continues to grow, restart node**
   ```bash
   docker restart ultradag-node
   ```

**Prevention:**
- Ensure `--pruning-depth` is set (default: 1000)
- Monitor for memory leaks in new releases
- Set memory limits in Docker/systemd

---

### ⚠️ WARNING: Disk Space Low

**Symptoms:**
- Disk usage > 80%
- Checkpoint pruning not working
- Write errors in logs

**Immediate Actions:**
1. **Check disk usage**
   ```bash
   df -h
   du -sh /data/ultradag/*
   ```

2. **Check checkpoint count**
   ```bash
   ls -lh /data/ultradag/checkpoint_*.json | wc -l
   ```

3. **Verify checkpoint pruning is enabled**
   ```bash
   # Check logs for pruning messages
   docker logs ultradag-node | grep "Pruned.*checkpoints"
   ```

4. **Manual checkpoint cleanup (if needed)**
   ```bash
   # Keep only last 5 checkpoints
   cd /data/ultradag
   ls -t checkpoint_*.json | tail -n +6 | xargs rm -f
   ```

5. **Check for old log files**
   ```bash
   # Clean up old logs if using file logging
   find /var/log/ultradag -name "*.log" -mtime +7 -delete
   ```

6. **Check Docker volumes**
   ```bash
   docker system df
   docker system prune -a --volumes
   ```

**Prevention:**
- Set up disk usage monitoring
- Configure log rotation
- Ensure checkpoint pruning is working

---

## Troubleshooting Guide

### Slow Transaction Processing

**Symptoms:**
- Transactions stuck in mempool
- High mempool count (>1000)
- Users reporting slow confirmations

**Diagnosis:**
1. Check mempool size
   ```bash
   curl http://localhost:10333/mempool | jq 'length'
   ```

2. Check if node is producing vertices
   ```bash
   docker logs ultradag-node --tail 50 | grep "Produced vertex"
   ```

3. Check finality lag
   ```bash
   curl http://localhost:10333/health/detailed | jq .components.finality.finality_lag
   ```

**Solutions:**
- If finality lag high: See finality lag procedure
- If mempool full: Transactions will be processed as blocks are produced
- If node not producing: Check validator status and stake

---

### Checkpoint Sync Failures

**Symptoms:**
- New nodes fail to sync
- "CheckpointSync state_root mismatch" errors
- Fast-sync failures in metrics

**Diagnosis:**
1. Check checkpoint metrics
   ```bash
   curl http://localhost:10333/metrics | grep checkpoint
   ```

2. Check available checkpoints
   ```bash
   ls -lh /data/ultradag/checkpoint_*.json
   ```

3. Verify checkpoint signatures
   ```bash
   # Check logs for validation failures
   docker logs ultradag-node | grep -i "checkpoint.*invalid\|insufficient signatures"
   ```

**Solutions:**
- Ensure at least 2/3 validators are co-signing checkpoints
- Verify checkpoint files are not corrupted
- Check network connectivity between validators
- Restart node to retry checkpoint sync

---

### Peer Connection Issues

**Symptoms:**
- Low peer count (<3)
- Frequent peer disconnections
- "Peer disconnected" messages in logs

**Diagnosis:**
1. Check current peers
   ```bash
   curl http://localhost:10333/peers | jq .
   ```

2. Check for banned peers
   ```bash
   docker logs ultradag-node | grep -i "banned\|rejected"
   ```

3. Test bootstrap node connectivity
   ```bash
   nc -zv ultradag-node-1.fly.dev 9333
   ```

**Solutions:**
- Verify firewall allows port 9333 (P2P)
- Check bootstrap nodes in config
- Restart node to clear banned peer list
- Verify system time is synchronized (NTP)

---

### State Divergence

**Symptoms:**
- Different state roots between validators
- Checkpoint validation failures
- "state_root mismatch" errors

**Diagnosis:**
1. Compare state roots with other validators
   ```bash
   curl http://localhost:10333/status | jq .state.state_root
   ```

2. Check for equivocation
   ```bash
   docker logs ultradag-node | grep -i equivocation
   ```

3. Verify deterministic ordering
   ```bash
   # Check logs for vertex processing order
   docker logs ultradag-node | grep "apply_finalized_vertices"
   ```

**Solutions:**
- **CRITICAL:** State divergence indicates a consensus bug
- Stop the node immediately
- Preserve all logs and state data
- Contact core development team
- Do NOT restart without guidance

---

## Recovery Procedures

### Fast-Sync from Checkpoint

**When to Use:**
- Node fell behind by >1000 rounds
- Fresh node deployment
- After state corruption

**Procedure:**
1. Stop the node
   ```bash
   docker stop ultradag-node
   ```

2. Clear existing state (BACKUP FIRST!)
   ```bash
   # Backup current state
   tar -czf state-backup-$(date +%s).tar.gz /data/ultradag/state.json
   
   # Remove state
   rm /data/ultradag/state.json
   ```

3. Start node (will auto-request checkpoint)
   ```bash
   docker start ultradag-node
   ```

4. Monitor sync progress
   ```bash
   docker logs -f ultradag-node | grep -i "fast-sync\|checkpoint"
   ```

5. Verify sync completion
   ```bash
   curl http://localhost:10333/health/detailed | jq .components.network.sync_complete
   ```

**Expected Time:** 30-120 seconds depending on checkpoint size

---

### State Restoration from Backup

**When to Use:**
- State corruption detected
- After failed upgrade
- Data loss incident

**Procedure:**
1. Stop the node
   ```bash
   docker stop ultradag-node
   ```

2. Locate latest backup
   ```bash
   ls -lht /backups/ultradag/state-*.tar.gz | head -5
   ```

3. Restore state
   ```bash
   cd /data/ultradag
   tar -xzf /backups/ultradag/state-TIMESTAMP.tar.gz
   ```

4. Verify state integrity
   ```bash
   # Check file exists and is valid JSON
   jq . /data/ultradag/state.json > /dev/null
   ```

5. Start node
   ```bash
   docker start ultradag-node
   ```

6. Verify recovery
   ```bash
   curl http://localhost:10333/health/detailed | jq .
   ```

---

### Binary Rollback

**When to Use:**
- New version causes crashes
- Performance regression
- Consensus issues

**Procedure:**
1. Stop the node
   ```bash
   docker stop ultradag-node
   ```

2. Identify previous working version
   ```bash
   docker images | grep ultradag-node
   ```

3. Update deployment to use previous version
   ```bash
   # Docker Compose
   # Edit docker-compose.yml, change image tag
   vim docker-compose.yml
   # Change: image: ultradag/node:v0.9.0
   # To:     image: ultradag/node:v0.8.0
   
   docker-compose up -d
   ```

4. Verify rollback
   ```bash
   docker logs ultradag-node --tail 50
   curl http://localhost:10333/health/detailed | jq .
   ```

5. Document rollback reason
   ```bash
   echo "Rolled back from v0.9.0 to v0.8.0 due to [REASON]" >> /var/log/ultradag/rollbacks.log
   ```

---

## Monitoring & Alerting

### Prometheus Alert Rules

```yaml
groups:
  - name: ultradag_critical
    interval: 30s
    rules:
      - alert: NodeDown
        expr: up{job="ultradag"} == 0
        for: 1m
        labels:
          severity: critical
        annotations:
          summary: "UltraDAG node is down"
          description: "Node {{ $labels.instance }} has been down for 1 minute"

      - alert: HighFinalityLag
        expr: ultradag_finality_lag > 100
        for: 5m
        labels:
          severity: critical
        annotations:
          summary: "High finality lag detected"
          description: "Finality lag is {{ $value }} rounds (threshold: 100)"

      - alert: NoPeers
        expr: ultradag_peer_count == 0
        for: 2m
        labels:
          severity: critical
        annotations:
          summary: "No peer connections"
          description: "Node has 0 connected peers"

      - alert: CheckpointStale
        expr: ultradag_checkpoint_age_seconds > 600
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "Checkpoint is stale"
          description: "Last checkpoint is {{ $value }} seconds old"

      - alert: HighMemoryUsage
        expr: process_resident_memory_bytes > 500000000
        for: 10m
        labels:
          severity: warning
        annotations:
          summary: "High memory usage"
          description: "Memory usage is {{ $value | humanize }}B"

      - alert: DiskSpaceLow
        expr: node_filesystem_avail_bytes{mountpoint="/data"} / node_filesystem_size_bytes{mountpoint="/data"} < 0.2
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "Disk space low"
          description: "Only {{ $value | humanizePercentage }} disk space remaining"
```

### Health Check Monitoring

```bash
#!/bin/bash
# health-monitor.sh - Run every 30 seconds via cron

ENDPOINT="http://localhost:10333/health/detailed"
ALERT_WEBHOOK="https://hooks.slack.com/services/YOUR/WEBHOOK/URL"

response=$(curl -s "$ENDPOINT")
status=$(echo "$response" | jq -r .status)

if [ "$status" != "healthy" ]; then
    warnings=$(echo "$response" | jq -r '.warnings | join(", ")')
    
    # Send alert
    curl -X POST "$ALERT_WEBHOOK" \
        -H 'Content-Type: application/json' \
        -d "{\"text\": \"⚠️ UltraDAG Health Alert: Status=$status, Warnings: $warnings\"}"
fi
```

---

## Deployment Procedures

### Production Deployment Checklist

**Pre-Deployment:**
- [ ] Review changelog and breaking changes
- [ ] Test on staging environment
- [ ] Backup current state and configuration
- [ ] Notify team in #ultradag-ops channel
- [ ] Schedule maintenance window (if needed)
- [ ] Prepare rollback plan

**Deployment:**
1. Pull new version
   ```bash
   docker pull ultradag/node:v0.9.0
   ```

2. Stop current node
   ```bash
   docker stop ultradag-node
   ```

3. Backup state
   ```bash
   tar -czf state-backup-$(date +%s).tar.gz /data/ultradag/
   ```

4. Start new version
   ```bash
   docker-compose up -d
   ```

5. Monitor startup
   ```bash
   docker logs -f ultradag-node
   ```

6. Verify health
   ```bash
   curl http://localhost:10333/health/detailed | jq .
   ```

**Post-Deployment:**
- [ ] Monitor for 30 minutes
- [ ] Check metrics dashboard
- [ ] Verify peer connectivity
- [ ] Confirm finality lag is normal
- [ ] Document deployment in runbook
- [ ] Update team in #ultradag-ops

---

## Rollback Procedures

See [Binary Rollback](#binary-rollback) section above.

**Rollback Decision Criteria:**
- Node crashes within 5 minutes of deployment
- Finality lag > 100 rounds for > 10 minutes
- State divergence detected
- Critical bugs discovered
- Performance regression > 50%

---

## Performance Tuning

### Memory Optimization

```bash
# Set pruning depth (lower = less memory)
--pruning-depth 500

# Limit checkpoint retention
# (automatically set to 10 in code)
```

### Network Optimization

```bash
# Increase peer connections (if bandwidth allows)
# Default is dynamic based on bootstrap nodes

# Adjust round duration for faster finality
# (requires consensus - all validators must agree)
```

### Disk I/O Optimization

```bash
# Use SSD for data directory
# Enable filesystem caching
# Consider using tmpfs for high-frequency writes (with backup)
```

---

## Security Incidents

### Suspected Private Key Compromise

**Immediate Actions:**
1. **STOP THE NODE IMMEDIATELY**
   ```bash
   docker stop ultradag-node
   ```

2. **Rotate validator keys**
   - Generate new keypair offline
   - Update configuration with new keys
   - Do NOT restart node until keys rotated

3. **Notify team and users**
   - Post incident notice
   - Explain key rotation procedure
   - Provide timeline for resolution

4. **Investigate compromise**
   - Review access logs
   - Check for unauthorized transactions
   - Preserve evidence

5. **Post-incident review**
   - Document how compromise occurred
   - Implement additional security measures
   - Update security procedures

---

### DDoS Attack

**Symptoms:**
- High connection count
- RPC endpoints slow or unresponsive
- Bandwidth saturation

**Immediate Actions:**
1. Enable rate limiting (already built-in)
   ```bash
   # Rate limiting is automatic in RPC server
   # 100 requests per IP per minute
   ```

2. Check connection sources
   ```bash
   netstat -an | grep :10333 | awk '{print $5}' | cut -d: -f1 | sort | uniq -c | sort -rn
   ```

3. Block malicious IPs
   ```bash
   # Block specific IP
   sudo ufw deny from 1.2.3.4
   
   # Or use iptables
   sudo iptables -A INPUT -s 1.2.3.4 -j DROP
   ```

4. Enable cloud-level DDoS protection
   - Cloudflare
   - AWS Shield
   - Provider-specific DDoS mitigation

---

## Appendix

### Log Locations

```
Docker:           docker logs ultradag-node
Binary (systemd): /var/log/ultradag/node.log
Binary (manual):  ./ultradag-node.log
```

### Configuration Files

```
Docker Compose:   docker-compose.yml
Systemd:          /etc/systemd/system/ultradag-node.service
Config:           /etc/ultradag/config.toml
```

### Data Directories

```
State:            /data/ultradag/state.json
Checkpoints:      /data/ultradag/checkpoint_*.json
Backups:          /backups/ultradag/
```

### Useful Commands

```bash
# Get current round
curl http://localhost:10333/status | jq .dag.current_round

# Get finality lag
curl http://localhost:10333/health/detailed | jq .components.finality.finality_lag

# Get peer count
curl http://localhost:10333/peers | jq 'length'

# Get validator status
curl http://localhost:10333/validators | jq .

# Get mempool size
curl http://localhost:10333/mempool | jq 'length'

# Export metrics
curl http://localhost:10333/metrics > metrics.txt

# Check checkpoint count
ls /data/ultradag/checkpoint_*.json | wc -l
```

---

## Revision History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2026-03-10 | UltraDAG Team | Initial runbook creation |

---

**END OF RUNBOOK**

For questions or updates, contact: ops@ultradag.com
