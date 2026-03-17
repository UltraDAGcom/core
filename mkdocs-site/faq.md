---
title: FAQ
---

# Frequently Asked Questions

---

## General

### What is UltraDAG?

UltraDAG is a lightweight DAG-BFT cryptocurrency purpose-built for IoT and machine-to-machine micropayments. It delivers deterministic finality in two rounds (~10 seconds), runs on hardware as small as a $5/month VPS, and ships as a single binary under 2 MB.

### How is UltraDAG different from other blockchains?

UltraDAG is the only chain where a full validator fits in a sub-2 MB binary with bounded storage, fast finality, and proper staking/slashing. Unlike IOTA (no predictable finality), Helium (LoRa-only), or IoTeX (heavy EVM), UltraDAG is minimal by design.

### What is the target use case?

Sensors, IoT devices, and autonomous agents making frequent tiny payments without human intervention. Example: a weather sensor selling data to a drone for 0.001 UDAG per reading, confirmed in under 10 seconds.

### Is UltraDAG a blockchain?

Not exactly. UltraDAG uses a **Directed Acyclic Graph** (DAG) instead of a linear chain of blocks. Multiple validators produce vertices in parallel, and BFT finality determines which vertices are confirmed. There is no single chain of blocks.

### Is UltraDAG open source?

