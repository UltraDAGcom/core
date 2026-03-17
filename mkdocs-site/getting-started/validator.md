---
title: Run a Validator
---

# Run a Validator

This guide walks you through becoming an UltraDAG validator — from generating keys to staking UDAG and earning rewards.

---

## Overview

Validators produce DAG vertices, participate in BFT finality, and earn block rewards proportional to their effective stake. UltraDAG supports up to **21 active validators** at any time, selected by effective stake (own stake + delegations).

**Requirements:**

- Minimum stake: **10,000 UDAG**
- Stable internet connection
- Low-latency network path to other validators
- Hardware: any machine capable of running the < 2 MB binary (1 CPU core, 128 MB RAM minimum)

---

## Step 1: Generate a Keypair

Generate a fresh Ed25519 keypair:

```bash
curl http://localhost:10333/keygen
```

```json
{
  "address": "e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8",
  "public_key": "3a4b5c6d7e8f9a0b1c2d3e4f5a6b7c8d9e0f1a2b3c4d5e6f7a8b9c0d1e2f3a4b",
  "private_key": "9f8e7d6c5b4a39281706f5e4d3c2b1a09f8e7d6c5b4a39281706f5e4d3c2b1a0"
}
```

!!! warning "Secure your private key"
    Save the private key securely. Anyone with access to this key can control your validator and its staked funds. Never share it or commit it to version control.

Save the private key to a file:

```bash
echo "9f8e7d6c5b4a..." > validator.key
chmod 600 validator.key
```

---

## Step 2: Get Testnet UDAG

On the testnet, use the faucet to get initial funds:

```bash
curl -X POST http://localhost:10333/faucet -H "Content-Type: application/json" -d '{"address":"e7f8a9b0c1d2...","amount":100000000}'
```

```json
{
  "tx_hash": "abc123...",
  "amount": 100000000000,
  "message": "Sent 1000 UDAG to e7f8a9b0c1d2..."
}
```

You need at least 10,000 UDAG to stake. Request the faucet multiple times if needed (there may be rate limits).

Verify your balance:

```bash
curl http://localhost:10333/balance/e7f8a9b0c1d2...
```

---

## Step 3: Run the Validator Node

Start the node with your private key and auto-staking enabled:

```bash
cargo run --release -p ultradag-node -- \
  --port 9333 \
  --validate \
  --pkey 9f8e7d6c5b4a... \
  --auto-stake \
  --testnet
```

Or using the key file:

```bash
cargo run --release -p ultradag-node -- \
  --port 9333 \
  --validate \
  --validator-key validator.key \
  --auto-stake \
  --testnet
```

**Flags explained:**

| Flag | Purpose |
|------|---------|
| `--validate` | Enable validator mode (produce vertices) |
| `--pkey` | Provide private key directly (alternative to `--validator-key`) |
| `--validator-key` | Path to file containing private key |
| `--auto-stake` | Automatically stake all available balance on startup |
| `--testnet` | Connect to testnet (enables faucet and testnet endpoints) |

!!! tip "Key priority"
    If both `--pkey` and `--validator-key` are provided, `--pkey` takes precedence. If neither is provided, a new keypair is generated automatically.

---

## Step 4: Verify Validator Status

### Check Your Stake

```bash
curl http://localhost:10333/stake/e7f8a9b0c1d2...
```

```json
{
  "address": "e7f8a9b0c1d2...",
  "staked_amount": 1000000000000,
  "commission_percent": 10,
  "effective_stake": 1000000000000,
  "delegator_count": 0,
  "is_active": true
}
```

### View the Active Validator Set

```bash
curl http://localhost:10333/validators
```

```json
{
  "validators": [
    {
      "address": "e7f8a9b0c1d2...",
      "effective_stake": 1000000000000,
      "commission_percent": 10,
      "is_active": true
    }
  ],
  "total_staked": 1000000000000,
  "active_count": 1,
  "max_validators": 21
}
```

---

## Step 5: Manual Staking (Without Auto-Stake)

If you prefer to stake manually rather than using `--auto-stake`:

### Stake UDAG

```bash
curl -X POST http://localhost:10333/stake \
  -H "Content-Type: application/json" \
  -d '{
    "from": "e7f8a9b0c1d2...",
    "amount": 1000000000000,
    "private_key": "9f8e7d6c5b4a..."
  }'
```

```json
{
  "tx_hash": "stake_abc123...",
  "status": "accepted",
  "message": "Staked 10000 UDAG"
}
```

