#!/usr/bin/env bash
# Deploy and restart all 5 MAINNET nodes simultaneously.
#
# Usage:
#   ./tools/operations/deployment/fly/deploy-mainnet.sh              # Build + deploy new code to all 5 mainnet nodes
#   ./tools/operations/deployment/fly/deploy-mainnet.sh --clean      # Same but wipes state on all nodes (hard fork!)
#   ./tools/operations/deployment/fly/deploy-mainnet.sh --restart    # Just restart (no rebuild)
#
# TOML files live in tools/operations/deployment/fly/fly-mainnet-{1,2,3,4,5}.toml.
#
# ⚠️  --clean on mainnet wipes ALL balances, stakes, proposals, and finalized
# rounds across every validator. This is the right flag for a coordinated
# hard-fork restart (new tokenomics, new GENESIS_CHECKPOINT_HASH, etc.). It is
# the WRONG flag for any other operational reason — there is no recovery path.
# The script prompts for explicit confirmation before wiping.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../../../.." && pwd)"
TOML_DIR="$SCRIPT_DIR"

NODES=(ultradag-mainnet-1 ultradag-mainnet-2 ultradag-mainnet-3 ultradag-mainnet-4 ultradag-mainnet-5)
CLEAN=false
RESTART_ONLY=false

for arg in "$@"; do
    case "$arg" in
        --clean) CLEAN=true ;;
        --restart) RESTART_ONLY=true ;;
        *) echo "Unknown arg: $arg"; exit 1 ;;
    esac
done

# Safety prompt for --clean on mainnet.
if $CLEAN; then
    echo ""
    echo "⚠️  ⚠️  ⚠️   MAINNET HARD-FORK RESTART   ⚠️  ⚠️  ⚠️"
    echo ""
    echo "You are about to WIPE STATE on all 5 mainnet nodes:"
    for n in "${NODES[@]}"; do echo "    - $n"; done
    echo ""
    echo "All balances, stakes, delegations, proposals, and finalized rounds"
    echo "will be lost. The network will restart from the new genesis hash"
    echo "baked into the build."
    echo ""
    echo "Current mainnet GENESIS_CHECKPOINT_HASH (from constants.rs):"
    grep -A 4 'cfg(feature = "mainnet")' "$PROJECT_ROOT/crates/ultradag-coin/src/constants.rs" \
        | grep -A 4 "GENESIS_CHECKPOINT_HASH" | tail -4 || true
    echo ""
    read -r -p "Type 'HARD FORK MAINNET' to continue: " confirm
    if [ "$confirm" != "HARD FORK MAINNET" ]; then
        echo "Aborted."
        exit 1
    fi
fi

# --clean: uncomment CLEAN_STATE in all TOML files
if $CLEAN; then
    echo "==> Enabling CLEAN_STATE in TOML files..."
    for i in 1 2 3 4 5; do
        sed -i '' 's/^  # CLEAN_STATE = "true"/  CLEAN_STATE = "true"/' "$TOML_DIR/fly-mainnet-$i.toml"
    done

    # Stop all machines BEFORE deploying to prevent new nodes from syncing stale
    # data from old instances that are still running during sequential deploy.
    echo "==> Stopping all machines before clean deploy..."
    for node in "${NODES[@]}"; do
        MACHINE_ID=$(fly machines list -a "$node" --json 2>/dev/null | python3 -c "import sys,json; print(json.load(sys.stdin)[0]['id'])" 2>/dev/null || echo "")
        if [ -n "$MACHINE_ID" ]; then
            echo "  Stopping $node ($MACHINE_ID)..."
            fly machine stop "$MACHINE_ID" -a "$node" 2>/dev/null &
        fi
    done
    wait
    sleep 5
    echo "    All machines stopped."
fi

if ! $RESTART_ONLY; then
    echo "==> Building and deploying new code to all mainnet nodes..."
    CACHEBUST=$(git -C "$PROJECT_ROOT" rev-parse --short HEAD 2>/dev/null || date +%s)
    for i in 1 2 3 4 5; do
        echo "  Deploying ultradag-mainnet-$i..."
        fly deploy -a "ultradag-mainnet-$i" -c "$TOML_DIR/fly-mainnet-$i.toml" --remote-only --build-arg "CACHEBUST=$CACHEBUST" 2>&1 | grep -E "succeeded|Visit|Error" || true
    done
    echo "    All mainnet nodes deployed."

    # fly deploy starts machines sequentially, causing staggered startup.
    # Stop all machines after deploy so the restart step starts them simultaneously.
    if $CLEAN; then
        echo "==> Stopping all machines after deploy (prevent staggered state)..."
        for node in "${NODES[@]}"; do
            MACHINE_ID=$(fly machines list -a "$node" --json 2>/dev/null | python3 -c "import sys,json; print(json.load(sys.stdin)[0]['id'])" 2>/dev/null || echo "")
            if [ -n "$MACHINE_ID" ]; then
                fly machine stop "$MACHINE_ID" -a "$node" 2>/dev/null &
            fi
        done
        wait
        sleep 3
        echo "    All machines stopped."
    fi
fi

# --clean or --restart: restart all machines simultaneously
if $RESTART_ONLY || $CLEAN; then
    echo "==> Restarting all mainnet machines simultaneously..."
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
    for i in 1 2 3 4 5; do
        sed -i '' 's/^  CLEAN_STATE = "true"/  # CLEAN_STATE = "true"/' "$TOML_DIR/fly-mainnet-$i.toml"
    done
fi

echo ""
echo "==> Waiting 30s for nodes to connect and start producing..."
sleep 30

echo "==> Checking health..."
for i in 1 2 3 4 5; do
    STATUS=$(curl -s --max-time 5 "https://ultradag-mainnet-$i.fly.dev/status" 2>/dev/null || echo '{}')
    ROUND=$(echo "$STATUS" | python3 -c "import sys,json; print(json.load(sys.stdin).get('dag_round', '?'))" 2>/dev/null || echo "?")
    FIN=$(echo "$STATUS" | python3 -c "import sys,json; print(json.load(sys.stdin).get('last_finalized_round', '?'))" 2>/dev/null || echo "?")
    PEERS=$(echo "$STATUS" | python3 -c "import sys,json; print(json.load(sys.stdin).get('peer_count', '?'))" 2>/dev/null || echo "?")
    echo "  ultradag-mainnet-$i: round=$ROUND fin=$FIN peers=$PEERS"
done
