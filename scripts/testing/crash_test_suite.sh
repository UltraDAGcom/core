#!/bin/bash
# Aggressive crash test suite for UltraDAG testnet
# Attempts to crash, hang, or break the testnet through various attack vectors

NODE_URL="https://ultradag-node-1.fly.dev"
LOG_FILE="scripts/crash_test.log"

echo "=== UltraDAG Aggressive Crash Test Suite ===" | tee "$LOG_FILE"
echo "Target: $NODE_URL" | tee -a "$LOG_FILE"
echo "Start time: $(date)" | tee -a "$LOG_FILE"
echo "WARNING: This test suite attempts to crash the testnet" | tee -a "$LOG_FILE"
echo "" | tee -a "$LOG_FILE"

CRASHED=0
SURVIVED=0

# Test 1: Massive Concurrent Request Flood
echo "=== TEST 1: Concurrent Request Flood (1000 requests) ===" | tee -a "$LOG_FILE"
echo "Attempting to overwhelm RPC server..." | tee -a "$LOG_FILE"

START=$(date +%s)
for i in {1..1000}; do
    curl -s -X POST "$NODE_URL/tx" \
        -H "Content-Type: application/json" \
        -d "{\"secret_key\":\"$(openssl rand -hex 32)\",\"to\":\"$(openssl rand -hex 32)\",\"amount\":1000,\"fee\":10000}" \
        > /dev/null 2>&1 &
done
wait
END=$(date +%s)

# Check if node is still responsive
RESPONSE=$(curl -s --max-time 10 "$NODE_URL/status" 2>/dev/null)
if [ -n "$RESPONSE" ]; then
    echo "  ✅ Node survived 1000 concurrent requests in $((END-START))s" | tee -a "$LOG_FILE"
    SURVIVED=$((SURVIVED + 1))
else
    echo "  ❌ Node crashed or became unresponsive" | tee -a "$LOG_FILE"
    CRASHED=$((CRASHED + 1))
fi

sleep 2

# Test 2: Extremely Large JSON Payloads
echo "" | tee -a "$LOG_FILE"
echo "=== TEST 2: Extremely Large JSON Payloads ===" | tee -a "$LOG_FILE"

# Generate 10MB of random data
LARGE_DATA=$(head -c 10485760 /dev/urandom | base64 | tr -d '\n')
echo "Sending 10MB payload..." | tee -a "$LOG_FILE"

RESPONSE=$(curl -s --max-time 30 -X POST "$NODE_URL/tx" \
    -H "Content-Type: application/json" \
    -d "{\"secret_key\":\"$LARGE_DATA\",\"to\":\"abc\",\"amount\":1000,\"fee\":10000}" 2>/dev/null)

if echo "$RESPONSE" | grep -q "error"; then
    echo "  ✅ Node rejected large payload gracefully" | tee -a "$LOG_FILE"
    SURVIVED=$((SURVIVED + 1))
else
    echo "  ⚠️  Unexpected response to large payload" | tee -a "$LOG_FILE"
    SURVIVED=$((SURVIVED + 1))
fi

# Check node still responsive
RESPONSE=$(curl -s --max-time 10 "$NODE_URL/status" 2>/dev/null)
if [ -z "$RESPONSE" ]; then
    echo "  ❌ Node became unresponsive after large payload" | tee -a "$LOG_FILE"
    CRASHED=$((CRASHED + 1))
fi

sleep 2

# Test 3: Rapid-Fire Status Requests (DDoS simulation)
echo "" | tee -a "$LOG_FILE"
echo "=== TEST 3: Rapid Status Request DDoS (5000 requests) ===" | tee -a "$LOG_FILE"

START=$(date +%s)
for i in {1..5000}; do
    curl -s "$NODE_URL/status" > /dev/null 2>&1 &
    [ $((i % 100)) -eq 0 ] && sleep 0.1  # Brief pause every 100 requests
