---
title: Bug Bounty
---

# Bug Bounty Program

UltraDAG operates an active bug bounty program to incentivize responsible disclosure of security vulnerabilities.

---

## Program Overview

| Detail | Value |
|--------|-------|
| Status | Active |
| Total pool | 500,000 UDAG |
| Submission method | GitHub Security Advisories |
| Response time | Validation within 24 hours |
| Embargo period | 90 days |

---

## Reward Tiers

| Severity | Reward Range | Examples |
|----------|-------------|---------|
| **Critical** | 10,000 - 50,000 UDAG | Consensus break, double-spend, supply inflation, cryptographic break, remote code execution |
| **High** | 5,000 - 10,000 UDAG | DoS causing network-wide impact, resource exhaustion affecting all nodes, staking exploit, eclipse attack |
| **Medium** | 1,000 - 5,000 UDAG | RPC vulnerability, rate limiting bypass, mempool manipulation, non-determinism in non-critical path |
| **Low** | 100 - 1,000 UDAG | Input validation gaps, performance issues, minor information disclosure, edge case bugs |

### Reward Factors

The exact reward within each tier depends on:

- **Impact**: how severe the consequences would be if exploited
- **Exploitability**: how difficult it is to exploit in practice
- **Quality of report**: completeness of description, proof of concept, and suggested fix
- **Scope**: how many users or nodes would be affected

---

## Scope

### In Scope

| Component | Description |
|-----------|-------------|
| Consensus (`ultradag-coin`) | DAG-BFT finality, vertex validation, equivocation detection |
| P2P network (`ultradag-network`) | Noise protocol, message handling, sync protocol, rate limiting |
| State engine | Account state, staking, delegation, supply invariant |
| Staking | Reward distribution, slashing, epoch transitions, delegation cascade |
| RPC server | All HTTP endpoints, input validation, authentication |
| Cryptography | Ed25519 signatures, Blake3 hashing, Noise handshake, domain separation |

### Out of Scope

- Vulnerabilities in third-party dependencies (report to upstream maintainer)
- Social engineering attacks
- Denial of service via resource exhaustion on the testnet (rate limiting is already in place)
- Issues in documentation or website (non-security)
- Issues requiring physical access to the host machine

---

## Submission Process

### Step 1: Discover

Find a vulnerability in an in-scope component.

### Step 2: Document

Prepare a report including:

- Description of the vulnerability
- Steps to reproduce
- Proof of concept (code, scripts, or detailed explanation)
- Estimated impact and severity
- Suggested fix (optional but appreciated)

### Step 3: Submit

Submit via [GitHub Security Advisories](https://github.com/UltraDAGcom/core/security/advisories/new). This creates a private report visible only to the repository maintainers.

!!! warning "Do not disclose publicly"
    Please do not open public GitHub issues for security vulnerabilities. Use the private Security Advisories feature. The 90-day embargo gives us time to develop and deploy a fix.

### Step 4: Validation

We will:

1. Acknowledge receipt within 24 hours
2. Validate the vulnerability within 72 hours
3. Assign a severity rating
4. Develop and test a fix
5. Deploy the fix to testnet
6. Issue the reward

---

## Mainnet Token Conversion

Bug bounty rewards are tracked in `BOUNTY_LEDGER.md` and will be honored with mainnet UDAG tokens at launch:

| Milestone | Unlock |
|-----------|--------|
| Mainnet launch | 25% of reward unlocked |
| 3 months post-launch | 25% unlocked |
| 6 months post-launch | 25% unlocked |
| 12 months post-launch | Final 25% unlocked |

To claim mainnet tokens, prove ownership of the testnet address that received the bounty reward.

---

## Rules

1. **One vulnerability per report** — submit separate reports for separate issues
2. **No automated scanning** — do not run automated scanners against the public testnet
3. **No disruption** — do not exploit vulnerabilities on the live testnet beyond proof of concept
4. **Good faith** — act in good faith to avoid privacy violations, data destruction, and service disruption
5. **First reporter** — rewards go to the first reporter of a unique vulnerability
6. **No duplicates** — issues already known and documented (in CLAUDE.md, BOUNTY_LEDGER.md, or closed PRs) are not eligible

---

## Hall of Fame

Security researchers who have contributed valid vulnerability reports will be recognized here upon mainnet launch (with their permission).

---

## Contact

- **Security reports**: [GitHub Security Advisories](https://github.com/UltraDAGcom/core/security/advisories/new)
- **General questions**: Open a GitHub Discussion

---

## Next Steps

- [Security Model](model.md) — understanding the security architecture
- [Audit Reports](audits.md) — past audit findings