### Unstake UDAG

```bash
curl -X POST http://localhost:10333/unstake \
  -H "Content-Type: application/json" \
  -d '{
    "from": "e7f8a9b0c1d2...",
    "amount": 500000000000,
    "private_key": "9f8e7d6c5b4a..."
  }'
```

!!! note "Unstaking cooldown"
    Unstaked funds have a cooldown period of **2,016 rounds** (~2.8 hours at 5-second rounds) before they become liquid again. During cooldown, the funds do not earn rewards and cannot be transferred.

---

## Staking Economics

### Minimum Stake

The minimum stake to become a validator is **10,000 UDAG** (1,000,000,000,000 sats).

### Active Validator Set

Only the top **21 validators** by effective stake are active in each epoch. The validator set is recalculated every **210,000 rounds** (one epoch, approximately 12 days at 5-second rounds).

### Effective Stake

Your effective stake includes both your own stake and any delegations from other users:

$$
\text{effective\_stake} = \text{own\_stake} + \sum \text{delegations}
$$

### Reward Distribution

Active validators earn rewards proportional to their effective stake:

$$
\text{reward}_i = \text{round\_reward} \times \frac{\text{effective\_stake}_i}{\sum \text{effective\_stakes}}
$$

For delegation rewards, the validator takes their commission percentage, and the remainder is distributed to delegators.

### Fee Exemptions

Staking operations are **fee-exempt** — you pay zero transaction fees for:

- Stake
- Unstake
- Delegate
- Undelegate
- SetCommission

---

## Epoch System

UltraDAG operates on an epoch system:

| Parameter | Value |
|-----------|-------|
| Epoch length | 210,000 rounds |
| Duration (at 5s rounds) | ~12.15 days |
| Validator set recalculation | Once per epoch |
| Supply halving | Every 50 epochs (10,500,000 rounds) |

At each epoch boundary:

1. Active validator set is recalculated based on effective stake rankings
2. Validators ranked 1-21 become active for the next epoch
3. Validators outside top 21 continue earning passive staking rewards (20% rate)

---

## Commission for Delegations

As a validator, you earn commission on delegated stake:

```bash
curl -X POST http://localhost:10333/set-commission \
  -H "Content-Type: application/json" \
  -d '{
    "from": "e7f8a9b0c1d2...",
    "commission_percent": 15,
    "private_key": "9f8e7d6c5b4a..."
  }'
```

- Default commission: **10%**
- Range: **0-100%**
- Commission applies to the delegator portion of rewards

**Example:** If a round produces 0.5 UDAG in rewards for your validator slot, and 40% of your effective stake comes from delegations:

- Validator base share (60%): 0.30 UDAG
- Delegation pool (40%): 0.20 UDAG
- Your commission (15% of 0.20): 0.03 UDAG
- Delegators receive: 0.17 UDAG (split proportionally)
- **Your total**: 0.33 UDAG

---

## Slashing Risks

Validators can be slashed for equivocation (producing conflicting vertices in the same round):

- **Penalty**: 50% of staked amount (burned, reducing total supply)
- **Governable range**: 10-100% via Council proposal
- **Cascading**: Delegators also lose proportionally

!!! danger "Equivocation prevention"
    Never run two validator instances with the same key simultaneously. This is the primary cause of equivocation. Use a single, well-monitored node per validator key.

See [Security Model](../security/model.md) for full details on slashing mechanics.

---

## Troubleshooting

### "Insufficient stake" error

Ensure you have at least 10,000 UDAG (1,000,000,000,000 sats) available for staking. Check your balance:

```bash
curl http://localhost:10333/balance/YOUR_ADDRESS
```

### Not in active validator set

If there are already 21 validators with more effective stake, you will not be active. You still earn passive staking rewards (20% rate). Increase your stake or attract delegations to enter the active set.

### Node not producing vertices

1. Verify `--validate` flag is set
2. Check that your key is loaded: `curl /status` should show your address
3. Ensure you are connected to peers: `curl /peers`
4. Check finality is progressing: `curl /status` — `finalized_round` should be advancing

---

## Next Steps

- [Validator Handbook](../operations/validator-handbook.md) — detailed operational guide
- [Staking & Delegation](../tokenomics/staking.md) — full tokenomics details
- [Monitoring](../operations/monitoring.md) — set up metrics and alerts
- [Governance](../tokenomics/governance.md) — participate in protocol governance
