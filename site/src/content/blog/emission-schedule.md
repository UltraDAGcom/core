---
title: "Fixing the Emission Schedule"
date: "2026-03-14"
category: "Tokenomics"
summary: "Why UltraDAG's emission schedule changed from 50 UDAG/round to 1 UDAG/round, and the math behind credible tokenomics for a mainnet launch."
---

> **Update (2026-04-10):** This post is a historical snapshot from 2026-03-14 covering the switch from 50 UDAG/round to 1 UDAG/round. Since then the distribution model was overhauled again:
>
> - **7-bucket model, not 4-bucket**: the per-round split is now 44% validators / 10% council / 16% treasury / 5% founder / 8% ecosystem / 5% reserve = **88%** of the nominal reward. The remaining **12% is never minted** — it's offset by a 2,520,000 UDAG IDO genesis pre-mine that bootstraps the private round and Uniswap liquidity. Total supply cap is unchanged at 21M.
> - **Only one pre-mine**: the IDO distributor address. All other buckets (validators, council, treasury, founder, ecosystem, reserve) still start at zero and earn through per-round emission.
> - **Per-validator emission table**: `MAX_ACTIVE_VALIDATORS` is 100, and the table below has been recomputed with the 44% validator share of the nominal reward.
> - **"Year 5.7 supply cap"**: incorrect. The geometric series converges over tens of halvings until the per-round reward integer-shifts to zero (around halving 27, ~45 years at 5 s/round). The `block_reward()` function returns 0 for `halvings >= 64`.
>
> See [tokenomics/supply](/docs/tokenomics/supply) for the current canonical numbers.

---

UltraDAG inherited Bitcoin's halving constants: 50 coins per block, halving every 210,000 blocks, 21 million max supply. Bitcoin produces blocks every 10 minutes. UltraDAG produces rounds every 5 seconds.

That difference -- a factor of 120 -- meant the entire emission schedule would complete in three months instead of a hundred years. We caught it before mainnet launch and redesigned the schedule from first principles.

## The Constraint

Bitcoin's emission has an elegant identity that most people don't think about:

```
initial_reward x halving_interval x 2 = max_supply
50 BTC x 210,000 blocks x 2 = 21,000,000 BTC
```

The factor of 2 comes from the geometric series `1 + 1/2 + 1/4 + ... = 2`. This means the three constants -- initial reward, halving interval, and max supply -- are mathematically coupled. You can't change one without adjusting another.

UltraDAG kept 21M max supply (the right choice for brand clarity). It kept 210,000 as the halving interval (wrong -- this is a block count, not a time duration). And it kept 50 as the initial reward (wrong -- this was sized for 10-minute blocks).

## Two Bugs, Not One

### Bug 1: Per-Validator Instead of Per-Round

**Pre-Staking Emission Bug:** Before staking is active, the pre-staking emission model should split `block_reward` equally among all validators. Instead, each validator received the **full** reward. With 5 validators: 250 UDAG/round instead of 50.

Root cause: the validator count used for splitting was derived from DAG vertex counts in the previous round, which was 0 at startup. `max(0, 1) = 1`, so each validator got the undivided reward.

**Fix:** Added `configured_validator_count` field to StateEngine, set from the `--validators N` CLI flag. Both engine and validator loop now use this deterministic value instead of counting vertices in finality batches.

### Bug 2: The Schedule Itself

Even after fixing the per-validator split, the emission was still far too fast. At 50 UDAG/round with 5-second rounds:

- 17,280 rounds/day x 50 UDAG = **864,000 UDAG/day**
- First halving at 210,000 rounds = **12.15 days**
- 60% of all mining rewards emitted in under two weeks
- Supply effectively exhausted within 3 months

For comparison, Bitcoin's first halving took 4 years. Most serious L1s measure halvings in years, not days.

## The Redesign

We fixed `max_supply` at 21M and solved for reward and interval combinations that produce credible timelines:

