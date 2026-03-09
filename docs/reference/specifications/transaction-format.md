# UltraDAG Transaction Format Specification

**Version:** 1.0  
**Last Updated:** March 2026  
**Status:** Production

---

## Table of Contents

1. [Overview](#overview)
2. [Transaction Types](#transaction-types)
3. [Transfer Transaction](#transfer-transaction)
4. [Stake Transaction](#stake-transaction)
5. [Unstake Transaction](#unstake-transaction)
6. [Governance Transactions](#governance-transactions)
7. [Signature Scheme](#signature-scheme)
8. [Validation Rules](#validation-rules)
9. [Serialization](#serialization)
10. [Examples](#examples)

---

## Overview

UltraDAG uses an account-based transaction model with Ed25519 signatures. All transactions are deterministically serialized and signed to prevent replay attacks and ensure authenticity.

**Key Properties:**
- **Cryptography:** Ed25519 signatures, Blake3 hashing
- **Replay Protection:** Network ID prefix, per-account nonce
- **Deterministic:** Canonical serialization for consistent hashing
- **Compact:** Minimal overhead, efficient encoding

---

## Transaction Types

UltraDAG supports five transaction types:

| Type | Purpose | Signable |
|------|---------|----------|
| `Transfer` | Send UDAG between addresses | Yes |
| `Stake` | Stake tokens to become validator | Yes |
| `Unstake` | Unstake tokens (with cooldown) | Yes |
| `CreateProposal` | Create governance proposal | Yes |
| `Vote` | Vote on governance proposal | Yes |

All transaction types share common fields and signing requirements.

---

## Transfer Transaction

### Structure

```rust
pub struct TransferTx {
    pub from: Address,        // Sender address (32 bytes)
    pub to: Address,          // Recipient address (32 bytes)
    pub amount: u64,          // Transfer amount in satoshis
    pub fee: u64,             // Transaction fee in satoshis
    pub nonce: u64,           // Sender's current nonce
    pub pub_key: PublicKey,   // Ed25519 public key (32 bytes)
    pub signature: Signature, // Ed25519 signature (64 bytes)
}
```

### Field Descriptions

**`from` (32 bytes)**
- Sender's address
- Derived from public key: `Blake3(pub_key)`
- Must match `Blake3(pub_key)` field

**`to` (32 bytes)**
- Recipient's address
- Any valid 32-byte address
- Can be same as `from` (self-transfer)

**`amount` (8 bytes, u64)**
- Transfer amount in satoshis
- 1 UDAG = 100,000,000 satoshis
- Must be > 0 for transfers

**`fee` (8 bytes, u64)**
- Transaction fee paid to block proposer
- Minimum: 1,000 satoshis (0.00001 UDAG)
- Recommended: 10,000 satoshis (0.0001 UDAG)

**`nonce` (8 bytes, u64)**
- Sender's current nonce
- Starts at 0 for new accounts
- Increments by 1 for each transaction
- Prevents replay attacks

**`pub_key` (32 bytes)**
- Ed25519 public key
- Used to verify signature
- Must satisfy: `Blake3(pub_key) == from`

**`signature` (64 bytes)**
- Ed25519 signature over signable bytes
- Proves transaction authorization
- Verified using `pub_key`

### Signable Bytes

```
signable_bytes = NETWORK_ID || from || to || amount_LE64 || fee_LE64 || nonce_LE64
```

**Components:**
- `NETWORK_ID` = `b"ultradag-testnet-v1"` (19 bytes)
- `||` = concatenation
- `_LE64` = 64-bit little-endian encoding

**Total Length:** 19 + 32 + 32 + 8 + 8 + 8 = 107 bytes

### JSON Representation

```json
{
  "from": "a1b2c3d4e5f6789012345678901234567890123456789012345678901234abcd",
  "to": "f6e5d4c3b2a1098765432109876543210987654321098765432109876543dcba",
  "amount": 100000000,
  "fee": 10000,
  "nonce": 5,
  "pub_key": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
  "signature": "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210"
}
```

### Validation Rules

1. **Address Derivation:** `Blake3(pub_key) == from`
2. **Signature Valid:** Ed25519 signature verification passes
3. **Sufficient Balance:** `balance(from) >= amount + fee`
4. **Correct Nonce:** `nonce == current_nonce(from)`
5. **Non-Zero Amount:** `amount > 0`
6. **Non-Zero Fee:** `fee >= MIN_FEE_SATS` (1,000)

---

## Stake Transaction

### Structure

```rust
pub struct StakeTx {
    pub staker: Address,      // Staker address (32 bytes)
    pub amount: u64,          // Stake amount in satoshis
    pub nonce: u64,           // Staker's current nonce
    pub pub_key: PublicKey,   // Ed25519 public key (32 bytes)
    pub signature: Signature, // Ed25519 signature (64 bytes)
}
```

### Field Descriptions

**`staker` (32 bytes)**
- Address staking tokens
- Becomes validator address
- Must match `Blake3(pub_key)`

**`amount` (8 bytes, u64)**
- Stake amount in satoshis
- Minimum: 10,000,000 satoshis (0.1 UDAG)
- Deducted from account balance

**`nonce` (8 bytes, u64)**
- Staker's current nonce
- Increments after transaction

**`pub_key` (32 bytes)**
- Ed25519 public key
- Used for validator vertex signing

**`signature` (64 bytes)**
- Ed25519 signature over signable bytes

### Signable Bytes

```
signable_bytes = NETWORK_ID || staker || amount_LE64 || nonce_LE64
```

**Total Length:** 19 + 32 + 8 + 8 = 67 bytes

### JSON Representation

```json
{
  "staker": "a1b2c3d4e5f6789012345678901234567890123456789012345678901234abcd",
  "amount": 100000000,
  "nonce": 5,
  "pub_key": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
  "signature": "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210"
}
```

### Validation Rules

1. **Address Derivation:** `Blake3(pub_key) == staker`
2. **Signature Valid:** Ed25519 signature verification passes
3. **Sufficient Balance:** `balance(staker) >= amount + fee`
4. **Correct Nonce:** `nonce == current_nonce(staker)`
5. **Minimum Stake:** `amount >= MIN_STAKE_SATS` (10,000,000)
6. **Not Already Staked:** First stake, or adding to existing stake

---

## Unstake Transaction

### Structure

```rust
pub struct UnstakeTx {
    pub staker: Address,      // Staker address (32 bytes)
    pub amount: u64,          // Unstake amount in satoshis
    pub nonce: u64,           // Staker's current nonce
    pub pub_key: PublicKey,   // Ed25519 public key (32 bytes)
    pub signature: Signature, // Ed25519 signature (64 bytes)
}
```

### Field Descriptions

**`staker` (32 bytes)**
- Address unstaking tokens
- Must be current validator

**`amount` (8 bytes, u64)**
- Unstake amount in satoshis
- Can be partial or full stake
- Subject to cooldown period

**`nonce` (8 bytes, u64)**
- Staker's current nonce

**`pub_key` (32 bytes)**
- Ed25519 public key

**`signature` (64 bytes)**
- Ed25519 signature over signable bytes

### Signable Bytes

```
signable_bytes = NETWORK_ID || staker || amount_LE64 || nonce_LE64
```

**Total Length:** 19 + 32 + 8 + 8 = 67 bytes

### JSON Representation

```json
{
  "staker": "a1b2c3d4e5f6789012345678901234567890123456789012345678901234abcd",
  "amount": 50000000,
  "nonce": 6,
  "pub_key": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
  "signature": "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210"
}
```

### Validation Rules

1. **Address Derivation:** `Blake3(pub_key) == staker`
2. **Signature Valid:** Ed25519 signature verification passes
3. **Correct Nonce:** `nonce == current_nonce(staker)`
4. **Sufficient Stake:** `staked_amount(staker) >= amount`
5. **Minimum Remaining:** If partial: `remaining_stake >= MIN_STAKE_SATS` OR `remaining_stake == 0`
6. **Cooldown Applied:** Tokens locked for 2,016 rounds

---

## Governance Transactions

### Create Proposal Transaction

**Structure:**
```rust
pub struct CreateProposalTx {
    pub proposer: Address,
    pub proposal_type: ProposalType,
    pub nonce: u64,
    pub pub_key: PublicKey,
    pub signature: Signature,
}
```

**Proposal Types:**

**1. Text Proposal:**
```rust
pub struct TextProposal {
    pub title: String,
    pub description: String,
}
```

**2. Parameter Change:**
```rust
pub struct ParameterProposal {
    pub title: String,
    pub description: String,
    pub parameter: String,
    pub new_value: String,
}
```

**3. Validator Set Change:**
```rust
pub struct ValidatorSetProposal {
    pub title: String,
    pub description: String,
    pub add: Vec<Address>,
    pub remove: Vec<Address>,
}
```

**Signable Bytes:**
```
signable_bytes = NETWORK_ID || proposer || proposal_hash || nonce_LE64
```

Where `proposal_hash = Blake3(proposal_type_serialized)`

**JSON Example:**
```json
{
  "proposer": "a1b2c3d4e5f6789012345678901234567890123456789012345678901234abcd",
  "proposal_type": {
    "ParameterChange": {
      "title": "Reduce minimum stake",
      "description": "Lower barrier to entry",
      "parameter": "MIN_STAKE_SATS",
      "new_value": "5000000"
    }
  },
  "nonce": 10,
  "pub_key": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
  "signature": "fedcba9876543210..."
}
```

### Vote Transaction

**Structure:**
```rust
pub struct VoteTx {
    pub voter: Address,
    pub proposal_id: u64,
    pub vote: bool,           // true = yes, false = no
    pub nonce: u64,
    pub pub_key: PublicKey,
    pub signature: Signature,
}
```

**Signable Bytes:**
```
signable_bytes = NETWORK_ID || voter || proposal_id_LE64 || vote_byte || nonce_LE64
```

Where `vote_byte = 1` if yes, `0` if no

**JSON Example:**
```json
{
  "voter": "a1b2c3d4e5f6789012345678901234567890123456789012345678901234abcd",
  "proposal_id": 1,
  "vote": true,
  "nonce": 11,
  "pub_key": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
  "signature": "fedcba9876543210..."
}
```

---

## Signature Scheme

### Ed25519 Overview

**Algorithm:** Ed25519 (Curve25519 + SHA-512)  
**Key Size:** 32 bytes (public), 64 bytes (secret)  
**Signature Size:** 64 bytes  
**Security Level:** ~128-bit

### Key Generation

```rust
use ed25519_dalek::{SigningKey, VerifyingKey};

// Generate keypair
let secret_key = SigningKey::generate(&mut rng);
let public_key = secret_key.verifying_key();

// Derive address
let address = Blake3::hash(public_key.as_bytes());
```

### Signing Process

```rust
use ed25519_dalek::Signer;

// 1. Construct signable bytes
let mut signable = Vec::new();
signable.extend_from_slice(NETWORK_ID);
signable.extend_from_slice(&from);
signable.extend_from_slice(&to);
signable.extend_from_slice(&amount.to_le_bytes());
signable.extend_from_slice(&fee.to_le_bytes());
signable.extend_from_slice(&nonce.to_le_bytes());

// 2. Sign
let signature = secret_key.sign(&signable);
```

### Verification Process

```rust
use ed25519_dalek::Verifier;

// 1. Reconstruct signable bytes (same as signing)
let mut signable = Vec::new();
signable.extend_from_slice(NETWORK_ID);
// ... (same as signing)

// 2. Verify signature
public_key.verify(&signable, &signature)?;

// 3. Verify address derivation
assert_eq!(Blake3::hash(public_key.as_bytes()), from);
```

### Network ID

**Purpose:** Prevent cross-network replay attacks

**Testnet:** `b"ultradag-testnet-v1"` (19 bytes)  
**Mainnet:** `b"ultradag-mainnet-v1"` (19 bytes)

Transactions signed for testnet cannot be replayed on mainnet and vice versa.

---

## Validation Rules

### Common Validation

All transactions must pass:

1. **Signature Verification**
   - Ed25519 signature is valid
   - Signed over correct signable bytes
   - Includes correct NETWORK_ID

2. **Address Derivation**
   - `Blake3(pub_key) == sender_address`
   - Prevents public key substitution

3. **Nonce Check**
   - `nonce == current_nonce(sender)`
   - Prevents replay attacks
   - Enforces transaction ordering

4. **Balance Check**
   - `balance(sender) >= amount + fee` (for transfers)
   - `balance(sender) >= amount` (for stakes)

### Transaction-Specific Validation

**Transfer:**
- Amount > 0
- Fee >= MIN_FEE_SATS (1,000)
- Sender != recipient (optional, self-transfers allowed)

**Stake:**
- Amount >= MIN_STAKE_SATS (10,000,000)
- Not already at max stake (if limit exists)

**Unstake:**
- Staked amount >= unstake amount
- Remaining stake >= MIN_STAKE_SATS OR == 0

**Governance:**
- Proposer is active validator (for proposals)
- Voter is active validator (for votes)
- Proposal exists and is active (for votes)

---

## Serialization

### Binary Serialization

UltraDAG uses canonical binary serialization for hashing and signing:

**Transfer Transaction:**
```
[NETWORK_ID (19)] [from (32)] [to (32)] [amount (8 LE)] [fee (8 LE)] [nonce (8 LE)]
```

**Stake Transaction:**
```
[NETWORK_ID (19)] [staker (32)] [amount (8 LE)] [nonce (8 LE)]
```

**Little-Endian Encoding:**
All integers (u64) are encoded as 8-byte little-endian:
```rust
let bytes = amount.to_le_bytes(); // [u8; 8]
```

### JSON Serialization

For RPC API and human readability:

**Hex Encoding:**
- Addresses: 64 hex characters (32 bytes)
- Public keys: 64 hex characters (32 bytes)
- Signatures: 128 hex characters (64 bytes)

**Integer Encoding:**
- Amounts, fees, nonces: Decimal strings or numbers

**Example:**
```json
{
  "from": "a1b2c3d4...",
  "to": "f6e5d4c3...",
  "amount": 100000000,
  "fee": 10000,
  "nonce": 5,
  "pub_key": "0123456789abcdef...",
  "signature": "fedcba9876543210..."
}
```

---

## Examples

### Example 1: Simple Transfer

**Scenario:** Alice sends 1 UDAG to Bob

**Parameters:**
- Alice's address: `a1b2...abcd`
- Bob's address: `f6e5...dcba`
- Amount: 100,000,000 satoshis (1 UDAG)
- Fee: 10,000 satoshis
- Alice's nonce: 5

**Signable Bytes (hex):**
```
756c74726164616767746573746e65742d7631  // "ultradag-testnet-v1"
a1b2c3d4e5f6789012345678901234567890123456789012345678901234abcd  // from
f6e5d4c3b2a1098765432109876543210987654321098765432109876543dcba  // to
00e1f50500000000  // amount (100000000 LE)
1027000000000000  // fee (10000 LE)
0500000000000000  // nonce (5 LE)
```

**Signature:** Ed25519 signature over above bytes

**JSON Transaction:**
```json
{
  "from": "a1b2c3d4e5f6789012345678901234567890123456789012345678901234abcd",
  "to": "f6e5d4c3b2a1098765432109876543210987654321098765432109876543dcba",
  "amount": 100000000,
  "fee": 10000,
  "nonce": 5,
  "pub_key": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
  "signature": "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210"
}
```

### Example 2: Stake Transaction

**Scenario:** Alice stakes 1 UDAG to become validator

**Parameters:**
- Alice's address: `a1b2...abcd`
- Amount: 100,000,000 satoshis (1 UDAG)
- Alice's nonce: 6

**Signable Bytes (hex):**
```
756c74726164616767746573746e65742d7631  // "ultradag-testnet-v1"
a1b2c3d4e5f6789012345678901234567890123456789012345678901234abcd  // staker
00e1f50500000000  // amount (100000000 LE)
0600000000000000  // nonce (6 LE)
```

**JSON Transaction:**
```json
{
  "staker": "a1b2c3d4e5f6789012345678901234567890123456789012345678901234abcd",
  "amount": 100000000,
  "nonce": 6,
  "pub_key": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
  "signature": "fedcba9876543210..."
}
```

### Example 3: Vote Transaction

**Scenario:** Alice votes YES on proposal #1

**Parameters:**
- Alice's address: `a1b2...abcd`
- Proposal ID: 1
- Vote: true (yes)
- Alice's nonce: 7

**Signable Bytes (hex):**
```
756c74726164616767746573746e65742d7631  // "ultradag-testnet-v1"
a1b2c3d4e5f6789012345678901234567890123456789012345678901234abcd  // voter
0100000000000000  // proposal_id (1 LE)
01                // vote (1 = yes)
0700000000000000  // nonce (7 LE)
```

**JSON Transaction:**
```json
{
  "voter": "a1b2c3d4e5f6789012345678901234567890123456789012345678901234abcd",
  "proposal_id": 1,
  "vote": true,
  "nonce": 7,
  "pub_key": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
  "signature": "fedcba9876543210..."
}
```

---

## Implementation Notes

### Security Considerations

1. **Never reuse nonces** - Always fetch current nonce before signing
2. **Verify network ID** - Ensure correct network (testnet vs mainnet)
3. **Validate addresses** - Check address derivation before accepting
4. **Use constant-time comparison** - For signature verification
5. **Protect secret keys** - Never expose over network or in logs

### Performance Optimization

1. **Batch signature verification** - Verify multiple signatures in parallel
2. **Cache public key derivations** - Avoid repeated Blake3 hashing
3. **Reuse signable byte buffers** - Reduce allocations
4. **Validate cheapest checks first** - Nonce before signature

### Common Pitfalls

1. **Wrong byte order** - Must use little-endian for integers
2. **Missing network ID** - Must include in signable bytes
3. **Incorrect nonce** - Must match current account nonce exactly
4. **Insufficient balance** - Check balance + fee, not just amount
5. **Invalid hex encoding** - Must be exactly 64 chars for addresses

---

## Reference Implementation

See `crates/ultradag-coin/src/tx/` for production implementation:

- `transfer.rs` - Transfer transaction
- `stake.rs` - Stake transaction
- `unstake.rs` - Unstake transaction
- `governance.rs` - Governance transactions

---

## Additional Resources

- **RPC API Reference:** [docs/reference/api/rpc-endpoints.md](../api/rpc-endpoints.md)
- **Integration Guide:** [docs/guides/development/integration-guide.md](../../guides/development/integration-guide.md)
- **Whitepaper:** [docs/reference/specifications/whitepaper.md](whitepaper.md)

---

**Last Updated:** March 10, 2026  
**Document Version:** 1.0  
**Maintainer:** UltraDAG Core Team
