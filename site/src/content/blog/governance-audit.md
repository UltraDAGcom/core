---
title: "Adversarial Audit of On-Chain Governance"
date: "2026-03-14"
category: "Security"
summary: "Adversarial audit of UltraDAG's on-chain governance: 22 attack vectors tested, 6 vulnerabilities found and fixed, 16 confirmed safe."
---

On-chain governance is where protocol risk concentrates. A consensus bug can stall a network; a governance bug can rewrite its rules. We audited UltraDAG's governance implementation against 22 specific attack vectors across five categories: vote manipulation, threshold gaming, liveness griefing, execution safety, and timing attacks.

Six vulnerabilities were found. All six were fixed before this post was published.

## Methodology

The audit was structured as a series of adversarial questions. For each vector, we traced the code path from transaction submission through consensus finalization to parameter execution, producing a verdict with file and line evidence.

Three verdicts were used:

- **SAFE** -- the attack is prevented by existing code, with specific line numbers cited
- **VULNERABLE** -- the attack succeeds against the current implementation
- **BY DESIGN** -- the behavior is intentional and documented

## Results Summary

| Category | Vectors Tested | Safe | Vulnerable |
|----------|---------------|------|------------|
| Vote Manipulation | 7 | 5 | 2 |
| Threshold Gaming | 4 | 2 | 2 |
| Liveness / Griefing | 4 | 2 | 2 |
| Execution Safety | 4 | 2 | 2 |
| Timing Attacks | 3 | 3 | 0 |
| **Total** | **22** | **14 + 2 by design** | **6** |

## Vulnerabilities Found and Fixed

### V-1: Zero-Stake Voting

**Vulnerability:** `engine.rs` set `vote_weight = 0` for unstaked addresses but allowed the vote transaction to succeed. Addresses with no votable stake could cast zero-weight votes, wasting fees and polluting the vote record.

**Fix:** Added explicit rejection: `if vote_weight == 0 { return Err("cannot vote with zero votable stake") }`. Zero-stake votes are now rejected at the engine level before recording.

### V-2: Quorum Denominator Inflation

**Vulnerability:** `tick_governance()` used `total_staked()` as the quorum denominator, which includes validators in unstake cooldown. But vote weight already excluded unstaking validators (weight = 0). This inflated the denominator, making quorum harder to reach than intended. A coordinated attack could begin unstaking to inflate the denominator and block governance.

**Fix:** Added `total_votable_stake()` method that excludes addresses with `unlock_at_round.is_some()`. Quorum calculation now uses votable stake only, matching the vote weight logic.

### V-3: Self-Modifying Quorum Threshold

**Vulnerability:** Governance could lower `quorum_numerator` to 1% via a ParameterChange proposal. Once lowered, a single large staker could unilaterally pass proposals. The governance system could weaken its own security guarantees.

**Fix:** Hard floor of 5% on `quorum_numerator` in `params.rs`. Minimum `voting_period_rounds` raised to 1,000 (~1.4 hours). These bounds cannot be changed by governance.

### V-4: Execution Timelock Too Short

**Vulnerability:** The `execution_delay_rounds` hard floor was 100 rounds (~8 minutes at 5s rounds). A coordinated attack could pass a malicious proposal and execute it before the community noticed, especially across time zones.

**Fix:** Raised hard floor to 2,016 rounds (~2.8 hours), matching the unstake cooldown period. This gives the community meaningful time to detect and respond to malicious governance actions.

### V-5: Signable Bytes Concatenation Ambiguity

**Vulnerability:** `CreateProposalTx::signable_bytes()` concatenated variable-length fields (title, description, param, new_value) without length delimiters. Two different proposals could produce identical signable bytes: title="AB" + description="CD" would match title="ABC" + description="D".

**Fix:** Added 4-byte little-endian length prefix before every variable-length field in `signable_bytes()`. Each field is now unambiguously delimited in the byte representation.

### V-6: RPC Duplicate Vote Accepted

**Vulnerability:** The `/vote` RPC endpoint did not check whether the sender had already voted. While the engine correctly rejected duplicates at finalization (fee charged, vote discarded), the RPC layer would sign and broadcast a transaction destined to waste the user's fee.

**Fix:** Added `state.get_vote(proposal_id, &sender).is_some()` check in the `/vote` RPC handler. Duplicate votes are now rejected before signing.

