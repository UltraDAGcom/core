---
title: "RPC Endpoints"
description: "Complete JSON-over-HTTP RPC API reference for UltraDAG nodes"
order: 1
section: "api"
---

# RPC API Reference

UltraDAG exposes a JSON-over-HTTP RPC API for wallets, explorers, and integrations. The default RPC port is **P2P port + 1000** (e.g., P2P `9333` = RPC `10333`).

---

## Core Endpoints

### GET /status

Returns node status and network overview.

```bash
curl http://localhost:10333/status
```

```json
{
  "dag_round": 1542,
  "last_finalized_round": 1540,
  "peers": 4,
  "total_supply": 1050154200000000,
  "total_supply_udag": 10501542.0,
  "total_staked": 50000000000000,
  "active_stakers": 5,
  "mempool_size": 3,
  "dag_vertices": 7710
}
```

### GET /balance/:address

Returns the balance, staking state, smart-account flag, and bech32m encoding for an address. Accepts either a 40-char hex address, a bech32m address (`tudg1…` / `udg1…`), or a registered `@name`.

```bash
curl http://localhost:10333/balance/a1b2c3d4e5f60011223344556677889900aabbcc
```

```json
{
  "address": "a1b2c3d4e5f60011223344556677889900aabbcc",
  "address_bech32": "tudg159j0p84uhcqq3yv6x32kvau7yeq42e77m4wcyg",
  "balance": 500000000000,
  "balance_udag": 5000.0,
  "nonce": 7,
  "staked": 200000000000,
  "staked_udag": 2000.0,
  "is_council_member": false,
  "is_smart_account": false
}
```

Addresses are 20 bytes (40 hex chars). The minimum active stake is 2,000 UDAG = 200,000,000,000 sats — anything below that counts as unstaked even if the field is non-zero.

### GET /health

Simple health check. Returns HTTP 200 if the node is running.

```bash
curl http://localhost:10333/health
```

```json
{
  "status": "ok"
}
```

### GET /health/detailed

Component-level diagnostics for monitoring systems.

```bash
curl http://localhost:10333/health/detailed
```

```json
{
  "status": "healthy",
  "components": {
    "dag": { "status": "healthy", "round": 1542, "vertices": 7710 },
    "finality": { "status": "healthy", "finalized_round": 1540, "lag": 2 },
    "state": { "status": "healthy", "accounts": 42, "supply": 1050154200000000 },
    "mempool": { "status": "healthy", "size": 3, "capacity": 10000 },
    "network": { "status": "healthy", "peers": 4 },
    "checkpoints": { "status": "healthy", "latest_round": 1500 }
  }
}
```

### GET /keygen

Generate a new Ed25519 keypair. Returns the private key in plaintext.

```bash
curl http://localhost:10333/keygen
```

```json
{
  "address": "e7f8a9b0c1d2...",
  "secret_key": "9f8e7d6c5b4a...",
  "warning": "TESTNET ONLY"
}
```

<div class="callout callout-warning"><div class="callout-title">Testnet only</div>This endpoint is disabled in mainnet mode (returns HTTP 410 GONE). On mainnet, generate keys client-side using an <a href="/docs/api/sdks">SDK</a>.</div>

### POST /faucet

Request testnet UDAG. Rate limited to 1 request per 10 minutes per IP.

```bash
curl -X POST http://localhost:10333/faucet \
  -H "Content-Type: application/json" \
  -d '{"address": "e7f8a9b0c1d2...", "amount": 10000000000}'
```

```json
{
  "tx_hash": "abc123...",
  "amount": 10000000000,
  "amount_udag": 100.0,
  "message": "Sent 100 UDAG to e7f8a9b0c1d2..."
}
```

<div class="callout callout-note"><div class="callout-title">Faucet limits</div>Maximum 100 UDAG per request. Testnet only (disabled in mainnet mode).</div>

### GET /mempool

List pending transactions (top 100 by fee).

```bash
curl http://localhost:10333/mempool
```

### GET /peers

List connected peers and bootstrap node status.

```bash
curl http://localhost:10333/peers
```

---

## Transaction Endpoints

### POST /tx

Submit a transaction with a private key (testnet convenience).

```bash
curl -X POST http://localhost:10333/tx \
  -H "Content-Type: application/json" \
  -d '{
    "secret_key": "9f8e7d6c5b4a...",
    "to": "1a2b3c4d5e6f...",
    "amount": 50000000000,
    "fee": 10000
  }'
```

```json
{
  "tx_hash": "def456...",
  "status": "accepted"
}
```

<div class="callout callout-warning"><div class="callout-title">Testnet only</div>The <code>/tx</code> endpoint accepts private keys in the request body. This is disabled in mainnet mode. Use <code>/tx/submit</code> with pre-signed transactions for mainnet.</div>

