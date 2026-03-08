#!/bin/bash
# Fuzzing test suite for UltraDAG testnet
# Tests transaction validation with malformed, edge case, and random inputs

NODE_URL="https://ultradag-node-1.fly.dev"
LOG_FILE="scripts/fuzzing_test.log"

echo "=== UltraDAG Fuzzing Test Suite Started ===" | tee "$LOG_FILE"
echo "Target: $NODE_URL" | tee -a "$LOG_FILE"
echo "Start time: $(date)" | tee -a "$LOG_FILE"
echo "" | tee -a "$LOG_FILE"

PASS=0
FAIL=0

test_case() {
    local name="$1"
    local data="$2"
    local expected="$3"
    
    echo "Test: $name" | tee -a "$LOG_FILE"
    RESPONSE=$(curl -s -X POST "$NODE_URL/tx" -H "Content-Type: application/json" -d "$data" 2>/dev/null)
    
    if echo "$RESPONSE" | grep -q "$expected"; then
        echo "  ✅ PASS" | tee -a "$LOG_FILE"
        PASS=$((PASS + 1))
    else
        echo "  ❌ FAIL - Expected: $expected" | tee -a "$LOG_FILE"
        echo "  Response: $RESPONSE" | tee -a "$LOG_FILE"
        FAIL=$((FAIL + 1))
    fi
}

echo "=== CATEGORY 1: Malformed JSON ===" | tee -a "$LOG_FILE"
echo "" | tee -a "$LOG_FILE"

test_case "Empty body" "" "invalid JSON"
test_case "Invalid JSON syntax" "{invalid json}" "invalid JSON"
test_case "Missing closing brace" '{"secret_key":"abc"' "invalid JSON"
test_case "Extra commas" '{"secret_key":"abc",,}' "invalid JSON"

echo "" | tee -a "$LOG_FILE"
echo "=== CATEGORY 2: Missing Required Fields ===" | tee -a "$LOG_FILE"
echo "" | tee -a "$LOG_FILE"

test_case "Missing secret_key" '{"to":"abc","amount":100,"fee":10000}' "invalid JSON"
test_case "Missing to" '{"secret_key":"'$(openssl rand -hex 32)'","amount":100,"fee":10000}' "invalid JSON"
test_case "Missing amount" '{"secret_key":"'$(openssl rand -hex 32)'","to":"abc","fee":10000}' "invalid JSON"
test_case "Missing fee" '{"secret_key":"'$(openssl rand -hex 32)'","to":"abc","amount":100}' "invalid JSON"

echo "" | tee -a "$LOG_FILE"
echo "=== CATEGORY 3: Invalid Secret Key Formats ===" | tee -a "$LOG_FILE"
echo "" | tee -a "$LOG_FILE"

test_case "Secret key too short" '{"secret_key":"abc","to":"'$(openssl rand -hex 32)'","amount":100,"fee":10000}' "must be 64 hex chars"
test_case "Secret key too long" '{"secret_key":"'$(openssl rand -hex 33)'","to":"'$(openssl rand -hex 32)'","amount":100,"fee":10000}' "must be 64 hex chars"
test_case "Secret key non-hex chars" '{"secret_key":"zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz","to":"'$(openssl rand -hex 32)'","amount":100,"fee":10000}' "invalid hex"
test_case "Secret key with spaces" '{"secret_key":"0000 0000 0000 0000 0000 0000 0000 0000 0000 0000 0000 0000 0000 0000 0000 0000","to":"'$(openssl rand -hex 32)'","amount":100,"fee":10000}' "must be 64 hex chars"

echo "" | tee -a "$LOG_FILE"
echo "=== CATEGORY 4: Invalid Address Formats ===" | tee -a "$LOG_FILE"
echo "" | tee -a "$LOG_FILE"

test_case "Address too short" '{"secret_key":"'$(openssl rand -hex 32)'","to":"abc","amount":100,"fee":10000}' "invalid.*address"
test_case "Address too long" '{"secret_key":"'$(openssl rand -hex 32)'","to":"'$(openssl rand -hex 33)'","amount":100,"fee":10000}' "invalid.*address"
test_case "Address non-hex" '{"secret_key":"'$(openssl rand -hex 32)'","to":"zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz","amount":100,"fee":10000}' "invalid.*address"

echo "" | tee -a "$LOG_FILE"
echo "=== CATEGORY 5: Boundary Value Testing ===" | tee -a "$LOG_FILE"
echo "" | tee -a "$LOG_FILE"

test_case "Amount = 0" '{"secret_key":"'$(openssl rand -hex 32)'","to":"'$(openssl rand -hex 32)'","amount":0,"fee":10000}' "insufficient balance"
test_case "Amount = 1" '{"secret_key":"'$(openssl rand -hex 32)'","to":"'$(openssl rand -hex 32)'","amount":1,"fee":10000}' "insufficient balance"
test_case "Amount = MAX_U64" '{"secret_key":"'$(openssl rand -hex 32)'","to":"'$(openssl rand -hex 32)'","amount":18446744073709551615,"fee":10000}' "insufficient balance"

