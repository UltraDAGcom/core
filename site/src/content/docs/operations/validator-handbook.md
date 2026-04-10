---
title: "Validator Handbook"
description: "Comprehensive operational guide for UltraDAG validators covering key management, rewards, slashing, and performance"
order: 2
section: "operations"
---

# Validator Handbook

A comprehensive operational guide for UltraDAG validators covering key management, rewards, slashing prevention, and performance optimization.

---

## Requirements

| Requirement | Minimum |
|------------|---------|
| Stake | 2,000 UDAG |
| CPU | 1 core |
| RAM | 256 MB |
| Disk | 1 GB (with default 1000-round pruning) |
| Network | Stable internet, low latency to peers |
| Uptime | 99%+ recommended |

<div class="callout callout-info"><div class="callout-title">Lightweight by design</div>UltraDAG is purpose-built for constrained hardware. A $5/month VPS or a <strong>$15 Raspberry Pi Zero 2 W</strong> can run a full validator. See the <a href="/docs/getting-started/raspberry-pi">Raspberry Pi bringup guide</a> for a complete walkthrough.</div>

---

## Key Management

### Private Key Sources

The validator identity is loaded in priority order:

1. **`--pkey` flag**: hex-encoded 64-character Ed25519 secret key on the command line
2. **`validator.key` file**: plain text file in the data directory containing the hex key
3. **Auto-generated**: a new keypair is generated if neither source is available

### Key File

```bash
# Generate a key offline and save it
echo "YOUR_64_CHAR_HEX_SECRET_KEY" > /etc/ultradag/validator.key
chmod 600 /etc/ultradag/validator.key
chown ultradag:ultradag /etc/ultradag/validator.key
```

### Key Security Best Practices

- Generate keys on an air-gapped machine
- Never transmit private keys over the network
- Store backups in encrypted cold storage
- Use file permissions to restrict access (`chmod 600`)
- Never commit keys to version control
- Consider hardware wallet storage for mainnet

<div class="callout callout-danger"><div class="callout-title">One key, one node</div>Never run two validator instances with the same key. This causes equivocation (two different vertices in the same round), which triggers automatic slashing of 50% of your stake.</div>

---

## Staking Lifecycle

*The staking lifecycle progresses through the following states: starting from Unstaked, a validator stakes at least 2,000 UDAG to become Staked. At the next epoch boundary, if ranked in the top 100 by effective stake (own + delegated) they become Active (otherwise Passive). Active validators run the validator loop and earn rewards each round. Equivocation causes slashing (50% burned). Both Active and Passive validators can submit Unstake to enter the Unstaking state, which completes after a 2,016-round cooldown back to Unstaked.*

### Step-by-Step

1. **Fund your address**: acquire at least 2,000 UDAG
2. **Stake**: submit a `Stake` transaction
3. **Wait for epoch boundary**: active set recalculated every 210,000 rounds (~12 days at 5s/round)
4. **Produce vertices**: your node produces one vertex per round
5. **Earn rewards**: proportional to your effective stake
6. **Unstake (optional)**: submit `Unstake`, wait 2,016 rounds for cooldown

---

## Reward Calculation

### Active Validator

Each round, the protocol distributes the block reward:

$$
\text{your\_reward} = \text{round\_reward} \times (1 - \text{council\_pct}) \times \frac{\text{your\_effective\_stake}}{\sum \text{all\_effective\_stakes}}
$$

**Example** at default settings (1 UDAG/round, 10% council, 5 equal validators):

| Component | Value |
|-----------|-------|
| Round reward | 1.0 UDAG |
| Council share (10%) | 0.1 UDAG |
| Validator pool | 0.9 UDAG |
| Your share (1/5) | 0.18 UDAG |
| Per day (~17,280 rounds) | ~3,110 UDAG |

### Passive Staker

If ranked outside the top 100 active set, you earn 50% of the active rate:

$$
\text{passive\_reward} = \text{active\_equivalent} \times 0.50
$$