## Confirmed Safe

The remaining 16 vectors were confirmed safe with specific code evidence:

| Vector | Verdict | Evidence |
|--------|---------|----------|
| Double voting | SAFE | `engine.rs` -- `AlreadyVoted` error on duplicate `(proposal_id, address)` key |
| Vote change after cast | SAFE | Same mechanism -- votes are permanent once stored |
| Proposer self-voting | BY DESIGN | Proposer votes like any staker -- no special privilege or restriction |
| Execute before voting ends | SAFE | `engine.rs` -- only checks proposals after `current_round > voting_ends` |
| Double execution | SAFE | `engine.rs` -- `PassedPending -> Executed` is one-way; no further transitions |
| Threshold boundary (off-by-one) | SAFE | `proposals.rs` -- ceiling division ensures threshold is inclusive |
| Proposal spam | SAFE | `MAX_ACTIVE_PROPOSALS = 20` + `MIN_STAKE_TO_PROPOSE` cap |
| Slashed validator votes | SAFE | Slashing removes stake -> vote weight becomes 0 -> now rejected |
| Parameter change breaks consensus | SAFE | Governable params are governance-only; `MAX_ACTIVE_VALIDATORS` is not governable |
| Failed execution state | BY DESIGN | Failed `apply_change()` logs error; proposal marked Executed to prevent retry loops |
| Timelock bypass | SAFE | `execution_delay_rounds` enforced; hard floor at 2,016 rounds |
| Proposal resubmission | SAFE | Resubmission costs `MIN_STAKE_TO_PROPOSE` + fee each time; bounded by `MAX_ACTIVE_PROPOSALS` |
| Sequential proposal IDs | BY DESIGN | Predictable but harmless -- no front-running advantage in on-chain governance |
| Stake movement mid-vote | SAFE | Vote weight captured at execution time; unstake cooldown (2,016 rounds) prevents rapid cycling |
| Single-validator quorum | SAFE | Requires 10% of total stake -- with 5+ validators, no single validator holds enough |
| Minority veto via no-votes | SAFE | 66% supermajority is standard BFT threshold; minority blocking is intentional safeguard |

## Design Decisions Worth Noting

**Stake-weighted voting without delegation.** UltraDAG's governance uses direct stake-weighted voting. There is no delegation mechanism. This is simpler but means governance participation requires active staking. For a network targeting machine-to-machine payments with a small validator set, this is appropriate.

**No vote-escrow or locking.** Stake is not locked during voting periods. A validator can vote, then unstake. However, the 2,016-round unstake cooldown (matching the execution delay) provides a natural friction against rapid stake cycling.

**TextProposal has no execution.** Text proposals are signaling-only. They pass through the same voting lifecycle but trigger no state changes. This is intentional: on-chain signaling without the risk of automated execution.

## Hard Floors on Governable Parameters

The most important architectural decision in this audit was establishing hard floors that governance cannot breach:

```
quorum_numerator:        5 - 100  (minimum 5% of votable stake)
approval_numerator:     51 - 100  (minimum simple majority)
voting_period_rounds: 1000+       (minimum ~1.4 hours)
execution_delay_rounds: 2016+     (minimum ~2.8 hours)
max_active_proposals:   1 - 100   (at least 1 proposal slot)
min_fee_sats:           1+        (cannot set fees to zero)
min_stake_to_propose:   1+        (cannot remove stake requirement)
```

These bounds prevent governance from weakening its own security guarantees. A malicious proposal that attempts to set `quorum_numerator` to 1 would be rejected at execution time with a validation error.

## What's Next

Two areas remain for mainnet hardening:

- **Snapshot-based quorum.** Quorum is currently calculated dynamically at each `tick_governance()` call. Snapshotting total votable stake at proposal creation would make quorum requirements immutable during the voting period, preventing manipulation via strategic staking/unstaking.
- **Proposal cooldown.** Failed proposals can be immediately resubmitted. Adding a cooldown period would reduce governance spam from wealthy attackers who can afford repeated `MIN_STAKE_TO_PROPOSE` costs.

Neither is critical for testnet. Both are recommended before mainnet.

> The full audit report is available at `GOVERNANCE_SECURITY_AUDIT.md` in the repository. All fixes were committed in [UltraDAGcom/core](https://github.com/UltraDAGcom/core).
