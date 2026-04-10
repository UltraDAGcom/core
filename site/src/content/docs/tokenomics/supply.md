---
title: "Supply & Emission"
description: "Fixed 21M supply, 7-bucket distribution, Bitcoin-style halving, deflationary slashing"
order: 1
section: "tokenomics"
---

# Supply & Emission

UltraDAG has a fixed maximum supply of 21 million UDAG. The distribution model uses **7 buckets**: 6 are distributed through per-round protocol emission (validators, council, treasury, founder, ecosystem, reserve), and 1 is a genesis pre-mine (IDO / liquidity) to bootstrap the private round and Uniswap liquidity. This page covers the emission curve, the bucket split, and supply enforcement.

---

## Maximum Supply

| Parameter | Value |
|-----------|-------|
| Max supply | 21,000,000 UDAG |
| Smallest unit | 1 sat = 0.00000001 UDAG |
| Sats per UDAG | 100,000,000 |
| Max supply in sats | 2,100,000,000,000,000 |

The maximum supply is a hard protocol constant — no governance proposal can increase it.

---

## Emission Schedule

### Block Reward

New UDAG is minted each round (not per vertex) according to a halving schedule:

| Era | Rounds | Reward per Round | Cumulative Emission |
|-----|--------|-----------------|-------------------|
| 1 | 0 — 10,499,999 | 1.00000000 UDAG | 10,500,000 UDAG |
| 2 | 10,500,000 — 20,999,999 | 0.50000000 UDAG | 15,750,000 UDAG |
| 3 | 21,000,000 — 31,499,999 | 0.25000000 UDAG | 18,375,000 UDAG |
| 4 | 31,500,000 — 41,999,999 | 0.12500000 UDAG | 19,687,500 UDAG |
| ... | ... | ... | ... |
| 64 | ... | < 1 sat | 21,000,000 UDAG |

### Halving Interval

$$
\text{halving\_interval} = 10{,}500{,}000 \text{ rounds}
$$

At 5-second rounds:

$$
10{,}500{,}000 \times 5\text{s} = 52{,}500{,}000\text{s} \approx 1.66 \text{ years}
$$

### Full Emission Timeline

The nominal emission curve follows a geometric series summing to 21M UDAG over 64 halvings (~106 years). Because each round only mints 88% of the nominal reward (the six emission buckets — see below), the **actual protocol-emitted supply** converges to **18.48M UDAG**, and the remaining **2.52M UDAG** is the IDO genesis pre-mine — together totalling exactly 21M.

```
Nominal curve:
  Era  1: +10,500,000.00 UDAG  (50.00% of max)
  Era  2:  +5,250,000.00 UDAG  (75.00% of max)
  Era  3:  +2,625,000.00 UDAG  (87.50% of max)
  Era  4:  +1,312,500.00 UDAG  (93.75% of max)
  ...
  Era 64:          < 1 sat     (100.00% of max)

Actual per round:
  0.88 × nominal  →  emitted to the 6 buckets
  0.12 × nominal  →  offset by 2.52M IDO pre-mine at genesis
```

<div class="callout callout-info"><div class="callout-title">Reward precision</div>When the halved reward drops below 1 sat (the smallest representable unit), the reward becomes 0 and emission stops permanently. This occurs after approximately 64 halvings.</div>

---

## Genesis Allocation & Emission Buckets

| Bucket | Share | UDAG | Delivery |
|---|---|---|---|
| **Validators / Staking** | 44% | 9,240,000 | Per-round emission, proportional to effective stake |
| **Council of 21** | 10% | 2,100,000 | Per-round emission, equal split among seated members |
| **DAO Treasury** | 16% | 3,360,000 | Per-round emission, spent via `TreasurySpend` proposals |
| **Founder** | 5% | 1,050,000 | Per-round emission, liquid balance |
| **Ecosystem** | 8% | 1,680,000 | Per-round emission to ecosystem multisig (airdrops, grants) |
| **Reserve** | 5% | 1,050,000 | Per-round emission to reserve multisig (strategic use) |
| **IDO / Liquidity** | 12% | 2,520,000 | **Genesis pre-mine** to IDO distributor (private round + Uniswap seed) |
| **Total** | 100% | 21,000,000 | |

<div class="callout callout-note"><div class="callout-title">Only one pre-mine</div>Of all seven buckets, only the IDO distributor is pre-minted at genesis (2.52M UDAG). Every other bucket starts at zero and accumulates through per-round emission. This preserves the fair-launch spirit for protocol participants while still allowing a working day-1 market for private-round buyers and Uniswap liquidity providers.</div>

<div class="callout callout-note"><div class="callout-title">Testnet faucet</div>Testnet builds add a 1,000,000 UDAG faucet reserve for testing. This is feature-gated and excluded from mainnet genesis.</div>

---

## Reward Distribution

Each round, the nominal block reward is split across the six emission buckets as follows:

### Distribution Flow