| Reward/Round | Halving Interval | First Halving |
|-------------|-----------------|---------------|
| 50 UDAG | 210,000 | 12 days |
| 5 UDAG | 2,100,000 | 4 months |
| 2 UDAG | 5,250,000 | 10 months |
| **1 UDAG** | **10,500,000** | **~1.66 years** |

We chose **1 UDAG per round** with a **10,500,000-round halving interval**. The identity holds:

```
1 UDAG x 10,500,000 rounds x 2 = 21,000,000 UDAG
```

## The New Schedule

Genesis allocates 2,050,000 UDAG (faucet reserve + developer allocation), leaving 18,950,000 UDAG for mining. The supply cap in StateEngine enforces the 21M ceiling -- once cumulative mining + genesis reaches 21M, block rewards drop to zero regardless of the halving schedule.

| Period | Reward/Round | Mined | Cumulative | % of 21M |
|--------|-------------|-------|------------|----------|
| Year 0-1.7 | 1.0000 UDAG | 10,500,000 | 12,550,000 | 59.8% |
| Year 1.7-3.3 | 0.5000 UDAG | 5,250,000 | 17,800,000 | 84.8% |
| Year 3.3-5.0 | 0.2500 UDAG | 2,625,000 | 20,425,000 | 97.3% |
| Year 5.0-5.7 | 0.1250 UDAG | 575,000 | 21,000,000 | 100% |

Supply cap reached at approximately **year 5.7**. After that, validators earn only from transaction fees.

## Validator Economics

Daily nominal reward is 17,280 UDAG (one per round × 17,280 rounds per day). The validator pool is 44% of that — 7,603.2 UDAG/day. Split among validators:

| Validators | UDAG/Day Each | UDAG/Year Each |
|-----------|--------------|----------------|
| 5 (current testnet) | 1,520 | 555,000 |
| 10 | 760 | 277,500 |
| 25 | 304 | 111,000 |
| 100 (max active) | 76 | 27,750 |

*Math: 86,400 s/day ÷ 5 s/round = 17,280 rounds/day. Validator pool per day = 17,280 × 0.44 = 7,603.2 UDAG. Divide equally by N validators for per-node income, before any delegation effects or commission. At the full 100-validator cap each node earns ~76 UDAG/day in the first halving period. The remaining 44% of the nominal reward is split 10% council + 16% treasury + 5% founder + 8% ecosystem + 5% reserve; the final 12% is uncreated, matching the IDO genesis pre-mine.*

> **Historical note:** this table was updated on 2026-04-10 when the distribution model changed from the 75% validator / 10% council / 10% treasury / 5% founder split to the current 7-bucket model. Earlier versions of this post showed higher per-validator daily numbers based on the 75% validator share.

## Why Not Exactly Bitcoin's Timeline?

Bitcoin's 4-year halvings at 5-second rounds would require an initial reward of ~0.033 UDAG per round. That's too small to meaningfully reward early validators, especially when split across up to 100 active nodes. The daily emission per validator would be a fraction of a UDAG — not enough to matter.

1.66-year halvings hit the sweet spot: aggressive enough to reward early participants, conservative enough that the schedule extends over 5+ years of meaningful emission. The supply cap provides a hard ceiling regardless.

## What Changed

```rust
// Before
INITIAL_REWARD_SATS = 50 * COIN      // 50 UDAG per round
HALVING_INTERVAL    = 210,000         // ~12 days at 5s rounds

// After
INITIAL_REWARD_SATS = 1 * COIN       // 1 UDAG per round
HALVING_INTERVAL    = 10,500,000      // ~1.66 years at 5s rounds
```

Two constants changed. The rest of the system -- supply cap enforcement, stake-proportional distribution, observer penalties, halving logic -- was already correct. The fix was purely parametric.

> The emission schedule is enforced at three levels: `block_reward(height)` computes the halved reward, `apply_vertex_with_validators()` caps coinbase at `MAX_SUPPLY - total_supply`, and the supply invariant (`liquid + staked == total_supply`) is checked unconditionally in release builds.
