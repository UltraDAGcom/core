# Bug Bounty Program Launch Summary

**Launch Date:** March 8, 2026  
**Mainnet Launch:** April 10, 2026  
**Status:** ✅ Active — private vulnerability reporting enabled on the repo; program covers both testnet hunting and post-mainnet findings.

## What's In Place

### Core Documentation
1. [`PROGRAM.md`](./PROGRAM.md) — Full bug bounty program details
   - Reward tiers (Critical: 10k–50k, High: 5k–10k, Medium: 1k–5k, Low: 100–1k UDAG)
   - Submission process and evaluation timeline
   - Rules, eligibility, disclosure policy
   - Testing resources and example attacks

2. [`LEDGER.md`](./LEDGER.md) — Transparent reward tracking
   - All bounties recorded with timestamps
   - Mainnet conversion schedule (25% at launch, 75% vested over 12 months)
   - Statistics and leaderboard
   - Git-tracked audit trail

3. [`../../../SECURITY.md`](../../../SECURITY.md) — Security policy at repo root
   - Responsible disclosure guidelines
   - Response timeline SLAs
   - Private reporting channel (GitHub Security Advisories)
   - Known security considerations + project assumptions

4. [`GUIDE.md`](./GUIDE.md) — Hunter's quick start
   - Get testnet UDAG in 5 minutes
   - Attack vectors, tooling, and example exploits
   - Reporting template

5. [`PROMOTION.md`](./PROMOTION.md) — Outreach strategy
   - Channels for finding hunters
   - Messaging guidelines and response templates

### Payout Mechanism
- **Dashboard `Pay Bounty` button** (`dashboard/src/components/bounty/PayBountyModal.tsx`)
  uses a passkey-signed `SmartTransfer` to send UDAG to the hunter's address,
  with a `bounty:#<issue>` memo tag for traceability. That's the only automation
  needed — the LEDGER.md entry is a small manual commit after each payout.
- **No separate `bounty_reward.sh` script exists.** Previous versions of this
  doc referenced one; the dashboard flow replaces it.

## How It Works

### For Hunters
1. **Discover** vulnerability while testing against the public testnet
2. **Report** privately at <https://github.com/UltraDAGcom/core/security/advisories/new>
3. **Receive** testnet UDAG within 24h of validation
4. **Get recorded** in [`LEDGER.md`](./LEDGER.md) with mainnet conversion schedule
5. **Claim** mainnet UDAG per the vesting schedule (25% immediate, 75% over 12 months)

### For the Maintainer (you)
1. **Receive** the report as a private GitHub Security Advisory
2. **Acknowledge** within 24h (SLA from `SECURITY.md`)
3. **Validate** the vulnerability — reproduce it on a local node
4. **Assign severity** and corresponding reward range (see `PROGRAM.md`)
5. **Pay** via the dashboard: open `https://ultradag.com/dashboard/bounties`, find the bounty, click **Pay Bounty**, paste the hunter's testnet address, sign with your passkey. The payment tx carries a `bounty:#<issue>` memo.
6. **Record** the payout in [`LEDGER.md`](./LEDGER.md) — add an entry under "Active Bounties" with the issue ID, hunter handle, amount, and severity, then commit.
7. **Fix** the vulnerability on a private branch
8. **Test** the fix locally + against testnet
9. **Deploy** to mainnet (rolling or coordinated depending on severity)
10. **Coordinate** public disclosure with the hunter after the fix is live

## Activation Status (as of 2026-04-11)

### 1. GitHub Security Advisories
- [x] **Private vulnerability reporting enabled** on `UltraDAGcom/core`
- [x] `SECURITY.md` at repo root
- [x] `PROGRAM.md` and `GUIDE.md` published under `docs/security/bug-bounty/`
- [x] Advisory submission URL live: <https://github.com/UltraDAGcom/core/security/advisories/new>

