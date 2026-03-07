#!/bin/bash
# Parallel TPS Test for UltraDAG
# Sends transactions concurrently using background curl processes

NUM_TXS=${1:-200}
CONCURRENCY=${2:-10}
RPC_URL=${3:-"https://ultradag-node-1.fly.dev"}

SENDER_SK="3f8efc5578b787786b059127f795a9ec4736880aa38aad974387e8a3423b15bd"
RECIPIENT="ba2b4e32f49d409b913f9845276533c67297967ab169e6ad86944d0ff3830b70"
TMPDIR=$(mktemp -d)

echo "=== UltraDAG Parallel TPS Test ==="
echo "Target: $RPC_URL"
echo "Transactions: $NUM_TXS"
echo "Concurrency: $CONCURRENCY"
echo ""

# Check initial status
INIT_STATUS=$(curl -s "$RPC_URL/status")
INIT_ROUND=$(echo "$INIT_STATUS" | python3 -c "import sys,json; print(json.load(sys.stdin)['dag_round'])")
INIT_FINALIZED=$(echo "$INIT_STATUS" | python3 -c "import sys,json; print(json.load(sys.stdin)['finalized_count'])")
echo "Initial: round=$INIT_ROUND finalized=$INIT_FINALIZED"

# Check balance
BALANCE=$(curl -s "$RPC_URL/balance/5c23f1097d7887ca6dbc2455e71ef2836772cde4ae58d3d9cf4be07b7d9b3f3f" | python3 -c "import sys,json; print(json.load(sys.stdin)['balance'])")
echo "Sender balance: $((BALANCE / 100000000)) UDAG"

echo ""
echo "--- Sending $NUM_TXS transactions ($CONCURRENCY concurrent) ---"

START_TIME=$(python3 -c "import time; print(time.time())")

# Send transactions with controlled concurrency
ACTIVE=0
SENT=0
for i in $(seq 1 $NUM_TXS); do
    curl -s -o "$TMPDIR/tx_$i.json" -X POST "$RPC_URL/tx" \
        -H "Content-Type: application/json" \
        -d "{\"from_secret\":\"$SENDER_SK\",\"to\":\"$RECIPIENT\",\"amount\":100000,\"fee\":100000}" &
    ACTIVE=$((ACTIVE + 1))
    SENT=$((SENT + 1))

    if [ $ACTIVE -ge $CONCURRENCY ]; then
        wait -n 2>/dev/null || wait
        ACTIVE=$((ACTIVE - 1))
    fi

    if [ $((SENT % 50)) -eq 0 ]; then
        echo "  Queued $SENT/$NUM_TXS..."
    fi
done

# Wait for all remaining
wait

END_TIME=$(python3 -c "import time; print(time.time())")
DURATION=$(python3 -c "print(f'{$END_TIME - $START_TIME:.2f}')")

# Count results
SUCCESS=0
FAIL=0
for f in "$TMPDIR"/tx_*.json; do
    if python3 -c "import sys,json; d=json.load(open('$f')); sys.exit(0 if 'hash' in d else 1)" 2>/dev/null; then
        SUCCESS=$((SUCCESS + 1))
    else
        FAIL=$((FAIL + 1))
    fi
done

TPS=$(python3 -c "d=$END_TIME-$START_TIME; print(f'{$SUCCESS/d:.2f}' if d > 0 else 'inf')")

echo ""
echo "=== Submission Results ==="
echo "Successful: $SUCCESS / $NUM_TXS"
echo "Failed: $FAIL"
echo "Duration: ${DURATION}s"
echo "Submission TPS: $TPS tx/s"

# Wait for finalization
echo ""
echo "--- Waiting 30s for finalization ---"
sleep 30

FINAL_STATUS=$(curl -s "$RPC_URL/status")
FINAL_ROUND=$(echo "$FINAL_STATUS" | python3 -c "import sys,json; print(json.load(sys.stdin)['dag_round'])")
FINAL_FINALIZED=$(echo "$FINAL_STATUS" | python3 -c "import sys,json; print(json.load(sys.stdin)['finalized_count'])")
FINAL_MEMPOOL=$(echo "$FINAL_STATUS" | python3 -c "import sys,json; print(json.load(sys.stdin)['mempool_size'])")

ROUNDS_ELAPSED=$((FINAL_ROUND - INIT_ROUND))
FINALIZED_ELAPSED=$((FINAL_FINALIZED - INIT_FINALIZED))

echo "Final: round=$FINAL_ROUND finalized=$FINAL_FINALIZED mempool=$FINAL_MEMPOOL"
echo "Rounds elapsed: $ROUNDS_ELAPSED"
echo "New finalized: $FINALIZED_ELAPSED"
echo ""

if [ "$FINAL_MEMPOOL" -eq 0 ]; then
    echo "All transactions finalized!"
else
    echo "WARNING: $FINAL_MEMPOOL transactions still pending"
fi

# Cleanup
rm -rf "$TMPDIR"
