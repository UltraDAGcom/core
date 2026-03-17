---
title: Transaction Format
---

# Transaction Format

UltraDAG supports 8 transaction types, all signed with Ed25519. This page specifies the transaction structure, signing process, and validation rules required to construct valid transactions for the `/tx/submit` endpoint.

---

## Transaction Types

| Type | Discriminator | Description | Fee Required |
|------|--------------|-------------|-------------|
| `Transfer` | `b"transfer"` | Send UDAG between addresses | Yes |
| `Stake` | `b"stake"` | Lock UDAG as validator stake | No |
| `Unstake` | `b"unstake"` | Begin unstaking cooldown | No |
| `Delegate` | `b"delegate"` | Delegate UDAG to a validator | No |
| `Undelegate` | `b"undelegate"` | Begin undelegation cooldown | No |
| `SetCommission` | `b"set_commission"` | Set validator commission rate | No |
| `CreateProposal` | `b"proposal"` | Create a governance proposal | Yes |
| `Vote` | `b"vote"` | Vote on a governance proposal | Yes |

---

## Common Fields

Every transaction includes:

| Field | Type | Description |
|-------|------|-------------|
| `from` | `Address` (32 bytes hex) | Sender address |
| `nonce` | `u64` | Sequential transaction counter |
| `pub_key` | `PublicKey` (32 bytes hex) | Ed25519 public key of the sender |
| `signature` | `Signature` (64 bytes hex) | Ed25519 signature over `signable_bytes()` |

---

## Signing Process

### Step 1: Construct signable_bytes

The signable bytes are constructed by concatenating:

```
NETWORK_ID || type_discriminator || field_bytes
```

Where:

- **NETWORK_ID**: `b"ultradag-testnet-v1"` (testnet) or `b"ultradag-mainnet-v1"` (mainnet)
- **type_discriminator**: a unique byte string per transaction type (see table above)
- **field_bytes**: type-specific fields serialized in a defined order

!!! warning "Domain separation"
    The `NETWORK_ID` prefix ensures that a transaction signed for testnet cannot be replayed on mainnet (and vice versa). The type discriminator prevents cross-type signature reuse.

### Step 2: Sign with Ed25519

```
signature = ed25519_sign(secret_key, signable_bytes)
```

UltraDAG uses `ed25519-dalek` with `verify_strict` — signatures must be canonical.

### Step 3: Submit

POST the complete transaction (including signature) to `/tx/submit`.

---

## Address Derivation

Addresses are derived from Ed25519 public keys using Blake3:

```
address = blake3(ed25519_public_key)  // 32 bytes
address_hex = hex_encode(address)      // 64 characters
```

This is a one-way derivation. Given an address, you cannot recover the public key (the public key must be included in each transaction).

---

## Type-Specific Formats

### Transfer

Send UDAG from one address to another.

**Fields:**

| Field | Type | Description |
|-------|------|-------------|
| `to` | `Address` | Recipient address |
| `amount` | `u64` | Amount in sats |
| `fee` | `u64` | Transaction fee in sats (>= 10,000) |
| `memo` | `Option<Vec<u8>>` | Optional memo (max 256 bytes) |

**signable_bytes:**

```
NETWORK_ID || b"transfer" || from(32) || to(32) || amount(8 LE) || fee(8 LE) || nonce(8 LE) || [memo_len(4 LE) || memo_bytes]?
```

**Validation rules:**

- `amount > 0`
- `fee >= MIN_FEE_SATS` (10,000 sats)
- `balance >= amount + fee`
- `nonce == account.nonce`
- `memo.len() <= 256` bytes (if present)

### Stake

Lock UDAG as validator stake.

**Fields:**

| Field | Type | Description |
|-------|------|-------------|
| `amount` | `u64` | Amount to stake in sats |

**signable_bytes:**

```
NETWORK_ID || b"stake" || from(32) || amount(8 LE) || nonce(8 LE)
```

**Validation rules:**

- `amount >= MIN_STAKE_SATS` (10,000 UDAG = 1,000,000,000,000 sats)
- `balance >= amount`
- Fee is zero (fee-exempt)

### Unstake

Begin unstaking cooldown. All staked amount enters cooldown.

**Fields:** (no additional fields)

**signable_bytes:**

```
NETWORK_ID || b"unstake" || from(32) || nonce(8 LE)
```

**Validation rules:**

- Sender must have an active stake
- Sender must not already be unstaking

### Delegate

Delegate UDAG to a validator.

**Fields:**

| Field | Type | Description |
|-------|------|-------------|
| `validator` | `Address` | Target validator address |
| `amount` | `u64` | Amount to delegate in sats |

**signable_bytes:**

```
NETWORK_ID || b"delegate" || from(32) || validator(32) || amount(8 LE) || nonce(8 LE)
```

**Validation rules:**