### 2. Announcement — TODO
**Recommended channels** (see [`PROMOTION.md`](./PROMOTION.md) for the full plan):
- GitHub README.md — add security badge + link to `SECURITY.md`
- Twitter/X announcement
- Telegram community (@ultra_dag)
- Reddit (r/cryptocurrency, r/crypto, r/netsec, r/bugbounty)
- HackerOne / Immunefi cross-post (optional)

**Sample announcement:**
```
🎯 UltraDAG Bug Bounty Program is LIVE!

💰 500,000 UDAG pool
🔴 Up to 50,000 UDAG for critical bugs
🧪 Test on live testnet (5 nodes)
📝 Post-mainnet coverage too

Target: Consensus, P2P, State, Staking, RPC, SmartAccount
Docs: github.com/UltraDAGcom/core/blob/main/SECURITY.md

Help us harden a live mainnet DAG-BFT chain. 🎯
```

### 3. Monitoring & Operations — TODO
- [ ] Set up email notifications for GitHub Security Advisories
- [ ] Monitor testnet for suspicious activity (metric: equivocation_vertex_count, mempool pressure)
- [ ] Prepare incident response plan for Critical findings on mainnet
- [ ] Generate a key used for signing advisory responses (optional; GitHub already signs)

### 4. Promote to Researchers — TODO
**Target communities:**
- HackerOne top researchers
- Immunefi (crypto-focused)
- Trail of Bits / Zellic / ChainSecurity researchers
- Independent security researchers on Twitter/X
- r/netsec, r/bugbounty community members

**Outreach template:**
```
Subject: New Bug Bounty: UltraDAG — live mainnet DAG-BFT chain

Hi [Researcher],

We've launched a bug bounty program for UltraDAG, a minimal DAG-BFT
cryptocurrency (sub-4 MB full-node binary, 5 mainnet validators, passkey
wallets, 7-bucket tokenomics). Would love an experienced pair of eyes.

Rewards: 100 - 50,000 UDAG (rewarded in testnet UDAG, convertible to
mainnet UDAG per our published schedule).
Scope: Consensus/DAG, state engine, P2P, RPC, SmartAccount, bridge.
Testnet: https://ultradag-node-1.fly.dev (faucet live; 5-node quorum).

Docs: https://github.com/UltraDAGcom/core/blob/main/SECURITY.md

Happy to answer any questions.

Best,
[Your name]
```

