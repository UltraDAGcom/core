# Bug Bounty Quick Start

## For Security Researchers

**Want to earn UDAG?** Here's how to get started in 5 minutes:

### 1. Read the Program
📖 [bug-bounty/PROGRAM.md](./bug-bounty/PROGRAM.md) - Full details  
🎯 [bug-bounty/GUIDE.md](./bug-bounty/GUIDE.md) - Quick start guide

### 2. Get Testnet Tokens
```bash
curl -X POST https://ultradag-node-1.fly.dev/faucet \
  -H "Content-Type: application/json" \
  -d '{"address":"your_udag_address"}'
```

### 3. Test the Network
```bash
# Check status
curl https://ultradag-node-1.fly.dev/status | jq

# View rounds
curl https://ultradag-node-1.fly.dev/round/100 | jq
```

### 4. Report Vulnerabilities
Use **GitHub Security Advisory** for private reporting

### Rewards
- 🔴 Critical: 10k-50k UDAG
- 🟠 High: 5k-10k UDAG
- 🟡 Medium: 1k-5k UDAG
- 🟢 Low: 100-1k UDAG

---

## For Maintainers

### Enable the Program
1. Enable GitHub Security Advisories in repo settings
2. Announce on social media (see [bug-bounty/PROMOTION.md](./bug-bounty/PROMOTION.md))
3. Monitor submissions daily

### Award Bounties
```bash
./scripts/bounty_reward.sh <address> <amount> <severity> "description"
```

### Track Rewards
All rewards tracked in [bug-bounty/LEDGER.md](./bug-bounty/LEDGER.md)

---

**Questions?** See [README.md](./README.md) for full documentation.
