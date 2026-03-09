# UltraDAG RPC API Reference

**Version:** 1.0  
**Last Updated:** March 2026

---

## Table of Contents

1. [Overview](#overview)
2. [Base URL](#base-url)
3. [Authentication](#authentication)
4. [Rate Limiting](#rate-limiting)
5. [Error Handling](#error-handling)
6. [Health & Status](#health--status)
7. [Account & Balance](#account--balance)
8. [Transactions](#transactions)
9. [Staking](#staking)
10. [Governance](#governance)
11. [Network & Peers](#network--peers)
12. [Metrics & Monitoring](#metrics--monitoring)
13. [Utilities](#utilities)

---

## Overview

The UltraDAG RPC API provides HTTP/JSON endpoints for interacting with UltraDAG nodes. All endpoints return JSON responses unless otherwise specified.

**Protocol:** HTTP/1.1  
**Content-Type:** `application/json`  
**Default Port:** 10333

---

## Base URL

```
http://localhost:10333
```

For production deployments, replace `localhost` with your node's IP or domain.

---

## Authentication

Currently, the RPC API does not require authentication. For production deployments, use:
- Firewall rules to restrict access
- Reverse proxy with authentication (nginx, Caddy)
- VPN or private network access

---

## Rate Limiting

The API implements per-IP rate limiting to prevent abuse:

| Endpoint | Limit | Window |
|----------|-------|--------|
| `/tx` | 100 requests | 1 minute |
| `/faucet` | 1 request | 1 minute |
| `/stake` | 10 requests | 1 minute |
| `/unstake` | 10 requests | 1 minute |
| `/proposal` | 5 requests | 1 minute |
| `/vote` | 20 requests | 1 minute |
| All others | 100 requests | 1 minute |

**Rate Limit Response:**
```json
{
  "error": "rate limit exceeded: too many requests"
}
```
HTTP Status: `429 Too Many Requests`

---

## Error Handling

### Standard Error Response

```json
{
  "error": "error message description"
}
```

### HTTP Status Codes

| Code | Meaning | Common Causes |
|------|---------|---------------|
| 200 | OK | Request successful |
| 400 | Bad Request | Invalid parameters, malformed JSON |
| 429 | Too Many Requests | Rate limit exceeded |
| 503 | Service Unavailable | Node busy, locks contended |

---

## Health & Status

### GET /health

Simple health check for load balancers and monitoring.

**Response:**
```json
{
  "status": "ok"
}
```

**Characteristics:**
- Lock-free (never blocks)
- Always responds immediately
- Use for: Load balancer health probes, uptime monitoring

**Example:**
```bash
curl http://localhost:10333/health
```

---

### GET /health/detailed

Comprehensive health diagnostics with component-level status.

**Response:**
```json
{
  "status": "healthy",
  "warnings": [],
  "timestamp": 1709943000,
  "components": {
    "dag": {
      "available": true,
      "current_round": 4523,
      "vertex_count": 4523,
      "tips_count": 1,
      "pruning_floor": 3523
    },
    "finality": {
      "available": true,
      "last_finalized_round": 4521,
      "finality_lag": 2,
      "validator_count": 4
    },
    "state": {
      "available": true,
      "total_supply": 21226250000000000,
      "account_count": 42,
      "total_staked": 400000000000,
      "active_validators": 4,
      "next_proposal_id": 3
    },
    "mempool": {
      "available": true,
      "transaction_count": 5
    },
    "network": {
      "peer_count": 3,
      "sync_complete": true
    },
    "checkpoints": {
      "last_checkpoint_round": 4500,
      "checkpoint_age_seconds": 115,
      "pending_checkpoints": 1,
      "disk_count": 10
    }
  }
}
```

**Status Levels:**
- `healthy` - All components available, finality lag ≤10, peers >0
- `warning` - Finality lag >10 or no peers
- `unhealthy` - Finality lag >100
- `degraded` - Component locks contended (high load)

**Characteristics:**
- Non-blocking (uses `try_read()`)
- Returns partial diagnostics if locks unavailable
- Suitable for frequent polling (every 5-10 seconds)

**Example:**
```bash
curl http://localhost:10333/health/detailed | jq .
```

---

### GET /status

Full node status with caching for dashboard display.

**Response:**
```json
{
  "dag": {
    "current_round": 4523,
    "finalized_round": 4521,
    "finality_lag": 2,
    "vertex_count": 4523,
    "tips_count": 1
  },
  "state": {
    "total_supply": 21226250000000000,
    "account_count": 42,
    "last_finalized_round": 4521
  },
  "network": {
    "peer_count": 3,
    "sync_complete": true
  },
  "mempool": {
    "transaction_count": 5
  }
}
```

**Characteristics:**
- 500ms timeout with cached fallback
- Cached response if locks contended
- Use for: Dashboard display, status pages

**Example:**
```bash
curl http://localhost:10333/status | jq .
```

---

## Account & Balance

### GET /balance/{address}

Get account balance and nonce.

**Parameters:**
- `address` (path) - 64-character hex address

**Response:**
```json
{
  "address": "a1b2c3...",
  "balance": 1000000000000,
  "nonce": 5
}
```

**Fields:**
- `balance` - Balance in satoshis (1 UDAG = 10^8 sats)
- `nonce` - Current transaction nonce (next tx must use this value)

**Example:**
```bash
curl http://localhost:10333/balance/a1b2c3d4e5f6...
```

**Error Cases:**
- Invalid address format → 400 Bad Request
- Address not found → Returns balance=0, nonce=0

---

### GET /round/{round}

Get all vertices in a specific round.

**Parameters:**
- `round` (path) - Round number (u64)

**Response:**
```json
[
  {
    "hash": "abc123...",
    "round": 100,
    "validator": "def456...",
    "parent_count": 4,
    "transaction_count": 3
  }
]
```

**Example:**
```bash
curl http://localhost:10333/round/100 | jq .
```

---

## Transactions

### POST /tx

Submit a signed transaction to the mempool.

**Request Body:**
```json
{
  "from": "a1b2c3d4e5f6...",
  "to": "f6e5d4c3b2a1...",
  "amount": 1000000000,
  "fee": 1000000,
  "nonce": 5,
  "pub_key": "0123456789abcdef...",
  "signature": "fedcba9876543210..."
}
```

**Fields:**
- `from` - Sender address (64 hex chars)
- `to` - Recipient address (64 hex chars)
- `amount` - Transfer amount in satoshis
- `fee` - Transaction fee in satoshis
- `nonce` - Must equal sender's current nonce
- `pub_key` - Ed25519 public key (64 hex chars)
- `signature` - Ed25519 signature (128 hex chars)

**Response (Success):**
```json
{
  "status": "accepted",
  "hash": "tx_hash_here..."
}
```

**Response (Failure):**
```json
{
  "error": "insufficient balance"
}
```

**Validation Rules:**
1. `Blake3(pub_key) == from`
2. Valid Ed25519 signature
3. `balance(from) >= amount + fee`
4. `nonce == current_nonce(from)`

**Example:**
```bash
curl -X POST http://localhost:10333/tx \
  -H "Content-Type: application/json" \
  -d '{
    "from": "a1b2...",
    "to": "f6e5...",
    "amount": 1000000000,
    "fee": 1000000,
    "nonce": 5,
    "pub_key": "0123...",
    "signature": "fedc..."
  }'
```

---

### GET /mempool

Get pending transactions in the mempool.

**Response:**
```json
[
  {
    "hash": "tx_hash_1",
    "from": "a1b2c3...",
    "to": "f6e5d4...",
    "amount": 1000000000,
    "fee": 1000000,
    "nonce": 5
  }
]
```

**Notes:**
- Returns up to 100 highest-fee transactions
- Sorted by fee (descending)

**Example:**
```bash
curl http://localhost:10333/mempool | jq .
```

---

### POST /faucet

Request testnet tokens from the faucet.

**Request Body:**
```json
{
  "address": "a1b2c3d4e5f6..."
}
```

**Response (Success):**
```json
{
  "status": "sent",
  "amount": 100000000000,
  "tx_hash": "abc123..."
}
```

**Response (Failure):**
```json
{
  "error": "faucet empty"
}
```

**Rate Limit:** 1 request per minute per IP

**Faucet Amount:** 1 UDAG (100,000,000 satoshis)

**Example:**
```bash
curl -X POST http://localhost:10333/faucet \
  -H "Content-Type: application/json" \
  -d '{"address": "a1b2c3d4e5f6..."}'
```

---

## Staking

### POST /stake

Stake tokens to become a validator.

**Request Body:**
```json
{
  "staker": "a1b2c3d4e5f6...",
  "amount": 10000000000,
  "nonce": 5,
  "pub_key": "0123456789abcdef...",
  "signature": "fedcba9876543210..."
}
```

**Fields:**
- `staker` - Address staking tokens
- `amount` - Stake amount in satoshis (must be ≥ MIN_STAKE_SATS)
- `nonce` - Current nonce of staker
- `pub_key` - Ed25519 public key
- `signature` - Ed25519 signature

**Minimum Stake:** 0.1 UDAG (10,000,000 satoshis)

**Response (Success):**
```json
{
  "status": "staked",
  "amount": 10000000000,
  "total_stake": 10000000000
}
```

**Example:**
```bash
curl -X POST http://localhost:10333/stake \
  -H "Content-Type: application/json" \
  -d '{
    "staker": "a1b2...",
    "amount": 10000000000,
    "nonce": 5,
    "pub_key": "0123...",
    "signature": "fedc..."
  }'
```

---

### POST /unstake

Unstake tokens (subject to cooldown period).

**Request Body:**
```json
{
  "staker": "a1b2c3d4e5f6...",
  "amount": 5000000000,
  "nonce": 6,
  "pub_key": "0123456789abcdef...",
  "signature": "fedcba9876543210..."
}
```

**Cooldown Period:** 2,016 rounds (~2.8 hours at 5s rounds)

**Response (Success):**
```json
{
  "status": "unstaked",
  "amount": 5000000000,
  "remaining_stake": 5000000000,
  "cooldown_ends_round": 6539
}
```

**Example:**
```bash
curl -X POST http://localhost:10333/unstake \
  -H "Content-Type: application/json" \
  -d '{
    "staker": "a1b2...",
    "amount": 5000000000,
    "nonce": 6,
    "pub_key": "0123...",
    "signature": "fedc..."
  }'
```

---

### GET /stake/{address}

Get staking information for an address.

**Response:**
```json
{
  "address": "a1b2c3d4e5f6...",
  "staked_amount": 10000000000,
  "is_active_validator": true,
  "unstaking": []
}
```

**Example:**
```bash
curl http://localhost:10333/stake/a1b2c3d4e5f6... | jq .
```

---

### GET /validators

Get list of active validators.

**Response:**
```json
[
  {
    "address": "a1b2c3d4e5f6...",
    "stake": 10000000000,
    "is_active": true
  }
]
```

**Example:**
```bash
curl http://localhost:10333/validators | jq .
```

---

## Governance

### POST /proposal

Create a governance proposal.

**Request Body:**
```json
{
  "proposer": "a1b2c3d4e5f6...",
  "proposal_type": {
    "Text": {
      "title": "Proposal Title",
      "description": "Detailed description"
    }
  },
  "nonce": 7,
  "pub_key": "0123456789abcdef...",
  "signature": "fedcba9876543210..."
}
```

**Proposal Types:**

**Text Proposal:**
```json
{
  "Text": {
    "title": "string",
    "description": "string"
  }
}
```

**Parameter Change:**
```json
{
  "ParameterChange": {
    "title": "string",
    "description": "string",
    "parameter": "MIN_FEE_SATS",
    "new_value": "500000"
  }
}
```

**Validator Set Change:**
```json
{
  "ValidatorSet": {
    "title": "string",
    "description": "string",
    "add": ["addr1", "addr2"],
    "remove": ["addr3"]
  }
}
```

**Response (Success):**
```json
{
  "status": "created",
  "proposal_id": 1
}
```

**Voting Period:** 2,016 rounds (~2.8 hours)

**Example:**
```bash
curl -X POST http://localhost:10333/proposal \
  -H "Content-Type: application/json" \
  -d '{
    "proposer": "a1b2...",
    "proposal_type": {
      "Text": {
        "title": "Increase block size",
        "description": "Proposal to increase max block size"
      }
    },
    "nonce": 7,
    "pub_key": "0123...",
    "signature": "fedc..."
  }'
```

---

### POST /vote

Vote on a governance proposal.

**Request Body:**
```json
{
  "voter": "a1b2c3d4e5f6...",
  "proposal_id": 1,
  "vote": true,
  "nonce": 8,
  "pub_key": "0123456789abcdef...",
  "signature": "fedcba9876543210..."
}
```

**Fields:**
- `vote` - `true` for yes, `false` for no

**Response (Success):**
```json
{
  "status": "voted",
  "proposal_id": 1,
  "vote": true
}
```

**Voting Rules:**
- Only active validators can vote
- Vote weight = validator's stake amount
- Votes are immutable once cast

**Example:**
```bash
curl -X POST http://localhost:10333/vote \
  -H "Content-Type: application/json" \
  -d '{
    "voter": "a1b2...",
    "proposal_id": 1,
    "vote": true,
    "nonce": 8,
    "pub_key": "0123...",
    "signature": "fedc..."
  }'
```

---

### GET /proposals

Get all governance proposals.

**Response:**
```json
[
  {
    "id": 1,
    "proposer": "a1b2c3d4e5f6...",
    "proposal_type": "Text",
    "title": "Proposal Title",
    "status": "Active",
    "yes_votes": 15000000000,
    "no_votes": 5000000000,
    "voting_starts": 4000,
    "voting_ends": 6016
  }
]
```

**Proposal Status:**
- `Pending` - Not yet active
- `Active` - Currently accepting votes
- `Passed` - Approved by majority
- `Rejected` - Failed to reach majority
- `Executed` - Passed and executed

**Example:**
```bash
curl http://localhost:10333/proposals | jq .
```

---

### GET /proposal/{id}

Get details of a specific proposal.

**Response:**
```json
{
  "id": 1,
  "proposer": "a1b2c3d4e5f6...",
  "proposal_type": {
    "Text": {
      "title": "Proposal Title",
      "description": "Detailed description"
    }
  },
  "status": "Active",
  "yes_votes": 15000000000,
  "no_votes": 5000000000,
  "voting_starts": 4000,
  "voting_ends": 6016
}
```

**Example:**
```bash
curl http://localhost:10333/proposal/1 | jq .
```

---

### GET /vote/{proposal_id}/{address}

Get a specific vote on a proposal.

**Response:**
```json
{
  "voter": "a1b2c3d4e5f6...",
  "proposal_id": 1,
  "vote": true,
  "weight": 10000000000
}
```

**Example:**
```bash
curl http://localhost:10333/vote/1/a1b2c3d4e5f6... | jq .
```

---

### GET /governance/config

Get governance configuration parameters.

**Response:**
```json
{
  "voting_period_rounds": 2016,
  "min_stake_to_propose": 10000000000,
  "quorum_threshold": "50%"
}
```

**Example:**
```bash
curl http://localhost:10333/governance/config | jq .
```

---

## Network & Peers

### GET /peers

Get connected peer information.

**Response:**
```json
{
  "connected_count": 3,
  "peer_addrs": [
    "192.168.1.100:9333",
    "192.168.1.101:9333",
    "192.168.1.102:9333"
  ],
  "listen_addrs": [
    "node1.example.com:9333",
    "node2.example.com:9333",
    "node3.example.com:9333"
  ]
}
```

**Example:**
```bash
curl http://localhost:10333/peers | jq .
```

---

## Metrics & Monitoring

### GET /metrics

Get Prometheus-formatted metrics.

**Response Format:** Prometheus text format

**Example Response:**
```
# HELP checkpoint_produced_total Total checkpoints produced
# TYPE checkpoint_produced_total counter
checkpoint_produced_total 42

# HELP checkpoint_production_duration_ms Checkpoint production duration
# TYPE checkpoint_production_duration_ms gauge
checkpoint_production_duration_ms 145

# HELP checkpoints_cosigned_total Total checkpoints co-signed
# TYPE checkpoints_cosigned_total counter
checkpoints_cosigned_total 156

# HELP fast_sync_success_total Successful fast-sync operations
# TYPE fast_sync_success_total counter
fast_sync_success_total 5
```

**Content-Type:** `text/plain; version=0.0.4`

**Example:**
```bash
curl http://localhost:10333/metrics
```

---

### GET /metrics/json

Get metrics in JSON format for custom dashboards.

**Response:**
```json
{
  "production": {
    "checkpoints_produced_total": 42,
    "checkpoint_production_duration_ms": 145,
    "checkpoint_size_bytes": 2048576,
    "checkpoint_production_errors": 0
  },
  "cosigning": {
    "checkpoints_cosigned_total": 156,
    "checkpoint_signatures_collected": 624,
    "checkpoint_quorum_reached_total": 38,
    "checkpoint_validation_failures": 3
  },
  "fast_sync": {
    "fast_sync_attempts_total": 5,
    "fast_sync_success_total": 5,
    "fast_sync_failures_total": 0,
    "fast_sync_duration_ms": 4523,
    "fast_sync_bytes_downloaded_total": 31457280
  },
  "storage": {
    "checkpoint_persist_success": 42,
    "checkpoint_persist_failures": 0,
    "checkpoint_load_success": 5,
    "checkpoint_load_failures": 0
  },
  "health": {
    "last_checkpoint_round": 4200,
    "last_checkpoint_age_seconds": 12,
    "pending_checkpoints": 2
  },
  "pruning": {
    "checkpoints_pruned_total": 32,
    "checkpoint_disk_count": 10
  }
}
```

**Example:**
```bash
curl http://localhost:10333/metrics/json | jq .
```

---

## Utilities

### GET /keygen

Generate a new Ed25519 keypair.

**Response:**
```json
{
  "secret_key": "0123456789abcdef...",
  "public_key": "fedcba9876543210...",
  "address": "a1b2c3d4e5f6..."
}
```

**Security Warning:** Only use this endpoint for testing. In production, generate keys offline and never expose secret keys over the network.

**Example:**
```bash
curl http://localhost:10333/keygen | jq .
```

---

## Code Examples

### JavaScript/Node.js

```javascript
const axios = require('axios');

const BASE_URL = 'http://localhost:10333';

// Get balance
async function getBalance(address) {
  const response = await axios.get(`${BASE_URL}/balance/${address}`);
  return response.data;
}

// Submit transaction
async function submitTransaction(tx) {
  const response = await axios.post(`${BASE_URL}/tx`, tx);
  return response.data;
}

// Get health status
async function getHealth() {
  const response = await axios.get(`${BASE_URL}/health/detailed`);
  return response.data;
}
```

### Python

```python
import requests

BASE_URL = 'http://localhost:10333'

# Get balance
def get_balance(address):
    response = requests.get(f'{BASE_URL}/balance/{address}')
    return response.json()

# Submit transaction
def submit_transaction(tx):
    response = requests.post(f'{BASE_URL}/tx', json=tx)
    return response.json()

# Get health status
def get_health():
    response = requests.get(f'{BASE_URL}/health/detailed')
    return response.json()
```

### cURL

```bash
# Get balance
curl http://localhost:10333/balance/a1b2c3d4e5f6...

# Submit transaction
curl -X POST http://localhost:10333/tx \
  -H "Content-Type: application/json" \
  -d @transaction.json

# Get health
curl http://localhost:10333/health/detailed | jq .

# Get metrics
curl http://localhost:10333/metrics

# Get validators
curl http://localhost:10333/validators | jq .
```

---

## Appendix

### Transaction Signing

Transactions must be signed with Ed25519. The signable bytes are:

```
NETWORK_ID || from || to || amount_LE64 || fee_LE64 || nonce_LE64
```

Where:
- `NETWORK_ID` = `b"ultradag-testnet-v1"` (19 bytes)
- `||` = concatenation
- `_LE64` = 64-bit little-endian encoding

### Address Derivation

Addresses are derived from Ed25519 public keys:

```
address = Blake3(public_key)
```

Result is a 32-byte (64 hex character) address.

### Nonce Management

Each account has a nonce that increments with each transaction:
- First transaction: nonce = 0
- Second transaction: nonce = 1
- Etc.

Always fetch current nonce via `/balance/{address}` before signing.

---

**For more information, see:**
- [Whitepaper](../specifications/whitepaper.md)
- [Node Operator Guide](../../guides/operations/node-operator-guide.md)
- [Integration Guide](../../guides/development/integration-guide.md)