Yes. The full codebase is available at [github.com/UltraDAGcom/core](https://github.com/UltraDAGcom/core) under the MIT license.

---

## Getting Started

### How do I run a node?

Install Rust and build from source:

```bash
git clone https://github.com/UltraDAGcom/core.git
cd core
cargo build --release -p ultradag-node
cargo run --release -p ultradag-node -- --port 9333 --validate --testnet
```

See the [Quick Start](getting-started/quickstart.md) for full instructions.

### How do I get testnet UDAG?

Use the faucet endpoint:

```bash
curl -X POST http://localhost:10333/faucet -H "Content-Type: application/json" -d '{"address":"YOUR_ADDRESS","amount":100000000}'
```

The faucet distributes up to 100 UDAG per request, rate limited to 1 request per 10 minutes.

### How do I send a transaction?

On testnet, use the convenience endpoint:

```bash
curl -X POST http://localhost:10333/tx \
  -H "Content-Type: application/json" \
  -d '{"secret_key":"YOUR_KEY","to":"RECIPIENT","amount":50000000000,"fee":10000}'
```

For mainnet, sign transactions client-side using an [SDK](api/sdks.md) and submit via `/tx/submit`.

### What are the system requirements?

- **Minimum**: 1 CPU core, 128 MB RAM, 1 GB disk
- **Recommended**: 1 CPU core, 512 MB RAM, 5 GB disk
- The node binary is under 2 MB

---

## Staking

### What is the minimum stake?

**10,000 UDAG** (1,000,000,000,000 sats) is required to become eligible as a validator.

### How do I earn rewards?

Stake UDAG and your node will earn rewards proportional to your effective stake (own stake + delegations). Active validators (top 21) earn 100% of their proportional share. Passive stakers earn 20%.

### How long is the unstaking cooldown?

**2,016 rounds** (~2.8 hours at 5-second rounds). During cooldown, funds earn no rewards and cannot be transferred.

### What happens if I get slashed?

Equivocation (producing two different vertices in the same round) results in **50% of your stake being burned**. The slash cascades proportionally to delegators. The burned amount is permanently removed from total supply.

### Can I delegate without running a node?

Yes. Delegation allows you to earn rewards without operating validator infrastructure. Delegate to a validator using the `/delegate` endpoint or an SDK. The minimum delegation is 100 UDAG.

### How does commission work?

Validators set a commission rate (default 10%, range 0-100%) that determines their cut of delegation rewards. Delegators receive the remainder proportionally. Check a validator's commission before delegating.

---

## Governance

### Who can vote on proposals?

Only **council members** can create proposals and vote. Council membership is granted through DAO proposals — no stake is required to be a council member.

### What can governance change?

10 protocol parameters including minimum fee, slash percentage, voting period, council emission share, and observer reward rate. See [Governance](tokenomics/governance.md) for the full list.

### What is the DAO activation gate?

ParameterChange proposals require at least **8 active validators** to execute. This prevents a small group from modifying protocol parameters before the network is sufficiently decentralized.

### How does the Council of 21 work?

The council has 21 seats across 6 categories (Technical, Business, Legal, Academic, Community, Foundation). Each member has exactly 1 vote regardless of stake. Seats are granted and revoked through `CouncilMembership` proposals.

---

## Technical

### How fast is finality?

**2 rounds**, approximately **10 seconds** at the default 5-second round time. This is deterministic BFT finality, not probabilistic — once finalized, a transaction cannot be reversed.

### What happens to old data?

UltraDAG automatically **prunes** DAG vertices older than 1000 rounds (configurable). Account state is retained in the redb database. New nodes sync from checkpoints instead of replaying the full history. Use `--archive` to disable pruning.

### What is the maximum supply?

**21,000,000 UDAG** (same as Bitcoin). The initial reward is 1 UDAG per round, halving every 10,500,000 rounds (~1.66 years). Full emission takes approximately 106 years.

### What is the fee structure?

- **Transfers**: minimum 10,000 sats (0.0001 UDAG)
- **Governance** (proposals, votes): minimum 10,000 sats
- **Staking operations**: zero fee (Stake, Unstake, Delegate, Undelegate, SetCommission)

### How many validators can participate?

Up to **21 active validators** (top by effective stake). Additional stakers earn passive rewards at 20% rate. The validator set is recalculated every 210,000 rounds (~12 days).

### How is the state root computed?

The state root is a Blake3 hash of a **canonical byte representation** of all state (accounts, stakes, delegations, governance). It uses hand-rolled serialization (not serde) with a version prefix to ensure determinism across binary versions.

---

## Troubleshooting

### My node won't start

Common causes:

- **Port in use**: check with `ss -tlnp | grep 9333`
- **Corrupted state**: delete the data directory or use `--skip-fast-sync`
- **Bad key format**: ensure `--pkey` is exactly 64 hex characters

### Finality is stuck (high lag)

- **Check peers**: `curl http://localhost:10333/peers` — need at least 2 connected peers
- **Check validators**: `curl http://localhost:10333/validators` — active set must be >= 3
- **Check health**: `curl http://localhost:10333/health/detailed` — look for degraded components

### My node has no peers

- Verify the P2P port (default 9333) is reachable from the internet
- Check firewall rules
- Try explicit seed: `--seed known-peer-ip:9333`
- Ensure bootstrap nodes are reachable: `curl https://ultradag-node-1.fly.dev/status`

### My transaction is stuck as pending

- **Verify nonce**: `curl http://localhost:10333/balance/YOUR_ADDRESS` — nonce must match
- **Check fee**: minimum 10,000 sats for transfers and governance transactions
- **Check mempool**: `curl http://localhost:10333/mempool` — is your tx in the pool?
- **Wait for finality**: transactions are finalized in ~10 seconds under normal conditions

### Balance not updating after transaction

- **Wait for finality**: balance updates only after the transaction is included in a finalized vertex
- **Check tx status**: `curl http://localhost:10333/tx/TX_HASH`
- **Verify correct address**: addresses are 64-character hex strings

### Node using too much memory

- Verify pruning is enabled: default is 1000 rounds. `--archive` disables pruning.
- Check log level: `RUST_LOG=trace` can cause memory growth from log buffering
- Expected usage: 128-512 MB under normal operation

---

## Network

### Where are the testnet nodes?

5 nodes on Fly.io in Amsterdam:

| Node | RPC Endpoint |
|------|-------------|
| Node 1 | `https://ultradag-node-1.fly.dev` |
| Node 2 | `https://ultradag-node-2.fly.dev` |
| Node 3 | `https://ultradag-node-3.fly.dev` |
| Node 4 | `https://ultradag-node-4.fly.dev` |
| Node 5 | `https://ultradag-node-5.fly.dev` |

### Is mainnet live?

Not yet. The testnet is live and operational. See the [mainnet launch checklist](security/audits.md) for progress.

### How do I check if the network is healthy?

```bash
curl https://ultradag-node-1.fly.dev/health/detailed
```

Look for: `finality_lag <= 3`, `peers >= 3`, all components `healthy`.

---

## SDKs

### Which SDKs are available?

Python, JavaScript/TypeScript, Rust, and Go. All support local key generation, transaction signing, and complete RPC access. See [SDKs](api/sdks.md).

### Do I need an SDK?

For testnet convenience, you can use `curl` with the RPC endpoints directly. For mainnet (where private keys cannot be sent to the server), you need an SDK for client-side transaction signing.
