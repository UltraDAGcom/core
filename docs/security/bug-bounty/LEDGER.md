# UltraDAG Bug Bounty Ledger

**Program Start:** March 8, 2026  
**Total Allocated:** 500,000 UDAG (mainnet)  
**Total Awarded:** 0 UDAG  
**Total Paid (Testnet):** 0 UDAG  

This ledger tracks all bug bounty rewards. Each entry is cryptographically signed and represents a binding commitment to distribute mainnet UDAG tokens.

---

## Reward Entries

### Format
```
ID: BB-YYYY-NNNN
Date: YYYY-MM-DD
Hunter: GitHub username or testnet address
Severity: Critical/High/Medium/Low
Reward: X,XXX UDAG (mainnet promise)
Testnet Paid: X,XXX UDAG
Issue: Brief description
Status: Validated/Fixed/Paid
Signature: [GPG signature]
```

---

## Active Bounties

*No bounties awarded yet*

---

## Pending Validation

*No submissions pending*

---

## Statistics by Month

### March 2026
- Submissions: 0
- Validated: 0
- Rewards: 0 UDAG
- Unique hunters: 0

### April 2026
- Submissions: 0 valid + 1 pending (received 2026-04-10, disclosure channel was offline at the time)
- Validated: 0
- Rewards: 0 UDAG
- Unique hunters: 0

### Mainnet launched: 2026-04-10
- All reward entries from this date onward are paid in live mainnet UDAG
  (not converted from testnet promises). See "Mainnet Conversion Tracking"
  below for how earlier testnet-promise entries are honored.

---

## Top Contributors

*Leaderboard will appear here*

---

## Mainnet Conversion Tracking

At mainnet launch, all rewards in this ledger will be converted 1:1 to mainnet UDAG with the following vesting schedule:

- **25% unlocked** at mainnet launch (immediate)
- **75% vested** linearly over 12 months

### Vesting Formula
```
unlocked_amount = total_reward * 0.25 + (total_reward * 0.75 * days_since_launch / 365)
```

### Claim Process
1. Hunter proves ownership of testnet address via signature
2. Provides mainnet address for distribution
3. Tokens distributed according to vesting schedule
4. Entry marked as "Claimed"

---

## Audit Trail

All changes to this ledger are tracked in git history. Each reward entry includes:
- Git commit hash
- GPG signature from UltraDAG team
- Timestamp of validation
- Link to GitHub Security Advisory (if applicable)

---

**Ledger Maintainer:** UltraDAG Core Team  
**Last Updated:** April 11, 2026  
**Next Audit:** May 11, 2026