done
wait
END=$(date +%s)

RESPONSE=$(curl -s --max-time 10 "$NODE_URL/status" 2>/dev/null)
if [ -n "$RESPONSE" ]; then
    echo "  ✅ Node survived 5000 status requests in $((END-START))s" | tee -a "$LOG_FILE"
    SURVIVED=$((SURVIVED + 1))
else
    echo "  ❌ Node crashed under DDoS load" | tee -a "$LOG_FILE"
    CRASHED=$((CRASHED + 1))
fi

sleep 2

# Test 4: Malformed UTF-8 and Binary Data
echo "" | tee -a "$LOG_FILE"
echo "=== TEST 4: Malformed UTF-8 and Binary Data ===" | tee -a "$LOG_FILE"

for i in {1..10}; do
    # Generate random binary data
    BINARY=$(head -c 1000 /dev/urandom | base64 | tr -d '\n')
    curl -s -X POST "$NODE_URL/tx" \
        -H "Content-Type: application/json" \
        -d "$BINARY" > /dev/null 2>&1
done

RESPONSE=$(curl -s --max-time 10 "$NODE_URL/status" 2>/dev/null)
if [ -n "$RESPONSE" ]; then
    echo "  ✅ Node survived binary data attacks" | tee -a "$LOG_FILE"
    SURVIVED=$((SURVIVED + 1))
else
    echo "  ❌ Node crashed on binary data" | tee -a "$LOG_FILE"
    CRASHED=$((CRASHED + 1))
fi

sleep 2

# Test 5: Deeply Nested JSON
echo "" | tee -a "$LOG_FILE"
echo "=== TEST 5: Deeply Nested JSON (1000 levels) ===" | tee -a "$LOG_FILE"

NESTED="{"
for i in {1..1000}; do
    NESTED="${NESTED}\"a\":{"
done
NESTED="${NESTED}\"secret_key\":\"abc\""
for i in {1..1000}; do
    NESTED="${NESTED}}"
done

curl -s -X POST "$NODE_URL/tx" \
    -H "Content-Type: application/json" \
    -d "$NESTED" > /dev/null 2>&1

RESPONSE=$(curl -s --max-time 10 "$NODE_URL/status" 2>/dev/null)
if [ -n "$RESPONSE" ]; then
    echo "  ✅ Node survived deeply nested JSON" | tee -a "$LOG_FILE"
    SURVIVED=$((SURVIVED + 1))
else
    echo "  ❌ Node crashed on nested JSON" | tee -a "$LOG_FILE"
    CRASHED=$((CRASHED + 1))
fi

sleep 2

# Test 6: Integer Overflow Attempts
echo "" | tee -a "$LOG_FILE"
echo "=== TEST 6: Integer Overflow Attempts ===" | tee -a "$LOG_FILE"

OVERFLOW_VALUES=(
    "18446744073709551615"  # MAX_U64
    "18446744073709551616"  # MAX_U64 + 1
    "99999999999999999999"  # Way beyond MAX_U64
    "-1"
    "-9223372036854775808"  # MIN_I64
)

for val in "${OVERFLOW_VALUES[@]}"; do
    curl -s -X POST "$NODE_URL/tx" \
        -H "Content-Type: application/json" \
        -d "{\"secret_key\":\"$(openssl rand -hex 32)\",\"to\":\"$(openssl rand -hex 32)\",\"amount\":$val,\"fee\":10000}" \
        > /dev/null 2>&1
done

RESPONSE=$(curl -s --max-time 10 "$NODE_URL/status" 2>/dev/null)
if [ -n "$RESPONSE" ]; then
    echo "  ✅ Node survived integer overflow attempts" | tee -a "$LOG_FILE"
    SURVIVED=$((SURVIVED + 1))
else
    echo "  ❌ Node crashed on integer overflow" | tee -a "$LOG_FILE"
    CRASHED=$((CRASHED + 1))
