# UltraDAG Validator Handbook

**Version:** 1.1
**Last Updated:** March 2026
**Target Audience:** Validators, stakers, delegators, consensus participants

---

## Table of Contents

1. [Overview](#overview)
2. [Becoming a Validator](#becoming-a-validator)
3. [Staking Mechanics](#staking-mechanics)
4. [Delegated Staking](#delegated-staking)
5. [Rewards & Economics](#rewards--economics)
6. [Validator Responsibilities](#validator-responsibilities)
7. [Best Practices](#best-practices)
8. [Slashing & Penalties](#slashing--penalties)
9. [Governance Participation](#governance-participation)
10. [Performance Optimization](#performance-optimization)
11. [FAQ](#faq)

---

## Overview

Validators are the backbone of the UltraDAG network. They participate in consensus by producing signed vertices, validate transactions, and maintain network security. In return, validators earn block rewards and transaction fees.

**Key Facts:**
- **Minimum Stake:** 0.1 UDAG (10,000,000 satoshis)
- **Consensus:** Leaderless DAG-BFT (no leader election)
- **Block Rewards:** 50 UDAG per finalized vertex (halves every 210,000 rounds)
- **Unstaking Period:** 2,016 rounds (~2.8 hours at 5s rounds)
- **Validator Count:** Configurable (testnet: 4, mainnet: TBD)

---

## Becoming a Validator

### Prerequisites

**Technical Requirements:**
- Reliable server with 99.9%+ uptime
- 4+ CPU cores, 4GB+ RAM, 50GB+ SSD
- 100 Mbps symmetric network connection
- Static IP address or dynamic DNS
- Basic Linux system administration skills

**Financial Requirements:**
- Minimum 0.1 UDAG stake
- Recommended 1+ UDAG for meaningful rewards
- Sufficient UDAG for transaction fees

**Operational Requirements:**
- 24/7 monitoring capability
- Incident response procedures
- Backup and disaster recovery plan

### Step-by-Step Guide

#### 1. Set Up Node Infrastructure

Follow the [Node Operator Guide](../operations/node-operator-guide.md) to:
- Install UltraDAG node software
- Configure firewall and networking
- Set up monitoring and alerting
- Implement backup procedures

#### 2. Generate Validator Keys

**Option A: Using RPC endpoint (testing only)**
```bash
curl http://localhost:10333/keygen
```

**Option B: Offline key generation (recommended for production)**
```bash
# On air-gapped machine
git clone https://github.com/UltraDAGcom/core.git
cd core
cargo run --bin keygen

# Output:
# Secret Key: abc123def456...
# Public Key: def456abc123...
# Address: 789xyz...
```

**⚠️ Critical Security:**
- Store secret key in encrypted vault (HashiCorp Vault, AWS Secrets Manager)
- Never expose secret key over network
- Keep offline backup in secure location
- Use hardware security module (HSM) for production

#### 3. Acquire Stake

**Testnet:**
```bash
# Request from faucet
curl -X POST http://localhost:10333/faucet \
  -H "Content-Type: application/json" \
  -d '{"address": "YOUR_ADDRESS"}'
```

**Mainnet:**
- Purchase UDAG from exchange
- Transfer to validator address
- Verify balance: `curl http://localhost:10333/balance/YOUR_ADDRESS`

#### 4. Stake Tokens

Create and sign stake transaction:

```bash
# Example stake transaction
{
  "staker": "YOUR_ADDRESS",
  "amount": 100000000,  # 1 UDAG in satoshis
  "nonce": 0,
  "pub_key": "YOUR_PUBLIC_KEY",
  "signature": "SIGNATURE"
}
```

Submit stake:
```bash
curl -X POST http://localhost:10333/stake \
  -H "Content-Type: application/json" \
  -d @stake.json
```

Verify staking status:
```bash
curl http://localhost:10333/stake/YOUR_ADDRESS | jq .
```

#### 5. Start Validator Node

**Create environment file:**
```bash
sudo nano /etc/ultradag/validator.env
```

Add:
```bash
VALIDATOR_SECRET_KEY=your_secret_key_here
```

Secure it:
```bash
sudo chmod 600 /etc/ultradag/validator.env
sudo chown ultradag:ultradag /etc/ultradag/validator.env
```

**Start validator:**
```bash
sudo systemctl start ultradag
sudo systemctl status ultradag
```

**Verify validator is producing:**
```bash
# Check logs for vertex production
sudo journalctl -u ultradag -f | grep "produced vertex"

# Check validator list
curl http://localhost:10333/validators | jq .
```

---

## Staking Mechanics

### Stake Lifecycle

```
┌─────────────┐
│   Unstaked  │
└──────┬──────┘
       │ Stake Transaction
       ▼
┌─────────────┐
│   Staked    │ ◄─── Active Validator
└──────┬──────┘      (Earning Rewards)
       │ Unstake Transaction
       ▼
┌─────────────┐
│  Unbonding  │ ◄─── Cooldown Period
└──────┬──────┘      (2,016 rounds)
       │ Cooldown Complete
       ▼
┌─────────────┐
│   Unstaked  │
└─────────────┘
```

### Minimum Stake

**Current Minimum:** 0.1 UDAG (10,000,000 satoshis)

**Rationale:**
- Low barrier to entry for decentralization
- High enough to deter Sybil attacks
- Adjustable via governance proposal

### Staking Transaction

**Transaction Structure:**
```rust
StakeTx {
    staker: Address,      // Your validator address
    amount: u64,          // Stake amount in satoshis
    nonce: u64,           // Current account nonce
    pub_key: [u8; 32],    // Ed25519 public key
    signature: [u8; 64],  // Ed25519 signature
}
```

**Validation Rules:**
1. `amount >= MIN_STAKE_SATS` (10,000,000)
2. `balance(staker) >= amount + fee`
3. `nonce == current_nonce(staker)`
4. Valid Ed25519 signature
5. `Blake3(pub_key) == staker`

### Unstaking Process

**Initiate Unstaking:**
```bash
curl -X POST http://localhost:10333/unstake \
  -H "Content-Type: application/json" \
  -d '{
    "staker": "YOUR_ADDRESS",
    "amount": 50000000,
    "nonce": 5,
    "pub_key": "YOUR_PUBLIC_KEY",
    "signature": "SIGNATURE"
  }'
```

**Cooldown Period:**
- Duration: 2,016 rounds (~2.8 hours at 5s rounds)
- During cooldown: Stake remains locked, no rewards earned
- After cooldown: Tokens automatically returned to balance

**Check Unstaking Status:**
```bash
curl http://localhost:10333/stake/YOUR_ADDRESS | jq '.unstaking'
```

**Response:**
```json
{
  "unstaking": [
    {
      "amount": 50000000,
      "cooldown_ends_round": 6539
    }
  ]
}
```

### Partial Unstaking

You can unstake partial amounts:
- Minimum remaining stake must be ≥ MIN_STAKE_SATS
- Or unstake entire amount to exit validator set

**Example:**
```
Initial stake: 1 UDAG (100,000,000 sats)
Unstake: 0.5 UDAG (50,000,000 sats)
Remaining: 0.5 UDAG (still validator)

Unstake: 0.4 UDAG (40,000,000 sats)
Remaining: 0.1 UDAG (still validator, at minimum)

Unstake: 0.1 UDAG (10,000,000 sats)
Remaining: 0 UDAG (no longer validator)
```

---

## Delegated Staking

Delegated staking allows token holders to earn rewards without running a validator node. Delegators assign their UDAG to an active validator, increasing that validator's effective stake and sharing in the rewards.

### How Delegation Works

```
┌──────────────┐         ┌──────────────────┐
│  Delegator A │──100──▶ │                  │
└──────────────┘         │   Validator X    │
┌──────────────┐         │                  │
│  Delegator B │──500──▶ │  Own Stake: 1000 │
└──────────────┘         │  Delegated: 600  │
                         │  Effective: 1600 │
                         │  Commission: 10% │
                         └──────────────────┘
```

**Effective stake** = validator's own stake + total delegations from all delegators. This is the value used for active set ranking — validators with higher effective stake are more likely to remain in the top 21 active set.

### For Validators: Accepting Delegations

Any staked validator can accept delegations. Delegations increase your effective stake, which improves your ranking in the active validator set and increases your share of block rewards.

#### Commission Rate

Validators earn a commission on rewards generated by delegated stake.

- **Default commission:** 10%
- **Configurable range:** 0-100%
- **Adjustable via:** `SetCommissionTx` or `/set-commission` RPC endpoint

**Set commission via RPC:**
```bash
curl -X POST http://localhost:10333/set-commission \
  -H "Content-Type: application/json" \
  -d '{
    "secret_key": "YOUR_SECRET_KEY",
    "commission_percent": 15
  }'
```

**Commission economics example:**
```
Validator's own stake:   1,000 UDAG
Total delegated stake:     500 UDAG
Effective stake:         1,500 UDAG
Commission rate:            10%

Block reward for a round: 1 UDAG (split among all validators)
Validator's share:        1 × (1500 / total_effective_stake)

Of the delegated portion's rewards:
  Delegator reward portion = reward × (500 / 1500) = 0.333 UDAG
  Commission to validator  = 0.333 × 10% = 0.033 UDAG
  Paid to delegators       = 0.333 × 90% = 0.300 UDAG
```

**Tips for setting commission:**
- Lower commission attracts more delegators (higher effective stake)
- Higher commission earns more per delegated UDAG
- Check competitor rates via `/validators` to stay competitive
- Changing commission too frequently may erode delegator trust

#### Viewing Your Delegators

```bash
curl http://localhost:10333/validator/YOUR_ADDRESS/delegators | jq .
```

**Response:**
```json
{
  "validator": "YOUR_ADDRESS",
  "commission_percent": 10,
  "own_stake_udag": 1000,
  "total_delegated_udag": 500,
  "effective_stake_udag": 1500,
  "delegators": [
    {
      "address": "abc123...",
      "delegated_udag": 300,
      "delegated_sats": 30000000000
    },
    {
      "address": "def456...",
      "delegated_udag": 200,
      "delegated_sats": 20000000000
    }
  ]
}
```

#### Slashing Impact on Delegators

If a validator equivocates (produces two vertices in the same round), **both the validator's own stake and all delegated stake are slashed 50%**. This makes validator reputation critically important -- delegators will avoid validators with a history of slashing events or poor operational practices.

### For Delegators: Earning Passive Rewards

Delegation is a passive way to earn UDAG rewards. You do not need to run a node, maintain uptime, or participate in consensus. You simply choose a validator and delegate your tokens.

#### Minimum Delegation

- **Minimum amount:** 100 UDAG per delegation
- **No maximum:** You can delegate any amount above the minimum

#### Delegating Tokens

**Submit a delegation transaction:**
```bash
curl -X POST http://localhost:10333/tx/submit \
  -H "Content-Type: application/json" \
  -d '{
    "Delegate": {
      "from": "YOUR_ADDRESS",
      "validator": "VALIDATOR_ADDRESS",
      "amount": 10000000000,
      "nonce": 0,
      "pub_key": "YOUR_PUBLIC_KEY",
      "signature": "SIGNATURE"
    }
  }'
```

**DelegateTx Structure:**
```rust
DelegateTx {
    from: Address,         // Delegator address
    validator: Address,    // Target validator address
    amount: u64,           // Amount in satoshis (min 100 UDAG)
    nonce: u64,            // Current account nonce
    pub_key: [u8; 32],     // Ed25519 public key
    signature: [u8; 64],   // Ed25519 signature
}
```

**Validation Rules:**
1. `amount >= 100 UDAG` (10,000,000,000 satoshis)
2. `balance(from) >= amount`
3. Target validator must have an active stake
4. `nonce == current_nonce(from)`
5. Valid Ed25519 signature

#### Delegation Rewards

Delegator rewards are proportional to the amount delegated, minus the validator's commission.

**Reward formula:**
```
delegator_reward = block_reward
                   × (delegated_amount / total_effective_stake)
                   × (1 - commission / 100)
```

**Example:**
```
Your delegation:          200 UDAG
Validator's effective stake: 2,000 UDAG (total network effective: 10,000 UDAG)
Validator commission:     10%
Block reward per round:   1 UDAG

Your share of round reward:
  = 1 × (200 / 10,000) × (1 - 10/100)
  = 1 × 0.02 × 0.90
  = 0.018 UDAG per round
```

#### Undelegating Tokens

Undelegation follows the same cooldown as unstaking: **2,016 rounds (~2.8 hours at 5s rounds)**.

**Submit an undelegation transaction:**
```bash
curl -X POST http://localhost:10333/tx/submit \
  -H "Content-Type: application/json" \
  -d '{
    "Undelegate": {
      "from": "YOUR_ADDRESS",
      "validator": "VALIDATOR_ADDRESS",
      "nonce": 1,
      "pub_key": "YOUR_PUBLIC_KEY",
      "signature": "SIGNATURE"
    }
  }'
```

**Check undelegation status:**
```bash
curl http://localhost:10333/stake/YOUR_ADDRESS | jq '.delegations'
```

After the cooldown period completes, the delegated tokens are automatically returned to your liquid balance.

#### Choosing a Validator

Choosing the right validator is important because your delegated tokens are at risk of slashing if the validator misbehaves. Consider the following factors:

| Factor | What to Check | Where to Find It |
|--------|--------------|-------------------|
| Commission rate | Lower = more rewards for you | `/validators` |
| Effective stake | Higher = more established | `/validators` |
| Uptime | Higher = more consistent rewards | Node monitoring tools |
| Track record | No slashing history | `/validator/:address` |
| Self-stake ratio | Higher own stake = more skin in the game | `/validator/:address/delegators` |

**Check available validators:**
```bash
curl http://localhost:10333/validators | jq '.[] | {address, commission_percent, effective_stake_udag, own_stake_udag}'
```

#### Slashing Risk

If your chosen validator equivocates (produces conflicting vertices in the same round), **you lose 50% of your delegated amount**. The slash applies equally to the validator's own stake and all delegated stake.

**Mitigating slashing risk:**
- Spread delegations across multiple validators
- Prefer validators with high self-stake (they have more to lose)
- Avoid validators with previous slashing events
- Monitor your validator's performance regularly

### Delegation Lifecycle

```
┌──────────────┐
│   Liquid     │
└──────┬───────┘
       │ DelegateTx (min 100 UDAG)
       ▼
┌──────────────┐
│  Delegated   │ ◄─── Earning Rewards
└──────┬───────┘      (minus commission)
       │ UndelegateTx
       ▼
┌──────────────┐
│  Unbonding   │ ◄─── Cooldown Period
└──────┬───────┘      (2,016 rounds)
       │ Cooldown Complete
       ▼
┌──────────────┐
│   Liquid     │
└──────────────┘
```

---

## Rewards & Economics

### Block Rewards

**Reward Schedule:**
```
Initial Reward: 50 UDAG per finalized vertex
Halving Interval: 210,000 rounds
Total Supply Cap: 21,000,000 UDAG
```

**Halving Schedule:**

| Rounds | Reward per Vertex | Approximate Time |
|--------|------------------|------------------|
| 0 - 209,999 | 50 UDAG | ~12.2 days |
| 210,000 - 419,999 | 25 UDAG | ~12.2 days |
| 420,000 - 629,999 | 12.5 UDAG | ~12.2 days |
| 630,000 - 839,999 | 6.25 UDAG | ~12.2 days |
| ... | ... | ... |
| 13,440,000+ | 0 UDAG | After 64 halvings |

**Reward Calculation:**
```rust
fn block_reward(round: u64) -> u64 {
    let halvings = round / 210_000;
    if halvings >= 64 {
        return 0;
    }
    let initial_reward = 50_0000_0000; // 50 UDAG in satoshis
    initial_reward >> halvings
}
```

### Transaction Fees

**Fee Structure:**
- Minimum fee: 1,000 satoshis (0.00001 UDAG)
- Recommended fee: 10,000 satoshis (0.0001 UDAG)
- High priority: 100,000+ satoshis

**Fee Distribution:**
- 100% of transaction fees go to vertex proposer
- Fees collected from all transactions in the vertex
- Paid via coinbase transaction

### Reward Distribution

**Per-Vertex Rewards:**
```
Total Reward = Block Reward + Transaction Fees
```

**Example:**
```
Round: 100,000
Block Reward: 50 UDAG
Transactions in Vertex: 10
Average Fee: 0.0001 UDAG
Total Fees: 0.001 UDAG
Total Reward: 50.001 UDAG
```

**Validator Earnings:**

With 4 validators producing 1 vertex each per round:

```
Rounds per Day: 17,280 (at 5s rounds)
Vertices per Validator per Day: 4,320
Daily Reward (early epochs): 4,320 × 50 = 216,000 UDAG
```

**Important:** This assumes equal vertex production. Actual earnings depend on:
- Network participation rate
- Vertex finalization success
- Transaction volume (fees)
- Validator uptime

### Expected Returns

**Assumptions:**
- 4 validators
- 5-second rounds
- 99% uptime
- Early epoch (50 UDAG reward)

**Annual Yield:**

| Stake | Daily Earnings | Annual Earnings | APY |
|-------|---------------|-----------------|-----|
| 1 UDAG | ~54,000 UDAG | ~19.7M UDAG | 1,970,000% |
| 10 UDAG | ~54,000 UDAG | ~19.7M UDAG | 197,000% |
| 100 UDAG | ~54,000 UDAG | ~19.7M UDAG | 19,700% |

**Note:** These are theoretical early-epoch returns. APY decreases with:
- More validators (dilutes rewards)
- Later epochs (halving reduces rewards)
- Lower uptime (missed vertices)

### Supply Cap Enforcement

**Maximum Supply:** 21,000,000 UDAG (2,100,000,000,000,000 satoshis)

**Enforcement Mechanism:**
```rust
if total_supply + reward > MAX_SUPPLY_SATS {
    reward = MAX_SUPPLY_SATS - total_supply;
}
```

This ensures the 21M cap is never exceeded, even if the halving schedule would allow it.

---

## Validator Responsibilities

### Core Responsibilities

**1. Vertex Production**
- Produce one signed vertex per round
- Include valid transactions from mempool
- Reference all known DAG tips as parents
- Sign with Ed25519 validator key

**2. Transaction Validation**
- Verify transaction signatures
- Check account balances and nonces
- Reject invalid transactions
- Prevent double-spends

**3. Network Participation**
- Maintain connections to peers
- Relay vertices and transactions
- Respond to sync requests
- Participate in checkpoint co-signing

**4. State Maintenance**
- Apply finalized vertices to state
- Maintain accurate account balances
- Track validator set and stakes
- Persist state to disk

### Performance Requirements

**Uptime:**
- Target: 99.9%+ uptime
- Acceptable: 95%+ uptime
- Below 90%: Consider exiting validator set

**Latency:**
- Vertex production: <1 second per round
- Network propagation: <500ms to peers
- Finality lag: <10 rounds

**Resource Usage:**
- CPU: <50% average utilization
- Memory: <500 MB
- Disk I/O: <10 MB/s
- Network: <1 Mbps

### Monitoring Requirements

**Critical Metrics:**
- Node health status
- Finality lag
- Peer connection count
- Vertex production rate
- Memory and CPU usage

**Alerting Thresholds:**
- Node down: Immediate alert
- Finality lag >10: Warning
- Finality lag >100: Critical
- No peers: Critical
- High memory (>500MB): Warning

**Monitoring Tools:**
- Prometheus + Grafana
- Health check automation
- Log aggregation (ELK, Loki)
- Uptime monitoring (UptimeRobot, Pingdom)

---

## Best Practices

### Infrastructure

**1. Redundancy**
- Primary and backup nodes
- Automated failover
- Geographic distribution
- Multiple network providers

**2. Security**
- Hardware security modules (HSM) for keys
- Encrypted backups
- Regular security audits
- Principle of least privilege

**3. Monitoring**
- 24/7 monitoring
- Automated alerting
- Incident response procedures
- Regular health checks

### Operational

**1. Key Management**
- Offline key generation
- Encrypted storage
- Access control
- Regular key rotation (with unstaking)

**2. Updates**
- Test updates on testnet first
- Schedule during low-activity periods
- Have rollback plan ready
- Monitor closely after updates

**3. Backup**
- Daily automated backups
- Offsite backup storage
- Regular restoration tests
- Documented recovery procedures

### Economic

**1. Stake Management**
- Start with minimum stake
- Increase stake as confidence grows
- Diversify across multiple validators
- Monitor reward rates

**2. Fee Optimization**
- Monitor mempool for fee trends
- Prioritize high-fee transactions
- Balance throughput vs. fees

**3. Risk Management**
- Don't stake more than you can afford to lose
- Understand unstaking cooldown period
- Monitor network health
- Have exit strategy

---

## Slashing & Penalties

### Current Implementation

**Equivocation Slashing:**
- **Trigger:** Producing two different vertices in the same round
- **Penalty:** 50% of validator's own stake burned (removed from total supply)
- **Delegator impact:** 50% of all delegated stake also burned
- **Detection:** Deterministic -- applied during `apply_finalized_vertices()` when duplicate (validator, round) pairs are found in the sorted finality batch
- **Evidence:** Broadcast to all peers via `EquivocationEvidence` P2P message
- **Active set:** Validator removed from active set if remaining stake falls below `MIN_STAKE_SATS`

**Slashing is deflationary:** The burned stake is permanently removed from `total_supply`, reducing circulating supply.

### Future Slashing Mechanisms

**Planned for Future Versions:**

**1. Downtime Penalties**
- Penalty: Reduced rewards for missed vertices
- Threshold: <90% uptime over epoch
- Calculation: Proportional to downtime

**2. Invalid State Slashing**
- Penalty: Partial stake confiscation
- Trigger: Producing vertex with invalid state transition
- Severity: Based on impact

### Avoiding Penalties

**Best Practices:**
1. **Never run multiple validators with same key**
2. **Ensure proper failover mechanisms**
3. **Monitor for clock drift**
4. **Validate all transactions before inclusion**
5. **Maintain high uptime**

---

## Governance Participation

### Voting Rights

**Who Can Vote:**
- Active validators only
- Must have staked tokens

**Vote Weight:**
- Weight = Stake amount
- 1 satoshi = 1 vote
- Larger stakes have more influence

### Proposal Types

**1. Text Proposals**
- Non-binding signaling
- Community sentiment
- Feature requests

**2. Parameter Changes**
- Minimum stake amount
- Fee requirements
- Round duration
- Checkpoint interval

**3. Validator Set Changes**
- Add new validators
- Remove inactive validators
- Requires epoch-based implementation

### Voting Process

**1. Create Proposal:**
```bash
curl -X POST http://localhost:10333/proposal \
  -H "Content-Type: application/json" \
  -d '{
    "proposer": "YOUR_ADDRESS",
    "proposal_type": {
      "ParameterChange": {
        "title": "Reduce minimum stake to 0.05 UDAG",
        "description": "Lower barrier to entry for new validators",
        "parameter": "MIN_STAKE_SATS",
        "new_value": "5000000"
      }
    },
    "nonce": 10,
    "pub_key": "YOUR_PUBLIC_KEY",
    "signature": "SIGNATURE"
  }'
```

**2. Vote on Proposal:**
```bash
curl -X POST http://localhost:10333/vote \
  -H "Content-Type: application/json" \
  -d '{
    "voter": "YOUR_ADDRESS",
    "proposal_id": 1,
    "vote": true,
    "nonce": 11,
    "pub_key": "YOUR_PUBLIC_KEY",
    "signature": "SIGNATURE"
  }'
```

**3. Monitor Proposal:**
```bash
curl http://localhost:10333/proposal/1 | jq .
```

### Voting Strategy

**Considerations:**
- Network health impact
- Economic implications
- Technical feasibility
- Community consensus

**Best Practices:**
- Review proposal details thoroughly
- Discuss with community
- Consider long-term effects
- Vote consistently with values

---

## Performance Optimization

### Vertex Production

**Optimize for:**
- Fast transaction validation
- Efficient signature verification
- Quick parent selection
- Minimal lock contention

**Tips:**
- Keep mempool size reasonable (<1000 txs)
- Prioritize high-fee transactions
- Cache frequently accessed state
- Use efficient data structures

### Network Performance

**Optimize for:**
- Low latency to peers
- High bandwidth utilization
- Efficient message propagation

**Tips:**
- Use dedicated network connection
- Optimize TCP parameters
- Maintain full mesh with other validators
- Monitor network latency

### Resource Management

**CPU:**
- Use release builds (not debug)
- Monitor for CPU spikes
- Profile hot paths
- Consider hardware upgrades

**Memory:**
- Monitor for memory leaks
- Restart periodically if needed
- Use memory profiling tools
- Optimize data structures

**Disk:**
- Use SSD/NVMe storage
- Monitor disk I/O
- Regular checkpoint pruning (automatic)
- Optimize filesystem (ext4, xfs)

---

## FAQ

### General

**Q: How much can I earn as a validator?**

A: Earnings depend on:
- Number of validators (dilutes rewards)
- Your uptime (missed vertices = missed rewards)
- Transaction volume (fees)
- Current epoch (halving reduces rewards)

Early epochs with few validators can earn 50+ UDAG per vertex. Later epochs earn less.

**Q: What happens if my node goes offline?**

A: You miss vertex production opportunities and don't earn rewards for that period. No slashing currently, but future versions may penalize extended downtime.

**Q: Can I run multiple validators?**

A: Yes, but each requires separate stake and infrastructure. Never use the same key for multiple validators (equivocation).

**Q: How do I increase my stake?**

A: Submit additional stake transactions. Your total stake accumulates.

**Q: What's the minimum hardware requirement?**

A: 4 CPU cores, 4GB RAM, 50GB SSD, 100 Mbps network. See [Node Operator Guide](../operations/node-operator-guide.md) for details.

### Staking

**Q: How long does unstaking take?**

A: 2,016 rounds (~2.8 hours at 5s rounds). This is the cooldown period.

**Q: Can I unstake partially?**

A: Yes, as long as remaining stake ≥ MIN_STAKE_SATS (0.1 UDAG).

**Q: What happens to my stake if I'm offline?**

A: Stake remains locked. You don't earn rewards while offline, but stake is not slashed (currently).

**Q: Can I transfer my validator to another address?**

A: No direct transfer. You must unstake (wait cooldown), then stake from new address.

### Rewards

**Q: When do I receive rewards?**

A: Immediately when your vertex is finalized (typically 2-3 rounds after production).

**Q: Are rewards automatically compounded?**

A: No. Rewards are added to your balance, not your stake. You can manually stake rewards.

**Q: What if two validators produce vertices in the same round?**

A: Both earn rewards if both vertices are finalized. UltraDAG is leaderless - all validators produce concurrently.

**Q: Do I earn rewards during unstaking cooldown?**

A: No. During cooldown, your stake is locked but you're not an active validator.

### Technical

**Q: What is equivocation?**

A: Producing two different vertices in the same round. This results in permanent ban from the network.

**Q: How do I avoid equivocation?**

A: Never run multiple nodes with the same validator key. Ensure proper failover mechanisms.

**Q: Can I change my validator key?**

A: Yes, but requires unstaking with old key (wait cooldown), then staking with new key.

**Q: What's the difference between full node and validator?**

A: Full nodes sync DAG and provide RPC access. Validators also produce vertices and earn rewards.

### Delegation

**Q: Can I delegate without running a node?**

A: Yes, delegation is entirely passive. Submit a `DelegateTx` to assign your tokens to a validator and earn rewards automatically. You do not need to run any software, maintain uptime, or participate in consensus. Your only ongoing responsibility is monitoring your chosen validator's performance.

**Q: How do I choose a validator to delegate to?**

A: Check the `/validators` endpoint for a list of active validators with their commission rates, effective stake, and own stake. Prefer validators with low commission, high self-stake (skin in the game), strong uptime history, and no previous slashing events. You can also spread your delegation across multiple validators to reduce risk.

**Q: What happens if my validator is slashed?**

A: You lose 50% of your delegated amount, the same percentage as the validator loses from their own stake. Slashing occurs when a validator equivocates (produces two conflicting vertices in the same round). This is why choosing a reliable, well-operated validator is important.

**Q: Can I change my delegation to a different validator?**

A: There is no direct re-delegation. You must first undelegate from the current validator (which starts a 2,016-round cooldown of approximately 2.8 hours), wait for the cooldown to complete and tokens to return to your liquid balance, then submit a new `DelegateTx` to the new validator.

**Q: Is there a minimum delegation amount?**

A: Yes, the minimum delegation is 100 UDAG per transaction. There is no maximum.

**Q: How are delegation rewards distributed?**

A: Rewards are calculated proportionally each round based on your delegated amount relative to the total effective stake across the network, minus your validator's commission percentage. Rewards are credited to your liquid balance, not added to your delegation.

**Q: Can I delegate to multiple validators?**

A: Yes, you can submit separate `DelegateTx` transactions to different validators. Each delegation is tracked independently, and each earns rewards based on that validator's effective stake share and commission rate.

---

## Additional Resources

- **Node Operator Guide:** [docs/guides/operations/node-operator-guide.md](../operations/node-operator-guide.md)
- **RPC API Reference:** [docs/reference/api/rpc-endpoints.md](../../reference/api/rpc-endpoints.md)
- **Whitepaper:** [docs/reference/specifications/whitepaper.md](../../reference/specifications/whitepaper.md)
- **Operations Runbook:** [docs/operations/RUNBOOK.md](../../operations/RUNBOOK.md)

---

## Support

**Validator Support:**
- GitHub Discussions: https://github.com/UltraDAGcom/core/discussions
- Discord: https://discord.gg/ultradag
- Email: validators@ultradag.io

**Emergency Contact:**
- Security Issues: security@ultradag.io
- Critical Incidents: ops@ultradag.io

---

**Last Updated:** March 15, 2026
**Document Version:** 1.1
**Maintainer:** UltraDAG Core Team