test_case "Fee = 0" '{"secret_key":"'$(openssl rand -hex 32)'","to":"'$(openssl rand -hex 32)'","amount":100,"fee":0}' "fee too low"
test_case "Fee = 9999" '{"secret_key":"'$(openssl rand -hex 32)'","to":"'$(openssl rand -hex 32)'","amount":100,"fee":9999}' "fee too low"
test_case "Fee = 10000" '{"secret_key":"'$(openssl rand -hex 32)'","to":"'$(openssl rand -hex 32)'","amount":100,"fee":10000}' "insufficient balance"
test_case "Fee = 10001" '{"secret_key":"'$(openssl rand -hex 32)'","to":"'$(openssl rand -hex 32)'","amount":100,"fee":10001}' "insufficient balance"

echo "" | tee -a "$LOG_FILE"
echo "=== CATEGORY 6: Type Confusion ===" | tee -a "$LOG_FILE"
echo "" | tee -a "$LOG_FILE"

test_case "Amount as string" '{"secret_key":"'$(openssl rand -hex 32)'","to":"'$(openssl rand -hex 32)'","amount":"100","fee":10000}' "invalid JSON"
test_case "Fee as string" '{"secret_key":"'$(openssl rand -hex 32)'","to":"'$(openssl rand -hex 32)'","amount":100,"fee":"10000"}' "invalid JSON"
test_case "Amount as float" '{"secret_key":"'$(openssl rand -hex 32)'","to":"'$(openssl rand -hex 32)'","amount":100.5,"fee":10000}' "invalid JSON"
test_case "Negative amount" '{"secret_key":"'$(openssl rand -hex 32)'","to":"'$(openssl rand -hex 32)'","amount":-100,"fee":10000}' "invalid JSON"
test_case "Negative fee" '{"secret_key":"'$(openssl rand -hex 32)'","to":"'$(openssl rand -hex 32)'","amount":100,"fee":-10000}' "invalid JSON"

echo "" | tee -a "$LOG_FILE"
echo "=== CATEGORY 7: Overflow/Underflow Testing ===" | tee -a "$LOG_FILE"
echo "" | tee -a "$LOG_FILE"

test_case "Amount + Fee overflow" '{"secret_key":"'$(openssl rand -hex 32)'","to":"'$(openssl rand -hex 32)'","amount":18446744073709551615,"fee":18446744073709551615}' "insufficient balance"

echo "" | tee -a "$LOG_FILE"
echo "=== CATEGORY 8: Special Characters and Injection ===" | tee -a "$LOG_FILE"
echo "" | tee -a "$LOG_FILE"

test_case "SQL injection in address" '{"secret_key":"'$(openssl rand -hex 32)'","to":"'\'' OR 1=1--","amount":100,"fee":10000}' "invalid.*address"
test_case "Script injection in address" '{"secret_key":"'$(openssl rand -hex 32)'","to":"<script>alert(1)</script>","amount":100,"fee":10000}' "invalid.*address"
test_case "Null bytes in secret" '{"secret_key":"0000000000000000\u0000000000000000000000000000000000000000000000000000","to":"'$(openssl rand -hex 32)'","amount":100,"fee":10000}' "invalid hex"

echo "" | tee -a "$LOG_FILE"
echo "=== CATEGORY 9: Random Fuzzing (100 iterations) ===" | tee -a "$LOG_FILE"
echo "" | tee -a "$LOG_FILE"

for i in {1..100}; do
    # Generate random lengths and random hex
    SK_LEN=$((RANDOM % 128))
    TO_LEN=$((RANDOM % 128))
    AMOUNT=$((RANDOM * RANDOM))
    FEE=$((RANDOM * RANDOM))
    
    SK=$(head -c $SK_LEN /dev/urandom 2>/dev/null | xxd -p | tr -d '\n')
    TO=$(head -c $TO_LEN /dev/urandom 2>/dev/null | xxd -p | tr -d '\n')
    
    RESPONSE=$(curl -s -X POST "$NODE_URL/tx" \
        -H "Content-Type: application/json" \
        -d "{\"secret_key\":\"$SK\",\"to\":\"$TO\",\"amount\":$AMOUNT,\"fee\":$FEE}" 2>/dev/null)
    
    # Should always get an error response, never crash
    if echo "$RESPONSE" | grep -q "error"; then
        PASS=$((PASS + 1))
    else
        echo "  Iteration $i: Unexpected response (no error field)" | tee -a "$LOG_FILE"
        FAIL=$((FAIL + 1))
    fi
    
    [ $((i % 20)) -eq 0 ] && echo "  Progress: $i/100" | tee -a "$LOG_FILE"
done

echo "" | tee -a "$LOG_FILE"
echo "=== Fuzzing Test Complete ===" | tee -a "$LOG_FILE"
echo "End time: $(date)" | tee -a "$LOG_FILE"
echo "" | tee -a "$LOG_FILE"
echo "Results:" | tee -a "$LOG_FILE"
echo "  PASSED: $PASS" | tee -a "$LOG_FILE"
echo "  FAILED: $FAIL" | tee -a "$LOG_FILE"
echo "  TOTAL:  $((PASS + FAIL))" | tee -a "$LOG_FILE"

if [ $FAIL -eq 0 ]; then
    echo "  ✅ All fuzzing tests passed!" | tee -a "$LOG_FILE"
else
    echo "  ⚠️  Some tests failed - review log for details" | tee -a "$LOG_FILE"
fi
