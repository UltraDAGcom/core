# UltraDAG Bridge - Relayer Operator Setup Guide

Complete guide for recruiting, setting up, and operating the 5 relayer operators for the UltraDAG bridge.

---

## 📋 Overview

### What Do Relayers Do?

Relayers are trusted operators that:
1. **Monitor both chains** (Arbitrum + UltraDAG native)
2. **Observe bridge events** (deposits, withdrawals)
3. **Sign attestations** for bridge transfers
4. **Submit signatures** to complete bridge operations

### Why 5 Relayers?

- **3-of-5 multi-sig threshold** provides security + redundancy
- **Tolerates 2 failures** while still operating
- **Geographic distribution** prevents single point of failure
- **Independent operators** prevents collusion

---

## 👥 Recruiting Relayer Operators

### Ideal Candidates

| Candidate Type | Pros | Cons |
|---------------|------|------|
| **Team members** | Trusted, aligned incentives | Centralization risk |
| **Validators** | Already invested in network | May have competing priorities |
| **Infrastructure providers** | Professional, reliable | Cost |
| **Community members** | Decentralized | Variable reliability |
| **Partner projects** | Aligned incentives | Coordination overhead |

### Recommended Mix

```
2x Core team members (high trust)
2x Validator operators (network stakeholders)
1x Infrastructure partner (professional ops)
```

### Vetting Criteria

- [ ] **Technical competence** - Can operate node infrastructure
- [ ] **Reliability** - 99.9%+ uptime track record
- [ ] **Security practices** - Key management, access control
- [ ] **Communication** - Responsive on Discord/Telegram
- [ ] **Geographic diversity** - Different regions/timezones
- [ ] **Infrastructure diversity** - Different cloud providers

---

## 🔧 Technical Setup (Per Relayer)

### Prerequisites

Each relayer operator needs:

1. **Server** (minimum specs):
   - 4 CPU cores
   - 8 GB RAM
   - 100 GB SSD
   - 1 Gbps network

2. **Software**:
   - Node.js 18+
   - Docker (optional)
   - Git

3. **Access**:
   - Arbitrum RPC endpoint
   - UltraDAG node RPC endpoint
   - Relayer private key (provided by governor)

### Step 1: Clone Repository

```bash
git clone https://github.com/UltraDAGcom/core.git
cd core/bridge
```

### Step 2: Install Dependencies

```bash
# If using Node.js relayer
npm install

# Or if using Docker
docker-compose pull
```

### Step 3: Configure Environment

```bash
# Copy environment template
cp .env.example .env

# Edit with your values
nano .env
```

### Step 4: Configure Private Key

**SECURITY: Never commit private keys!**

Option A - Environment variable (development):
```bash
export RELAYER_PRIVATE_KEY=0x...
```

Option B - AWS Secrets Manager (production):
```bash
aws secretsmanager create-secret \
  --name ultradag-relayer-key \
  --secret-string "0x..."
```

Option C - HashiCorp Vault (production):
```bash
vault kv put secret/relayer key=0x...
```

### Step 5: Start Relayer

```bash
# Development
npm run relayer

# Production (with PM2)
pm2 start ecosystem.config.js

# Production (with Docker)
docker-compose up -d
```

### Step 6: Verify Operation

```bash
# Check health endpoint
curl http://localhost:3000/health

# Expected response:
{
  "status": "healthy",
  "relayer": "0x...",
  "arbitrumBlock": 123456789,
  "ultradagBlock": 987654321,
  "pendingSignatures": 0,
  "completedToday": 0
}
```

---

## 🔐 Security Best Practices

### Private Key Management

**NEVER:**
- ❌ Commit keys to git
- ❌ Store in plain text files
- ❌ Share via email/chat
- ❌ Use same key across multiple relayers

**ALWAYS:**
- ✅ Use secrets manager (AWS/GCP/Vault)
- ✅ Encrypt keys at rest
- ✅ Rotate keys every 90 days
- ✅ Use separate keys per relayer

### Server Security

```bash
# Enable firewall
ufw enable
ufw allow 22/tcp  # SSH
ufw allow 3000/tcp  # Health check (internal only)

# Disable root login
echo "PermitRootLogin no" >> /etc/ssh/sshd_config
systemctl restart sshd

# Set up fail2ban
apt install fail2ban
systemctl enable fail2ban

# Enable automatic security updates
apt install unattended-upgrades
dpkg-reconfigure unattended-upgrades
```

