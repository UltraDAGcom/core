#!/bin/bash
# Staking functionality test script
# Tests staking, unstaking, and epoch transitions

NODE_URL="https://ultradag-node-1.fly.dev"
LOG_FILE="scripts/staking_test.log"

echo "=== UltraDAG Staking Test Started ===" | tee "$LOG_FILE"
echo "Target: $NODE_URL" | tee -a "$LOG_FILE"
echo "Start time: $(date)" | tee -a "$LOG_FILE"
echo "" | tee -a "$LOG_FILE"

# Test 1: Check current staking status
echo "TEST 1: Current Staking Status" | tee -a "$LOG_FILE"
echo "===============================" | tee -a "$LOG_FILE"

VALIDATORS=$(curl -s "$NODE_URL/validators" 2>/dev/null)
echo "$VALIDATORS" | python3 -c "
import sys, json
try:
    v = json.load(sys.stdin)
    count = v.get('count', 0)
    total_staked = v.get('total_staked', 0) / 100000000
    print(f'  Active validators: {count}')
    print(f'  Total staked: {total_staked:.2f} UDAG')
    
    if count == 0:
        print('  Status: Pre-staking mode (no validators staked yet)')
    else:
        print('  Validators:')
        for val in v.get('validators', []):
            print(f'    - {val}')
except Exception as e:
    print(f'  Parse error: {e}')
" | tee -a "$LOG_FILE"

echo "" | tee -a "$LOG_FILE"

# Test 2: Check epoch information
echo "TEST 2: Epoch Information" | tee -a "$LOG_FILE"
echo "=========================" | tee -a "$LOG_FILE"

STATUS=$(curl -s "$NODE_URL/status" 2>/dev/null)
echo "$STATUS" | python3 -c "
import sys, json
try:
    s = json.load(sys.stdin)
    round_num = s.get('dag_round', 0)
    epoch_length = 210000  # EPOCH_LENGTH_ROUNDS constant
    current_epoch = round_num // epoch_length
    rounds_until_next = epoch_length - (round_num % epoch_length)
    
    print(f'  Current round: {round_num}')
    print(f'  Current epoch: {current_epoch}')
    print(f'  Rounds until next epoch: {rounds_until_next:,}')
    print(f'  Epoch progress: {(round_num % epoch_length) / epoch_length * 100:.2f}%')
except Exception as e:
    print(f'  Parse error: {e}')
" | tee -a "$LOG_FILE"

echo "" | tee -a "$LOG_FILE"

# Test 3: Attempt to stake (will fail without funds, but tests endpoint)
echo "TEST 3: Stake Endpoint Test" | tee -a "$LOG_FILE"
echo "============================" | tee -a "$LOG_FILE"

# Generate a test keypair
TEST_SECRET=$(openssl rand -hex 32)

echo "  Testing stake endpoint with random keypair..." | tee -a "$LOG_FILE"
STAKE_RESPONSE=$(curl -s -X POST "$NODE_URL/stake" \
    -H "Content-Type: application/json" \
    -d "{
        \"from_secret\": \"$TEST_SECRET\",
        \"amount\": 1000000000000
    }" 2>/dev/null)

echo "$STAKE_RESPONSE" | python3 -c "
import sys, json
try:
    r = json.load(sys.stdin)
    if 'error' in r:
        if 'insufficient balance' in r['error']:
            print('  ✅ Stake endpoint functional (rejected due to insufficient balance)')
        elif 'below minimum' in r['error']:
            print('  ✅ Stake endpoint functional (rejected due to below minimum stake)')
        else:
            print(f'  ⚠️  Error: {r[\"error\"]}')
    elif 'hash' in r:
        print(f'  ✅ Stake transaction accepted: {r[\"hash\"][:16]}...')
    else:
        print(f'  ❓ Unexpected response')
except Exception as e:
    print(f'  ❌ Parse error: {e}')
" | tee -a "$LOG_FILE"

echo "" | tee -a "$LOG_FILE"

# Test 4: Attempt to unstake (will fail without stake, but tests endpoint)
echo "TEST 4: Unstake Endpoint Test" | tee -a "$LOG_FILE"
echo "==============================" | tee -a "$LOG_FILE"

echo "  Testing unstake endpoint with random keypair..." | tee -a "$LOG_FILE"
UNSTAKE_RESPONSE=$(curl -s -X POST "$NODE_URL/unstake" \
    -H "Content-Type: application/json" \
    -d "{
        \"from_secret\": \"$TEST_SECRET\"
    }" 2>/dev/null)

echo "$UNSTAKE_RESPONSE" | python3 -c "
import sys, json
try:
    r = json.load(sys.stdin)
    if 'error' in r:
        if 'no stake' in r['error'].lower() or 'nothing to unstake' in r['error'].lower():
            print('  ✅ Unstake endpoint functional (rejected due to no stake)')
        else:
            print(f'  ⚠️  Error: {r[\"error\"]}')
    elif 'hash' in r:
        print(f'  ✅ Unstake transaction accepted: {r[\"hash\"][:16]}...')
    else:
        print(f'  ❓ Unexpected response')
except Exception as e:
    print(f'  ❌ Parse error: {e}')
" | tee -a "$LOG_FILE"

echo "" | tee -a "$LOG_FILE"

# Test 5: Check minimum stake requirement
echo "TEST 5: Minimum Stake Requirement" | tee -a "$LOG_FILE"
echo "==================================" | tee -a "$LOG_FILE"

echo "  MIN_STAKE_SATS = 10,000 UDAG (1,000,000,000 sats)" | tee -a "$LOG_FILE"
echo "  Testing stake below minimum..." | tee -a "$LOG_FILE"

BELOW_MIN_RESPONSE=$(curl -s -X POST "$NODE_URL/stake" \
    -H "Content-Type: application/json" \
    -d "{
        \"from_secret\": \"$TEST_SECRET\",
        \"amount\": 500000000
    }" 2>/dev/null)

echo "$BELOW_MIN_RESPONSE" | python3 -c "
import sys, json
try:
    r = json.load(sys.stdin)
    if 'error' in r:
        if 'below minimum' in r['error'] or 'minimum stake' in r['error']:
            print('  ✅ Correctly rejected (below minimum stake)')
        elif 'insufficient balance' in r['error']:
            print('  ⚠️  Balance check before minimum check (acceptable)')
        else:
            print(f'  ⚠️  Error: {r[\"error\"]}')
    else:
        print(f'  ❌ Should have been rejected')
except Exception as e:
    print(f'  ❌ Parse error: {e}')
" | tee -a "$LOG_FILE"

echo "" | tee -a "$LOG_FILE"
echo "=== Staking Test Complete ===" | tee -a "$LOG_FILE"
echo "End time: $(date)" | tee -a "$LOG_FILE"
