# UltraDAG Deployment Guide

## Overview

UltraDAG uses a streamlined CI/CD workflow with pre-built binaries for fast, reliable deployments.

**Deployment Speed:** ~60 seconds per node (vs 15+ minutes with source builds)

## Architecture

### CI/CD Pipeline

1. **GitHub Actions** (`build-and-publish.yml`)
   - Triggers on every push to `main` branch
   - Builds Linux binary (x86_64-unknown-linux-gnu)
   - Publishes to GitHub Releases as "latest" tag
   - Build time: ~2-3 minutes

2. **Optimized Dockerfile**
   - Downloads pre-built binary from GitHub Releases
   - No Rust compilation during deployment
   - Minimal image size: 29 MB
   - Fast deployment: ~60 seconds per node

3. **Fly.io Deployment**
   - Uses pre-built binaries from GitHub Releases
   - Nodes deployed to Amsterdam (ams) region
   - Internal networking via Fly.io private network
   - Persistent storage via Fly volumes

## Deployment Methods

### Testnet Deployment

Deploy all 4 testnet nodes:

```bash
./tools/operations/deployment/fly/deploy-testnet.sh
```

Deploy with clean state (wipes all node data):

```bash
./tools/operations/deployment/fly/deploy-testnet.sh --clean
```

Restart nodes without rebuilding:

```bash
./tools/operations/deployment/fly/deploy-testnet.sh --restart
```

### Individual Node Deployment

Deploy a specific node:

```bash
fly deploy -a ultradag-node-1 -c tools/operations/deployment/fly/fly-node-1.toml --remote-only
```

### Manual Binary Deployment

For non-Fly.io deployments, download the pre-built binary:

```bash
# Download latest binary
curl -L "https://github.com/UltraDAGcom/core/releases/download/latest/ultradag-node-linux-x86_64" \
    -o ultradag-node

chmod +x ultradag-node

# Run the node
./ultradag-node --port 9333 --rpc-port 10333 --data-dir /data
```

## Configuration

### Environment Variables

- `PORT`: P2P network port (default: 9333)
- `RPC_PORT`: HTTP RPC port (default: 10333)
- `DATA_DIR`: Data directory path (default: /data)
- `VALIDATORS`: Number of validators (default: 4)
- `SEED`: Comma-separated list of seed nodes
- `NO_BOOTSTRAP`: Skip bootstrap node connection
- `CLEAN_STATE`: Wipe state on startup (use with caution)
- `RUST_LOG`: Logging level (info, debug, warn, error)

### Node Configuration Files

Configuration files are located in `tools/operations/deployment/fly/`:

- `fly-node-1.toml` - Node 1 configuration
- `fly-node-2.toml` - Node 2 configuration
- `fly-node-3.toml` - Node 3 configuration
- `fly-node-4.toml` - Node 4 configuration

## Monitoring

### Check Node Status

```bash
# Check all nodes
for i in 1 2 3 4; do
  echo "=== Node $i ==="
  curl -s "https://ultradag-node-$i.fly.dev/status" | jq
done
```

### View Logs

```bash
# View logs for a specific node
fly logs -a ultradag-node-1

# Follow logs in real-time
fly logs -a ultradag-node-1 --no-tail=false
```

### Check Machine Status

```bash
# List machines for a node
fly machines list -a ultradag-node-1

# Get machine details
fly machine status <machine-id> -a ultradag-node-1
```

## Troubleshooting

### Nodes Not Connecting

1. Check if all nodes are running:
   ```bash
   fly status -a ultradag-node-1
   ```

2. Verify internal networking:
   ```bash
   fly logs -a ultradag-node-1 | grep "Connected to"
   ```

3. Restart nodes if needed:
   ```bash
   ./tools/operations/deployment/fly/deploy-testnet.sh --restart
   ```

### State Conflicts

If nodes have conflicting state (stuck at same round, rejecting vertices):

```bash
# Clean state on all nodes
./tools/operations/deployment/fly/deploy-testnet.sh --clean
```

### Deployment Failures

1. Check GitHub Actions workflow status
2. Verify binary was published to GitHub Releases
3. Check Fly.io status page
4. Review deployment logs

## Best Practices

1. **Always deploy to testnet first** before mainnet
2. **Use `--clean` flag sparingly** - it wipes all node data
3. **Monitor logs after deployment** for connection issues
4. **Wait 30-60 seconds** after deployment for nodes to stabilize
5. **Check finality** to ensure consensus is working

## Workflow Summary

```
Code Change → Push to main → GitHub Actions builds binary → 
Binary published to Releases → Fly.io pulls binary → 
Nodes deployed in ~60 seconds → Network operational
```

## Migration from Docker Builds

The old Docker-based deployment (building from source) has been replaced with pre-built binaries:

- **Old:** 15+ minutes per node, often timed out
- **New:** ~60 seconds per node, reliable

Archived Docker files are in `/archive/` for reference.