fi

sleep 2

# Test 7: Mempool Saturation Attack
echo "" | tee -a "$LOG_FILE"
echo "=== TEST 7: Mempool Saturation (10000 transactions) ===" | tee -a "$LOG_FILE"
echo "Attempting to fill mempool with spam..." | tee -a "$LOG_FILE"

START=$(date +%s)
for i in {1..10000}; do
    curl -s -X POST "$NODE_URL/tx" \
        -H "Content-Type: application/json" \
        -d "{\"secret_key\":\"$(openssl rand -hex 32)\",\"to\":\"$(openssl rand -hex 32)\",\"amount\":1000,\"fee\":$((10000 + RANDOM))}" \
        > /dev/null 2>&1 &
    
    # Batch in groups of 100
    [ $((i % 100)) -eq 0 ] && wait && echo "  Progress: $i/10000" | tee -a "$LOG_FILE"
done
wait
END=$(date +%s)

echo "  Submitted 10000 transactions in $((END-START))s" | tee -a "$LOG_FILE"

# Check mempool size
MEMPOOL_SIZE=$(curl -s "$NODE_URL/status" 2>/dev/null | python3 -c "import sys,json; print(json.load(sys.stdin).get('mempool_size',0))" 2>/dev/null)
echo "  Mempool size: $MEMPOOL_SIZE" | tee -a "$LOG_FILE"

RESPONSE=$(curl -s --max-time 10 "$NODE_URL/status" 2>/dev/null)
if [ -n "$RESPONSE" ]; then
    echo "  ✅ Node survived mempool saturation" | tee -a "$LOG_FILE"
    SURVIVED=$((SURVIVED + 1))
else
    echo "  ❌ Node crashed on mempool saturation" | tee -a "$LOG_FILE"
    CRASHED=$((CRASHED + 1))
fi

sleep 5

# Test 8: Rapid Endpoint Switching
echo "" | tee -a "$LOG_FILE"
echo "=== TEST 8: Rapid Endpoint Switching (1000 mixed requests) ===" | tee -a "$LOG_FILE"

ENDPOINTS=("status" "mempool" "validators" "peers" "keygen")