### 5. Monitor and Respond
**Daily:**
- Check GitHub Security Advisories (<https://github.com/UltraDAGcom/core/security/advisories>)
- Monitor testnet + mainnet for suspicious activity (equivocation events, unusual mempool pressure)
- Review [`LEDGER.md`](./LEDGER.md) for updates

**Weekly:**
- Update statistics in [`PROGRAM.md`](./PROGRAM.md)
- Respond to all submissions
- Deploy fixes for validated issues

**Monthly:**
- Publish security bulletin / changelog entry
- Update reward tiers if needed
- Review program effectiveness (submissions, hit rate, severity distribution)

## Budget Management

**Total Pool:** 500,000 UDAG (mainnet)

**Recommended allocation:**
- Critical: 200,000 UDAG (4-20 bugs at 10k-50k each)
- High: 150,000 UDAG (15-30 bugs at 5k-10k each)
- Medium: 100,000 UDAG (20-100 bugs at 1k-5k each)
- Low: 50,000 UDAG (50-500 bugs at 100-1k each)

**Reserve:** Keep 20% (100k UDAG) for exceptional findings or program extensions.

**Tracking:** All rewards tracked in [`LEDGER.md`](./LEDGER.md) with git history as proof.

## Legal Considerations

**Mainnet Promise:**
- Legally binding commitment tracked in git
- GPG-signed entries for authenticity
- Vesting schedule clearly documented
- Claim process defined

**Recommendations:**
1. Consult lawyer about token promise enforceability
2. Consider creating formal legal agreement template
3. Add terms of service for bounty program
4. Clarify tax implications for hunters
5. Define dispute resolution process

**Risk mitigation:**
- All promises are public and git-tracked
- Vesting reduces immediate liability
- Clear eligibility rules prevent abuse
- Reserve right to adjust for extraordinary circumstances

## Success Metrics

**Track these KPIs:**
- Number of submissions (target: 10+ in first month)
- Valid vulnerabilities found (target: 5+ in first month)
- Critical/High severity bugs (target: 1-2 in first quarter)
- Average response time (target: <24h acknowledgment)
- Fix deployment time (target: <7 days for critical)
- Hunter satisfaction (gather feedback)
- Cost per valid bug (should be <10k UDAG average)

**Review quarterly:**
- Adjust reward tiers based on submissions
- Expand or narrow scope
- Update documentation based on feedback
- Celebrate top contributors

## Example Workflow

### Scenario: Critical Consensus Bug Found

1. **Hunter submits** via GitHub Security Advisory
   - Title: "Double-spend via DAG finality race condition"
   - Severity: Critical
   - PoC: Detailed reproduction steps

2. **You acknowledge** within 24h
   - "Thanks for the report! Validating now..."

3. **You validate** (2 days)
   - Reproduce the bug on testnet
   - Confirm it's a real issue
   - Assess severity: Critical ✅

4. **You award bounty** (immediately after validation)
   - Open the dashboard: `https://ultradag.com/dashboard/bounties`
   - Find the bounty entry (created automatically when the advisory is accepted, or manually)
   - Click **Pay Bounty**, enter the hunter's `tudg1…` / `udag1…` address, sign with your passkey
   - The `SmartTransfer` carries a `bounty:#<advisory_id>` memo tag
   - Append a signed entry to [`LEDGER.md`](./LEDGER.md) and commit

5. **You fix** (3-5 days)
   - Develop patch on a private branch
   - Test thoroughly (unit + integration + testnet soak)

6. **You verify** (1 day)
   - Deploy to testnet, confirm fix works
   - Hunter verifies the fix

7. **You disclose** (coordinated)
   - Deploy fix to mainnet
   - Publish security advisory (drafts → public)
   - Credit hunter (unless anonymous)
   - Update `docs/project/development/changelog.md` with the fix reference

**Total time:** ~7 days from report to public disclosure

## Tips for Success

**Be responsive:**
- Acknowledge all reports within 24h
- Provide status updates every 2 weeks
- Be transparent about timeline

**Be fair:**
- Consistent severity assessment
- Clear reasoning for decisions
- Allow appeals

**Be grateful:**
- Thank hunters publicly (if they want)
- Feature top contributors
- Build long-term relationships

**Be secure:**
- Don't rush fixes
- Test thoroughly
- Coordinate disclosure timing

## Resources

**Internal:**
- [`PROGRAM.md`](./PROGRAM.md) — Program details, scope, tiers, rules
- [`GUIDE.md`](./GUIDE.md) — Hunter quick-start + attack vectors
- [`LEDGER.md`](./LEDGER.md) — Reward tracking + mainnet conversion schedule
- [`PROMOTION.md`](./PROMOTION.md) — Outreach channels and templates
- [`../../../SECURITY.md`](../../../SECURITY.md) — Security policy at repo root
- [`../../reference/api/rpc-endpoints.md`](../../reference/api/rpc-endpoints.md) — RPC API reference
- `dashboard/src/components/bounty/PayBountyModal.tsx` — Payout UI (passkey-signed SmartTransfer)

**External:**
- <https://github.com/UltraDAGcom/core/security/advisories/new> — Private vulnerability submission
- <https://github.com/UltraDAGcom/core/discussions> — Public Q&A
- HackerOne (optional cross-listing)
- Immunefi (crypto-focused platform)

---

**Program Status:** ✅ Active — Testnet (Mainnet paused 2026-04-22)  
**Private reporting:** Enabled on the repo; advisory URL live  
**Next Action:** Announce publicly via `PROMOTION.md` channels  
**Last Updated:** April 22, 2026
