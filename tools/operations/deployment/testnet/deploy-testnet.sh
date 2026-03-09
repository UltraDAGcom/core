#!/usr/bin/env bash
# Deploy and restart all 4 testnet nodes simultaneously.
#
# Usage:
#   ./scripts/deploy-testnet.sh              # Build + deploy new code to all 4 nodes
#   ./scripts/deploy-testnet.sh --clean      # Same but wipes state (CLEAN_STATE)
#   ./scripts/deploy-testnet.sh --restart    # Just restart (no rebuild), useful after code is already deployed
#
# The key insight: fly deploy is sequential per-node and takes minutes.
# For simultaneous starts, we first deploy the image (which restarts nodes),
# then if --clean, we set CLEAN_STATE and restart all machines at once.

set -euo pipefail

NODES=(ultradag-node-1 ultradag-node-2 ultradag-node-3 ultradag-node-4)
CLEAN=false
RESTART_ONLY=false

for arg in "$@"; do
    case "$arg" in
        --clean) CLEAN=true ;;
        --restart) RESTART_ONLY=true ;;
        *) echo "Unknown arg: $arg"; exit 1 ;;
    esac
done

if $CLEAN; then
    echo "==> Setting CLEAN_STATE=true on all nodes..."
    for node in "${NODES[@]}"; do
        fly secrets set CLEAN_STATE=true -a "$node" --stage &
    done
    wait
    echo "    Staged. Will take effect on next restart."
fi

if ! $RESTART_ONLY; then
    echo "==> Building and deploying new code to all nodes..."
    # Deploy sequentially (shared remote builder can't parallelize well)
    for i in 1 2 3 4; do
        echo "  Deploying node $i..."
        fly deploy -a "ultradag-node-$i" -c "fly-node-$i.toml" --remote-only 2>&1 | grep -E "succeeded|Visit|Error" || true
    done
    echo "    All nodes deployed."
fi

if $RESTART_ONLY || $CLEAN; then
    echo "==> Restarting all machines simultaneously..."
    # Get machine IDs
    for node in "${NODES[@]}"; do
        MACHINE_ID=$(fly machines list -a "$node" --json 2>/dev/null | python3 -c "import sys,json; print(json.load(sys.stdin)[0]['id'])")
        echo "  Restarting $node ($MACHINE_ID)..."
        fly machine restart "$MACHINE_ID" -a "$node" &
    done
    wait
    echo "    All machines restarted."
fi

if $CLEAN; then
    echo "==> Removing CLEAN_STATE secret (staged for next deploy)..."
    for node in "${NODES[@]}"; do
        fly secrets unset CLEAN_STATE -a "$node" --stage 2>/dev/null &
    done
    wait
    echo "    Done."
fi

echo ""
echo "==> Waiting 30s for nodes to connect and start producing..."
sleep 30

echo "==> Checking health..."
for i in 1 2 3 4; do
    STATUS=$(curl -s --max-time 5 "https://ultradag-node-$i.fly.dev/status" 2>/dev/null || echo '{}')
    ROUND=$(echo "$STATUS" | python3 -c "import sys,json; print(json.load(sys.stdin).get('dag_round', '?'))" 2>/dev/null || echo "?")
    FIN=$(echo "$STATUS" | python3 -c "import sys,json; print(json.load(sys.stdin).get('last_finalized_round', '?'))" 2>/dev/null || echo "?")
    PEERS=$(echo "$STATUS" | python3 -c "import sys,json; print(json.load(sys.stdin).get('peer_count', '?'))" 2>/dev/null || echo "?")
    echo "  Node $i: round=$ROUND fin=$FIN peers=$PEERS"
done
