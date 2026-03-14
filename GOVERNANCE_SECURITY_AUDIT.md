# UltraDAG Governance Security Audit Report

## MANIPULATION

**Can a validator vote multiple times on the same proposal?**
- **SAFE** - `engine.rs:801-803` checks `self.votes.contains_key(&(tx.proposal_id, tx.from))` and returns `AlreadyVoted` error

**Can a validator change their vote after casting it?**
- **SAFE** - Same mechanism prevents revoting; vote is stored permanently in `self.votes` map

**Can the proposer vote on their own proposal?**
- **SAFE** - No restrictions found; proposer can vote like any other staked address

**Can a validator with zero stake cast a vote?**
- **VULNERABLE** - `engine.rs:813-815` sets `vote_weight = 0` for addresses with no stake but allows the vote to proceed, wasting fees

**Can stake be moved mid-vote to double the voting weight?**
- **VULNERABLE** - Vote weight captured at transaction execution time (`engine.rs:813-815`). No lock on stake during voting period. Stake can be moved after voting.

**Can a proposal be executed before the voting period ends?**
- **SAFE** - `engine.rs:858-868` only checks proposals after `current_round > proposal.voting_ends`

**Can a proposal be executed more than once?**
- **SAFE** - `engine.rs:870-874` transitions from `PassedPending` to `Executed` only once; no further transitions possible

## THRESHOLDS

**What is the exact quorum requirement?**
- **TOTAL STAKE** - `proposals.rs:48-52` calculates quorum as `total_staked * quorum_numerator / denominator` where `total_staked` comes from `engine.rs:853`

**What happens if stake changes during a vote?**
- **VULNERABLE** - Quorum recalculated each time `tick_governance` runs using current `total_staked()`. Stake changes during voting affect quorum calculation.

**Can quorum be reached with only 1 validator?**
- **VULNERABLE** - Yes. With 10% quorum requirement, a single validator holding >10% of total stake can meet quorum alone.

**What happens at exactly the threshold?**
- **SAFE** - `proposals.rs:49-52` uses ceiling division with `(denominator - 1)` to ensure threshold is inclusive

## LIVENESS / GRIEFING

**Can a validator block all proposals by voting no on everything?**
- **VULNERABLE** - Yes. Large validator can consistently vote against proposals, preventing the 66% approval threshold.

**Can a validator spam proposals to exhaust governance capacity?**
- **PARTIALLY VULNERABLE** - Limited by `MAX_ACTIVE_PROPOSALS` (20) and `MIN_STAKE_TO_PROPOSE` (10,000 UDAG), but wealthy attacker can fill capacity.

**What happens if proposer gets slashed mid-vote?**
- **UNKNOWN** - Slashing affects stake but vote remains valid. No mechanism found to invalidate votes from slashed validators.

**Is there minimum stake requirement? Can it be gamed?**
- **VULNERABLE** - `MIN_STAKE_TO_PROPOSE` is 10,000 UDAG. Wealthy attacker can create multiple addresses to bypass per-address limits.

## EXECUTION SAFETY

**What state changes does a passing proposal trigger?**
- **ParameterChange only** - `engine.rs:883-896` calls `self.governance_params.apply_change(param, new_value)`
- **TextProposal** - No state changes (informational only)

**Are those state changes atomic?**
- **PARTIALLY VULNERABLE** - `engine.rs:889-895` logs errors but continues execution. Failed parameter changes still mark proposal as Executed.

**Can a proposal change quorum threshold itself?**
- **VULNERABLE** - Yes. `params.rs:43-79` allows changing `quorum_numerator`. Attacker could lower quorum to 1%.

**Can governance break consensus safety?**
- **VULNERABLE** - No validation prevents changing validator parameters below BFT safety minimums. Could reduce `MAX_ACTIVE_VALIDATORS` below 4.

## TIMING

**Is there a timelock between passing and execution?**
- **SAFE** - `engine.rs:861-863` adds `execution_delay_rounds` (2,016 rounds ≈ 1.4 hours) before execution

**Can the same proposal text be resubmitted immediately after failing?**
- **VULNERABLE** - No cooldown period. Failed proposals can be immediately resubmitted with new sequential ID.

**Are proposal IDs sequential and predictable?**
- **VULNERABLE** - `engine.rs:714-716` requires `tx.proposal_id == self.next_proposal_id`. IDs are sequential starting from 0, fully predictable.

## CRITICAL VULNERABILITIES SUMMARY

1. **Stake manipulation during voting** - Move stake after voting to retain voting power
2. **Dynamic quorum calculation** - Stake changes affect quorum requirements mid-vote  
3. **Single validator quorum** - One wealthy validator can meet 10% quorum alone
4. **Self-modifying governance** - Proposals can lower their own quorum requirements
5. **Consensus safety bypass** - Governance can reduce validator count below BFT minimum
6. **Predictable proposal IDs** - Enables front-running and targeted attacks
7. **Zero-stake voting** - Wastes fees and complicates vote counting
8. **Non-atomic execution** - Failed parameter changes still consume proposal slots

## RECOMMENDATIONS

1. **Lock stake during voting period** to prevent manipulation
2. **Snapshot quorum at proposal creation** instead of dynamic calculation
3. **Increase minimum quorum** or require minimum validator participation
4. **Add parameter change validation** to prevent BFT safety violations
5. **Add proposal cooldown periods** to prevent spam
6. **Use unpredictable proposal IDs** (hash-based instead of sequential)
7. **Reject votes from zero-stake addresses** to prevent fee waste
8. **Make parameter changes atomic** - reject entire proposal if any change fails
