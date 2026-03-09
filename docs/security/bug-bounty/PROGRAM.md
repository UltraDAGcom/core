# UltraDAG Bug Bounty Program

**Status:** Active (Testnet Phase)  
**Launch Date:** March 8, 2026  
**Total Pool:** 500,000 UDAG (mainnet allocation)

## Overview

UltraDAG is offering rewards for security researchers and developers who discover and responsibly disclose vulnerabilities in our testnet. All rewards are tracked and will be honored with mainnet UDAG tokens at launch.

## Scope

**In Scope:**
- Consensus mechanism (DAG, finality, validator logic)
- P2P networking layer
- State engine and transaction processing
- Staking mechanism
- RPC endpoints and rate limiting
- Memory safety and resource management
- Cryptographic implementations

**Out of Scope:**
- Third-party dependencies (report to upstream)
- Social engineering attacks
- Physical attacks on infrastructure
- Testnet-only issues that won't affect mainnet

## Reward Tiers

### 🔴 Critical: 10,000 - 50,000 UDAG
Vulnerabilities that could catastrophically compromise the network:
- **Consensus breaks:** Double-spend, finality violation, fork attacks
- **Network-wide failures:** Permanent stalls, cascading crashes
- **Cryptographic breaks:** Private key extraction, signature forgery
- **State corruption:** Supply inflation, balance manipulation
- **Examples:**
  - Exploit allowing creation of UDAG from nothing
  - Attack forcing permanent network halt
  - Method to finalize conflicting transactions

### 🟠 High: 5,000 - 10,000 UDAG
Severe vulnerabilities affecting availability or individual nodes:
- **DoS attacks:** Crash individual nodes or small groups
- **Resource exhaustion:** Memory leaks, CPU exhaustion
- **Staking exploits:** Unauthorized unstaking, reward manipulation
- **Network attacks:** Partition attacks, eclipse attacks
- **Examples:**
  - Crafted message causing node crash
  - Method to prevent validator from producing vertices
  - Exploit to steal staking rewards

### 🟡 Medium: 1,000 - 5,000 UDAG
Moderate vulnerabilities with limited impact:
- **RPC vulnerabilities:** Authentication bypass, data leakage
- **Rate limiting bypass:** Circumventing DoS protections
- **Mempool manipulation:** Transaction censorship, fee manipulation
- **DAG pruning bugs:** Data loss, incorrect state
- **Examples:**
  - Bypass rate limiting to spam transactions
  - Cause mempool to reject valid transactions
  - Trigger incorrect DAG pruning

### 🟢 Low: 100 - 1,000 UDAG
Minor issues with minimal security impact:
- **Input validation:** Missing checks, edge cases
- **Performance issues:** Inefficient algorithms, slow queries
- **Documentation errors:** Critical security documentation gaps
- **Minor bugs:** Edge cases in non-critical paths
- **Examples:**
  - Missing null checks in RPC handlers
  - Inefficient DAG traversal causing slowdown
  - Incorrect error messages leaking info

## Submission Process

### 1. **Discovery**
- Test against live testnet: https://ultradag-node-1.fly.dev
- Use provided tools: faucet, RPC endpoints, monitoring scripts
- Document reproduction steps clearly

### 2. **Responsible Disclosure**
**DO NOT:**
- Publicly disclose before fix is deployed
- Exploit vulnerabilities for personal gain
- Attack the network maliciously
- Share vulnerabilities with others

**DO:**
- Report privately via GitHub Security Advisory
- Provide detailed reproduction steps
- Suggest potential fixes if possible
- Allow 90 days for fix before public disclosure

### 3. **Submission Format**
Create a GitHub Security Advisory with:

```markdown
## Vulnerability Summary
[One-line description]

## Severity
[Critical/High/Medium/Low] - [Your assessment]

## Affected Component
[Consensus/Network/RPC/State/etc.]

## Reproduction Steps
1. [Detailed step-by-step]
2. [Include commands, code, or scripts]
3. [Expected vs actual behavior]

## Impact
[What can an attacker achieve?]

## Suggested Fix
[Optional - your recommendation]

## Testnet Address
[Your testnet address for reward: udag1...]
```