### POST /tx/submit

Submit a pre-signed transaction. This is the **only transaction path on mainnet**.

```bash
curl -X POST http://localhost:10333/tx/submit \
  -H "Content-Type: application/json" \
  -d '{
    "type": "Transfer",
    "from": "a1b2c3...",
    "to": "d4e5f6...",
    "amount": 50000000000,
    "fee": 10000,
    "nonce": 7,
    "pub_key": "3a4b5c...",
    "signature": "e7f8a9..."
  }'
```

See [Transaction Format](/docs/api/transactions) for the signing specification.

### GET /tx/:hash

Look up a transaction by hash. Returns status: `pending`, `finalized`, or 404.

```bash
curl http://localhost:10333/tx/abc123def456...
```

```json
{
  "hash": "abc123def456...",
  "status": "finalized",
  "round": 1540,
  "vertex_hash": "789abc...",
  "validator": "a1b2c3..."
}
```

---

## Staking Endpoints

### POST /stake

Stake UDAG (testnet convenience).

```bash
curl -X POST http://localhost:10333/stake \
  -H "Content-Type: application/json" \
  -d '{
    "secret_key": "9f8e7d6c...",
    "amount": 200000000000
  }'
```

`amount` is in sats. 200,000,000,000 sats = 2,000 UDAG, the protocol minimum (`MIN_STAKE_SATS`). Any value at or above this is accepted.

### POST /unstake

Begin the unstaking process (testnet convenience).

```bash
curl -X POST http://localhost:10333/unstake \
  -H "Content-Type: application/json" \
  -d '{
    "secret_key": "9f8e7d6c..."
  }'
```

### GET /stake/:address

Get staking details for an address.

```bash
curl http://localhost:10333/stake/a1b2c3d4...
```

```json
{
  "address": "a1b2c3d4...",
  "staked_amount": 200000000000,
  "staked_udag": 2000.0,
  "commission_percent": 10,
  "effective_stake": 500000000000,
  "delegator_count": 3,
  "is_active_validator": true
}
```

### GET /validators

List all active validators.

```bash
curl http://localhost:10333/validators
```

```json
{
  "validators": [
    {
      "address": "a1b2c3d4...",
      "effective_stake": 1500000000000,
      "commission_percent": 10,
      "delegator_count": 3,
      "is_active": true
    }
  ],
  "total_staked": 5000000000000,
  "active_count": 5,
  "max_validators": 100
}
```

---

## Delegation Endpoints

### POST /delegate

Delegate UDAG to a validator (testnet convenience).

```bash
curl -X POST http://localhost:10333/delegate \
  -H "Content-Type: application/json" \
  -d '{
    "secret_key": "9f8e7d6c...",
    "validator": "a1b2c3d4...",
    "amount": 10000000000
  }'
```

### POST /undelegate

Begin undelegation (testnet convenience).

```bash
curl -X POST http://localhost:10333/undelegate \
  -H "Content-Type: application/json" \
  -d '{
    "secret_key": "9f8e7d6c..."
  }'
```

### POST /set-commission

Set validator commission rate (testnet convenience).

```bash
curl -X POST http://localhost:10333/set-commission \
  -H "Content-Type: application/json" \
  -d '{
    "secret_key": "9f8e7d6c...",
    "commission_percent": 15
  }'
```

### GET /delegation/:address

Get delegation info for a delegator address.

```bash
curl http://localhost:10333/delegation/e7f8a9b0...
```

```json
{
  "delegator": "e7f8a9b0...",
  "validator": "a1b2c3d4...",
  "amount": 10000000000,
  "amount_udag": 100.0,
  "undelegating": false,
  "unlock_at_round": null
}
```

### GET /validator/:address/delegators

List delegators to a validator (max 500 entries).

```bash
curl http://localhost:10333/validator/a1b2c3d4.../delegators
```

---

## Governance Endpoints

### POST /proposal

Create a governance proposal (testnet convenience).

```bash
curl -X POST http://localhost:10333/proposal \
  -H "Content-Type: application/json" \
  -d '{
    "secret_key": "9f8e7d6c...",
    "title": "Reduce minimum fee",
    "description": "Lower min_fee_sats from 10000 to 5000...",
    "proposal_type": { "ParameterChange": { "param": "min_fee_sats", "value": 5000 } },
    "fee": 10000
  }'
```

### POST /vote

Vote on a proposal (testnet convenience).

```bash
curl -X POST http://localhost:10333/vote \
  -H "Content-Type: application/json" \
  -d '{
    "secret_key": "9f8e7d6c...",
    "proposal_id": 1,
    "vote": true,
    "fee": 10000
  }'
```

### GET /proposals

