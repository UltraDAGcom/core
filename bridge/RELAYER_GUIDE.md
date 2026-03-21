# UltraDAG Bridge Relayer

Relayer infrastructure for the UltraDAG bridge. Relayers observe both chains (Arbitrum and UltraDAG native) and sign attestations for bridge transfers.

## Architecture

```
┌─────────────────┐      ┌─────────────────┐
│  Arbitrum Node  │      │ UltraDAG Node   │
│   (RPC/WS)      │      │   (RPC/WS)      │
└────────┬────────┘      └────────┬────────┘
         │                        │
         │                        │
         ▼                        ▼
┌─────────────────────────────────────────┐
│         Relayer Service                 │
│  ┌──────────┐  ┌──────────┐            │
│  │ Arbitrum │  │ Native   │            │
│  │ Monitor  │  │ Monitor  │            │
│  └────┬─────┘  └────┬─────┘            │
│       │             │                   │
│       └──────┬──────┘                   │
│              │                          │
│       ┌──────▼──────┐                   │
│       │  Signer     │                   │
│       │  (Private   │                   │
│       │   Key)      │                   │
│       └──────┬──────┘                   │
│              │                          │
│       ┌──────▼──────┐                   │
│       │  Broadcaster│                   │
│       └─────────────┘                   │
└─────────────────────────────────────────┘
```

## Setup

### Prerequisites

- Node.js 18+
- Access to Arbitrum RPC endpoint
- Access to UltraDAG node RPC endpoint
- Relayer private key (keep secure!)

### Installation

```bash
npm install
```

### Configuration

Create `.env` file:

```bash
# Relayer private key (KEEP SECRET!)
RELAYER_PRIVATE_KEY=your_private_key_here

# Arbitrum RPC endpoint
ARBITRUM_RPC_URL=https://arb1.arbitrum.io/rpc

# UltraDAG node RPC endpoint
ULTRADAG_RPC_URL=https://ultradag-node-1.fly.dev:10333

# Bridge contract address (Arbitrum)
BRIDGE_ADDRESS=0x...

# Polling interval (milliseconds)
POLL_INTERVAL=5000

# Log level (debug, info, warn, error)
LOG_LEVEL=info
```

### Running the Relayer

```bash
# Development
npm run relayer

# Production (with PM2)
pm2 start ecosystem.config.js
```

## Monitoring

### Health Check

```bash
curl http://localhost:3000/health
```

Response:
```json
{
  "status": "healthy",
  "relayer": "0x...",
  "arbitrumBlock": 123456789,
  "ultradagBlock": 987654321,
  "pendingSignatures": 3,
  "completedToday": 15
}
```

### Metrics (Prometheus)

```bash
curl http://localhost:3000/metrics
```

Metrics:
- `relayer_pending_signatures`: Number of pending signatures
- `relayer_completed_total`: Total completed bridge operations
- `relayer_arbitrum_block_height`: Current Arbitrum block
- `relayer_ultradag_block_height`: Current UltraDAG block
- `relayer_uptime_seconds`: Relayer uptime

## Security

### Private Key Management

**NEVER commit private keys to version control!**

Use one of these methods:

1. **Environment variable** (development only):
   ```bash
   export RELAYER_PRIVATE_KEY=...
   ```

2. **AWS Secrets Manager** (production):
   ```bash
   aws secretsmanager get-secret-value --secret-id relayer-key
   ```

3. **HashiCorp Vault** (production):
   ```bash
   vault kv get -field=key secret/relayer
   ```

### Relayer Best Practices

1. **Run on separate infrastructure** - Don't run all 5 relayers on same server
2. **Use hardware security modules** - AWS KMS, HashiCorp Vault, etc.
3. **Monitor uptime** - Set up alerts if relayer goes offline
4. **Rotate keys periodically** - Update relayer keys every 90 days
5. **Limit RPC access** - Use allowlists for RPC endpoints

## Operations

### Adding a New Relayer

1. Governor calls `bridge.addRelayer(newRelayer)`
2. Deploy relayer with new private key
3. Verify relayer is signing correctly
4. Monitor for successful signatures

### Removing a Relayer

1. Governor calls `bridge.removeRelayer(relayer)`
2. Stop relayer service
3. Revoke access to private key
4. Update monitoring to exclude removed relayer

### Emergency Procedures

#### Relayer Compromised

1. **Immediately pause bridge**: Any relayer can call `bridge.pause()`
2. **Remove compromised relayer**: Governor calls `removeRelayer()`
3. **Rotate all keys**: Deploy new relayers with new keys
4. **Unpause bridge**: Governor calls `unpause()`

#### Bridge Stuck

1. Check relayer logs for errors
2. Verify RPC endpoints are accessible
3. Check if threshold is met (3-of-5 relayers online)
4. If needed, refund users after timeout (7 days)

## Troubleshooting

### Relayer Not Signing

Check logs:
```bash
tail -f logs/relayer.log | grep "ERROR"
```

Common issues:
- Private key invalid
- RPC endpoint unreachable
- Bridge contract address wrong
- Insufficient gas (Arbitrum)

### Signatures Not Accepted

Verify:
1. Relayer is in the relayer list (`bridge.isRelayer(relayer)`)
2. Signatures are sorted correctly
3. Required threshold is met (3 signatures)
4. Message hash matches expected format

### High Latency

Check:
1. RPC endpoint response times
2. Network connectivity
3. Relayer server resources (CPU, memory)
4. Database performance (if using persistence)

## Development

### Running Tests

```bash
npm test
```

### Local Testing

1. Start local Arbitrum node (Anvil):
   ```bash
   anvil --fork-url https://arb1.arbitrum.io/rpc
   ```

2. Deploy bridge contracts to local node

3. Run relayer against local node:
   ```bash
   ARBITRUM_RPC_URL=http://localhost:8545 npm run relayer
   ```

## Support

- **Documentation**: https://ultradag.com/docs/bridge
- **Discord**: https://discord.gg/ultradag
- **Email**: ops@ultradag.com