### 4. **Evaluation Process**
1. **Acknowledgment:** Within 24 hours
2. **Validation:** 1-7 days (we reproduce the issue)
3. **Severity Assessment:** Team evaluates impact and assigns tier
4. **Fix Development:** 7-90 days depending on severity
5. **Reward Distribution:** After fix is deployed and verified

## Reward Distribution

### Testnet Phase (Now)
1. **Immediate testnet UDAG:** Sent to your testnet address within 24h of validation
2. **Bounty ledger entry:** Your reward is recorded in `BOUNTY_LEDGER.md`
3. **Signed promise:** GPG-signed commitment for mainnet conversion

### Mainnet Launch
1. **1:1 Conversion:** Testnet bounty UDAG → Mainnet UDAG
2. **Vesting Schedule:**
   - 25% unlocked at mainnet launch
   - 75% vested linearly over 12 months
3. **Claim Process:** Prove ownership of testnet address via signature

## Rules and Guidelines

### Eligibility
✅ **Allowed:**
- Security researchers, developers, anyone
- Automated tools and fuzzing
- Multiple submissions per person
- Team submissions (reward split as specified)

❌ **Not Allowed:**
- UltraDAG team members and immediate family
- Vulnerabilities discovered during paid audits
- Issues already known or reported
- Duplicate submissions (first valid report wins)

### Quality Standards
**Valid submissions must:**
- Be reproducible on current testnet
- Include clear proof-of-concept
- Represent a real security risk
- Not be publicly known

**Invalid submissions:**
- Theoretical issues without PoC
- Already fixed vulnerabilities
- Out-of-scope items
- Spam or low-effort reports

### Disclosure Policy
- **Private disclosure required:** 90-day embargo
- **Coordinated disclosure:** We'll work with you on timing
- **Credit:** You'll be credited in release notes (if desired)
- **CVE assignment:** For critical/high severity issues

## Testing Resources

### Testnet Access
- **Nodes:** https://ultradag-node-[1-4].fly.dev
- **Faucet:** `curl -X POST https://ultradag-node-1.fly.dev/faucet -d '{"address":"udag1..."}'`
- **RPC Docs:** See `CLAUDE.md` for endpoint documentation


### Example Attacks to Test
- Send malformed transactions
- Spam RPC endpoints
- Create conflicting DAG vertices
- Manipulate staking state
- Partition network connections
- Exhaust node resources

## Bounty Statistics

**Total Allocated:** 500,000 UDAG  
**Total Awarded:** 0 UDAG (as of March 8, 2026)  
**Active Hunters:** 0  
**Vulnerabilities Fixed:** 0  

Updated monthly in `BOUNTY_LEDGER.md`

## FAQ

**Q: Can I test on mainnet when it launches?**  
A: No. Mainnet attacks are illegal. This program is testnet-only.

**Q: What if I find something but can't reproduce it reliably?**  
A: Submit anyway with as much detail as possible. We'll investigate.

**Q: Can I share my findings with my team?**  
A: Yes, but only for collaboration on the submission. No public sharing.

**Q: What if my submission is rejected?**  
A: We'll provide detailed reasoning. You can appeal or resubmit with more evidence.

**Q: How do I get testnet UDAG to start testing?**  
A: Use the faucet endpoint. You'll receive 10,000 testnet UDAG per request (rate limited).

**Q: Is there a maximum reward per person?**  
A: No limit, but we reserve the right to adjust for extraordinary circumstances.

## Contact

- **Security Issues:** GitHub Security Advisory (preferred)
- **Questions:** Create a GitHub Discussion

## Legal

- This program is subject to change at any time
- Final reward amounts are at UltraDAG team's discretion
- Participation constitutes agreement to these terms
- Mainnet conversion is a binding commitment
- All decisions are final

---

**Last Updated:** March 8, 2026  
**Program Version:** 1.0  
**Next Review:** April 8, 2026
