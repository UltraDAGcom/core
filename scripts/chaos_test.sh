#!/bin/bash
# Chaos testing script for UltraDAG testnet
# Tests resilience to node failures and network issues

LOG_FILE="scripts/chaos_test.log"

echo "=== UltraDAG Chaos Test Started ===" | tee "$LOG_FILE"
echo "Start time: $(date)" | tee -a "$LOG_FILE"
echo "" | tee -a "$LOG_FILE"

# Test 1: Baseline - All nodes healthy
echo "TEST 1: Baseline - All Nodes Healthy" | tee -a "$LOG_FILE"
echo "=====================================" | tee -a "$LOG_FILE"

for n in 1 2 3 4; do
    STATUS=$(curl -s --max-time 10 "https://ultradag-node-$n.fly.dev/status" 2>/dev/null)
    if [ -n "$STATUS" ]; then
        echo "$STATUS" | python3 -c "
import sys, json
d = json.load(sys.stdin)
print(f'  Node $n: round={d[\"dag_round\"]} fin={d.get(\"last_finalized_round\",0)} lag={d[\"dag_round\"]-d.get(\"last_finalized_round\",0)} ✅')
" 2>/dev/null || echo "  Node $n: ❌ Parse error"
    else
        echo "  Node $n: ❌ Unreachable"
    fi
done

echo "" | tee -a "$LOG_FILE"

# Test 2: Simulate single node failure (stop node 4)
echo "TEST 2: Single Node Failure Simulation" | tee -a "$LOG_FILE"
echo "=======================================" | tee -a "$LOG_FILE"
echo "  Stopping node 4..." | tee -a "$LOG_FILE"

export FLY_API_TOKEN="FlyV1 fm2_lJPECAAAAAAACF4CxBALWiHG4Gt7uR26M+mFlRmwwrVodHRwczovL2FwaS5mbHkuaW8vdjGUAJLOAA1/5B8Lk7lodHRwczovL2FwaS5mbHkuaW8vYWFhL3YxxDw27fOPGr9orsDIlVin0jyDbyvCHcgAWi4+fdnTZgRe/0SCsEBknwPRodCMLm7ydWhdoJFGjr7+oJb9zR3ETpfJErfeFNECQ5Od20dgGmrHp5Tvdd03sLmQkzo5lczXY2spU6a1HSB4KVTr5DNbeu1uvywAMmVnBkYcOGFOb0CCz0mYfDRMAZGpv9xPrMQg5PWAb+17uRTo2T7mbU3pyqXGgTVpUCiyAVUtsI2ct7A=,fm2_lJPETpfJErfeFNECQ5Od20dgGmrHp5Tvdd03sLmQkzo5lczXY2spU6a1HSB4KVTr5DNbeu1uvywAMmVnBkYcOGFOb0CCz0mYfDRMAZGpv9xPrMQQFTqHw6zg8DKKgC/nn6FAIsO5aHR0cHM6Ly9hcGkuZmx5LmlvL2FhYS92MZgEks5pq+NJzwAAAAElpAFnF84ADRLmCpHOAA0S5gzEEA4EQC5ivNmjUNsWyLct3nTEILAGVdvZetlULjEjEC3Qiai0MVI8cMQyUCtZBsS0cmMG"

flyctl machine stop --app ultradag-node-4 -s 2>&1 | grep -i "stopped\|error" | tee -a "$LOG_FILE"

echo "  Waiting 30 seconds for network to adapt..." | tee -a "$LOG_FILE"
sleep 30

echo "  Checking remaining nodes (should continue with 3/4 validators):" | tee -a "$LOG_FILE"
for n in 1 2 3; do
    STATUS=$(curl -s --max-time 10 "https://ultradag-node-$n.fly.dev/status" 2>/dev/null)
    if [ -n "$STATUS" ]; then
        echo "$STATUS" | python3 -c "
import sys, json
d = json.load(sys.stdin)
lag = d['dag_round'] - d.get('last_finalized_round', 0)
status = '✅' if lag <= 10 else '⚠️'
print(f'  Node $n: round={d[\"dag_round\"]} fin={d.get(\"last_finalized_round\",0)} lag={lag} peers={d.get(\"peer_count\",0)} {status}')
" 2>/dev/null || echo "  Node $n: ❌ Parse error"
    else
        echo "  Node $n: ❌ Unreachable"
    fi
done

echo "" | tee -a "$LOG_FILE"

# Test 3: Check vertex density with 3 validators
echo "TEST 3: Vertex Density with 3 Validators" | tee -a "$LOG_FILE"
echo "=========================================" | tee -a "$LOG_FILE"

CURRENT_ROUND=$(curl -s "https://ultradag-node-1.fly.dev/status" 2>/dev/null | python3 -c "import sys,json; print(json.load(sys.stdin)['dag_round'])" 2>/dev/null)

if [ -n "$CURRENT_ROUND" ]; then
    for offset in 1 2 3; do
        ROUND=$((CURRENT_ROUND - offset))
        VERTICES=$(curl -s "https://ultradag-node-1.fly.dev/round/$ROUND" 2>/dev/null | python3 -c "import sys,json; print(len(json.load(sys.stdin)))" 2>/dev/null)
        
        if [ -n "$VERTICES" ]; then
            STATUS="✅"
            [ "$VERTICES" -lt 2 ] && STATUS="⚠️"
            echo "  Round $ROUND: $VERTICES vertices $STATUS" | tee -a "$LOG_FILE"
        fi
    done
fi

echo "" | tee -a "$LOG_FILE"

# Test 4: Restart node 4
echo "TEST 4: Node Recovery" | tee -a "$LOG_FILE"
echo "=====================" | tee -a "$LOG_FILE"
echo "  Restarting node 4..." | tee -a "$LOG_FILE"

flyctl machine start --app ultradag-node-4 2>&1 | grep -i "started\|error" | tee -a "$LOG_FILE"

echo "  Waiting 60 seconds for node to rejoin..." | tee -a "$LOG_FILE"
sleep 60

echo "  Checking all nodes after recovery:" | tee -a "$LOG_FILE"
for n in 1 2 3 4; do
    STATUS=$(curl -s --max-time 10 "https://ultradag-node-$n.fly.dev/status" 2>/dev/null)
    if [ -n "$STATUS" ]; then
        echo "$STATUS" | python3 -c "
import sys, json
d = json.load(sys.stdin)
lag = d['dag_round'] - d.get('last_finalized_round', 0)
status = '✅' if lag <= 10 else '⚠️'
print(f'  Node $n: round={d[\"dag_round\"]} fin={d.get(\"last_finalized_round\",0)} lag={lag} peers={d.get(\"peer_count\",0)} {status}')
" 2>/dev/null || echo "  Node $n: ❌ Parse error"
    else
        echo "  Node $n: ❌ Unreachable"
    fi
done

echo "" | tee -a "$LOG_FILE"
echo "=== Chaos Test Complete ===" | tee -a "$LOG_FILE"
echo "End time: $(date)" | tee -a "$LOG_FILE"
