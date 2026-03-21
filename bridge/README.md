# UltraDAG Bridge Contracts

Solidity contracts for bridging UDAG between Arbitrum and UltraDAG native chain.

## Overview

The UltraDAG bridge enables secure transfer of UDAG tokens between:
- **Arbitrum**: ERC-20 representation (8 decimals)
- **UltraDAG Native**: Native UDAG coins (8 decimals = 1 UDAG)

## Contracts

| Contract | Description |
|----------|-------------|
| `UDAGToken.sol` | ERC-20 token with mint/burn roles |
| `UDAGBridge.sol` | Multi-sig bridge with rate limiting |

## Features

- ✅ **Multi-sig Relayers**: 3-of-5 threshold for bridge operations
- ✅ **Rate Limiting**: 100K UDAG/tx, 500K UDAG/day
- ✅ **Replay Protection**: Nonce-based prevention
- ✅ **Refund Timeout**: 7-day refund window if bridge stalls
- ✅ **Emergency Pause**: Any relayer can pause
- ✅ **Timelock Governance**: Delayed governance changes
- ✅ **Max Supply**: 21M UDAG enforced

## Quick Start

### Install Foundry

```bash
curl -L https://foundry.paradigm.xyz | bash
foundryup
```

### Build Contracts

```bash
cd bridge
forge build
```

### Run Tests

```bash
forge test -vvv
```

### Deploy to Testnet

```bash
# Copy environment template
cp .env.example .env

# Edit .env with your values
# - RPC_URL (Arbitrum Sepolia)
# - DEPLOYER_KEY
# - GOVERNOR_KEY
# - DEV_ADDRESS, TREASURY_ADDRESS
# - RELAYER_KEYS (5 keys for 3-of-5)

# Deploy
forge script script/Deploy.s.sol:DeployScript \
  --rpc-url $RPC_URL \
  --private-key $DEPLOYER_KEY \
  --broadcast \
  --verify \
  -vvvv
```

### Activate Bridge

```bash
# After testing and audit
forge script script/ActivateBridge.s.sol:ActivateBridgeScript \
  --rpc-url $RPC_URL \
  --private-key $GOVERNOR_KEY \
  --broadcast \
  -vvvv
```

## Architecture

```
┌─────────────────┐                    ┌─────────────────┐
│   Arbitrum      │                    │  UltraDAG Native│
│                 │                    │                 │
│  UDAG Token     │◄──── Bridge ──────►│  Native UDAG    │
│  (ERC-20)       │    (Multi-sig)     │  (Layer 1)      │
│                 │                    │                 │
└─────────────────┘                    └─────────────────┘
       ▲                                      ▲
       │                                      │
       └────────── Relayer Network ───────────┘
              (3-of-5 multi-sig)
```

## Bridge Flow

### Arbitrum → Native

1. User approves bridge: `token.approve(bridge, amount)`
2. User calls: `bridge.bridgeToNative(nativeRecipient, amount)`
3. Bridge escrows tokens
4. Relayers observe native chain, confirm delivery
5. Relayers sign: `bridge.completeBridgeToNative(nonce, signatures)`
6. Bridge burns escrowed tokens

### Native → Arbitrum

1. User locks on native chain
2. Relayers observe, sign attestation
3. User calls: `bridge.bridgeFromNative(...)`
4. Bridge verifies multi-sig (3-of-5)
5. Bridge mints tokens to recipient

## Security

### Multi-sig Threshold

- **5 relayers** operated by different parties
- **3-of-5 threshold** for bridge operations
- **Sorted signatures** to prevent replay

### Rate Limiting

- **Per transaction**: 100,000 UDAG
- **Daily cap**: 500,000 UDAG
- **Resets**: Every 24 hours

### Replay Protection

- **Nonces**: Monotonic incrementing
- **Processed tracking**: Prevents double-spend

### Emergency Controls

- **Pause**: Any relayer can pause
- **Unpause**: Only governor can unpause
- **Refund**: Users can refund after 7 days

## Governance

### TimelockController

All governance actions go through timelock:
- **Delay**: 1 day (configurable)
- **Proposer**: Governor address
- **Executor**: Timelock itself

### Governor Actions

| Action | Access | Timelock |
|--------|--------|----------|
| Activate bridge | Governor | Yes |
| Pause/unpause | Relayer/Governor | No/Yes |
| Add/remove relayer | Governor | Yes |
| Change threshold | Governor | Yes |
| Transfer governance | Governor | Yes |

## Testing

### Run All Tests

```bash
forge test -vvv
```

### Run Specific Test

```bash
# Token tests
forge test --match-contract UDAGTokenTest -vvv

# Bridge tests
forge test --match-contract UDAGBridgeTest -vvv
```

### Test Coverage

```bash
forge coverage
```

Expected coverage:
- UDAGToken: 100%
- UDAGBridge: 95%+

## Deployment Checklist

### Pre-Deployment

- [ ] External audit completed
- [ ] 5 relayer operators recruited
- [ ] Monitoring configured
- [ ] Emergency procedures documented
- [ ] Bug bounty launched

### Deployment

- [ ] Deploy to testnet first
- [ ] Test all bridge operations
- [ ] Run for 2-4 weeks on testnet
- [ ] Deploy to mainnet
- [ ] Verify contracts on Arbiscan

### Post-Deployment

- [ ] Activate bridge
- [ ] Monitor first transactions
- [ ] Verify relayer operations
- [ ] Document deployment addresses

## Documentation

| Document | Description |
|----------|-------------|
| [DEPLOYMENT_GUIDE.md](./DEPLOYMENT_GUIDE.md) | Complete deployment instructions |
| [RELAYER_GUIDE.md](./RELAYER_GUIDE.md) | Relayer setup and operations |
| [script/Deploy.s.sol](./script/Deploy.s.sol) | Deployment script |
| [script/ActivateBridge.s.sol](./script/ActivateBridge.s.sol) | Activation script |

## Contract Addresses

### Testnet (Arbitrum Sepolia)

| Contract | Address |
|----------|---------|
| UDAGToken | _TBD_ |
| UDAGBridge | _TBD_ |
| TimelockController | _TBD_ |

### Mainnet (Arbitrum One)

| Contract | Address |
|----------|---------|
| UDAGToken | _TBD_ |
| UDAGBridge | _TBD_ |
| TimelockController | _TBD_ |

## Support

- **Documentation**: https://ultradag.com/docs/bridge
- **Discord**: https://discord.gg/ultradag
- **Email**: security@ultradag.com

## License

BUSL-1.1
