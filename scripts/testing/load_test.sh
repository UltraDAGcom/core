#!/bin/bash
# Load testing script for UltraDAG testnet
# Tests mempool saturation and fee market behavior

NODE_URL="https://ultradag-node-1.fly.dev"
LOG_FILE="scripts/load_test.log"

echo "=== UltraDAG Load Test Started ===" | tee "$LOG_FILE"
echo "Target: $NODE_URL" | tee -a "$LOG_FILE"
echo "Start time: $(date)" | tee -a "$LOG_FILE"
echo "" | tee -a "$LOG_FILE"

# Test 1: Fee enforcement with varying fees
echo "TEST 1: Fee Enforcement with Varying Fees" | tee -a "$LOG_FILE"
echo "==========================================" | tee -a "$LOG_FILE"

for fee in 0 5000 9999 10000 10001 20000 50000; do
    echo "Testing fee: $fee sats" | tee -a "$LOG_FILE"
    
    RESPONSE=$(curl -s -X POST "$NODE_URL/tx" \
        -H "Content-Type: application/json" \
        -d "{
            \"from_secret\": \"$(openssl rand -hex 32)\",
            \"to\": \"$(openssl rand -hex 32)\",
            \"amount\": 1000,
            \"fee\": $fee
        }" 2>/dev/null)
    
    echo "$RESPONSE" | python3 -c "
import sys, json
try:
    r = json.load(sys.stdin)
    if 'error' in r:
        if 'fee too low' in r['error']:
            print('  ✅ Correctly rejected (fee too low)')
        elif 'insufficient balance' in r['error']:
            print('  ✅ Fee validation passed (rejected due to balance)')
        else:
            print(f'  ⚠️  Error: {r[\"error\"]}')
    elif 'hash' in r:
        print(f'  ✅ Transaction accepted: {r[\"hash\"][:16]}...')
    else:
        print(f'  ❓ Unexpected response')
except Exception as e:
    print(f'  ❌ Parse error: {e}')
" | tee -a "$LOG_FILE"
done

echo "" | tee -a "$LOG_FILE"

# Test 2: Mempool status check
echo "TEST 2: Mempool Status" | tee -a "$LOG_FILE"
echo "======================" | tee -a "$LOG_FILE"

MEMPOOL=$(curl -s "$NODE_URL/mempool" 2>/dev/null)
echo "$MEMPOOL" | python3 -c "
import sys, json
try:
    m = json.load(sys.stdin)
    count = m.get('count', 0)
    print(f'  Mempool size: {count} transactions')
    if count > 0:
        print(f'  Sample transactions:')
        for tx in m.get('transactions', [])[:5]:
            print(f'    - {tx.get(\"hash\", \"unknown\")[:16]}... fee={tx.get(\"fee\", 0)} sats')
except Exception as e:
    print(f'  Parse error: {e}')
" | tee -a "$LOG_FILE"

echo "" | tee -a "$LOG_FILE"

# Test 3: Rapid transaction submission (stress test)
echo "TEST 3: Rapid Transaction Submission (100 txs)" | tee -a "$LOG_FILE"
echo "===============================================" | tee -a "$LOG_FILE"

SUCCESS=0
FAILED=0
START_TIME=$(date +%s)

for i in {1..100}; do
    # Use varying fees to test fee market
    FEE=$((10000 + RANDOM % 40000))
    
    RESPONSE=$(curl -s -X POST "$NODE_URL/tx" \
        -H "Content-Type: application/json" \
        -d "{
            \"from_secret\": \"$(openssl rand -hex 32)\",
            \"to\": \"$(openssl rand -hex 32)\",
            \"amount\": 1000,
            \"fee\": $FEE
        }" 2>/dev/null)
    
    if echo "$RESPONSE" | grep -q '"hash"'; then
        SUCCESS=$((SUCCESS + 1))
    else
        FAILED=$((FAILED + 1))
    fi
    
    # Progress indicator every 10 transactions
    if [ $((i % 10)) -eq 0 ]; then
        echo "  Progress: $i/100 (Success: $SUCCESS, Failed: $FAILED)" | tee -a "$LOG_FILE"
    fi
done

END_TIME=$(date +%s)
DURATION=$((END_TIME - START_TIME))

echo "" | tee -a "$LOG_FILE"
echo "Results:" | tee -a "$LOG_FILE"
echo "  Total submitted: 100" | tee -a "$LOG_FILE"
echo "  Successful: $SUCCESS" | tee -a "$LOG_FILE"
echo "  Failed: $FAILED" | tee -a "$LOG_FILE"
echo "  Duration: ${DURATION}s" | tee -a "$LOG_FILE"
echo "  Rate: $((100 / DURATION)) tx/s" | tee -a "$LOG_FILE"

echo "" | tee -a "$LOG_FILE"
echo "=== Load Test Complete ===" | tee -a "$LOG_FILE"
echo "End time: $(date)" | tee -a "$LOG_FILE"
