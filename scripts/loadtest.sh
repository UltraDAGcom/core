#!/bin/bash

# UltraDAG Load Test Script
# Tests transaction throughput with concurrent requests

NODE_URL="http://127.0.0.1:8001"
VALIDATOR_SECRET="YOUR_SECRET_KEY_HERE"  # Replace with actual validator secret
RECEIVER="0000000000000000000000000000000000000000000000000000000000000001"

NUM_TXS=500
CONCURRENT=20
AMOUNT=1000
FEE=100

echo "=== UltraDAG Load Test ==="
echo "Node: $NODE_URL"
echo "Transactions: $NUM_TXS"
echo "Concurrent requests: $CONCURRENT"
echo ""

# Check initial status
echo "=== Initial Network Status ==="
curl -s "$NODE_URL/status" | jq
echo ""

START_TIME=$(date +%s)

echo "=== Sending $NUM_TXS transactions with $CONCURRENT concurrent requests ==="

# Function to send a transaction
send_tx() {
    local nonce=$1
    curl -s -X POST "$NODE_URL/tx" \
        -H "Content-Type: application/json" \
        -d "{\"from_secret\":\"$VALIDATOR_SECRET\",\"to\":\"$RECEIVER\",\"amount\":$AMOUNT,\"fee\":$FEE}" \
        > /dev/null 2>&1
    
    if [ $? -eq 0 ]; then
        echo -n "."
    else
        echo -n "x"
    fi
}

# Send transactions in batches
sent=0
while [ $sent -lt $NUM_TXS ]; do
    batch_size=$CONCURRENT
    if [ $((sent + batch_size)) -gt $NUM_TXS ]; then
        batch_size=$((NUM_TXS - sent))
    fi
    
    # Launch batch
    for i in $(seq 1 $batch_size); do
        send_tx $((sent + i)) &
    done
    
    # Wait for batch to complete
    wait
    
    sent=$((sent + batch_size))
    
    # Progress update every 100 txs
    if [ $((sent % 100)) -eq 0 ]; then
        echo " [$sent/$NUM_TXS]"
    fi
done

echo ""
END_TIME=$(date +%s)
DURATION=$((END_TIME - START_TIME))

echo ""
echo "=== Results ==="
echo "Total time: ${DURATION}s"
echo "Throughput: $(echo "scale=2; $NUM_TXS / $DURATION" | bc) TPS"
echo ""

# Wait for processing
echo "=== Waiting 10 seconds for transaction processing ==="
sleep 10

# Check final status
echo ""
echo "=== Final Network Status ==="
curl -s "$NODE_URL/status" | jq
echo ""

# Check mempool
echo "=== Mempool Status ==="
MEMPOOL_SIZE=$(curl -s "$NODE_URL/status" | jq '.mempool_size')
echo "Transactions in mempool: $MEMPOOL_SIZE"
echo ""

echo "=== Load test complete ==="
