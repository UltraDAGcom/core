# Validator Federation Bridge - Implementation Guide

## Overview

The Validator Federation Bridge enables trustless cross-chain transfers between UltraDAG and Arbitrum using the existing DAG validator set. No external relayers needed!

## Architecture

```
┌─────────────────┐                    ┌─────────────────┐
│  UltraDAG DAG   │                    │   Arbitrum      │
│                 │                    │                 │
│  Validators     │◄──── Attestations ─►│  Bridge Contract│
│  (2/3 threshold)│    (2/3+ sigs)     │                 │
│                 │                    │                 │
└─────────────────┘                    └─────────────────┘
```

## How It Works

### 1. User Deposits on DAG

```rust
// User creates deposit transaction
let deposit_tx = BridgeDepositTx {
    from: user_address,
    recipient: arbitrum_address,
    amount: 1000 * COIN,
    destination_chain_id: 42161, // Arbitrum
    nonce: 0,
    fee: MIN_FEE_SATS,
    ..
};

// Submit to DAG
rpc.submit_transaction(deposit_tx);
```

### 2. Validators Sign Attestation

```rust
// State engine creates attestation
let attestation = state.create_bridge_attestation(
    sender,
    recipient,
    amount,
    destination_chain_id,
)?;

// Each validator signs as part of block production
let signature = validator_sk.sign(&attestation.hash());
state.sign_bridge_attestation(nonce, validator, signature)?;
```

### 3. Collect 2/3+ Signatures

```rust
// Wait for threshold signatures
let threshold = state.get_bridge_threshold(); // ceil(2/3 * validators)
let signature_count = state.get_signature_count(nonce);

if signature_count >= threshold {
    // Build proof
    let proof = state.build_bridge_proof(nonce)?;
    
    // User can now claim on Arbitrum
}
```

### 4. User Claims on Arbitrum

```solidity
// Submit proof to Arbitrum bridge
bridge.claimWithdrawal(
    attestation.sender,
    attestation.recipient,
    attestation.amount,
    attestation.nonce,
    encoded_signatures,
    message_hash
);
```

## Rust Implementation

### Core Types

**`BridgeAttestation`** (`crates/ultradag-coin/src/bridge/mod.rs`):
```rust
pub struct BridgeAttestation {
    pub sender: Address,
    pub recipient: [u8; 20],
    pub amount: u64,
    pub nonce: u64,
    pub destination_chain_id: u64,
}
```

**`SignedBridgeAttestation`**:
```rust
pub struct SignedBridgeAttestation {
    pub attestation: BridgeAttestation,
    pub validator: Address,
    pub signature: Vec<u8>, // Ed25519 signature
}
```

**`BridgeProof`**:
```rust
pub struct BridgeProof {
    pub attestation: BridgeAttestation,
    pub signatures: Vec<SignedBridgeAttestation>,
    pub message_hash: [u8; 32],
}
```

### State Engine Methods

**Create Attestation**:
```rust
pub fn create_bridge_attestation(
    &mut self,
    sender: Address,
    recipient: [u8; 20],
    amount: u64,
    destination_chain_id: u64,
) -> Result<BridgeAttestation, CoinError>
```

**Sign Attestation**:
```rust
pub fn sign_bridge_attestation(
    &mut self,
    nonce: u64,
    validator: Address,
    signature: [u8; 64],
) -> Result<(), CoinError>
```

**Build Proof**:
```rust
pub fn build_bridge_proof(
    &self,
    nonce: u64,
) -> Result<BridgeProof, CoinError>
```

### Transaction Type

**`BridgeDepositTx`** (`crates/ultradag-coin/src/tx/bridge.rs`):
```rust
pub struct BridgeDepositTx {
    pub from: Address,
    pub recipient: [u8; 20],
    pub amount: u64,
    pub destination_chain_id: u64,
    pub nonce: u64,
    pub fee: u64,
    pub pub_key: [u8; 32],
    pub signature: Signature,
}
```

## Solidity Contract

**`UDAGBridgeValidator.sol`** (`bridge/src/UDAGBridgeValidator.sol`):

Key functions:
- `deposit(bytes20 nativeRecipient, uint256 amount)` - Lock tokens
- `claimWithdrawal(...)` - Claim with validator signatures
- `addValidator(address)` - Add validator (governor only)
- `removeValidator(address)` - Remove validator (governor only)

## Security Properties

### 1. Threshold Security
- Requires 2/3+ validator signatures
- Same security as DAG consensus
- Collusion resistance: attacker needs 2/3 of validators

### 2. Replay Protection
- Nonce-based prevention
- Each withdrawal has unique nonce
- Used nonces tracked on Arbitrum

### 3. Rate Limiting
- Max 100K UDAG per transaction
- Daily cap: 500K UDAG
- Prevents large exploits

### 4. Emergency Controls
- Governor can pause bridge
- Any validator can pause
- Refund after 7-day timeout

## Integration Steps

### 1. Add to Block Production

In validator loop:
```rust
// After processing transactions
for deposit in bridge_deposits {
    let attestation = state.create_bridge_attestation(...)?;
    let signature = validator_sk.sign(&attestation.hash());
    state.sign_bridge_attestation(...)?;
    
    // Include in block
    block.bridge_attestations.push(attestation);
}
```

### 2. Add RPC Endpoints

```rust
// GET /bridge/attestation/{nonce}
async fn get_attestation(nonce: u64) -> Result<BridgeProof> {
    let state = state.read().await;
    state.build_bridge_proof(nonce)
}

// GET /bridge/nonce
async fn get_nonce() -> u64 {
    state.read().await.get_bridge_nonce()
}
```

### 3. Persistence

Bridge state is included in snapshots:
```rust
pub struct StateSnapshot {
    // ... other fields
    pub bridge_attestations: Vec<(u64, BridgeAttestation)>,
    pub bridge_signatures: Vec<((u64, Address), Vec<u8>)>,
    pub bridge_nonce: u64,
}
```

## Testing

### Unit Tests
```bash
cargo test --package ultradag-coin --lib bridge
```

### Integration Tests
```bash
# TODO: Add integration tests
cargo test --package ultradag-coin --test bridge_integration
```

## Deployment Checklist

- [ ] Deploy `UDAGBridgeValidator.sol` to Arbitrum
- [ ] Add initial validator set
- [ ] Set threshold (2/3)
- [ ] Grant MINTER_ROLE to bridge
- [ ] Test deposit flow
- [ ] Test withdrawal flow
- [ ] Test pause/unpause
- [ ] Security audit

## Future Improvements

### Phase 1: Current Implementation ✅
- Validator federation (2/3 threshold)
- Basic attestation flow
- Manual proof submission

### Phase 2: Succinct Labs Integration (2-3 months)
- ZK light client proofs
- Permissionless proving
- Faster finality

### Phase 3: Full ZK Bridge (6-12 months)
- Custom ZK circuits for DAG consensus
- Permissionless prover network
- Trustless security

## References

- [Bridge Comparison](./BRIDGE_COMPARISON.md)
- [Relayer Operator Setup](./RELAYER_OPERATOR_SETUP.md)
- [Deployment Guide](./DEPLOYMENT_GUIDE.md)
- [Solidity Contract](../../bridge/src/UDAGBridgeValidator.sol)
- [Rust Implementation](../../crates/ultradag-coin/src/bridge/)
