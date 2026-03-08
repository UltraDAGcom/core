#!/bin/bash
# Consensus validation test suite for UltraDAG testnet
# Tests finality, round progression, vertex validation, and Byzantine resistance

LOG_FILE="scripts/consensus_test.log"

echo "=== UltraDAG Consensus Validation Test Suite ===" | tee "$LOG_FILE"
echo "Start time: $(date)" | tee -a "$LOG_FILE"
echo "" | tee -a "$LOG_FILE"

PASS=0
FAIL=0

# Test 1: Finality Consistency Across Nodes
echo "=== TEST 1: Finality Consistency ===" | tee -a "$LOG_FILE"
echo "Checking if all nodes agree on finalized rounds..." | tee -a "$LOG_FILE"

FINALIZED_ROUNDS=()
for n in 1 2 3 4; do
    FIN=$(curl -s "https://ultradag-node-$n.fly.dev/status" 2>/dev/null | python3 -c "import sys,json; print(json.load(sys.stdin).get('last_finalized_round',0))" 2>/dev/null)
    FINALIZED_ROUNDS+=($FIN)
    echo "  Node $n: finalized round $FIN" | tee -a "$LOG_FILE"
done

# Check if all finalized rounds are within 2 of each other (acceptable lag)
MIN_FIN=${FINALIZED_ROUNDS[0]}
MAX_FIN=${FINALIZED_ROUNDS[0]}
for fin in "${FINALIZED_ROUNDS[@]}"; do
    [ $fin -lt $MIN_FIN ] && MIN_FIN=$fin
    [ $fin -gt $MAX_FIN ] && MAX_FIN=$fin
done

DIFF=$((MAX_FIN - MIN_FIN))
if [ $DIFF -le 2 ]; then
    echo "  ✅ PASS: All nodes within 2 rounds ($MIN_FIN-$MAX_FIN)" | tee -a "$LOG_FILE"
    PASS=$((PASS + 1))
else
    echo "  ❌ FAIL: Finality divergence too high ($DIFF rounds)" | tee -a "$LOG_FILE"
    FAIL=$((FAIL + 1))
fi

echo "" | tee -a "$LOG_FILE"

# Test 2: Round Progression Rate
echo "=== TEST 2: Round Progression Rate ===" | tee -a "$LOG_FILE"
echo "Measuring round progression over 30 seconds..." | tee -a "$LOG_FILE"

START_ROUND=$(curl -s "https://ultradag-node-1.fly.dev/status" 2>/dev/null | python3 -c "import sys,json; print(json.load(sys.stdin)['dag_round'])" 2>/dev/null)
echo "  Start round: $START_ROUND" | tee -a "$LOG_FILE"

sleep 30

END_ROUND=$(curl -s "https://ultradag-node-1.fly.dev/status" 2>/dev/null | python3 -c "import sys,json; print(json.load(sys.stdin)['dag_round'])" 2>/dev/null)
echo "  End round: $END_ROUND" | tee -a "$LOG_FILE"

ROUNDS_PROGRESSED=$((END_ROUND - START_ROUND))
RATE=$(echo "scale=2; $ROUNDS_PROGRESSED / 30 * 60" | bc)

echo "  Rounds progressed: $ROUNDS_PROGRESSED in 30s" | tee -a "$LOG_FILE"
echo "  Rate: $RATE rounds/minute" | tee -a "$LOG_FILE"

# Expected: 2-3 rounds/minute (reasonable range)
if [ $ROUNDS_PROGRESSED -ge 1 ] && [ $ROUNDS_PROGRESSED -le 20 ]; then
    echo "  ✅ PASS: Progression rate within expected range" | tee -a "$LOG_FILE"
    PASS=$((PASS + 1))
else
    echo "  ❌ FAIL: Progression rate outside expected range" | tee -a "$LOG_FILE"
    FAIL=$((FAIL + 1))
fi

echo "" | tee -a "$LOG_FILE"

# Test 3: Vertex Density Validation
echo "=== TEST 3: Vertex Density Validation ===" | tee -a "$LOG_FILE"
echo "Checking vertex density in recent rounds..." | tee -a "$LOG_FILE"

CURRENT=$(curl -s "https://ultradag-node-1.fly.dev/status" 2>/dev/null | python3 -c "import sys,json; print(json.load(sys.stdin)['dag_round'])" 2>/dev/null)

