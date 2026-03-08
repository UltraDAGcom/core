# Bug Bounty Hunter's Guide

Quick start guide for security researchers participating in the UltraDAG bug bounty program.

## Getting Started (5 minutes)

### 1. Get Testnet UDAG
```bash
# Generate a test address (or use existing wallet)
curl -X POST https://ultradag-node-1.fly.dev/faucet \
  -H "Content-Type: application/json" \
  -d '{"address":"your_udag_address_here"}'
```

You'll receive 10,000 testnet UDAG to start testing.

### 2. Explore the Testnet
```bash
# Check node status
curl https://ultradag-node-1.fly.dev/status | jq

# View recent rounds
curl https://ultradag-node-1.fly.dev/round/100 | jq

# Check your balance
curl https://ultradag-node-1.fly.dev/balance/your_address | jq
```

### 3. Review the Codebase
```bash
git clone https://github.com/[your-org]/ultradag.git
cd ultradag
cargo test  # Run test suite
```

## Attack Vectors to Explore

### 🎯 High-Value Targets

**1. Consensus Mechanism** (`crates/ultradag-coin/src/dag/`)
- Try to create conflicting finalized transactions
- Attempt to stall the network
- Test validator quorum bypasses
- Look for round synchronization issues

**2. State Engine** (`crates/ultradag-coin/src/state.rs`)
- Balance overflow/underflow
- Nonce manipulation
- Fee bypass
- Supply inflation

**3. Staking System** (`crates/ultradag-coin/src/tx/stake.rs`)
- Unauthorized unstaking
- Reward manipulation
- Stake without locking funds
- Double-staking

**4. P2P Network** (`crates/ultradag-network/`)
- Eclipse attacks
- Partition attacks
- Message replay
- Peer flooding

**5. RPC Endpoints** (`crates/ultradag-node/src/rpc.rs`)
- Rate limiting bypass
- Input validation
- DoS attacks
- Information leakage

### 🔧 Testing Tools

**Fuzzing:**
```bash
# Run existing fuzz tests
./scripts/fuzzing_test.sh

# Create custom fuzz inputs
cargo fuzz run fuzz_target_name
```

**Load Testing:**
```bash
# Spam transactions
for i in {1..1000}; do
  curl -X POST https://ultradag-node-1.fly.dev/tx \
    -H "Content-Type: application/json" \
    -d '{"secret_key":"...","to":"...","amount":1000,"fee":10000}'
done
```

**Network Testing:**
```bash
# Monitor all 4 nodes
./scripts/monitor.sh

# Extended monitoring
./scripts/extended_monitor.sh
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

### Testnet Address
[Your address for bounty: udag1...]
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
- [BUG_BOUNTY.md](../BUG_BOUNTY.md) - Full program details
- [SECURITY.md](../SECURITY.md) - Security policy
- [CLAUDE.md](../CLAUDE.md) - Technical documentation
- [BOUNTY_LEDGER.md](../BOUNTY_LEDGER.md) - Reward tracking

### Code Locations
- Consensus: `crates/ultradag-coin/src/dag/`
- State: `crates/ultradag-coin/src/state.rs`
- Transactions: `crates/ultradag-coin/src/tx/`
- Network: `crates/ultradag-network/`
- RPC: `crates/ultradag-node/src/rpc.rs`
- Validator: `crates/ultradag-node/src/validator.rs`

### Test Suites
```bash
# Run all tests
cargo test

# Specific test suites
cargo test --test fuzzing_tests
cargo test --test consensus_tests
cargo test --test staking_tests
```

### Monitoring
```bash
# Real-time monitoring
./scripts/monitor.sh

# Extended 24-hour monitoring
./scripts/extended_monitor.sh

# Check specific node
curl https://ultradag-node-1.fly.dev/status | jq
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

- **Submit bugs:** GitHub Security Advisory
- **Questions:** GitHub Discussions
- **Emergency:** [Contact method for critical issues]

---

**Happy hunting! 🎯**

Help us make UltraDAG more secure and earn UDAG rewards in the process.
