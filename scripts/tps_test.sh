#!/bin/bash
# TPS Test for UltraDAG testnet on Fly.io
# Usage: ./scripts/tps_test.sh [NUM_TXS] [RPC_URL]

NUM_TXS=${1:-100}
RPC_URL=${2:-"https://ultradag-node-1.fly.dev"}

SENDER_SK="3f8efc5578b787786b059127f795a9ec4736880aa38aad974387e8a3423b15bd"
RECIPIENT="ba2b4e32f49d409b913f9845276533c67297967ab169e6ad86944d0ff3830b70"

echo "=== UltraDAG TPS Test ==="
echo "Target: $RPC_URL"
echo "Transactions: $NUM_TXS"
echo ""

# Check initial status
echo "--- Initial Status ---"
curl -s "$RPC_URL/status" | python3 -c "import sys,json; d=json.load(sys.stdin); print(f'Round: {d[\"dag_round\"]}, Finalized: {d[\"finalized_count\"]}, Supply: {d[\"total_supply\"]/100000000:.2f} UDAG')"

# Check sender balance
BALANCE=$(curl -s "$RPC_URL/balance/5c23f1097d7887ca6dbc2455e71ef2836772cde4ae58d3d9cf4be07b7d9b3f3f" | python3 -c "import sys,json; print(json.load(sys.stdin)['balance'])")
echo "Sender balance: $((BALANCE / 100000000)) UDAG ($BALANCE sats)"

if [ "$BALANCE" -lt "$((NUM_TXS * 200000))" ]; then
    echo "Insufficient balance, funding via faucet..."
    curl -s -X POST "$RPC_URL/faucet" \
        -H "Content-Type: application/json" \
        -d "{\"address\":\"5c23f1097d7887ca6dbc2455e71ef2836772cde4ae58d3d9cf4be07b7d9b3f3f\",\"amount\":$((NUM_TXS * 200000))}" > /dev/null
fi

echo ""
echo "--- Sending $NUM_TXS transactions ---"

START_TIME=$(python3 -c "import time; print(time.time())")
SUCCESS=0
FAIL=0

for i in $(seq 1 $NUM_TXS); do
    RESULT=$(curl -s -X POST "$RPC_URL/tx" \
        -H "Content-Type: application/json" \
        -d "{\"from_secret\":\"$SENDER_SK\",\"to\":\"$RECIPIENT\",\"amount\":100000,\"fee\":100000}" 2>&1)

    if echo "$RESULT" | python3 -c "import sys,json; d=json.load(sys.stdin); sys.exit(0 if 'hash' in d else 1)" 2>/dev/null; then
        SUCCESS=$((SUCCESS + 1))
    else
        FAIL=$((FAIL + 1))
        if [ "$FAIL" -le 3 ]; then
            echo "  TX $i failed: $RESULT"
        fi
    fi

    # Progress every 10 txs
    if [ $((i % 10)) -eq 0 ]; then
        echo "  Sent $i/$NUM_TXS (ok=$SUCCESS, fail=$FAIL)"
    fi
done

END_TIME=$(python3 -c "import time; print(time.time())")
DURATION=$(python3 -c "print(f'{$END_TIME - $START_TIME:.2f}')")
TPS=$(python3 -c "d=$END_TIME-$START_TIME; print(f'{$SUCCESS/d:.2f}' if d > 0 else 'inf')")

echo ""
echo "=== Results ==="
echo "Successful: $SUCCESS / $NUM_TXS"
echo "Failed: $FAIL"
echo "Duration: ${DURATION}s"
echo "Submission TPS: $TPS tx/s"
echo ""

# Wait for finalization
echo "--- Waiting 15s for finalization ---"
sleep 15

echo "--- Final Status ---"
curl -s "$RPC_URL/status" | python3 -c "import sys,json; d=json.load(sys.stdin); print(f'Round: {d[\"dag_round\"]}, Finalized: {d[\"finalized_count\"]}, Supply: {d[\"total_supply\"]/100000000:.2f} UDAG, Mempool: {d[\"mempool_size\"]}')"

# Check recipient balance
RECV_BAL=$(curl -s "$RPC_URL/balance/$RECIPIENT" | python3 -c "import sys,json; print(json.load(sys.stdin)['balance'])")
echo "Recipient received: $((RECV_BAL / 100000)) sats ($((RECV_BAL / 100000000)) UDAG)"
