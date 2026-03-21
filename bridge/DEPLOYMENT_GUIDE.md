# UltraDAG Bridge Deployment Guide

Complete guide for deploying the UltraDAG bridge to Arbitrum.

## Prerequisites

- Foundry installed (`curl -L https://foundry.paradigm.xyz | bash`)
- Arbitrum RPC endpoint
- UltraDAG node RPC endpoint
- 5 relayer operators recruited
- External audit completed

## Phase 1: Testnet Deployment

### 1.1 Configure Environment

```bash
cd bridge

# Copy example environment file
cp .env.example .env

# Edit .env with your values
```

### 1.2 Environment Variables

```bash
# Deployment
RPC_URL=https://sepolia-rollup.arbitrum.io/rpc
DEPLOYER_KEY=0x...  # Deployer private key
GOVERNOR_KEY=0x...  # Governor/admin address (will own timelock)

# Genesis allocations
DEV_ADDRESS=0x...   # Developer allocation recipient
TREASURY_ADDRESS=0x...  # Treasury allocation recipient

# Relayers (5 keys for 3-of-5 multi-sig)
RELAYER_KEYS=key1,key2,key3,key4,key5

# Bridge (after deployment)
BRIDGE_ADDRESS=0x...  # Will be filled after deployment
```

### 1.3 Deploy Contracts

```bash
# Run deployment script
forge script script/Deploy.s.sol:DeployScript \
  --rpc-url $RPC_URL \
  --private-key $DEPLOYER_KEY \
  --broadcast \
  --verify \
  -vvvv
```

### 1.4 Verify Deployment

```bash
# Check token
cast call $TOKEN_ADDRESS "name()(string)"
# Expected: "UltraDAG"

# Check bridge
cast call $BRIDGE_ADDRESS "bridgeActive()(bool)"
# Expected: false (starts inactive)

# Check relayers
cast call $BRIDGE_ADDRESS "relayerCount()(uint256)"
# Expected: 5

# Check threshold
cast call $BRIDGE_ADDRESS "requiredSignatures()(uint256)"
# Expected: 3
```

### 1.5 Test Bridge Operations

#### Test Arbitrum → Native

```bash
# 1. Approve bridge
cast send $TOKEN_ADDRESS \
  "approve(address,uint256)" \
  $BRIDGE_ADDRESS 100000000 \
  --private-key $USER_KEY

# 2. Bridge to native
cast send $BRIDGE_ADDRESS \
  "bridgeToNative(bytes20,uint256)" \
  0xaabbccddee00112233445566778899aabbccddee 100000000 \
  --private-key $USER_KEY

# 3. Check request created
cast call $BRIDGE_ADDRESS \
  "bridgeRequests(uint256)(address,bytes20,uint256,uint256,bool,bool)" \
  0
```

#### Test Native → Arbitrum

```bash
# This requires relayer infrastructure
# See RELAYER_GUIDE.md for setup
```

### 1.6 Run Testnet for 2-4 Weeks

- Monitor bridge operations
- Test all edge cases
- Verify relayer infrastructure
- Collect metrics

## Phase 2: Mainnet Deployment

### 2.1 Pre-Deployment Checklist

- [ ] Testnet deployment successful (2-4 weeks)
- [ ] External audit completed
- [ ] 5 relayer operators recruited and trained
- [ ] Monitoring and alerting configured
- [ ] Emergency procedures documented
- [ ] Bug bounty program launched

### 2.2 Deploy to Mainnet

```bash
# Update .env for mainnet
RPC_URL=https://arb1.arbitrum.io/rpc

# Deploy
forge script script/Deploy.s.sol:DeployScript \
  --rpc-url $RPC_URL \
  --private-key $DEPLOYER_KEY \
  --broadcast \
  --verify \
  -vvvv
```

### 2.3 Post-Deployment Verification

```bash
# Save deployment output
cat deployment-output.json

# Verify on Arbiscan
# Token: https://arbiscan.io/token/$TOKEN_ADDRESS
# Bridge: https://arbiscan.io/address/$BRIDGE_ADDRESS
```

### 2.4 Activate Bridge

**IMPORTANT**: Only activate after:
- All relayers are operational
- Monitoring is configured
- Team is ready to respond to issues

```bash
# Activate bridge
forge script script/ActivateBridge.s.sol:ActivateBridgeScript \
  --rpc-url $RPC_URL \
  --private-key $GOVERNOR_KEY \
  --broadcast \
  -vvvv
```

### 2.5 Monitor Activation

```bash
# Check bridge status
cast call $BRIDGE_ADDRESS "bridgeActive()(bool)"
# Expected: true

# Monitor first transactions
cast logs --address $BRIDGE_ADDRESS --from-block latest
```

## Phase 3: Operations

### 3.1 Daily Operations

- Monitor relayer uptime
- Check pending bridge requests
- Verify daily volume cap
- Review error logs

### 3.2 Weekly Operations

- Review bridge metrics
- Check relayer performance
- Audit completed transactions
- Update documentation

### 3.3 Monthly Operations

- Rotate relayer keys (if needed)
- Review and update procedures
- Test emergency procedures
- Report to stakeholders

## Emergency Procedures

### Pause Bridge

Any relayer can pause:

```bash
cast send $BRIDGE_ADDRESS "pause()" --private-key $RELAYER_KEY
```

### Unpause Bridge

Only governor can unpause:

```bash
cast send $BRIDGE_ADDRESS "unpause()" --private-key $GOVERNOR_KEY
```

### Refund Stuck Transactions

After 7 days, users can refund:

```bash
cast send $BRIDGE_ADDRESS \
  "refundBridge(uint256)" \
  $BRIDGE_NONCE \
  --private-key $USER_KEY
```

## Troubleshooting

### Deployment Fails

Check:
- Sufficient ETH for gas
- RPC endpoint is accessible
- Private keys are valid
- Network is not congested

### Bridge Not Activating

Check:
- Caller is governor
- Bridge not already active
- Timelock delay has passed (if applicable)

### Relayer Not Signing

Check:
- Relayer service is running
- Private key is valid
- RPC endpoints are accessible
- Bridge contract address is correct

## Support

- **Documentation**: https://ultradag.com/docs/bridge
- **Discord**: https://discord.gg/ultradag
- **Email**: ops@ultradag.com
