# Bug Bounty Hunter's Guide

Quick start guide for security researchers participating in the UltraDAG bug bounty program.

## Getting Started (5 minutes)

### 1. Get Testnet UDAG

> **⚠️ Back up your testnet secret key before doing anything else.** The only
> way to prove address ownership at mainnet conversion time is a cryptographic
> signature from the key behind the address you put in your bounty report.
> Lose the key → lose any future payout, permanently. Testnet UDAG balances
> can be wiped by a `--clean` restart, but the key + `LEDGER.md` entry
> together are what bind the commitment — see
> [testnet reset safety](./LEDGER.md#testnet-reset-safety) in `LEDGER.md`
> for the full explanation. Prefer a long-lived testnet address used across
> multiple reports; don't generate a fresh throwaway per report.

```bash
# Generate a test address (or use existing wallet). `amount` is in sats —
# 10_000_000_000 sats = 100 UDAG, which is the per-request maximum.
curl -X POST https://ultradag-node-1.fly.dev/faucet \
  -H "Content-Type: application/json" \
  -d '{"address":"tudg1your_testnet_address_here","amount":10000000000}'
```

You'll receive up to 100 testnet UDAG per request. Rate-limited to 1 request per 10 minutes. Use the output address from any wallet — testnet addresses use the `tudg1…` bech32m prefix.

### 2. Explore the Testnet
```bash
# Check node status (current round, validator count, supply, peers)
curl -s https://ultradag-node-1.fly.dev/status | jq

# Query a recent round. Round numbers older than ~1000 rounds are pruned;
# grab a current number from /status first.
ROUND=$(curl -s https://ultradag-node-1.fly.dev/status | jq '.last_finalized_round')
curl -s "https://ultradag-node-1.fly.dev/round/$ROUND" | jq

# Check your balance (accepts `tudg1…`, 40-hex, or a registered @name)
curl -s https://ultradag-node-1.fly.dev/balance/tudg1your_testnet_address | jq
```

All 5 testnet nodes are interchangeable: `https://ultradag-node-[1-5].fly.dev`.

### 3. Review the Codebase
```bash
git clone https://github.com/UltraDAGcom/core.git ultradag
cd ultradag
cargo test  # Run the full test suite
```

## Attack Vectors to Explore

### 🎯 High-Value Targets

**1. Consensus / DAG** (`crates/ultradag-coin/src/consensus/`)
- Try to create conflicting finalized transactions
- Attempt to stall the network (equivocation, stuck parents, partition)
- Test validator quorum bypasses
- Look for finality race conditions (2-3 round BFT finality)

**2. State Engine** (`crates/ultradag-coin/src/state/`)
- Balance overflow/underflow
- Nonce manipulation
- Fee bypass
- Supply inflation (look for any path that credits without a matching total_supply bump)
- Supply invariant violations — these are FATAL on the node (exit code 101), so any path that slips past validation is critical

**3. Transactions** (`crates/ultradag-coin/src/tx/`)
- Transfer (`tx/transaction.rs`)
- Staking (`tx/stake.rs`): unauthorized unstaking, reward manipulation, stake without locking funds, double-staking
- Delegation (`tx/stake.rs`): commission sandwich attacks, delegation slashing bypass
- SmartAccount / WebAuthn (`tx/smart_account.rs`): passkey signature bypass, AddKey/RemoveKey race conditions, pocket derivation collisions
- Name registry (`tx/name_registry.rs`): name squatting, expiry manipulation
- Bridge (`tx/bridge.rs`): attestation replay, release nonce reuse, chain-ID confusion
- Governance (`governance/`): quorum manipulation, proposal execution bypass, council-seat abuse

**4. P2P Network** (`crates/ultradag-network/`)
- Eclipse attacks
- Partition attacks
- Message replay (Noise handshake, postcard decode)
- Peer flooding / orphan buffer overflow
- Equivocation gossip

**5. RPC Endpoints** (`crates/ultradag-node/src/rpc.rs`)
- Rate limiting bypass
- Input validation (address parsing, bech32m, numeric overflow)
- DoS attacks
- Information leakage in error messages

### 🔧 Testing Tools

**Fuzzing:**
```bash
# Run the fuzz harness
./tools/development/testing/security/fuzzing-test.sh

# Run adversarial tests
./tools/development/testing/security/adversarial-test.sh

# Rate-limit tests
./tools/development/testing/security/rate-limiting-test.sh

# Custom cargo-fuzz targets (if you want to fuzz a specific parser):
# cd crates/<crate>/fuzz && cargo fuzz run <target>
```

**Load Testing:**
```bash
# Spam transactions via the testnet /tx endpoint (accepts secret-key-in-body;
# this format is DISABLED on mainnet — only /tx/submit with pre-signed txs
# is accepted there). For high-volume load tests, use:
./tools/development/testing/performance/tps-test.sh

# Or a quick inline loop:
for i in {1..100}; do
  curl -s -X POST https://ultradag-node-1.fly.dev/tx \
    -H "Content-Type: application/json" \
    -d '{"secret_key":"<your_testnet_hex>","to":"tudg1...","amount":1000,"fee":10000}'
done
```

**Network Testing:**
```bash
# Quick multi-node health sweep (all 5 testnet nodes):
for i in 1 2 3 4 5; do
  curl -s --max-time 5 "https://ultradag-node-$i.fly.dev/status" \
    | jq -c '{n:'\''node-'$i\''', round:.dag_round, fin:.last_finalized_round, peers:.peer_count}'
done

# Prometheus-style metrics:
curl -s https://ultradag-node-1.fly.dev/metrics/json | jq
```

## Common Vulnerability Patterns

### 1. Integer Overflow/Underflow
```rust
// Look for unchecked arithmetic
balance + amount  // Should be: balance.checked_add(amount)?
balance - fee     // Should be: balance.checked_sub(fee)?
```

### 2. Race Conditions
```rust
// Check for TOCTOU (Time-of-check to time-of-use)
if state.get_balance(addr) >= amount {
    // What if balance changes here?
    state.deduct_balance(addr, amount)?;
}
```

### 3. Replay Attacks
```rust
// Ensure nonces are enforced
// Try resubmitting old transactions
```

### 4. Input Validation
```bash
# Test edge cases
curl -X POST .../tx -d '{"amount":-1,...}'  # Negative amounts
curl -X POST .../tx -d '{"amount":18446744073709551615,...}'  # Max u64
curl -X POST .../tx -d '{"secret_key":"0000...","to":"0000...",...}'  # Invalid keys
```

### 5. DoS Vectors
```bash
# Memory exhaustion
# Send huge payloads
curl -X POST .../tx -d '{"data":"'$(python3 -c 'print("A"*10000000)')'"}'

# CPU exhaustion
# Trigger expensive operations repeatedly

# Connection exhaustion
# Open many connections simultaneously
```

## Example Exploits (Hypothetical)

### Double-Spend Attempt
```bash
# 1. Create two conflicting transactions with same nonce
TX1='{"secret_key":"...","to":"addr1","amount":1000,"fee":10000,"nonce":5}'
TX2='{"secret_key":"...","to":"addr2","amount":1000,"fee":10000,"nonce":5}'

# 2. Submit to different nodes simultaneously
curl -X POST https://ultradag-node-1.fly.dev/tx -d "$TX1" &
curl -X POST https://ultradag-node-2.fly.dev/tx -d "$TX2" &

# 3. Check if both get finalized
# (They shouldn't - this would be a critical bug!)
```

### Consensus Stall
```bash
# Try to prevent validators from producing vertices
# - Flood with invalid transactions
# - Partition the network
# - Exhaust resources
```

### Rate Limit Bypass
```bash
# Try various bypass techniques
# - IP spoofing (if possible)
# - Distributed requests
# - Timing attacks
# - Header manipulation
```

## Reporting Template

```markdown
## Vulnerability: [Short Title]

### Severity
[Your assessment: Critical/High/Medium/Low]

### Component
[Consensus/Network/RPC/State/Staking/Other]

### Description
[Detailed explanation of the vulnerability]

### Impact
- **Attacker capability:** [What can they do?]
- **Affected users:** [Who is impacted?]
- **Likelihood:** [Easy/Medium/Hard to exploit]
- **Damage potential:** [Financial loss? Network halt? Data leak?]

### Reproduction Steps
1. [Step 1 with exact commands]
2. [Step 2]
3. [Step 3]
4. [Expected behavior vs actual behavior]

### Proof of Concept
```bash
# Exact commands to reproduce
curl -X POST https://ultradag-node-1.fly.dev/... \
  -d '...'
```

### Suggested Fix
[Optional - your recommendation]

### Testnet Address (REQUIRED — this is how you'll be paid)
[Your tudg1... address. This goes into the ledger as your claim identity.
You'll need to sign a challenge with the key behind this address to claim
the mainnet UDAG reward. Make sure you have the secret key backed up.]
```

## Reward Examples

### Critical (10,000 - 50,000 UDAG)
- "Found method to create UDAG from nothing by exploiting integer overflow in state engine"
- "Discovered consensus bug allowing double-finalization of conflicting transactions"
- "Network-wide DoS causing permanent stall via malformed DAG vertex"

### High (5,000 - 10,000 UDAG)
- "RPC endpoint crash via crafted JSON payload"
- "Memory leak in P2P handler causing OOM after 1000 connections"
- "Staking exploit allowing withdrawal without lock period"

### Medium (1,000 - 5,000 UDAG)
- "Rate limiting bypass using header manipulation"
- "Mempool DoS via transaction spam with minimum fees"
- "Information disclosure in error messages revealing internal state"

### Low (100 - 1,000 UDAG)
- "Missing input validation on address field allows invalid formats"
- "Inefficient DAG traversal causing 10x slowdown on large graphs"
- "Minor race condition in metrics collection (no security impact)"

## Tips for Success

### ✅ Do This
- **Be thorough:** Document everything clearly
- **Be creative:** Think outside the box
- **Be responsible:** Don't harm the network
- **Be patient:** Give us time to fix issues
- **Be collaborative:** Work with us, not against us

### ❌ Avoid This
- **Don't spam:** Quality over quantity
- **Don't publicize:** Keep findings private until fixed
- **Don't exploit:** Testing only, no malicious use
- **Don't duplicate:** Check existing reports first
- **Don't rush:** Take time to validate your findings

## Resources

### Documentation
- [`PROGRAM.md`](./PROGRAM.md) — Full bug bounty program details
- [`../../../SECURITY.md`](../../../SECURITY.md) — Security policy & disclosure channel
- [`../../reference/api/rpc-endpoints.md`](../../reference/api/rpc-endpoints.md) — RPC API reference
- [`LEDGER.md`](./LEDGER.md) — Reward tracking + mainnet conversion schedule
- [`PROMOTION.md`](./PROMOTION.md) — Outreach channels (for program maintainers)

### Code Locations
- Consensus / DAG: `crates/ultradag-coin/src/consensus/`
- State engine: `crates/ultradag-coin/src/state/`
- Transactions: `crates/ultradag-coin/src/tx/`
- Governance: `crates/ultradag-coin/src/governance/`
- Network: `crates/ultradag-network/`
- RPC: `crates/ultradag-node/src/rpc.rs`
- Validator loop: `crates/ultradag-node/src/validator.rs`
- Bridge (Arbitrum): `bridge/`

### Test Suites
```bash
# Run the full workspace test suite
cargo test

# Run ultradag-coin tests only (fastest feedback for consensus/state work)
cargo test -p ultradag-coin

# Run a specific integration test file by name (see
# crates/ultradag-coin/tests/ for the full list):
cargo test -p ultradag-coin --test adversarial
cargo test -p ultradag-coin --test dag_bft_finality
cargo test -p ultradag-coin --test staking
cargo test -p ultradag-coin --test supply_invariant_fatal
cargo test -p ultradag-coin --test economics_audit
cargo test -p ultradag-coin --test cross_batch_equivocation
```

### Monitoring
```bash
# Quick multi-node health sweep:
for i in 1 2 3 4 5; do
  curl -s --max-time 5 "https://ultradag-node-$i.fly.dev/status" \
    | jq -c '{n:'\''node-'$i\''', round:.dag_round, fin:.last_finalized_round, peers:.peer_count, mempool:.mempool_size}'
done

# Single node with full detail:
curl -s https://ultradag-node-1.fly.dev/status | jq
curl -s https://ultradag-node-1.fly.dev/health/detailed | jq
curl -s https://ultradag-node-1.fly.dev/metrics/json | jq
```

## FAQ

**Q: How long does validation take?**  
A: 1-7 days typically. Critical issues are prioritized.

**Q: Can I test in production?**  
A: Only on testnet! Mainnet attacks are illegal.

**Q: What if I can't reproduce the bug reliably?**  
A: Submit anyway with as much detail as possible. We'll investigate.

**Q: Can I use automated tools?**  
A: Yes! Fuzzing, scanning, etc. are all allowed.

**Q: How do I prove I found it first?**  
A: Timestamp matters. Submit as soon as you validate the issue.

**Q: What if my submission is rejected?**  
A: We'll explain why. You can appeal or resubmit with more evidence.

## Contact

- **Submit bugs (private):** <https://github.com/UltraDAGcom/core/security/advisories/new>
  or click the green "Report a vulnerability" button at
  <https://github.com/UltraDAGcom/core/security>
- **General questions (public):** <https://github.com/UltraDAGcom/core/discussions>
- **Critical issues:** for anything that could threaten the running mainnet
  — use the private advisory URL above. The acknowledgment SLA per
  [`SECURITY.md`](../../../SECURITY.md) is 24 hours for Critical/High severity.
  Do NOT post exploit details on Twitter, Discord, Telegram, or the public
  issue tracker.

---

**Happy hunting! 🎯**

Help us make UltraDAG more secure and earn UDAG rewards in the process.
