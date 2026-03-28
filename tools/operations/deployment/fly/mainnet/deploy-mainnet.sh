#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../../../../.." && pwd)"
cd "$ROOT_DIR"

echo "==> UltraDAG Mainnet Deployment"
echo "    NETWORK_ID: ultradag-mainnet-v1"
echo "    Addresses: udag1..."
echo "    Faucet: DISABLED"
echo ""

NODES="1 2 3 4 5"

# Check for --clean flag
CLEAN=false
if [ "${1:-}" = "--clean" ]; then
    CLEAN=true
    echo "==> CLEAN deploy: will wipe all mainnet state!"
    read -p "Are you sure? Type 'yes' to confirm: " confirm
    if [ "$confirm" != "yes" ]; then
        echo "Aborted."
        exit 1
    fi
fi

if [ "$CLEAN" = true ]; then
    echo "==> Setting CLEAN_STATE in mainnet TOML files..."
    for i in $NODES; do
        sed -i '' 's/# CLEAN_STATE = "true"/CLEAN_STATE = "true"/' "$SCRIPT_DIR/fly-mainnet-$i.toml"
    done
fi

GIT_SHA=$(git rev-parse --short HEAD 2>/dev/null || echo "unknown")
echo "==> Deploying mainnet nodes (pre-built binary, git: $GIT_SHA)..."
for i in $NODES; do
    echo "  Deploying ultradag-mainnet-$i..."
    fly deploy -c "$SCRIPT_DIR/fly-mainnet-$i.toml" \
        --dockerfile Dockerfile \
        --build-arg VARIANT=-mainnet \
        --build-arg CACHEBUST="$GIT_SHA" \
        --wait-timeout 300 \
        --strategy rolling \
        2>&1 | tail -3
    echo "  ultradag-mainnet-$i deployed."
done

echo "==> Restarting all mainnet machines simultaneously..."
for i in $NODES; do
    echo "  Restarting ultradag-mainnet-$i..."
    fly machines restart -a "ultradag-mainnet-$i" --skip-health-checks 2>&1 | tail -2
done

if [ "$CLEAN" = true ]; then
    echo "==> Reverting CLEAN_STATE in TOML files..."
    for i in $NODES; do
        sed -i '' 's/CLEAN_STATE = "true"/# CLEAN_STATE = "true"/' "$SCRIPT_DIR/fly-mainnet-$i.toml"
    done
fi

echo ""
echo "==> Waiting 30s for nodes to connect and start producing..."
sleep 30

echo "==> Checking mainnet health..."
for i in $NODES; do
    STATUS=$(curl -s "https://ultradag-mainnet-$i.fly.dev/status" 2>/dev/null || echo '{"error":"unreachable"}')
    ROUND=$(echo "$STATUS" | python3 -c "import json,sys; print(json.load(sys.stdin).get('dag_round', 'N/A'))" 2>/dev/null || echo "N/A")
    FIN=$(echo "$STATUS" | python3 -c "import json,sys; print(json.load(sys.stdin).get('last_finalized_round', 'N/A'))" 2>/dev/null || echo "N/A")
    PEERS=$(echo "$STATUS" | python3 -c "import json,sys; print(json.load(sys.stdin).get('peer_count', 'N/A'))" 2>/dev/null || echo "N/A")
    echo "  Mainnet Node $i: round=$ROUND fin=$FIN peers=$PEERS"
done

echo ""
echo "==> Mainnet deployment complete!"
echo "    RPC endpoints:"
for i in $NODES; do
    echo "      https://ultradag-mainnet-$i.fly.dev"
done
