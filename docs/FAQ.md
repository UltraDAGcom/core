# UltraDAG FAQ & Troubleshooting Guide

**Version:** 1.0  
**Last Updated:** March 2026

---

## Table of Contents

- [General Questions](#general-questions)
- [Getting Started](#getting-started)
- [Transactions](#transactions)
- [Staking & Validation](#staking--validation)
- [Governance](#governance)
- [Technical Questions](#technical-questions)
- [Troubleshooting](#troubleshooting)
- [Performance](#performance)
- [Security](#security)

---

## General Questions

### What is UltraDAG?

UltraDAG is a leaderless DAG-BFT cryptocurrency with minimal complexity. It uses a directed acyclic graph (DAG) structure for consensus instead of a traditional blockchain, allowing all validators to produce blocks concurrently without leader election.

**Key Features:**
- Leaderless consensus (no single point of failure)
- Fast finality (2-3 rounds, ~10-15 seconds)
- Bitcoin-inspired tokenomics (21M supply cap)
- Minimal codebase (781 lines of consensus code)

### How is UltraDAG different from Bitcoin?

| Feature | Bitcoin | UltraDAG |
|---------|---------|----------|
| **Structure** | Linear blockchain | DAG (directed acyclic graph) |
| **Consensus** | Proof of Work | DAG-BFT (Byzantine Fault Tolerant) |
| **Block Production** | One miner per block | All validators concurrently |
| **Finality** | Probabilistic (~60 min) | Deterministic (~10-15 sec) |
| **Supply Cap** | 21 million BTC | 21 million UDAG |
| **Energy** | High (mining) | Low (no mining) |

### How is UltraDAG different from Ethereum?

| Feature | Ethereum | UltraDAG |
|---------|----------|----------|
| **Smart Contracts** | Yes (EVM) | No (governance only) |
| **Consensus** | Proof of Stake | DAG-BFT |
| **Account Model** | Account-based | Account-based |
| **Finality** | ~15 minutes | ~10-15 seconds |
| **Complexity** | High | Minimal |

### What can I build on UltraDAG?

**Supported:**
- Payment applications
- Wallets
- Exchanges
- Micropayment systems
- IoT payment networks
- Governance-based protocols

**Not Supported:**
- Smart contracts (no EVM)
- DeFi protocols (without off-chain components)
- NFTs (no token standard yet)

### Is UltraDAG production-ready?

**Current Status:** Testnet  
**Mainnet:** Planned (pending final audits and documentation)

**Production Features:**
- ✅ Complete consensus implementation
- ✅ Checkpoint system with fast-sync
- ✅ Governance protocol
- ✅ Comprehensive metrics
- ✅ Operations runbook
- ✅ 335+ tests passing

---

## Getting Started

### How do I get UDAG tokens?

**Testnet:**
```bash
curl -X POST http://testnet.ultradag.io:10333/faucet \
  -H "Content-Type: application/json" \
  -d '{"address": "YOUR_ADDRESS"}'
```

**Mainnet:** (when launched)
- Purchase from exchanges
- Earn as validator rewards
- Receive from other users

### How do I create a wallet?

**Option 1: Generate keys via RPC (testing only)**
```bash
curl http://localhost:10333/keygen
```

**Option 2: Use a library (recommended)**
```javascript
const nacl = require('tweetnacl');
const blake3 = require('blake3');

const keypair = nacl.sign.keyPair();
const address = blake3.hash(keypair.publicKey);
```

**Option 3: Use official wallet** (coming soon)

### How do I check my balance?

```bash
curl http://localhost:10333/balance/YOUR_ADDRESS
```

Response:
```json
{
  "address": "YOUR_ADDRESS",
  "balance": 100000000,
  "nonce": 0
}
```

### How do I send a transaction?

See the [Integration Guide](guides/development/integration-guide.md) for complete examples.

**Quick steps:**
1. Get current nonce
2. Build transaction
3. Sign with Ed25519
4. Submit via `/tx` endpoint

### What is the minimum transaction amount?

**Minimum Amount:** 1 satoshi (0.00000001 UDAG)  
**Minimum Fee:** 1,000 satoshis (0.00001 UDAG)  
**Recommended Fee:** 10,000 satoshis (0.0001 UDAG)

---

## Transactions

### How long do transactions take to confirm?

**Typical Confirmation Time:** 10-15 seconds (2-3 rounds)

**Stages:**
1. Submitted to mempool: Instant
2. Included in vertex: ~5 seconds (1 round)
3. Vertex finalized: ~10-15 seconds (2-3 rounds)

### What is a nonce and why do I need it?

A **nonce** is a transaction counter that prevents replay attacks. Each account has a nonce that starts at 0 and increments by 1 for each transaction.

**Example:**
- First transaction: nonce = 0
- Second transaction: nonce = 1
- Third transaction: nonce = 2

**Important:** Always fetch the current nonce before creating a transaction:
```bash
curl http://localhost:10333/balance/YOUR_ADDRESS | jq .nonce
```

### Why did my transaction fail?

**Common Reasons:**

1. **Insufficient Balance**
   - Error: "insufficient balance"
   - Solution: Ensure `balance >= amount + fee`

2. **Wrong Nonce**
   - Error: "invalid nonce"
   - Solution: Fetch current nonce and use exact value

3. **Invalid Signature**
   - Error: "signature verification failed"
   - Solution: Check signing process, ensure correct network ID

4. **Rate Limited**
   - Error: "rate limit exceeded"
   - Solution: Wait 1 minute and retry

5. **Malformed Transaction**
   - Error: "invalid address hex"
   - Solution: Verify address format (64 hex characters)

### Can I cancel a transaction?

**No.** Once submitted to the mempool, transactions cannot be cancelled. However:
- If not yet finalized, you can submit a competing transaction with higher fee
- Unconfirmed transactions may be dropped if mempool is full

### What happens if I send to an invalid address?

**If address format is invalid:** Transaction rejected immediately  
**If address format is valid but doesn't exist:** Transaction succeeds, creates new account with that address

**Note:** Always verify recipient addresses before sending.

### How do I speed up a transaction?

**During Submission:**
- Increase fee (higher fee = higher priority)
- Recommended fee: 10,000 satoshis

**After Submission:**
- Cannot speed up existing transaction
- Can submit new transaction with higher fee (if nonce allows)

---

## Staking & Validation

### How much do I need to stake?

**Minimum Stake:** 0.1 UDAG (10,000,000 satoshis)  
**Recommended Stake:** 1+ UDAG for meaningful rewards

### How do I become a validator?

See the [Validator Handbook](guides/validators/validator-handbook.md) for complete guide.

**Quick steps:**
1. Set up node infrastructure
2. Generate validator keys (offline)
3. Acquire minimum stake (0.1 UDAG)
4. Submit stake transaction
5. Start validator node with secret key

### How much can I earn as a validator?

**Earnings depend on:**
- Number of validators (dilutes rewards)
- Your uptime (missed vertices = missed rewards)
- Transaction volume (fees)
- Current epoch (halving reduces rewards)

**Early Epoch Example:**
- Block reward: 50 UDAG per vertex
- With 4 validators, 5s rounds: ~54,000 UDAG/day
- APY: Very high initially, decreases over time

### How long does unstaking take?

**Cooldown Period:** 2,016 rounds (~2.8 hours at 5s rounds)

**Process:**
1. Submit unstake transaction
2. Wait for cooldown period
3. Tokens automatically returned to balance

**During cooldown:** Stake is locked, no rewards earned

### Can I unstake partially?

**Yes**, as long as:
- Remaining stake ≥ 0.1 UDAG (minimum), OR
- Remaining stake = 0 (full unstake)

**Example:**
```
Staked: 1 UDAG
Unstake: 0.5 UDAG → Remaining: 0.5 UDAG ✅
Unstake: 0.4 UDAG → Remaining: 0.1 UDAG ✅
Unstake: 0.05 UDAG → Remaining: 0.05 UDAG ❌ (below minimum)
```

### What happens if my validator goes offline?

**Current Implementation:**
- No rewards earned while offline
- No slashing (no economic penalty)
- Can resume when back online

**Future Implementation:**
- Potential downtime penalties
- Slashing for extended outages

### What is equivocation?

**Equivocation** = Producing two different vertices in the same round

**Penalty:** Permanent ban from network (no economic slashing yet)

**How to Avoid:**
- Never run multiple nodes with same validator key
- Ensure proper failover mechanisms
- Monitor for clock drift

---

## Governance

### Who can vote on proposals?

**Only active validators** can vote on governance proposals.

**Vote Weight:** Proportional to stake amount (1 satoshi = 1 vote)

### How do I create a proposal?

```bash
curl -X POST http://localhost:10333/proposal \
  -H "Content-Type: application/json" \
  -d '{
    "proposer": "YOUR_ADDRESS",
    "proposal_type": {
      "Text": {
        "title": "Proposal Title",
        "description": "Description"
      }
    },
    "nonce": CURRENT_NONCE,
    "pub_key": "YOUR_PUBLIC_KEY",
    "signature": "SIGNATURE"
  }'
```

### How long is the voting period?

**Default Voting Period:** 2,016 rounds (~2.8 hours at 5s rounds)

### What types of proposals are supported?

1. **Text Proposals** - Non-binding signaling
2. **Parameter Changes** - Protocol parameter updates
3. **Validator Set Changes** - Add/remove validators (future)

### How are proposals executed?

**Automatic Execution:**
- Proposal passes if: `yes_votes > total_stake / 2` (simple majority)
- Execution occurs automatically when voting period ends
- Parameter changes take effect in next finalized vertex

---

## Technical Questions

### What consensus algorithm does UltraDAG use?

**DAG-BFT (Directed Acyclic Graph Byzantine Fault Tolerant)**

**Key Properties:**
- Leaderless (no leader election)
- Implicit voting (DAG structure = votes)
- Descendant-coverage finality
- 2f+1 quorum requirement

### What is finality lag?

**Finality Lag** = Current round - Last finalized round

**Typical Values:**
- Healthy: 2-3 rounds
- Warning: >10 rounds
- Critical: >100 rounds

**Check finality lag:**
```bash
curl http://localhost:10333/health/detailed | jq '.components.finality.finality_lag'
```

### What is a checkpoint?

**Checkpoint** = Snapshot of finalized state at specific round

**Purpose:**
- Fast-sync for new nodes (30-120 seconds)
- State recovery after crashes
- Reduce sync time

**Frequency:** Every 100 finalized rounds

### How does fast-sync work?

1. New node requests latest checkpoint from peers
2. Receives checkpoint with BFT quorum signatures
3. Verifies signatures and state root
4. Loads state directly (skips replaying history)
5. Syncs recent vertices after checkpoint
6. Node is current in 30-120 seconds

### What happens if a node crashes?

Between full snapshots (every 10 rounds), every finalized vertex batch is recorded in a **write-ahead log (WAL)** (`wal.jsonl`). Each WAL entry is fsync'd to disk before the node proceeds. On restart, WAL entries since the last snapshot are replayed with state_root verification, recovering any finalized transactions that weren't captured in the last snapshot. This ensures no finalized work is lost, even during a crash between snapshots.

### What cryptography does UltraDAG use?

| Purpose | Algorithm |
|---------|-----------|
| Signatures | Ed25519 |
| Hashing | Blake3 |
| Address Derivation | Blake3(public_key) |

### Is UltraDAG quantum-resistant?

**No.** Ed25519 is not quantum-resistant.

**Future Plans:**
- Post-quantum signature schemes
- Hybrid classical/quantum approach
- Requires protocol upgrade

---

## Troubleshooting

### Node won't start

**Check logs:**
```bash
sudo journalctl -u ultradag -n 100
```

**Common Issues:**

**1. Port already in use**
```
Error: Address already in use (os error 98)
```
**Solution:**
```bash
# Check what's using the port
sudo lsof -i :9333
# Kill the process or change port
ultradag-node --listen 0.0.0.0:9334
```

**2. Permission denied**
```
Error: Permission denied (os error 13)
```
**Solution:**
```bash
# Fix permissions
sudo chown -R ultradag:ultradag /var/lib/ultradag
# Or run as correct user
sudo -u ultradag ultradag-node
```

**3. Missing data directory**
```
Error: No such file or directory
```
**Solution:**
```bash
mkdir -p /var/lib/ultradag
```

### High finality lag

**Symptoms:** Finality lag >10 rounds

**Diagnosis:**
```bash
curl http://localhost:10333/health/detailed | jq '.components.finality'
```

**Common Causes & Solutions:**

**1. Network partition**
```bash
# Check peer connections
curl http://localhost:10333/peers
# Should have 3+ peers
```

**2. Clock drift**
```bash
# Sync system time
sudo timedatectl set-ntp true
sudo systemctl restart systemd-timesyncd
```

**3. Insufficient validators**
```bash
# Check validator count
curl http://localhost:10333/validators | jq 'length'
# Should match configured count
```

### No peer connections

**Symptoms:** `peer_count: 0`

**Diagnosis:**
```bash
curl http://localhost:10333/peers
sudo netstat -tulpn | grep 9333
```

**Solutions:**

**1. Check firewall**
```bash
sudo ufw status
sudo ufw allow 9333/tcp
```

**2. Verify bootstrap peers**
```bash
# Test connectivity
nc -zv node1.ultradag.io 9333
```

**3. Check node logs**
```bash
sudo journalctl -u ultradag | grep "peer"
```

### Transaction stuck in mempool

**Symptoms:** Transaction not confirming after 30+ seconds

**Diagnosis:**
```bash
curl http://localhost:10333/mempool | jq '.[] | select(.hash == "TX_HASH")'
```

**Possible Causes:**

1. **Low fee** - Increase fee for future transactions
2. **Network congestion** - Wait for mempool to clear
3. **Invalid transaction** - Check logs for validation errors

**Solution:**
```bash
# Wait for next round
# Or submit new transaction with higher fee
```

### Balance not updating

**Symptoms:** Sent transaction but balance unchanged

**Diagnosis:**
```bash
# Check transaction in mempool
curl http://localhost:10333/mempool | grep YOUR_ADDRESS

# Check finality lag
curl http://localhost:10333/health/detailed | jq '.components.finality.finality_lag'
```

**Solutions:**

1. **Wait for finalization** - Typically 10-15 seconds
2. **Check transaction status** - May have failed validation
3. **Verify nonce** - Ensure correct nonce was used

### High memory usage

**Symptoms:** Node using >500 MB RAM

**Diagnosis:**
```bash
ps aux | grep ultradag-node
```

**Solutions:**

**1. Restart node** (clears memory)
```bash
sudo systemctl restart ultradag
```

**2. Monitor over time**
```bash
watch -n 5 'ps aux | grep ultradag-node'
```

**3. Check for memory leak**
```bash
# If memory keeps increasing, report issue
```

---

## Performance

### How can I improve transaction speed?

**1. Increase fee**
- Higher fee = higher priority in mempool
- Recommended: 10,000 satoshis

**2. Use local node**
- Reduces network latency
- Faster transaction submission

**3. Batch transactions**
- Submit multiple transactions in sequence
- Manage nonces carefully

### How can I reduce node resource usage?

**1. Increase round duration**
```bash
ultradag-node --round-ms 10000  # 10 second rounds
```

**2. Limit peer connections**
```bash
ultradag-node --max-peers 4
```

**3. Use SSD storage**
- Faster checkpoint loading
- Better I/O performance

### What are the hardware requirements?

**Minimum (Full Node):**
- 2 CPU cores
- 2 GB RAM
- 20 GB SSD
- 10 Mbps network

**Recommended (Validator):**
- 4 CPU cores
- 4 GB RAM
- 50 GB NVMe SSD
- 100 Mbps network

---

## Security

### How do I secure my validator keys?

**Best Practices:**

1. **Generate offline**
   - Use air-gapped machine
   - Never expose over network

2. **Encrypt storage**
   - Use AES-256-GCM
   - Store in encrypted vault (HashiCorp Vault, AWS Secrets Manager)

3. **Restrict permissions**
   ```bash
   chmod 600 /etc/ultradag/validator.env
   chown ultradag:ultradag /etc/ultradag/validator.env
   ```

4. **Use HSM** (production)
   - Hardware security module
   - Prevents key extraction

5. **Regular rotation**
   - Unstake with old key
   - Wait cooldown
   - Stake with new key

### What if my keys are compromised?

**Immediate Actions:**

1. **Stop validator node**
   ```bash
   sudo systemctl stop ultradag
   ```

2. **Unstake tokens**
   - Submit unstake transaction
   - Wait for cooldown period

3. **Generate new keys**
   - Use secure offline method
   - Never reuse compromised keys

4. **Monitor for unauthorized activity**
   - Check transaction history
   - Look for unexpected vertices

### How do I protect against DDoS?

**1. Firewall rules**
```bash
# Rate limit connections
sudo iptables -A INPUT -p tcp --dport 9333 -m limit --limit 25/minute --limit-burst 100 -j ACCEPT
```

**2. Reverse proxy**
```nginx
# nginx rate limiting
limit_req_zone $binary_remote_addr zone=rpc:10m rate=10r/s;
```

**3. Multiple nodes**
- Distribute load
- Geographic redundancy

**4. DDoS protection service**
- Cloudflare
- AWS Shield
- Dedicated DDoS mitigation

---

## Additional Resources

**Documentation:**
- [Whitepaper](reference/specifications/whitepaper.md)
- [RPC API Reference](reference/api/rpc-endpoints.md)
- [Node Operator Guide](guides/operations/node-operator-guide.md)
- [Validator Handbook](guides/validators/validator-handbook.md)
- [Integration Guide](guides/development/integration-guide.md)

**Community:**
- GitHub: https://github.com/UltraDAGcom/core
- Discord: https://discord.gg/ultradag
- Twitter: @UltraDAG

**Support:**
- GitHub Discussions: https://github.com/UltraDAGcom/core/discussions
- Email: support@ultradag.io
- Emergency: ops@ultradag.io

---

**Last Updated:** March 10, 2026  
**Document Version:** 1.0  
**Maintainer:** UltraDAG Core Team
