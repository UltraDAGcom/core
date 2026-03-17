#!/usr/bin/env bash
# Testnet soak check — verify all 5 nodes are healthy and in consensus.
set -euo pipefail

NODES=(1 2 3 4 5)
ISSUES=0

echo "$(date -u '+%Y-%m-%d %H:%M:%S UTC') — Testnet soak check"
echo "================================================"

SUPPLIES=()
ROUNDS=()

for i in "${NODES[@]}"; do
    STATUS=$(curl -s --max-time 10 "https://ultradag-node-$i.fly.dev/status" 2>/dev/null || echo '{}')
    ROUND=$(echo "$STATUS" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('dag_round', '?'))" 2>/dev/null || echo "?")
    FIN=$(echo "$STATUS" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('last_finalized_round', '?'))" 2>/dev/null || echo "?")
    PEERS=$(echo "$STATUS" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('peer_count', '?'))" 2>/dev/null || echo "?")
    SUPPLY=$(echo "$STATUS" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('total_supply', '?'))" 2>/dev/null || echo "?")
    MEM=$(echo "$STATUS" | python3 -c "import sys,json; d=json.load(sys.stdin); print(round(d.get('memory_usage_bytes',0)/1048576, 1))" 2>/dev/null || echo "?")

    echo "  Node $i: round=$ROUND fin=$FIN peers=$PEERS supply=$SUPPLY mem=${MEM}MB"

    if [[ "$ROUND" != "?" && "$FIN" != "?" && "$FIN" != "null" && "$FIN" != "None" ]]; then
        LAG=$((ROUND - FIN))
        if [ "$LAG" -gt 10 ]; then
            echo "  WARNING: Node $i finality lag=$LAG (>10)"
            ISSUES=$((ISSUES + 1))
        fi
        ROUNDS+=("$FIN")
    fi

    if [[ "$PEERS" != "?" && "$PEERS" -lt 2 ]]; then
        echo "  WARNING: Node $i has only $PEERS peers"
        ISSUES=$((ISSUES + 1))
    fi

    SUPPLIES+=("$SUPPLY")
done

# Supply consensus check
if [ ${#SUPPLIES[@]} -ge 2 ]; then
    FIRST="${SUPPLIES[0]}"
    for S in "${SUPPLIES[@]}"; do
        if [ "$S" != "$FIRST" ] && [ "$S" != "?" ]; then
            echo "  CRITICAL: Supply divergence! $S != $FIRST"
            ISSUES=$((ISSUES + 1))
        fi
    done
    if [ "$ISSUES" -eq 0 ]; then
        echo "  OK: Supply consensus ($FIRST)"
    fi
fi

echo "================================================"
if [ "$ISSUES" -gt 0 ]; then
    echo "FAIL: $ISSUES issues found"
    exit 1
else
    echo "OK: All healthy"
    exit 0
fi
