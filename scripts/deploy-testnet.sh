#!/usr/bin/env bash
# Deploy and restart all 4 testnet nodes simultaneously.
#
# Usage:
#   ./scripts/deploy-testnet.sh              # Build + deploy new code to all 4 nodes
#   ./scripts/deploy-testnet.sh --clean      # Same but wipes state on all nodes
#   ./scripts/deploy-testnet.sh --restart    # Just restart (no rebuild)
#
# TOML files live in deployments/fly/fly-node-{1,2,3,4}.toml.
# --clean temporarily sets CLEAN_STATE=true in the TOML env, deploys,
# then reverts it. This is more reliable than fly secrets --stage.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
TOML_DIR="$PROJECT_ROOT/deployments/fly"

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

# --clean: uncomment CLEAN_STATE in all TOML files
if $CLEAN; then
    echo "==> Enabling CLEAN_STATE in TOML files..."
    for i in 1 2 3 4; do
        sed -i '' 's/^  # CLEAN_STATE = "true"/  CLEAN_STATE = "true"/' "$TOML_DIR/fly-node-$i.toml"
    done
fi

if ! $RESTART_ONLY; then
    echo "==> Building and deploying new code to all nodes..."
    for i in 1 2 3 4; do
        echo "  Deploying node $i..."
        fly deploy -a "ultradag-node-$i" -c "$TOML_DIR/fly-node-$i.toml" --remote-only 2>&1 | grep -E "succeeded|Visit|Error" || true
    done
    echo "    All nodes deployed."
fi

# --clean or --restart: restart all machines simultaneously
if $RESTART_ONLY || $CLEAN; then
    echo "==> Restarting all machines simultaneously..."
    for node in "${NODES[@]}"; do
        MACHINE_ID=$(fly machines list -a "$node" --json 2>/dev/null | python3 -c "import sys,json; print(json.load(sys.stdin)[0]['id'])")
        echo "  Restarting $node ($MACHINE_ID)..."
        fly machine restart "$MACHINE_ID" -a "$node" &
    done
    wait
    echo "    All machines restarted."
fi

# --clean: revert CLEAN_STATE in TOML files so normal deploys don't wipe state
if $CLEAN; then
    echo "==> Reverting CLEAN_STATE in TOML files..."
    for i in 1 2 3 4; do
        sed -i '' 's/^  CLEAN_STATE = "true"/  # CLEAN_STATE = "true"/' "$TOML_DIR/fly-node-$i.toml"
    done
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