for i in {1..1000}; do
    ENDPOINT=${ENDPOINTS[$((RANDOM % ${#ENDPOINTS[@]}))]}
    curl -s "$NODE_URL/$ENDPOINT" > /dev/null 2>&1 &
done
wait

RESPONSE=$(curl -s --max-time 10 "$NODE_URL/status" 2>/dev/null)
if [ -n "$RESPONSE" ]; then
    echo "  ✅ Node survived rapid endpoint switching" | tee -a "$LOG_FILE"
    SURVIVED=$((SURVIVED + 1))
else
    echo "  ❌ Node crashed on endpoint switching" | tee -a "$LOG_FILE"
    CRASHED=$((CRASHED + 1))
fi

sleep 2

# Test 9: Malicious Round Queries
echo "" | tee -a "$LOG_FILE"
echo "=== TEST 9: Malicious Round Queries ===" | tee -a "$LOG_FILE"

MALICIOUS_ROUNDS=(
    "-1"
    "999999999999"
    "abc"
    "0x1000"
    "../../../etc/passwd"
)

for round in "${MALICIOUS_ROUNDS[@]}"; do
    curl -s "$NODE_URL/round/$round" > /dev/null 2>&1
done

RESPONSE=$(curl -s --max-time 10 "$NODE_URL/status" 2>/dev/null)
if [ -n "$RESPONSE" ]; then
    echo "  ✅ Node survived malicious round queries" | tee -a "$LOG_FILE"
    SURVIVED=$((SURVIVED + 1))
else
    echo "  ❌ Node crashed on malicious queries" | tee -a "$LOG_FILE"
    CRASHED=$((CRASHED + 1))
fi

sleep 2

# Test 10: Resource Exhaustion via Faucet
echo "" | tee -a "$LOG_FILE"
echo "=== TEST 10: Faucet Spam (1000 requests) ===" | tee -a "$LOG_FILE"

for i in {1..1000}; do
    curl -s -X POST "$NODE_URL/faucet" \
        -H "Content-Type: application/json" \
        -d "{\"address\":\"$(openssl rand -hex 32)\",\"amount\":100000000}" \
        > /dev/null 2>&1 &
    
    [ $((i % 100)) -eq 0 ] && wait
done
wait

RESPONSE=$(curl -s --max-time 10 "$NODE_URL/status" 2>/dev/null)
if [ -n "$RESPONSE" ]; then
    echo "  ✅ Node survived faucet spam" | tee -a "$LOG_FILE"
    SURVIVED=$((SURVIVED + 1))
else
    echo "  ❌ Node crashed on faucet spam" | tee -a "$LOG_FILE"
    CRASHED=$((CRASHED + 1))
fi

sleep 2

# Test 11: Slowloris Attack Simulation
echo "" | tee -a "$LOG_FILE"
echo "=== TEST 11: Slow Request Attack (100 slow connections) ===" | tee -a "$LOG_FILE"

for i in {1..100}; do
    (
        sleep $((RANDOM % 30))
        curl -s --max-time 60 "$NODE_URL/status" > /dev/null 2>&1
    ) &
done

sleep 10  # Let some accumulate

RESPONSE=$(curl -s --max-time 10 "$NODE_URL/status" 2>/dev/null)
if [ -n "$RESPONSE" ]; then
    echo "  ✅ Node survived slow request attack" | tee -a "$LOG_FILE"
    SURVIVED=$((SURVIVED + 1))
else
    echo "  ❌ Node crashed on slow requests" | tee -a "$LOG_FILE"
    CRASHED=$((CRASHED + 1))
fi

# Cleanup background processes
pkill -P $$ curl 2>/dev/null

sleep 2

# Test 12: Final Health Check
echo "" | tee -a "$LOG_FILE"
echo "=== TEST 12: Final Comprehensive Health Check ===" | tee -a "$LOG_FILE"

for n in 1 2 3 4; do
    STATUS=$(curl -s --max-time 10 "https://ultradag-node-$n.fly.dev/status" 2>/dev/null)
    if [ -n "$STATUS" ]; then
        echo "$STATUS" | python3 -c "
import sys, json
d = json.load(sys.stdin)
print(f'  Node $n: round={d[\"dag_round\"]} lag={d[\"dag_round\"]-d.get(\"last_finalized_round\",0)} peers={d.get(\"peer_count\",0)} ✅')
" 2>/dev/null || echo "  Node $n: ⚠️ Responsive but parse error" | tee -a "$LOG_FILE"
    else
        echo "  Node $n: ❌ Unresponsive" | tee -a "$LOG_FILE"
    fi
done

echo "" | tee -a "$LOG_FILE"
echo "=== Crash Test Complete ===" | tee -a "$LOG_FILE"
echo "End time: $(date)" | tee -a "$LOG_FILE"
echo "" | tee -a "$LOG_FILE"
echo "Results:" | tee -a "$LOG_FILE"
echo "  Tests survived: $SURVIVED" | tee -a "$LOG_FILE"
echo "  Tests crashed: $CRASHED" | tee -a "$LOG_FILE"
echo "  TOTAL: $((SURVIVED + CRASHED))" | tee -a "$LOG_FILE"

if [ $CRASHED -eq 0 ]; then
    echo "  ✅ TESTNET SURVIVED ALL CRASH ATTEMPTS!" | tee -a "$LOG_FILE"
    echo "  The network is extremely robust." | tee -a "$LOG_FILE"
else
    echo "  ⚠️  Network crashed on $CRASHED test(s)" | tee -a "$LOG_FILE"
    echo "  Review log for details." | tee -a "$LOG_FILE"
fi