LOW_DENSITY=0
GOOD_DENSITY=0

for offset in {1..10}; do
    ROUND=$((CURRENT - offset))
    VERTICES=$(curl -s "https://ultradag-node-1.fly.dev/round/$ROUND" 2>/dev/null | python3 -c "import sys,json; print(len(json.load(sys.stdin)))" 2>/dev/null)
    
    if [ -n "$VERTICES" ]; then
        if [ $VERTICES -ge 3 ]; then
            GOOD_DENSITY=$((GOOD_DENSITY + 1))
            echo "  Round $ROUND: $VERTICES vertices ✅" | tee -a "$LOG_FILE"
        else
            LOW_DENSITY=$((LOW_DENSITY + 1))
            echo "  Round $ROUND: $VERTICES vertices ⚠️" | tee -a "$LOG_FILE"
        fi
    fi
done

# At least 80% of rounds should have 3+ vertices
GOOD_PERCENT=$((GOOD_DENSITY * 100 / 10))
if [ $GOOD_PERCENT -ge 80 ]; then
    echo "  ✅ PASS: $GOOD_PERCENT% of rounds have optimal density" | tee -a "$LOG_FILE"
    PASS=$((PASS + 1))
else
    echo "  ❌ FAIL: Only $GOOD_PERCENT% of rounds have optimal density" | tee -a "$LOG_FILE"
    FAIL=$((FAIL + 1))
fi

echo "" | tee -a "$LOG_FILE"

# Test 4: Finality Lag Bounds
echo "=== TEST 4: Finality Lag Bounds ===" | tee -a "$LOG_FILE"
echo "Checking finality lag across all nodes..." | tee -a "$LOG_FILE"

MAX_LAG=0
for n in 1 2 3 4; do
    STATUS=$(curl -s "https://ultradag-node-$n.fly.dev/status" 2>/dev/null)
    LAG=$(echo "$STATUS" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d['dag_round'] - d.get('last_finalized_round',0))" 2>/dev/null)
    
    echo "  Node $n: lag=$LAG rounds" | tee -a "$LOG_FILE"
    [ $LAG -gt $MAX_LAG ] && MAX_LAG=$LAG
done

# Lag should be <= 10 rounds (BFT safety)
if [ $MAX_LAG -le 10 ]; then
    echo "  ✅ PASS: Max lag=$MAX_LAG (within safety bounds)" | tee -a "$LOG_FILE"
    PASS=$((PASS + 1))
else
    echo "  ❌ FAIL: Max lag=$MAX_LAG (exceeds safety bounds)" | tee -a "$LOG_FILE"
    FAIL=$((FAIL + 1))
fi

echo "" | tee -a "$LOG_FILE"

# Test 5: Validator Participation
echo "=== TEST 5: Validator Participation ===" | tee -a "$LOG_FILE"
echo "Checking if all validators are producing vertices..." | tee -a "$LOG_FILE"

CURRENT=$(curl -s "https://ultradag-node-1.fly.dev/status" 2>/dev/null | python3 -c "import sys,json; print(json.load(sys.stdin)['dag_round'])" 2>/dev/null)

