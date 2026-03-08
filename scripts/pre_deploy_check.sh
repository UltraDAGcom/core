#!/bin/bash
# Pre-deployment safety check for UltraDAG
# Run this before EVERY deployment to mainnet

set -e  # Exit on any error

echo "🔍 UltraDAG Pre-Deployment Safety Check"
echo "========================================"
echo ""

# Configuration
NETWORK=${NETWORK:-"testnet"}
NETWORK_URL=${NETWORK_URL:-"https://ultradag-node-1.fly.dev"}
DATA_DIR=${DATA_DIR:-"$HOME/.ultradag/node"}
BINARY=${BINARY:-"target/release/ultradag-node"}

echo "Configuration:"
echo "  Network: $NETWORK"
echo "  Network URL: $NETWORK_URL"
echo "  Data directory: $DATA_DIR"
echo "  Binary: $BINARY"
echo ""

# 1. Verify binary exists
echo "📦 Checking binary..."
if [ ! -f "$BINARY" ]; then
    echo "❌ Binary not found at: $BINARY"
    echo "Run: cargo build --release"
    exit 1
fi
echo "✅ Binary found"

# 2. Check binary size (should be <5MB)
if [[ "$OSTYPE" == "darwin"* ]]; then
    SIZE=$(stat -f%z "$BINARY" 2>/dev/null)
else
    SIZE=$(stat -c%s "$BINARY" 2>/dev/null)
fi
SIZE_MB=$((SIZE / 1024 / 1024))
echo "✅ Binary size: ${SIZE_MB}MB"

if [ $SIZE_MB -gt 5 ]; then
    echo "⚠️  Binary is larger than expected (${SIZE_MB}MB > 5MB)"
fi

# 3. Run all tests
echo ""
echo "🧪 Running test suite..."
if cargo test --release --workspace --quiet 2>&1 | grep -q "test result: ok"; then
    TEST_COUNT=$(cargo test --release --workspace --quiet 2>&1 | grep "test result" | grep -oP '\d+(?= passed)' | head -1)
    echo "✅ All tests passed ($TEST_COUNT tests)"
else
    echo "❌ Tests failed"
    exit 1
fi

# 4. Query live network for current round
if [ -n "$NETWORK_URL" ]; then
    echo ""
    echo "🌐 Querying live network..."
    
    NETWORK_STATUS=$(curl -s --max-time 10 "$NETWORK_URL/status" 2>/dev/null)
    
    if [ -n "$NETWORK_STATUS" ]; then
        NETWORK_ROUND=$(echo "$NETWORK_STATUS" | jq -r '.dag_round' 2>/dev/null)
        NETWORK_SUPPLY=$(echo "$NETWORK_STATUS" | jq -r '.total_supply' 2>/dev/null)
        
        if [ -n "$NETWORK_ROUND" ] && [ "$NETWORK_ROUND" != "null" ]; then
            echo "✅ Network is at round: $NETWORK_ROUND"
            echo "   Network supply: $NETWORK_SUPPLY sats"
            
            # 5. Check local state if exists
            if [ -d "$DATA_DIR" ]; then
                # Check high-water mark
                HWM_FILE="$DATA_DIR/high_water_mark.json"
                if [ -f "$HWM_FILE" ]; then
                    HWM_ROUND=$(jq -r '.max_round' "$HWM_FILE" 2>/dev/null)
                    if [ -n "$HWM_ROUND" ] && [ "$HWM_ROUND" != "null" ]; then
                        echo "   Local high-water mark: round $HWM_ROUND"
                        
                        # Calculate lag
                        LAG=$((NETWORK_ROUND - HWM_ROUND))
                        
                        if [ $LAG -lt 0 ]; then
                            echo "❌ ERROR: Local state is AHEAD of network!"
                            echo "   This should never happen. Possible clock skew or network issue."
                            exit 1
                        elif [ $LAG -gt 1000 ]; then
                            echo "❌ ERROR: Local state is $LAG rounds behind network!"
                            echo "   This would cause a rollback. Aborting deployment."
                            echo ""
                            echo "   Options:"
                            echo "   1. Fast-sync from network before deploying"
                            echo "   2. Start with fresh state (delete $DATA_DIR)"
                            exit 1
                        elif [ $LAG -gt 100 ]; then
                            echo "⚠️  WARNING: Local state is $LAG rounds behind"
                            echo "   Consider syncing before deployment"
                        else
                            echo "✅ State lag is acceptable: $LAG rounds"
                        fi
                    fi
                else
                    echo "   No high-water mark found (fresh start)"
                fi
                
                # Check DAG state
                DAG_FILE="$DATA_DIR/dag.json"
                if [ -f "$DAG_FILE" ]; then
                    DAG_ROUND=$(jq -r '.current_round' "$DAG_FILE" 2>/dev/null)
                    if [ -n "$DAG_ROUND" ] && [ "$DAG_ROUND" != "null" ]; then
                        echo "   Local DAG round: $DAG_ROUND"
                    fi
                fi
            else
                echo "   No local state directory (fresh start)"
            fi
        else
            echo "⚠️  Could not parse network round"
        fi
    else
        echo "⚠️  Could not reach network at $NETWORK_URL"
        if [ "$NETWORK" = "mainnet" ]; then
            echo "❌ Cannot deploy to mainnet without verifying network state"
            exit 1
        fi
    fi
fi

# 6. Check for uncommitted changes
echo ""
echo "📝 Checking git status..."
if [ -n "$(git status --porcelain 2>/dev/null)" ]; then
    echo "⚠️  Uncommitted changes detected:"
    git status --short
    echo ""
    read -p "Continue anyway? (y/N) " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        echo "Deployment cancelled"
        exit 1
    fi
else
    echo "✅ No uncommitted changes"
fi

# 7. Verify deployment target
echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "Deployment Summary:"
echo "  Network: $NETWORK"
echo "  Binary size: ${SIZE_MB}MB"
echo "  Tests: ✅ All passing"
if [ -n "$NETWORK_ROUND" ]; then
    echo "  Network round: $NETWORK_ROUND"
    if [ -n "$HWM_ROUND" ]; then
        echo "  Local round: $HWM_ROUND"
        echo "  Lag: $LAG rounds"
    fi
fi
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

if [ "$NETWORK" = "mainnet" ]; then
    echo "⚠️  MAINNET DEPLOYMENT - PROCEED WITH CAUTION"
    echo ""
fi

read -p "Proceed with deployment? (y/N) " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    echo "Deployment cancelled"
    exit 1
fi

echo ""
echo "✅ All safety checks passed"
echo "Safe to deploy"