- `amount >= MIN_DELEGATION_SATS` (100 UDAG = 10,000,000,000 sats)
- `from != validator` (no self-delegation)
- `balance >= amount`
- Target must be a staked validator

### Undelegate

Begin undelegation cooldown.

**Fields:** (no additional fields)

**signable_bytes:**

```
NETWORK_ID || b"undelegate" || from(32) || nonce(8 LE)
```

**Validation rules:**

- Sender must have an active delegation
- Sender must not already be undelegating

### SetCommission

Set the validator's commission rate for delegated rewards.

**Fields:**

| Field | Type | Description |
|-------|------|-------------|
| `commission_percent` | `u8` | Commission rate (0-100) |

**signable_bytes:**

```
NETWORK_ID || b"set_commission" || from(32) || commission_percent(1) || nonce(8 LE)
```

**Validation rules:**

- `commission_percent <= 100`
- Sender must have an active stake

### CreateProposal

Create a governance proposal.

**Fields:**

| Field | Type | Description |
|-------|------|-------------|
| `title` | `String` | Proposal title (max 128 bytes) |
| `description` | `String` | Proposal description (max 4,096 bytes) |
| `proposal_type` | `ProposalType` | Text, ParameterChange, or CouncilMembership |
| `fee` | `u64` | Transaction fee in sats |

**signable_bytes:**

```
NETWORK_ID || b"proposal" || from(32) || title_len(4 LE) || title_bytes || desc_len(4 LE) || desc_bytes || proposal_type_bytes || fee(8 LE) || nonce(8 LE)
```

!!! note "Length prefixing"
    Variable-length fields (title, description) are length-prefixed with 4-byte little-endian u32. This prevents hash collisions between `title="AB" desc="CD"` and `title="ABC" desc="D"`.

**Validation rules:**

- `fee >= MIN_FEE_SATS`
- `title.len() <= 128` bytes
- `description.len() <= 4096` bytes
- Active proposals < `MAX_ACTIVE_PROPOSALS` (20)

### Vote

Vote on a governance proposal.

**Fields:**

| Field | Type | Description |
|-------|------|-------------|
| `proposal_id` | `u64` | ID of the proposal to vote on |
| `approve` | `bool` | `true` for yes, `false` for no |
| `fee` | `u64` | Transaction fee in sats |

**signable_bytes:**

```
NETWORK_ID || b"vote" || from(32) || proposal_id(8 LE) || approve(1) || fee(8 LE) || nonce(8 LE)
```

**Validation rules:**

- `fee >= MIN_FEE_SATS`
- Proposal must exist and be in `Active` status
- Sender must not have already voted on this proposal

---

## Fee Structure

| Category | Fee |
|----------|-----|
| Transfers | `MIN_FEE_SATS` = 10,000 sats (0.0001 UDAG) |
| Governance (CreateProposal, Vote) | `MIN_FEE_SATS` = 10,000 sats |
| Staking operations | **Zero fee** (fee-exempt) |

Fee-exempt transaction types: `Stake`, `Unstake`, `Delegate`, `Undelegate`, `SetCommission`.

Fees collected from transactions are added to the round's reward distribution pool.

---

## Nonce Management

Each address has a strictly sequential nonce:

1. First transaction from an address uses nonce `0`
2. Each subsequent transaction increments nonce by 1
3. Transactions with incorrect nonces are rejected
4. Nonces prevent replay attacks (same transaction cannot be submitted twice)

Query the current nonce:

```bash
curl http://localhost:10333/balance/YOUR_ADDRESS
# Response includes "nonce": 7
```

If you have pending transactions in the mempool, use `max_pending_nonce + 1` for the next transaction.

---

## Replay Protection

Transactions are protected from replay through three mechanisms:

1. **NETWORK_ID**: testnet and mainnet signatures are cryptographically incompatible
2. **Nonce**: each transaction uses a unique sequential nonce per address
3. **Type discriminator**: prevents cross-type signature reuse

---

## Example: Complete Signing Flow

Using the JavaScript SDK as reference:

```javascript
import { Keypair, Transaction } from 'ultradag';

// Generate or load keypair
const keypair = Keypair.generate();

// Build the transaction
const tx = Transaction.transfer({
  from: keypair.address,
  to: 'recipient_address_hex',
  amount: 50_000_000_000n,  // 500 UDAG
  fee: 10_000n,
  nonce: 7n,
});

// Sign (constructs signable_bytes internally)
const signed = tx.sign(keypair.secretKey);

// Submit to node
const response = await fetch('http://localhost:10333/tx/submit', {
  method: 'POST',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify(signed),
});
```

---

## Next Steps

- [RPC Endpoints](rpc.md) — full API reference
- [SDKs](sdks.md) — client libraries with signing support
- [Security Model](../security/model.md) — cryptographic rationale