*The nominal round reward is split: Validator Pool 44%, Council Pool 10%, DAO Treasury 16%, Founder 5%, Ecosystem 8%, Reserve 5% (sum = 88%). The Validator Pool is distributed proportionally to effective stake; the Council Pool is split equally among the 21 seats (unfilled seats flow to treasury); the other four go to fixed protocol addresses.*

### Validator Rewards

The validator pool (44% of the nominal round reward) is distributed to validators:

- **Active validators** (producing vertices): receive rewards proportional to effective stake
- **Passive stakers** (staked but not in the top-100 active set): receive 50% of what an equivalent active validator would earn

$$
\text{validator\_reward}_i = \text{round\_reward} \times \frac{\text{validator\_emission\_percent}}{100} \times \frac{\text{effective\_stake}_i}{\sum \text{effective\_stakes}}
$$

### Council Rewards

10% of the nominal round reward (default, governable 0–30%) is allocated to the Council of 21, split using a **fixed denominator**: each seat earns `council_total / 21` regardless of how many seats are filled. Unfilled-seat residual flows to the DAO treasury.

$$
\text{council\_share}_i = \frac{\text{round\_reward} \times \text{council\_emission\_percent}}{100 \times 21}
$$

---

## Per-Round Protocol Distribution

<div class="callout callout-warning"><div class="callout-title">Per-round, not per-vertex</div>Rewards are minted <strong>once per finalized round</strong>, not once per vertex. In a round with multiple finalized vertices from different validators, the protocol distributes exactly one round reward across the six buckets. This prevents inflation variance based on the number of vertices produced.</div>

The distribution sequence each round:

1. Calculate the nominal era reward: `reward = initial_reward >> (round / halving_interval)`
2. Cap at remaining supply: `reward = min(reward, MAX_SUPPLY - total_supply)`
3. Credit `reward × council_emission_percent / 100` to council (seated members) + residual to treasury
4. Credit `reward × treasury_emission_percent / 100` to treasury
5. Credit `reward × founder_emission_percent / 100` to founder address
6. Credit `reward × ecosystem_emission_percent / 100` to ecosystem address
7. Credit `reward × reserve_emission_percent / 100` to reserve address
8. Credit `reward × validator_emission_percent / 100` to validators proportional to effective stake
9. Verify supply invariant (`sum of balances + treasury == total_supply`)

---

## Fee Handling

Transaction fees are **not** part of the emission — they come from existing circulating supply:

| Aspect | Behavior |
|--------|----------|
| Fee collection | Fees are collected from the transaction sender |
| Fee destination | Fees go to the vertex producer via deferred coinbase (collected from successful txs only) |
| Coinbase | Vertex coinbase contains collected fees only (no minted reward) |
| Fee-exempt operations | Stake, Unstake, Delegate, Undelegate, SetCommission |
| Minimum fee | 10,000 sats (0.0001 UDAG) for non-exempt transactions |

Minted rewards are distributed separately via `distribute_round_rewards()`. Fees are included in the vertex producer's coinbase independently.

---

## Supply Cap Enforcement

The protocol enforces the supply cap at multiple levels:

### Minting Cap

```rust
let reward = base_reward >> (current_round / HALVING_INTERVAL);
let capped = std::cmp::min(reward, MAX_SUPPLY_SATS - total_supply);
```

If `total_supply` equals `MAX_SUPPLY_SATS`, no new UDAG is minted. The protocol continues operating on fees only.

### Supply Invariant

After every state transition:

$$
\text{liquid} + \text{staked} + \text{delegated} + \text{treasury} + \text{bridge} + \text{streamed} = \text{total\_supply} \leq \text{MAX\_SUPPLY}
$$

Where `liquid` includes the IDO pre-mine address, ecosystem and reserve multisig addresses, and all validator/founder balances. Violation triggers immediate node halt (exit code 101).

### Slashing is Deflationary

When a validator is slashed for equivocation, the slashed amount is **burned** — removed from `total_supply`. This makes slashing deflationary:

$$
\text{total\_supply}_{\text{new}} = \text{total\_supply}_{\text{old}} - \text{slashed\_amount}
$$

Burned supply can never be re-minted. The effective max supply decreases permanently with each slash event.

---

## Comparison with Bitcoin

| Property | UltraDAG | Bitcoin |
|----------|----------|--------|
| Max supply | 21,000,000 | 21,000,000 |
| Smallest unit | sat (10^-8) | sat (10^-8) |
| Halving interval | 10,500,000 rounds (~1.66 yr) | 210,000 blocks (~4 yr) |
| Initial reward | 1 UDAG/round | 50 BTC/block |
| Full emission | ~106 years | ~140 years |
| Deflation mechanism | Slashing burns | Lost coins |
| Fee model | Min fee + exempt staking ops | Market-driven fees |

---

## Next Steps

- [Staking & Delegation](/docs/tokenomics/staking) — how rewards are earned
- [Governance](/docs/tokenomics/governance) — council reward allocation