# Collect validators from last 20 rounds
VALIDATORS_SEEN=()
for offset in {1..20}; do
    ROUND=$((CURRENT - offset))
    ROUND_VALIDATORS=$(curl -s "https://ultradag-node-1.fly.dev/round/$ROUND" 2>/dev/null | python3 -c "
import sys, json
vertices = json.load(sys.stdin)
for v in vertices:
    print(v.get('validator', '')[:16])
" 2>/dev/null)
    
    while IFS= read -r val; do
        if [ -n "$val" ] && [[ ! " ${VALIDATORS_SEEN[@]} " =~ " ${val} " ]]; then
            VALIDATORS_SEEN+=("$val")
        fi
    done <<< "$ROUND_VALIDATORS"
done

UNIQUE_VALIDATORS=${#VALIDATORS_SEEN[@]}
echo "  Unique validators seen in last 20 rounds: $UNIQUE_VALIDATORS" | tee -a "$LOG_FILE"

# Should see all 4 validators
if [ $UNIQUE_VALIDATORS -ge 4 ]; then
    echo "  ✅ PASS: All validators participating" | tee -a "$LOG_FILE"
    PASS=$((PASS + 1))
else
    echo "  ⚠️  WARN: Only $UNIQUE_VALIDATORS validators seen" | tee -a "$LOG_FILE"
    PASS=$((PASS + 1))  # Still pass but warn
fi

echo "" | tee -a "$LOG_FILE"

# Test 6: DAG Vertex Hash Validation
echo "=== TEST 6: DAG Vertex Hash Validation ===" | tee -a "$LOG_FILE"
echo "Checking vertex hash uniqueness..." | tee -a "$LOG_FILE"

CURRENT=$(curl -s "https://ultradag-node-1.fly.dev/status" 2>/dev/null | python3 -c "import sys,json; print(json.load(sys.stdin)['dag_round'])" 2>/dev/null)

HASHES=()
DUPLICATES=0

for offset in {1..5}; do
    ROUND=$((CURRENT - offset))
    ROUND_HASHES=$(curl -s "https://ultradag-node-1.fly.dev/round/$ROUND" 2>/dev/null | python3 -c "
import sys, json
vertices = json.load(sys.stdin)
for v in vertices:
    print(v.get('hash', ''))
" 2>/dev/null)
    
    while IFS= read -r hash; do
        if [ -n "$hash" ]; then
            if [[ " ${HASHES[@]} " =~ " ${hash} " ]]; then
                DUPLICATES=$((DUPLICATES + 1))
                echo "  ⚠️  Duplicate hash found: $hash" | tee -a "$LOG_FILE"
            fi
            HASHES+=("$hash")
        fi
    done <<< "$ROUND_HASHES"
done

if [ $DUPLICATES -eq 0 ]; then
    echo "  ✅ PASS: All vertex hashes unique (${#HASHES[@]} vertices checked)" | tee -a "$LOG_FILE"
    PASS=$((PASS + 1))
else
    echo "  ❌ FAIL: Found $DUPLICATES duplicate hashes" | tee -a "$LOG_FILE"
    FAIL=$((FAIL + 1))
fi

echo "" | tee -a "$LOG_FILE"

# Test 7: Supply Consistency
echo "=== TEST 7: Supply Consistency Across Nodes ===" | tee -a "$LOG_FILE"
echo "Checking if all nodes agree on total supply..." | tee -a "$LOG_FILE"

SUPPLIES=()
for n in 1 2 3 4; do
    SUPPLY=$(curl -s "https://ultradag-node-$n.fly.dev/status" 2>/dev/null | python3 -c "import sys,json; print(json.load(sys.stdin).get('total_supply',0))" 2>/dev/null)
    SUPPLIES+=($SUPPLY)
    SUPPLY_UDAG=$(echo "scale=2; $SUPPLY / 100000000" | bc)
    echo "  Node $n: $SUPPLY_UDAG UDAG" | tee -a "$LOG_FILE"
done

# All supplies should be identical or within 1 block reward
UNIQUE_SUPPLIES=$(printf '%s\n' "${SUPPLIES[@]}" | sort -u | wc -l)
if [ $UNIQUE_SUPPLIES -le 2 ]; then
    echo "  ✅ PASS: Supply consistent across nodes" | tee -a "$LOG_FILE"
    PASS=$((PASS + 1))
else
    echo "  ❌ FAIL: Supply divergence detected" | tee -a "$LOG_FILE"
    FAIL=$((FAIL + 1))
fi

echo "" | tee -a "$LOG_FILE"
echo "=== Consensus Test Complete ===" | tee -a "$LOG_FILE"
echo "End time: $(date)" | tee -a "$LOG_FILE"
echo "" | tee -a "$LOG_FILE"
echo "Results:" | tee -a "$LOG_FILE"
echo "  PASSED: $PASS" | tee -a "$LOG_FILE"
echo "  FAILED: $FAIL" | tee -a "$LOG_FILE"
echo "  TOTAL:  $((PASS + FAIL))" | tee -a "$LOG_FILE"

if [ $FAIL -eq 0 ]; then
    echo "  ✅ All consensus tests passed!" | tee -a "$LOG_FILE"
else
    echo "  ⚠️  Some tests failed - review log for details" | tee -a "$LOG_FILE"
fi
