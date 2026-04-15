# UltraDAG Bug Bounty Ledger

**Program Start:** March 8, 2026  
**Total Allocated:** 500,000 UDAG (mainnet)  
**Total Awarded:** 32,500 UDAG  
**Total Paid (Testnet):** 0 UDAG (pending — faucet rate-limited)  
**UDAG Mainnet Token:** [`0x9cFD2011DF13d9E394B5Bb59f0f7e7A5C512155b`](https://arbiscan.io/token/0x9cFD2011DF13d9E394B5Bb59f0f7e7A5C512155b) (Arbitrum One, deployed 2026-04-12)  
**Bounty Payment Source:** Genesis allocation holder `0x9aEcb515361af7980eaa16fE40c064f69738EbF9` (to be reimbursed from treasury post-emission)  

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

### BB-2026-0001
```
ID: BB-2026-0001
Date: 2026-04-12
Hunter: Sumitshah00 (tudg17lzd76ue95ht07hxzna8mzey4tkpk85jtjns2d)
Severity: Critical
Reward: 15,000 UDAG (mainnet promise)
Testnet Paid: Pending (faucet rate-limited; will send via validator key)
Source: Treasury (paid from treasury emission post-launch)
Issue: SmartOp Vote/CreateProposal path triggers fatal supply invariant
       halt — fee debited + nonce incremented before authorization check,
       causing supply accounting mismatch and node exit code 101.
       Extended to all 17 SmartOp types with same validate-after-mutate pattern.
Advisory: GHSA-q8wx-2crx-c7pp
Fix: 45bcf706, 2f5a3a23
Status: Validated / Fixed / Testnet Paid / Pending Mainnet
```

### BB-2026-0002
```
ID: BB-2026-0002
Date: 2026-04-14
Hunter: Sumitshah00 (tudg17lzd76ue95ht07hxzna8mzey4tkpk85jtjns2d)
Severity: Critical
Reward: 10,000 UDAG (mainnet promise)
Testnet Paid: Pending (faucet rate-limited; will send via validator key)
Source: Treasury (paid from treasury emission post-launch)
Issue: Bridge release path enforced quorum as ceil(2n/3) of the active
       validator set with no floor on set size or vote count. When the
       active set degrades to n=1, the threshold collapses to 1 — a sole
       active validator can self-sign a BridgeReleaseTx with a fabricated
       deposit_nonce and drain the entire bridge_reserve in one tx. Report
       included a complete self-contained Rust PoC demonstrating the drain.
       Rated at the Critical floor because the bridge relayer is not yet
       live, bridge_reserve is currently 0, and mainnet P2P is closed to
       external staking — so no funds are at risk today. The bug would
       detonate the instant the bridge ships if left unpatched.
Fix: Added two new constants (MIN_BRIDGE_VALIDATORS=4, MIN_BRIDGE_QUORUM=3)
     and wired both into apply_bridge_release_tx: releases are now rejected
     when active_validator_set.len() < MIN_BRIDGE_VALIDATORS, and the
     dynamic threshold is clamped to max(ceil(2n/3), MIN_BRIDGE_QUORUM).
     Regression test: crates/ultradag-coin/tests/bridge_release_quorum.rs
     (3 tests covering n=1 drain, below-floor set, and normal quorum path).
     Deposit-nonce → source-chain proof binding (reporter's recommendation
     #2) remains open as a separate design-level issue; tracked for a future
     bridge-hardening pass.
Advisory: GHSA-6gwf-frh8-ppw7
Status: Validated / Fixed / Pending Testnet Payout / Pending Mainnet
```

### BB-2026-0003
```
ID: BB-2026-0003
Date: 2026-04-15
Hunter: Sumitshah00 (tudg17lzd76ue95ht07hxzna8mzey4tkpk85jtjns2d)
Severity: High
Reward: 7,500 UDAG (mainnet promise)
Testnet Paid: Pending (faucet rate-limited; will send via validator key)
Source: Treasury (paid from treasury emission post-launch)
Issue: Adaptive-quorum patch (commit 181b2e8b) was incomplete. The earlier
       fix only blocked registration-only phantom validators; producer-backed
       phantoms (attacker keys that each sign one DagVertex) were still
       counted by active_validator_count() in the LIVENESS_WINDOW, and the
       upper_bound in unconfigured mode still derived from validators.len().
       PoC: 4 honest validators + 3 phantom signers raised threshold to
       ceil(2*7/3)=5, stalling finality forever in honest-only post-attack
       rounds. Reporter included a fully self-contained Rust PoC that
       compiles against the public tree and demonstrates the stall.
       Production paths (--validators N, --validator-key <file>) were never
       exposed because they pin configured topology — but the unconfigured
       mode would have detonated for any operator that forgot the flag.
       Premium awarded for bypass-discovery quality on a previously-claimed-fixed
       advisory.
Fix: ValidatorSet now fails closed in permissionless mode. quorum_threshold
     and adaptive_quorum_threshold both return usize::MAX when neither
     configured_validators nor allowed_validators is set. adaptive_quorum_threshold's
     upper_bound now derives ONLY from declared topology, never from
     validators.len(), so producer-backed phantoms cannot raise the ceiling.
     Regression test: producer_backed_phantom_cannot_stall_finality in
     crates/ultradag-coin/tests/phantom_validator.rs (replays the reporter's
     exact 4-honest + 3-phantom scenario and asserts last_finalized_round
     advances past the attack round).
Advisory: GHSA-rprp-wjrh-hx7g
Status: Validated / Fixed / Pending Testnet Payout / Pending Mainnet
```

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
- Submissions: 3 valid (GHSA-q8wx-2crx-c7pp, GHSA-6gwf-frh8-ppw7, GHSA-rprp-wjrh-hx7g)
- Validated: 3
- Rewards: 32,500 UDAG
- Unique hunters: 1 (Sumitshah00)

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
**Last Updated:** April 15, 2026  
**Next Audit:** May 11, 2026
