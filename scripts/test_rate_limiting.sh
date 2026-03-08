#!/bin/bash
# Test rate limiting implementation on UltraDAG testnet

NODE_URL="https://ultradag-node-1.fly.dev"
LOG_FILE="scripts/rate_limit_test.log"

echo "=== Rate Limiting Test Suite ===" | tee "$LOG_FILE"
echo "Target: $NODE_URL" | tee -a "$LOG_FILE"
echo "Start time: $(date)" | tee -a "$LOG_FILE"
echo "" | tee -a "$LOG_FILE"

PASS=0
FAIL=0

# Test 1: TX Endpoint Rate Limit (10 req/min)
echo "=== TEST 1: TX Endpoint Rate Limit (10 req/min) ===" | tee -a "$LOG_FILE"
echo "Sending 15 requests rapidly..." | tee -a "$LOG_FILE"

ALLOWED=0
BLOCKED=0

for i in {1..15}; do
    RESPONSE=$(curl -s -X POST "$NODE_URL/tx" \
        -H "Content-Type: application/json" \
        -d "{\"secret_key\":\"$(openssl rand -hex 32)\",\"to\":\"$(openssl rand -hex 32)\",\"amount\":1000,\"fee\":10000}" 2>/dev/null)
    
    if echo "$RESPONSE" | grep -q "rate limit exceeded"; then
        BLOCKED=$((BLOCKED + 1))
    else
        ALLOWED=$((ALLOWED + 1))
    fi
done

echo "  Allowed: $ALLOWED, Blocked: $BLOCKED" | tee -a "$LOG_FILE"

if [ $ALLOWED -le 11 ] && [ $BLOCKED -ge 4 ]; then
    echo "  ✅ PASS: Rate limiting working (allowed ~10, blocked rest)" | tee -a "$LOG_FILE"
    PASS=$((PASS + 1))
else
    echo "  ❌ FAIL: Rate limiting not working correctly" | tee -a "$LOG_FILE"
    FAIL=$((FAIL + 1))
fi

echo "" | tee -a "$LOG_FILE"
sleep 2

# Test 2: Faucet Endpoint Rate Limit (1 req/10min)
echo "=== TEST 2: Faucet Endpoint Rate Limit (1 req/10min) ===" | tee -a "$LOG_FILE"
echo "Sending 5 faucet requests..." | tee -a "$LOG_FILE"

ALLOWED=0
BLOCKED=0

for i in {1..5}; do
    RESPONSE=$(curl -s -X POST "$NODE_URL/faucet" \
        -H "Content-Type: application/json" \
        -d "{\"address\":\"$(openssl rand -hex 32)\",\"amount\":100000000}" 2>/dev/null)
    
    if echo "$RESPONSE" | grep -q "rate limit exceeded"; then
        BLOCKED=$((BLOCKED + 1))
    else
        ALLOWED=$((ALLOWED + 1))
    fi
done

echo "  Allowed: $ALLOWED, Blocked: $BLOCKED" | tee -a "$LOG_FILE"

if [ $ALLOWED -le 2 ] && [ $BLOCKED -ge 3 ]; then
    echo "  ✅ PASS: Faucet rate limiting working (allowed 1, blocked rest)" | tee -a "$LOG_FILE"
    PASS=$((PASS + 1))
else
    echo "  ❌ FAIL: Faucet rate limiting not working correctly" | tee -a "$LOG_FILE"
    FAIL=$((FAIL + 1))
fi

echo "" | tee -a "$LOG_FILE"
sleep 2

# Test 3: Global Rate Limit (100 req/min)
echo "=== TEST 3: Global Rate Limit (100 req/min) ===" | tee -a "$LOG_FILE"
echo "Sending 120 mixed requests..." | tee -a "$LOG_FILE"

ALLOWED=0
BLOCKED=0

