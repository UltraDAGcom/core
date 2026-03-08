# UltraDAG Security

This directory contains all security-related documentation for the UltraDAG project.

## 🔒 Security Policy

**[POLICY.md](./POLICY.md)** - Our security policy and responsible disclosure guidelines
- How to report vulnerabilities
- Response timeline commitments
- Supported versions
- Security best practices

## 💰 Bug Bounty Program

**Status:** 🟢 Active (Testnet Phase)  
**Total Pool:** 500,000 UDAG (mainnet allocation)

### Quick Links
- **[bug-bounty/PROGRAM.md](./bug-bounty/PROGRAM.md)** - Full program details and reward tiers
- **[bug-bounty/GUIDE.md](./bug-bounty/GUIDE.md)** - Hunter's quick start guide
- **[bug-bounty/LEDGER.md](./bug-bounty/LEDGER.md)** - Transparent reward tracking
- **[bug-bounty/LAUNCH.md](./bug-bounty/LAUNCH.md)** - Launch guide (for maintainers)

### Reward Tiers
- 🔴 **Critical:** 10,000 - 50,000 UDAG
- 🟠 **High:** 5,000 - 10,000 UDAG
- 🟡 **Medium:** 1,000 - 5,000 UDAG
- 🟢 **Low:** 100 - 1,000 UDAG

### How to Participate
1. Read the [program details](./bug-bounty/PROGRAM.md)
2. Review the [hunter's guide](./bug-bounty/GUIDE.md)
3. Test the [live testnet](https://ultradag-node-1.fly.dev)
4. Report via GitHub Security Advisory

## 📢 Security Advisories

**[advisories/](./advisories/)** - Published security advisories

All fixed vulnerabilities are disclosed here after patches are deployed.

## 🛠️ Tools

**Reward Distribution:**
```bash
# Located in scripts/bounty_reward.sh
./scripts/bounty_reward.sh <address> <amount> <severity> "description"
```

## 📞 Contact

- **Report vulnerabilities:** GitHub Security Advisory (preferred)
- **Questions:** GitHub Discussions
- **Emergency:** [To be added]

## 🔐 Cryptographic Verification

All bounty rewards are:
- Tracked in git history
- GPG-signed by maintainers
- Publicly auditable
- Binding commitments for mainnet conversion

---

**Last Updated:** March 8, 2026  
**Program Version:** 1.0
