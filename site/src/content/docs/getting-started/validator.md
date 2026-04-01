---
title: "Run a Validator"
description: "Step-by-step guide to becoming an UltraDAG validator — generate keys, stake UDAG, and earn block rewards."
order: 2
section: "getting-started"
---

# Run a Validator

This guide walks you through becoming an UltraDAG validator — from generating keys to staking UDAG and earning rewards.

---

## Overview

Validators produce DAG vertices, participate in BFT finality, and earn block rewards proportional to their effective stake. UltraDAG supports up to **100 active validators** at any time, selected by effective stake (own stake + delegations).

**Requirements:**

- Minimum stake: **2,000 UDAG**
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
  "secret_key": "9f8e7d6c5b4a39281706f5e4d3c2b1a09f8e7d6c5b4a39281706f5e4d3c2b1a0"
}
```

<div class="callout callout-warning"><div class="callout-title">Secure your private key</div>Save the private key securely. Anyone with access to this key can control your validator and its staked funds. Never share it or commit it to version control.</div>

Save the private key to a file:

```bash
echo "9f8e7d6c5b4a..." > validator.key
chmod 600 validator.key
```

---

## Step 2: Get Testnet UDAG

On the testnet, use the faucet to get initial funds:

```bash
curl -X POST http://localhost:10333/faucet -H "Content-Type: application/json" -d '{"address":"e7f8a9b0c1d2...","amount":10000000000}'
```

```json
{
  "tx_hash": "abc123...",
  "amount": 10000000000,
  "message": "Sent 100 UDAG to e7f8a9b0c1d2..."
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
  --auto-stake 10000 \
  --testnet
```

**Flags explained:**

| Flag | Purpose |
|------|---------|
| `--validate` | Enable validator mode (produce vertices) |
| `--pkey` | Provide private key directly (64-char hex Ed25519 secret key) |
| `--validator-key` | Path to **allowlist file** of trusted validator addresses (one address per line). Only listed validators count toward quorum/finality. |
| `--auto-stake <UDAG>` | Automatically stake the specified amount of UDAG on startup (e.g., `--auto-stake 10000`) |
| `--testnet` | Connect to testnet (enables faucet and testnet endpoints) |

<div class="callout callout-tip"><div class="callout-title">Key priority</div><code>--pkey</code> takes priority over a key file on disk (<code>validator.key</code>). If neither is provided, a new keypair is generated automatically. Note: <code>--validator-key</code> is NOT a private key file -- it is the permissioned validator allowlist.</div>

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
  "is_active_validator": true
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
      "is_active_validator": true
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
    "secret_key": "9f8e7d6c5b4a...",
    "amount": 1000000000000
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
    "secret_key": "9f8e7d6c5b4a..."
  }'
```

<div class="callout callout-note"><div class="callout-title">Unstaking cooldown</div>Unstaked funds have a cooldown period of <strong>2,016 rounds</strong> (~2.8 hours at 5-second rounds) before they become liquid again. During cooldown, the funds do not earn rewards and cannot be transferred.</div>

---

## Staking Economics

### Minimum Stake

The minimum stake to become a validator is **2,000 UDAG** (200,000,000,000 sats).

### Active Validator Set

Only the top **100 validators** by effective stake are active in each epoch. The validator set is recalculated every **210,000 rounds** (one epoch, approximately 12 days at 5-second rounds).

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
2. Validators ranked 1-100 become active for the next epoch
3. Validators ranked 101+ continue earning passive staking rewards (50% rate)

---

## Commission for Delegations

As a validator, you earn commission on delegated stake:

```bash
curl -X POST http://localhost:10333/set-commission \
  -H "Content-Type: application/json" \
  -d '{
    "secret_key": "9f8e7d6c5b4a...",
    "commission_percent": 15
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

<div class="callout callout-danger"><div class="callout-title">Equivocation prevention</div>Never run two validator instances with the same key simultaneously. This is the primary cause of equivocation. Use a single, well-monitored node per validator key.</div>

See [Security Model](/docs/security/model) for full details on slashing mechanics.

---

## Troubleshooting

### "Insufficient stake" error

Ensure you have at least 2,000 UDAG (200,000,000,000 sats) available for staking. Check your balance:

```bash
curl http://localhost:10333/balance/YOUR_ADDRESS
```

### Not in active validator set

If there are already 100 validators with more effective stake, you will not be active. You still earn passive staking rewards (50% rate). Increase your stake or attract delegations to enter the active set.

### Node not producing vertices

1. Verify `--validate` flag is set
2. Check that your key is loaded: `curl /status` should show your address
3. Ensure you are connected to peers: `curl /peers`
4. Check finality is progressing: `curl /status` — `finalized_round` should be advancing

---

## Next Steps

- [Validator Handbook](/docs/operations/validator-handbook) — detailed operational guide
- [Staking & Delegation](/docs/tokenomics/staking) — full tokenomics details
- [Monitoring](/docs/operations/monitoring) — set up metrics and alerts
- [Governance](/docs/tokenomics/governance) — participate in protocol governance