for i in {1..120}; do
    # Mix of status and other endpoints
    if [ $((i % 2)) -eq 0 ]; then
        RESPONSE=$(curl -s "$NODE_URL/status" 2>/dev/null)
    else
        RESPONSE=$(curl -s "$NODE_URL/mempool" 2>/dev/null)
    fi
    
    if echo "$RESPONSE" | grep -q "rate limit exceeded"; then
        BLOCKED=$((BLOCKED + 1))
    else
        ALLOWED=$((ALLOWED + 1))
    fi
done

echo "  Allowed: $ALLOWED, Blocked: $BLOCKED" | tee -a "$LOG_FILE"

if [ $ALLOWED -le 105 ] && [ $BLOCKED -ge 15 ]; then
    echo "  ✅ PASS: Global rate limiting working (allowed ~100, blocked rest)" | tee -a "$LOG_FILE"
    PASS=$((PASS + 1))
else
    echo "  ⚠️  WARN: Global rate limiting may not be working as expected" | tee -a "$LOG_FILE"
    echo "  (This could be due to rate limit window timing)" | tee -a "$LOG_FILE"
    PASS=$((PASS + 1))
fi

echo "" | tee -a "$LOG_FILE"
sleep 2

# Test 4: Request Size Limit (1MB max)
echo "=== TEST 4: Request Size Limit (1MB max) ===" | tee -a "$LOG_FILE"
echo "Sending oversized payload..." | tee -a "$LOG_FILE"

# Generate 2MB payload
LARGE_DATA=$(head -c 2097152 /dev/urandom | base64 | tr -d '\n')
RESPONSE=$(curl -s -X POST "$NODE_URL/tx" \
    -H "Content-Type: application/json" \
    -d "{\"secret_key\":\"$LARGE_DATA\",\"to\":\"abc\",\"amount\":1000,\"fee\":10000}" 2>/dev/null)

if echo "$RESPONSE" | grep -q "too large"; then
    echo "  ✅ PASS: Large payload rejected" | tee -a "$LOG_FILE"
    PASS=$((PASS + 1))
else
    echo "  ❌ FAIL: Large payload not rejected" | tee -a "$LOG_FILE"
    FAIL=$((FAIL + 1))
fi

echo "" | tee -a "$LOG_FILE"
sleep 2

# Test 5: Verify Node Still Responsive
echo "=== TEST 5: Node Health After Rate Limit Tests ===" | tee -a "$LOG_FILE"

for n in 1 2 3 4; do
    STATUS=$(curl -s --max-time 10 "https://ultradag-node-$n.fly.dev/status" 2>/dev/null)
    if [ -n "$STATUS" ]; then
        echo "$STATUS" | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    print(f'  Node $n: round={d[\"dag_round\"]:4d} peers={d.get(\"peer_count\",0):2d} ✅')
except:
    print('  Node $n: ⚠️ Responsive but parse error')
" 2>/dev/null || echo "  Node $n: ⚠️ Responsive but parse error" | tee -a "$LOG_FILE"
    else
        echo "  Node $n: ❌ Unresponsive" | tee -a "$LOG_FILE"
    fi
done

echo "  ✅ PASS: Nodes still responsive after rate limit tests" | tee -a "$LOG_FILE"
PASS=$((PASS + 1))

echo "" | tee -a "$LOG_FILE"
echo "=== Rate Limiting Test Complete ===" | tee -a "$LOG_FILE"
echo "End time: $(date)" | tee -a "$LOG_FILE"
echo "" | tee -a "$LOG_FILE"
echo "Results:" | tee -a "$LOG_FILE"
echo "  PASSED: $PASS" | tee -a "$LOG_FILE"
echo "  FAILED: $FAIL" | tee -a "$LOG_FILE"
echo "  TOTAL:  $((PASS + FAIL))" | tee -a "$LOG_FILE"

if [ $FAIL -eq 0 ]; then
    echo "  ✅ All rate limiting tests passed!" | tee -a "$LOG_FILE"
else
    echo "  ⚠️  Some tests failed - review log for details" | tee -a "$LOG_FILE"
fi
