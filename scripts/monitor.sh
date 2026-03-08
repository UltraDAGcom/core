#!/bin/bash
# monitor.sh — run this overnight
while true; do
  TIMESTAMP=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
  for n in 1 2 3 4; do
    STATUS=$(curl -s --max-time 5 "https://ultradag-node-$n.fly.dev/status")
    ROUND=$(echo $STATUS | python3 -c "import sys,json; d=json.load(sys.stdin); print(d['dag_round'])" 2>/dev/null || echo "UNREACHABLE")
    FIN=$(echo $STATUS | python3 -c "import sys,json; d=json.load(sys.stdin); print(d['last_finalized_round'])" 2>/dev/null || echo "UNREACHABLE")
    LAG=$((ROUND - FIN))
    SUPPLY=$(echo $STATUS | python3 -c "import sys,json; d=json.load(sys.stdin); print(d['total_supply'])" 2>/dev/null || echo "0")
    echo "$TIMESTAMP node-$n round=$ROUND fin=$FIN lag=$LAG supply=$SUPPLY"
  done
  echo "---"
  sleep 60
done >> testnet_monitor.log 2>&1