### Access Control

| Role | Access |
|------|--------|
| Relayer operator | Server SSH, relayer logs |
| Governor | Bridge admin functions |
| Monitor | Health endpoint, metrics |
| Public | Bridge contract (read-only) |

---

## 📊 Monitoring Setup

### Health Check Endpoint

```bash
# Configure monitoring to check every 30 seconds
curl http://localhost:3000/health

# Alert if status != "healthy"
# Alert if uptime < 99.9%
```

### Prometheus Metrics

```yaml
# prometheus.yml
scrape_configs:
  - job_name: 'ultradag-relayer'
    static_configs:
      - targets: ['relayer1:3000', 'relayer2:3000', ...]
    metrics_path: '/metrics'
    scrape_interval: 30s
```

### Key Metrics to Monitor

| Metric | Alert Threshold |
|--------|-----------------|
| `relayer_uptime_seconds` | < 99.9% per day |
| `relayer_pending_signatures` | > 10 for 5 min |
| `relayer_arbitrum_block_height` | Stale > 5 min |
| `relayer_ultradag_block_height` | Stale > 5 min |
| `relayer_completed_total` | 0 for 1 hour |

### Alerting Configuration

```yaml
# alertmanager.yml
groups:
  - name: relayer
    rules:
      - alert: RelayerDown
        expr: up == 0
        for: 5m
        annotations:
          summary: "Relayer {{ $labels.instance }} is down"
          
      - alert: RelayerLagging
        expr: relayer_arbitrum_block_height < (max(relayer_arbitrum_block_height) - 10)
        for: 5m
        annotations:
          summary: "Relayer {{ $labels.instance }} is lagging"
```

---

## 🚀 Operations Procedures

### Daily Checks

```bash
# 1. Check relayer status
curl http://localhost:3000/health | jq

# 2. Check pending signatures
curl http://localhost:3000/metrics | grep pending

# 3. Check logs for errors
tail -100 logs/relayer.log | grep ERROR

# 4. Verify bridge contract
cast call $BRIDGE_ADDRESS "bridgeActive()(bool)"
```

### Weekly Checks

```bash
# 1. Review completed transactions
cast logs --address $BRIDGE_ADDRESS --from-block $(cast block-number - 10000)

# 2. Check relayer uptime
curl http://localhost:3000/metrics | grep uptime

# 3. Verify relayer is in list
cast call $BRIDGE_ADDRESS "isRelayer(address)(bool)" $RELAYER_ADDRESS

# 4. Review gas costs (for reimbursement)
```

### Monthly Checks

```bash
# 1. Rotate relayer keys (if policy requires)
# 2. Update software dependencies
# 3. Review and update procedures
# 4. Test emergency procedures
```

---

## 🚨 Emergency Procedures

### Relayer Goes Offline

1. **Detect**: Monitoring alerts on uptime < 99.9%
2. **Investigate**: SSH into server, check logs
3. **Restart**: `pm2 restart relayer` or `docker-compose restart`
4. **Verify**: Check health endpoint
5. **Document**: Log incident in incident tracker

### Bridge Needs Pausing

Any relayer can pause:

```bash
cast send $BRIDGE_ADDRESS "pause()" --private-key $RELAYER_KEY
```

### Relayer Compromised

1. **Pause bridge immediately** (any relayer)
2. **Remove compromised relayer**:
   ```bash
   cast send $BRIDGE_ADDRESS "removeRelayer(address)" $COMPROMISED --private-key $GOVERNOR_KEY
   ```
3. **Deploy replacement relayer**
4. **Add new relayer**:
   ```bash
   cast send $BRIDGE_ADDRESS "addRelayer(address)" $NEW --private-key $GOVERNOR_KEY
   ```
5. **Unpause bridge**:
   ```bash
   cast send $BRIDGE_ADDRESS "unpause()" --private-key $GOVERNOR_KEY
   ```

---

## 💰 Compensation Model

### Recommended Structure

| Component | Amount | Notes |
|-----------|--------|-------|
| **Base stipend** | $500-2000/month | Depends on operator type |
| **Gas reimbursement** | Actual cost | For signature submissions |
| **Uptime bonus** | +10% for 99.99% | Incentivize reliability |
| **Performance bonus** | Variable | Based on bridge volume |

### Gas Reimbursement