This is the `OBSERVER_REWARD_PERCENT` constant — raised from 20% to 50% to give small validators a meaningful reward while they accumulate enough stake (or delegations) to enter the active set.

### Commission from Delegations

As a validator with delegations, you earn commission on delegated rewards:

| Component | Formula |
|-----------|---------|
| Your own share | `reward * (own_stake / effective_stake)` |
| Delegation pool | `reward * (delegated / effective_stake)` |
| Your commission | `delegation_pool * (commission_pct / 100)` |
| **Total** | own share + commission |

---

## Commission Management

Set your commission rate (default 10%):

```bash
curl -X POST http://localhost:10333/set-commission \
  -H "Content-Type: application/json" \
  -d '{"secret_key": "YOUR_KEY", "commission_percent": 15}'
```

**Commission strategy considerations:**

- **Low commission** (0-5%): attracts more delegators, increases effective stake
- **Medium commission** (5-15%): balanced revenue and delegation attraction
- **High commission** (15-50%): maximizes per-delegation revenue but may deter delegators
- **100% commission**: takes all delegation rewards (delegators earn nothing)

---

## Slashing Prevention

### What Causes Slashing

The only slashing condition is **equivocation**: producing two different vertices for the same round.

### How to Prevent It

1. **Never run duplicate nodes**: one key = one active node, always
2. **Clean restarts**: ensure the old process is fully stopped before starting a new one
3. **Monitor process health**: use systemd or similar to prevent double-starts
4. **Avoid clock issues**: ensure NTP is configured and stable

### Slashing Impact

| Aspect | Impact |
|--------|--------|
| Stake burn | 50% of your staked amount (governable 10-100%) |
| Delegation cascade | All delegators also lose 50% |
| Active set | Removed if stake falls below 2,000 UDAG (minimum) |
| Supply effect | Slashed amount is burned (deflationary) |
| Detection | Automatic, deterministic, no appeals |

<div class="callout callout-danger"><div class="callout-title">Slashing is permanent</div>Burned stake cannot be recovered. There is no governance mechanism to reverse a slash -- the evidence is cryptographically verifiable.</div>

---

## Performance Optimization

### Network

- Use a VPS with low-latency connectivity to other validators
- Ensure P2P port (default 9333) is directly accessible (no NAT)
- Consider geographic proximity to existing validators

### Storage

- Default pruning (1000 rounds) keeps disk usage bounded
- Use SSD for the data directory (redb benefits from fast random I/O)
- Monitor disk usage: `du -sh ~/.ultradag/node/`

### Monitoring

- Set up [Prometheus + Grafana](/docs/operations/monitoring) for real-time metrics
- Alert on finality lag > 5 rounds
- Alert on peer count dropping below 2
- Monitor memory usage (should stay under 512 MB)

---

## Governance Participation

As a validator, you may also serve as a council member (no stake requirement). Council participation involves:

- **Voting on proposals**: review and vote on parameter changes
- **Creating proposals**: submit improvements for network governance
- **Monitoring**: watch for parameter change proposals that affect validator economics

See [Governance](/docs/tokenomics/governance) for full details.

---

## Operational Checklist

### Daily

- [ ] Verify node is producing vertices (`curl /status`)
- [ ] Check finality lag is <= 3 (`curl /health/detailed`)
- [ ] Review logs for warnings or errors

### Weekly

- [ ] Check disk usage
- [ ] Verify backup integrity
- [ ] Review delegator activity
- [ ] Check for software updates

### Per Epoch (~12 days)

- [ ] Verify you remained in the active set
- [ ] Review reward accumulation
- [ ] Assess commission rate competitiveness
- [ ] Check governance proposals requiring votes

---

## Next Steps

- [Monitoring](/docs/operations/monitoring) -- Prometheus and Grafana setup
- [Staking & Delegation](/docs/tokenomics/staking) -- full tokenomics details
- [CLI Reference](/docs/operations/cli) -- all configuration flags