List all governance proposals (max 200, newest first).

```bash
curl http://localhost:10333/proposals
```

### GET /proposal/:id

Get proposal details including voter breakdown.

```bash
curl http://localhost:10333/proposal/1
```

```json
{
  "id": 1,
  "title": "Reduce minimum fee",
  "description": "...",
  "proposal_type": "ParameterChange",
  "status": "Active",
  "votes_for": 14,
  "votes_against": 2,
  "snapshot_total_stake": 21,
  "voting_ends_at_round": 125000,
  "voters": [
    { "address": "a1b2c3...", "vote": "yes", "weight": 1 },
    { "address": "d4e5f6...", "vote": "no", "weight": 1 }
  ]
}
```

### GET /vote/:id/:address

Check a specific address's vote on a proposal.

### GET /governance/config

Get current governance parameters.

```bash
curl http://localhost:10333/governance/config
```

---

## DAG Endpoints

### GET /round/:round

Get all vertices in a specific round.

```bash
curl http://localhost:10333/round/1540
```

```json
{
  "round": 1540,
  "vertices": [
    {
      "hash": "abc123...",
      "validator": "a1b2c3...",
      "reward": 20000000,
      "tx_count": 2,
      "parent_count": 5
    }
  ]
}
```

### GET /vertex/:hash

Get a specific vertex by hash.

```bash
curl http://localhost:10333/vertex/abc123def456...
```

```json
{
  "hash": "abc123def456...",
  "round": 1540,
  "validator": "a1b2c3...",
  "parent_count": 5,
  "coinbase_reward": 20000000,
  "transactions": [
    { "type": "Transfer", "hash": "def789...", "fee": 10000 }
  ]
}
```

---

## Monitoring Endpoints

### GET /metrics

Prometheus-compatible metrics export.

```bash
curl http://localhost:10333/metrics
```

```
# HELP ultradag_finality_lag Rounds between DAG tip and last finalized
# TYPE ultradag_finality_lag gauge
ultradag_finality_lag 2
# HELP ultradag_peer_count Connected peers
# TYPE ultradag_peer_count gauge
ultradag_peer_count 4
...
```

### GET /metrics/json

JSON-formatted metrics for custom dashboards.

```bash
curl http://localhost:10333/metrics/json
```

---

## Rate Limits

| Endpoint | Limit | Window |
|----------|-------|--------|
| `/tx` | 100 | per minute |
| `/faucet` | 1 | per 10 minutes |
| `/stake` | 5 | per minute |
| `/unstake` | 5 | per minute |
| `/delegate` | 5 | per minute |
| `/undelegate` | 5 | per minute |
| `/set-commission` | 5 | per minute |
| `/proposal` | 5 | per minute |
| `/vote` | 10 | per minute |
| `/keygen` | 10 | per minute |
| Global | 1000 | per minute |

Rate limits are applied per client IP. Behind a reverse proxy (e.g., Fly.io), the real client IP is extracted from `Fly-Client-IP` or `X-Forwarded-For` headers (trusted proxies only).

---

## Testnet-Gated Endpoints

The following 10 endpoints accept private keys in the request body and are **disabled in mainnet mode** (`--testnet false`). They return HTTP 410 GONE with a message directing users to `/tx/submit`:

| Endpoint | Mainnet Alternative |
|----------|-------------------|
| `/tx` | `/tx/submit` (pre-signed) |
| `/stake` | `/tx/submit` (pre-signed StakeTx) |
| `/unstake` | `/tx/submit` (pre-signed UnstakeTx) |
| `/delegate` | `/tx/submit` (pre-signed DelegateTx) |
| `/undelegate` | `/tx/submit` (pre-signed UndelegateTx) |
| `/set-commission` | `/tx/submit` (pre-signed SetCommissionTx) |
| `/proposal` | `/tx/submit` (pre-signed CreateProposalTx) |
| `/vote` | `/tx/submit` (pre-signed VoteTx) |
| `/faucet` | N/A (no faucet on mainnet) |
| `/keygen` | Client-side via [SDK](/docs/api/sdks) |

---

## Security Headers

All RPC responses include:

- `X-Content-Type-Options: nosniff`
- `Cache-Control: no-store`
- `Access-Control-Max-Age: 3600` (on OPTIONS preflight responses only)

CORS headers (`Access-Control-Allow-Origin: *`, `Access-Control-Allow-Methods`, `Access-Control-Allow-Headers`) are included on all responses for browser-based wallet access.

---

## Next Steps

- [Transaction Format](/docs/api/transactions) -- signing specification for `/tx/submit`
- [SDKs](/docs/api/sdks) -- client libraries for all transaction types
- [Monitoring](/docs/operations/monitoring) -- using `/metrics` with Prometheus/Grafana
