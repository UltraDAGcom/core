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

Mainnet launched **2026-04-10**. All rewards recorded in this ledger —
whether filed before or after that date — convert 1:1 to mainnet UDAG
under the same vesting schedule:

- **25% unlocked** at the vesting anchor date (immediate)
- **75% vested** linearly over the 12 months following the anchor

**Vesting anchor date** (when the 25% / 12-month clock starts):

- **Pre-mainnet reports** (program start 2026-03-08 through mainnet launch 2026-04-10): anchor = **2026-04-10** (mainnet launch day)
- **Post-mainnet reports** (2026-04-10 and later): anchor = **date the bounty was validated** (the `Date:` field on the ledger entry)

### Vesting Formula
```
# Any day on or after the anchor date:
unlocked_amount = total_reward * 0.25 +
                  total_reward * 0.75 * min(1, days_since_anchor / 365)
```

### Claim Process
1. Hunter proves ownership of the testnet address recorded in the ledger entry.
   Proof is a signature over a maintainer-supplied challenge, using the Ed25519
   secret key (or WebAuthn passkey) behind that address. **This is a
   cryptographic challenge-response — it does NOT depend on the testnet address
   still having any on-chain balance**, and it does NOT depend on the testnet
   chain even still existing.
2. Hunter provides a mainnet address for distribution.
3. Maintainer submits a `TreasurySpend` governance proposal for the unlocked
   amount (or a direct mainnet `SmartTransfer` if the bounty pool is held by
   the founder rather than the treasury).
4. Entry in this file is updated with `Status: Claimed` and the paid tx hash.

## Testnet Reset Safety

**A testnet reset (`--clean` deploy, state wipe, nuclear restart, etc.)
does NOT affect any bounty promise in this ledger.** This is by design:

| Layer | Testnet reset behavior |
|---|---|
| Hunter's testnet UDAG balance (on-chain) | 💀 Gone. Resets to zero. |
| Hunter's testnet address bytes (20-byte Address) | ✅ Unchanged. Derived deterministically from the secret key. |
| Hunter's secret key / passkey | ✅ Unchanged. Held by the hunter on their own device — never depended on chain state. |
| **Ledger entry (this file, in git)** | ✅ **Unchanged. This is the authoritative commitment.** |

The testnet UDAG transfer we send to the hunter at validation time is a
**courtesy payment** — visible evidence that we took the report seriously,
a usable balance for further testnet testing, and an on-chain timestamp.
It is **not** the commitment. The binding commitment is the append-only
entry in this git-tracked ledger plus the hunter's ability to sign a
challenge with the key that derives their address.

**Operational discipline that keeps this promise honest:**

- `LEDGER.md` is **append-only**. Every bounty payout lands as a new
  commit that adds a line under "Active Bounties". Entries are never
  deleted or rewritten. A status change (Validated → Fixed → Claimed) is
  added as a subsequent line below the original, not an in-place edit.
- `git log docs/security/bug-bounty/LEDGER.md` is the hunter's audit
  trail. Anyone can run it and see exactly when each entry was added
  and by whom.
- Pushes to `origin/main` are the point of no return. Once an entry is
  pushed, it's the maintainer's public commitment.
- Maintainers **must not** force-push branches containing ledger entries.
  `git push --force-with-lease` on any branch that touches `LEDGER.md` is
  a policy violation.

**What the hunter must do to protect their own claim:**

- **Back up the testnet secret key** (32-byte hex or passkey credential)
  immediately after generating it. The ONLY way to prove address ownership
  at claim time is with this key. Lose the key → lose the payout. No recovery.
- **Record your testnet address in every report you submit**, in the
  "Testnet Address" field of the advisory template. The maintainer will
  copy it into the ledger entry.
- **Prefer a long-lived address** (not a throwaway) for your reports.
  Using the same testnet address across multiple reports batches your
  claim at mainnet conversion time.

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
