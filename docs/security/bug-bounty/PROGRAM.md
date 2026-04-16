# UltraDAG Bug Bounty Program

**Operator:** JMS Media Group LLC (Wyoming, USA; Filing ID 2026-001951812) — the legal entity behind the UltraDAG project and the payer of record for all bounty rewards.  
**Status:** Active — Mainnet & Testnet  
**Launch Date:** March 8, 2026  
**Mainnet Genesis:** April 10, 2026  
**Mainnet Public Open:** April 16, 2026 (anyone can run a validator)  
**Total Pool:** 500,000 UDAG

## Overview

UltraDAG is offering rewards for security researchers who discover and responsibly disclose vulnerabilities in the UltraDAG codebase. **Mainnet is now open** — anyone can run a validator, stake UDAG, and participate in consensus. Testing is welcome on both mainnet and testnet; please prefer testnet for destructive exploration. Mainnet nodes are reachable at `ultradag-mainnet-[1-5].fly.dev:9333` (P2P) and `https://ultradag-mainnet-[1-5].fly.dev` (RPC).

Valid reports are rewarded in UDAG, recorded in the append-only [`LEDGER.md`](./LEDGER.md), and convertible 1:1 to mainnet UDAG per the vesting schedule in that file. See [`LEDGER.md` → Testnet Reset Safety](./LEDGER.md#testnet-reset-safety) for why a testnet wipe does not affect your claim.

## Mainnet Access Policy

Mainnet is **fully open**:

- **P2P port 9333 is public** on all mainnet nodes at `ultradag-mainnet-[1-5].fly.dev:9333`. External validators and observers can connect directly.
- **RPC port 10333 is public** at `https://ultradag-mainnet-[1-5].fly.dev` for both reads and transaction submission.
- **Validator set is permissionless** — any address with enough UDAG can stake and enter the active set. The 5 founder-operated Fly nodes have no protocol-level privilege; ranking is by effective stake.
- **Please do not DoS mainnet.** In-scope attacks are those demonstrating a protocol or implementation bug via a minimal PoC — not brute traffic floods. If you can crash or halt a live mainnet node with a single crafted message, that's a valid Critical; sustained DoS traffic is out of scope and may be reported to the hosting provider.

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

### What you receive at validation (within 24h)
1. **Courtesy testnet UDAG payment** to the testnet address you included in
   your report — a visible "we took this seriously" signal plus a working
   balance for further testing. **This is NOT the binding commitment.** See
   [`LEDGER.md` → Testnet Reset Safety](./LEDGER.md#testnet-reset-safety)
   for why.
2. **Append-only entry in [`LEDGER.md`](./LEDGER.md)** — the actual binding
   commitment. Git-tracked, signed by the maintainer commit, survives any
   testnet reset.
3. **Acknowledgment reply on the private GitHub Security Advisory** with
   the severity assessment, reward range, and planned timeline.

### Mainnet conversion (applies to all ledger entries)
Mainnet launched **2026-04-10**. Every entry in the ledger converts 1:1 to
mainnet UDAG under the following rules:

1. **Vesting schedule:** 25% unlocked at the vesting anchor (immediate), 75%
   vested linearly over the 12 months following.
2. **Vesting anchor date:**
   - Pre-mainnet reports (2026-03-08 through 2026-04-10): anchor = 2026-04-10
   - Post-mainnet reports: anchor = validation date
3. **Claim process:** the hunter signs a maintainer-supplied challenge with
   the Ed25519 secret key (or passkey) behind their testnet address. This
   proves ownership without needing the testnet address to hold any balance
   or for the testnet to even still be running.

**Testnet reset safety:** testnet `--clean` restarts do not affect any ledger
entry. The commitments live in git, not on the testnet chain. See the
[testnet reset safety](./LEDGER.md#testnet-reset-safety) section in
`LEDGER.md` for the full explanation.

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
- **Nodes:** `https://ultradag-node-[1-5].fly.dev` (5 nodes, use any for queries)
- **Faucet:** `curl -X POST https://ultradag-node-1.fly.dev/faucet -H "Content-Type: application/json" -d '{"address":"tudg1...","amount":10000000000}'` (amount is in sats; 10,000,000,000 sats = 100 UDAG, the per-request max; rate-limited to 1 request per 10 minutes)
- **RPC Docs:** [`docs/reference/api/rpc-endpoints.md`](../../reference/api/rpc-endpoints.md)


### Example Attacks to Test
- Send malformed transactions
- Spam RPC endpoints
- Create conflicting DAG vertices
- Manipulate staking state
- Partition network connections
- Exhaust node resources

## Bounty Statistics

**Total Allocated:** 500,000 UDAG  
**Total Awarded:** 0 UDAG (as of April 11, 2026)  
**Active Hunters:** 0  
**Vulnerabilities Fixed:** 0  

Updated monthly in [`LEDGER.md`](./LEDGER.md).

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
A: Use the faucet endpoint. Cap is 100 UDAG (10,000,000,000 sats) per request, rate-limited to 1 request per 10 minutes. See [`GUIDE.md`](./GUIDE.md) for the exact curl command.

**Q: Is there a maximum reward per person?**  
A: No limit, but we reserve the right to adjust for extraordinary circumstances.

## Contact

- **Security Issues (private):**
  <https://github.com/UltraDAGcom/core/security/advisories/new>
  or click the green "Report a vulnerability" button at
  <https://github.com/UltraDAGcom/core/security>. See
  [`SECURITY.md`](../../../SECURITY.md) in the repo root for the full
  disclosure policy and response SLAs.
- **General questions (public):** Create a GitHub Discussion at
  <https://github.com/UltraDAGcom/core/discussions>. Do NOT post
  vulnerability details there.

## Legal

- This program is subject to change at any time
- Final reward amounts are at UltraDAG team's discretion
- Participation constitutes agreement to these terms
- Mainnet conversion is a binding commitment
- All decisions are final

---

**Last Updated:** April 11, 2026  
**Program Version:** 1.1  
**Next Review:** May 11, 2026