```bash
# Track gas costs
cast logs --address $BRIDGE_ADDRESS \
  --from-block $START \
  --to-block $END \
  | grep $RELAYER_ADDRESS \
  | jq '.gasUsed * .gasPrice'
```

---

## 📞 Communication Channels

### Recommended Setup

| Channel | Purpose | Participants |
|---------|---------|--------------|
| **Discord #relayer-ops** | General discussion | All relayers + team |
| **Telegram group** | Urgent alerts | All relayers |
| **PagerDuty** | Critical incidents | On-call relayer |
| **Status page** | Public uptime | Everyone |

### Escalation Path

```
Level 1: Individual relayer (auto-restart)
Level 2: Relayer group chat (peer assistance)
Level 3: Core team (governor intervention)
Level 4: Emergency pause (any relayer)
```

---

## 📋 Onboarding Checklist

### For New Relayer Operators

- [ ] **Legal**: Sign operator agreement
- [ ] **Security**: Receive private key securely
- [ ] **Infrastructure**: Set up server
- [ ] **Software**: Install and configure relayer
- [ ] **Testing**: Test on testnet first
- [ ] **Monitoring**: Configure health checks
- [ ] **Communication**: Join Discord/Telegram
- [ ] **Documentation**: Read all guides
- [ ] **Emergency**: Know pause procedure
- [ ] **Compensation**: Set up payment method

### For Governor (Setup)

- [ ] Recruit 5 operators
- [ ] Generate 5 relayer keypairs
- [ ] Add relayers to bridge contract
- [ ] Set threshold (3-of-5)
- [ ] Distribute keys securely
- [ ] Configure monitoring dashboard
- [ ] Set up alerting
- [ ] Document operator contacts
- [ ] Schedule onboarding calls
- [ ] Plan key rotation schedule

---

## 🔑 Key Distribution (Secure)

### Method 1: Encrypted Email

```bash
# Generate key
cast wallet new

# Encrypt with operator's PGP key
echo "0x..." | gpg --encrypt --recipient operator@domain.com

# Send encrypted blob
```

### Method 2: Secure Transfer Service

```bash
# Use Wormhole, OnionShare, or similar
# Set expiration: 24 hours
# Set password: Share via separate channel
```

### Method 3: In-Person Handoff

```bash
# For core team members
# Write key on paper
# Hand off in person
# Operator imports to secure storage
```

---

## 📊 Relayer Dashboard

### Recommended View

```
┌─────────────────────────────────────────────────────────────┐
│              UltraDAG Bridge - Relayer Dashboard            │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  Bridge Status: ● Active                                    │
│  Total Bridged: 1,234,567 UDAG                              │
│  Today's Volume: 45,678 UDAG / 500,000 cap                  │
│                                                             │
│  Relayer Status:                                            │
│  ┌─────────┬──────────┬───────────┬──────────┬─────────┐   │
│  │ Relayer │ Status   │ Pending   │ Completed│ Uptime  │   │
│  ├─────────┼──────────┼───────────┼──────────┼─────────┤   │
│  │ R1      │ ● Online │ 0         │ 15       │ 99.99%  │   │
│  │ R2      │ ● Online │ 1         │ 12       │ 99.95%  │   │
│  │ R3      │ ● Online │ 0         │ 18       │ 99.98%  │   │
│  │ R4      │ ○ Offline│ 0         │ 0        │ 98.50%  │   │
│  │ R5      │ ● Online │ 2         │ 14       │ 99.97%  │   │
│  └─────────┴──────────┴───────────┴──────────┴─────────┘   │
│                                                             │
│  ⚠️ Alert: R4 offline for 15 minutes                        │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

---

## 📚 Additional Resources

- [Bridge Contract Documentation](./README.md)
- [Deployment Guide](./DEPLOYMENT_GUIDE.md)
- [Relayer Technical Guide](./RELAYER_GUIDE.md)
- [Emergency Procedures](#emergency-procedures)

---

## 🎯 Summary

**To set up 5 relayer operators:**

1. **Recruit** 5 operators (2 team, 2 validators, 1 partner)
2. **Generate** 5 relayer keypairs
3. **Distribute** keys securely (encrypted)
4. **Configure** each relayer (server + software)
5. **Add** relayers to bridge contract
6. **Set** threshold to 3-of-5
7. **Monitor** uptime and performance
8. **Operate** with documented procedures

**Timeline:** 2-4 weeks for full setup and testing

**Budget:** $2,500-10,000/month (operator compensation + infrastructure)
