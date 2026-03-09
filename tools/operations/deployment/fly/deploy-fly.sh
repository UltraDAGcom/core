#!/bin/bash
# Deploy UltraDAG to Fly.io with safety checks
# Usage: ./deploy_fly.sh [testnet|mainnet]

set -e  # Exit on any error

NETWORK=${1:-testnet}

echo "🚀 UltraDAG Fly.io Deployment"
echo "=============================="
echo ""

# Set environment based on network
if [ "$NETWORK" = "mainnet" ]; then
    export NETWORK="mainnet"
    export NETWORK_URL="https://ultradag-node-1.fly.dev"
    export DATA_DIR="/root/.ultradag/node"
    export DEPLOY_TARGET="fly.io"
    
    echo "⚠️  MAINNET DEPLOYMENT"
    echo "This will deploy to production. All safety checks will be enforced."
    echo ""
else
    export NETWORK="testnet"
    export NETWORK_URL="https://ultradag-node-1.fly.dev"
    export DATA_DIR="/root/.ultradag/node"
    export DEPLOY_TARGET="fly.io"
    
    echo "📡 TESTNET DEPLOYMENT"
    echo ""
fi

# Run pre-deployment safety checks
echo "Running pre-deployment safety checks..."
./scripts/pre_deploy_check.sh

if [ $? -ne 0 ]; then
    echo "❌ Pre-deployment checks failed. Aborting."
    exit 1
fi

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "Deploying to Fly.io..."
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

# Deploy to all nodes
NODES=("ultradag-node-1" "ultradag-node-2" "ultradag-node-3" "ultradag-node-4")

for node in "${NODES[@]}"; do
    echo "Deploying to $node..."
    
    if fly deploy --app "$node" --remote-only; then
        echo "✅ $node deployed successfully"
    else
        echo "❌ Failed to deploy to $node"
        echo ""
        echo "Deployment failed. You may need to:"
        echo "1. Check Fly.io status"
        echo "2. Verify app configuration"
        echo "3. Roll back if needed: fly releases rollback --app $node"
        exit 1
    fi
    
    echo ""
done

echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "✅ Deployment complete"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
echo "Verifying deployment..."

# Wait for nodes to start
sleep 10

# Check each node
for node in "${NODES[@]}"; do
    URL="https://$node.fly.dev/status"
    echo -n "Checking $node... "
    
    STATUS=$(curl -s --max-time 10 "$URL" 2>/dev/null)
    if [ -n "$STATUS" ]; then
        ROUND=$(echo "$STATUS" | jq -r '.dag_round' 2>/dev/null)
        if [ -n "$ROUND" ] && [ "$ROUND" != "null" ]; then
            echo "✅ Round $ROUND"
        else
            echo "⚠️  Responding but no round info"
        fi
    else
        echo "❌ Not responding"
    fi
done

echo ""
echo "Deployment verification complete."
echo ""
echo "Monitor with: ./scripts/monitor.sh"
