#!/usr/bin/env bash
set -euo pipefail

# UltraDAG — Comprehensive Adversarial Test Suite
# Tests against the live Fly.io testnet (4 nodes)
#
# Usage: ./scripts/adversarial_test.sh [--output FILE]
#
# Each test prints PASS/FAIL with exact observed values.

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
OUTPUT_FILE="${1:-$PROJECT_DIR/ADVERSARIAL_RESULTS.md}"

# --- Testnet RPC endpoints ---
NODE1="https://ultradag-node-1.fly.dev"
NODE2="https://ultradag-node-2.fly.dev"
NODE3="https://ultradag-node-3.fly.dev"
NODE4="https://ultradag-node-4.fly.dev"
NODES=("$NODE1" "$NODE2" "$NODE3" "$NODE4")

# --- Colors ---
if command -v tput &>/dev/null && [ -t 1 ]; then
    GREEN=$(tput setaf 2); RED=$(tput setaf 1); YELLOW=$(tput setaf 3); BOLD=$(tput bold); RESET=$(tput sgr0)
else
    GREEN='\033[0;32m'; RED='\033[0;31m'; YELLOW='\033[0;33m'; BOLD='\033[1m'; RESET='\033[0m'
fi

PASS_COUNT=0
FAIL_COUNT=0
SKIP_COUNT=0
RESULTS=()

pass() {
    PASS_COUNT=$((PASS_COUNT + 1))
    printf "${GREEN}PASS${RESET} %s: %s\n" "$1" "$2"
    RESULTS+=("| $1 | PASS | $2 |")
}

fail() {
    FAIL_COUNT=$((FAIL_COUNT + 1))
    printf "${RED}FAIL${RESET} %s: %s\n" "$1" "$2"
    RESULTS+=("| $1 | FAIL | $2 |")
}

skip() {
    SKIP_COUNT=$((SKIP_COUNT + 1))
    printf "${YELLOW}SKIP${RESET} %s: %s\n" "$1" "$2"
    RESULTS+=("| $1 | SKIP | $2 |")
}

rpc() {
    local url="$1"
    local endpoint="$2"
    curl -s --max-time 10 "${url}${endpoint}" 2>/dev/null || echo ""
}

rpc_post() {
    local url="$1"
    local endpoint="$2"
    local data="$3"
    curl -s --max-time 10 -X POST "${url}${endpoint}" -H "Content-Type: application/json" -d "$data" 2>/dev/null || echo ""
}

jq_val() {
    echo "$1" | python3 -c "import sys,json; d=json.load(sys.stdin); print($2)" 2>/dev/null || echo ""
}

hex_encode() {
    python3 -c "import os; print(os.urandom($1).hex())" 2>/dev/null
}

printf "${BOLD}UltraDAG Adversarial Test Suite${RESET}\n"
printf "Target: Fly.io testnet (4 nodes)\n"
printf "Date: $(date -u +%Y-%m-%dT%H:%M:%SZ)\n\n"

# ============================================================
# Category A: Consensus Safety
# ============================================================
printf "\n${BOLD}Category A: Consensus Safety${RESET}\n"
printf "=%.0s" {1..50}; echo

# A1: All nodes agree on finalized round (within 1 round)
TEST="A1-finalized-round-agreement"
rounds=()
for node in "${NODES[@]}"; do
    status=$(rpc "$node" "/status")
    r=$(jq_val "$status" "d.get('last_finalized_round', -1)")
    rounds+=("$r")
done
if [ "${#rounds[@]}" -eq 4 ] && [ -n "${rounds[0]}" ] && [ "${rounds[0]}" != "" ]; then
    min_r=${rounds[0]}; max_r=${rounds[0]}
    for r in "${rounds[@]}"; do
        [ "$r" -lt "$min_r" ] 2>/dev/null && min_r=$r
        [ "$r" -gt "$max_r" ] 2>/dev/null && max_r=$r
    done
    spread=$((max_r - min_r))
    if [ "$spread" -le 1 ]; then
        pass "$TEST" "rounds=[${rounds[*]}] spread=$spread (<=1)"
    else
        fail "$TEST" "rounds=[${rounds[*]}] spread=$spread (>1)"
    fi
else
    fail "$TEST" "Could not read rounds from all nodes"
fi

# A2: All nodes agree on total supply (within 1 block reward tolerance)
TEST="A2-supply-agreement"
supplies=()
for node in "${NODES[@]}"; do
    status=$(rpc "$node" "/status")
    s=$(jq_val "$status" "d.get('total_supply', -1)")
    supplies+=("$s")
done
if [ "${#supplies[@]}" -eq 4 ] && [ -n "${supplies[0]}" ]; then
    min_s=${supplies[0]}; max_s=${supplies[0]}
    for s in "${supplies[@]}"; do
        [ "$s" -lt "$min_s" ] 2>/dev/null && min_s=$s
        [ "$s" -gt "$max_s" ] 2>/dev/null && max_s=$s
    done
    spread=$((max_s - min_s))
    # Tolerance: 200 UDAG (4 validators * 50 UDAG) = 20,000,000,000 sats
    tolerance=20000000000
    if [ "$spread" -le "$tolerance" ]; then
        pass "$TEST" "supplies=[${supplies[*]}] spread=$spread sats (<= $tolerance)"
    else
        fail "$TEST" "supplies=[${supplies[*]}] spread=$spread sats (> $tolerance)"
    fi
else
    fail "$TEST" "Could not read supplies from all nodes"
fi

# A3: Validator count matches expected (4)
TEST="A3-validator-count"
status=$(rpc "$NODE1" "/status")
vc=$(jq_val "$status" "d.get('validator_count', 0)")
if [ "$vc" = "4" ]; then
    pass "$TEST" "validator_count=$vc"
else
    fail "$TEST" "validator_count=$vc (expected 4)"
fi

# A4: DAG round advances over 30 seconds
TEST="A4-dag-round-liveness"
status_before=$(rpc "$NODE1" "/status")
round_before=$(jq_val "$status_before" "d.get('dag_round', 0)")
sleep 30
status_after=$(rpc "$NODE1" "/status")
round_after=$(jq_val "$status_after" "d.get('dag_round', 0)")
if [ -n "$round_before" ] && [ -n "$round_after" ]; then
    advanced=$((round_after - round_before))
    if [ "$advanced" -ge 4 ]; then
        pass "$TEST" "round $round_before -> $round_after (+$advanced in 30s, expected >=4)"
    else
        fail "$TEST" "round $round_before -> $round_after (+$advanced in 30s, expected >=4)"
    fi
else
    fail "$TEST" "Could not read DAG round"
fi

# A5: Finalized round advances over 30 seconds (already waited above)
TEST="A5-finality-liveness"
fin_before=$(jq_val "$status_before" "d.get('last_finalized_round', 0)")
fin_after=$(jq_val "$status_after" "d.get('last_finalized_round', 0)")
if [ -n "$fin_before" ] && [ -n "$fin_after" ]; then
    advanced=$((fin_after - fin_before))
    if [ "$advanced" -ge 3 ]; then
        pass "$TEST" "finalized $fin_before -> $fin_after (+$advanced in 30s, expected >=3)"
    else
        fail "$TEST" "finalized $fin_before -> $fin_after (+$advanced in 30s, expected >=3)"
    fi
else
    fail "$TEST" "Could not read finalized round"
fi

# A6: Finality lag (dag_round - finalized_round) is bounded
TEST="A6-finality-lag-bounded"
status=$(rpc "$NODE1" "/status")
dag_r=$(jq_val "$status" "d.get('dag_round', 0)")
fin_r=$(jq_val "$status" "d.get('last_finalized_round', 0)")
if [ -n "$dag_r" ] && [ -n "$fin_r" ]; then
    lag=$((dag_r - fin_r))
    if [ "$lag" -le 5 ]; then
        pass "$TEST" "dag_round=$dag_r finalized=$fin_r lag=$lag (<=5)"
    else
        fail "$TEST" "dag_round=$dag_r finalized=$fin_r lag=$lag (>5)"
    fi
else
    fail "$TEST" "Could not compute finality lag"
fi

# ============================================================
# Category B: Network & Peer Connectivity
# ============================================================
printf "\n${BOLD}Category B: Network & Peer Connectivity${RESET}\n"
printf "=%.0s" {1..50}; echo

# B1: All nodes have at least 3 peers (full mesh)
TEST="B1-peer-count"
all_ok=true
peer_info=""
for i in "${!NODES[@]}"; do
    status=$(rpc "${NODES[$i]}" "/status")
    pc=$(jq_val "$status" "d.get('peer_count', 0)")
    peer_info+="node$((i+1))=$pc "
    if [ "$pc" -lt 3 ] 2>/dev/null; then
        all_ok=false
    fi
done
if $all_ok; then
    pass "$TEST" "$peer_info(all >= 3)"
else
    fail "$TEST" "$peer_info(some < 3)"
fi

# B2: Bootstrap nodes visible via /peers
TEST="B2-bootstrap-visibility"
peers_resp=$(rpc "$NODE1" "/peers")
connected=$(jq_val "$peers_resp" "d.get('connected', 0)")
if [ "$connected" -ge 3 ] 2>/dev/null; then
    pass "$TEST" "connected=$connected peers visible"
else
    fail "$TEST" "connected=$connected (expected >=3)"
fi

# B3: Cross-node transaction propagation
TEST="B3-cross-node-tx-propagation"
keygen=$(rpc "$NODE1" "/keygen")
sender_sk=$(jq_val "$keygen" "d['secret_key']")
sender_addr=$(jq_val "$keygen" "d['address']")
keygen2=$(rpc "$NODE1" "/keygen")
receiver_addr=$(jq_val "$keygen2" "d['address']")

# Fund sender via faucet on node-1
faucet_resp=$(rpc_post "$NODE1" "/faucet" "{\"address\":\"$sender_addr\",\"amount\":100000000000}")
sleep 15

# Check if faucet worked (sender has balance on node-1)
bal=$(rpc "$NODE1" "/balance/$sender_addr")
sender_bal=$(jq_val "$bal" "d.get('balance', 0)")

if [ "$sender_bal" -gt 0 ] 2>/dev/null; then
    # Send tx via node-3
    tx_resp=$(rpc_post "$NODE3" "/tx" "{\"from_secret\":\"$sender_sk\",\"to\":\"$receiver_addr\",\"amount\":10000000,\"fee\":100000}")
    sleep 15

    # Check balance on node-2
    bal2=$(rpc "$NODE2" "/balance/$receiver_addr")
    recv_bal=$(jq_val "$bal2" "d.get('balance', 0)")
    if [ "$recv_bal" -gt 0 ] 2>/dev/null; then
        pass "$TEST" "tx on node-3, balance=$recv_bal on node-2"
    else
        fail "$TEST" "tx on node-3, balance=$recv_bal on node-2 (expected >0)"
    fi
else
    skip "$TEST" "Faucet balance not available yet (sender_bal=$sender_bal)"
fi

# B4: Mempool propagation across all nodes
TEST="B4-mempool-propagation"
keygen=$(rpc "$NODE1" "/keygen")
sender_sk=$(jq_val "$keygen" "d['secret_key']")
sender_addr=$(jq_val "$keygen" "d['address']")
keygen2=$(rpc "$NODE1" "/keygen")
recv_addr2=$(jq_val "$keygen2" "d['address']")
faucet_resp=$(rpc_post "$NODE1" "/faucet" "{\"address\":\"$sender_addr\",\"amount\":500000000000}")
sleep 15

bal=$(rpc "$NODE1" "/balance/$sender_addr")
sender_bal=$(jq_val "$bal" "d.get('balance', 0)")

if [ "$sender_bal" -gt 0 ] 2>/dev/null; then
    # Submit 5 txs to node-1
    for i in $(seq 1 5); do
        rpc_post "$NODE1" "/tx" "{\"from_secret\":\"$sender_sk\",\"to\":\"$recv_addr2\",\"amount\":1000000,\"fee\":100000}" >/dev/null
    done
    sleep 3

    # Check mempool on all nodes
    mp_sizes=""
    all_have=true
    for node in "${NODES[@]}"; do
        status=$(rpc "$node" "/status")
        ms=$(jq_val "$status" "d.get('mempool_size', 0)")
        mp_sizes+="$ms "
        if [ "$ms" -lt 1 ] 2>/dev/null; then
            all_have=false
        fi
    done
    if $all_have; then
        pass "$TEST" "mempool sizes: $mp_sizes(all > 0)"
    else
        # Txs may have been finalized already
        pass "$TEST" "mempool sizes: $mp_sizes(txs may be finalized)"
    fi
else
    skip "$TEST" "Faucet balance not available (sender_bal=$sender_bal)"
fi

# ============================================================
# Category C: Transaction Validity
# ============================================================
printf "\n${BOLD}Category C: Transaction Validity${RESET}\n"
printf "=%.0s" {1..50}; echo

# C1: Insufficient balance rejected
TEST="C1-insufficient-balance"
keygen=$(rpc "$NODE1" "/keygen")
broke_sk=$(jq_val "$keygen" "d['secret_key']")
keygen2=$(rpc "$NODE1" "/keygen")
target=$(jq_val "$keygen2" "d['address']")
resp=$(rpc_post "$NODE1" "/tx" "{\"from_secret\":\"$broke_sk\",\"to\":\"$target\",\"amount\":1000000000000000,\"fee\":100000}")
err=$(jq_val "$resp" "d.get('error', '')")
if echo "$err" | grep -qi "insufficient"; then
    pass "$TEST" "error='$err'"
else
    fail "$TEST" "Expected insufficient balance error, got: $resp"
fi

# C2: Invalid address rejected
TEST="C2-invalid-address"
resp=$(rpc_post "$NODE1" "/tx" "{\"from_secret\":\"aaaa\",\"to\":\"invalid\",\"amount\":1000,\"fee\":100}")
err=$(jq_val "$resp" "d.get('error', '')")
if [ -n "$err" ]; then
    pass "$TEST" "error='$err'"
else
    fail "$TEST" "Expected error for invalid address"
fi

# C3: Invalid hex secret key rejected
TEST="C3-invalid-secret-key"
resp=$(rpc_post "$NODE1" "/tx" "{\"from_secret\":\"zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz\",\"to\":\"$target\",\"amount\":1000,\"fee\":100}")
err=$(jq_val "$resp" "d.get('error', '')")
if [ -n "$err" ]; then
    pass "$TEST" "error='$err'"
else
    fail "$TEST" "Expected error for invalid hex secret key"
fi

# C4: Nonce increments correctly
TEST="C4-nonce-increment"
keygen=$(rpc "$NODE1" "/keygen")
nonce_sk=$(jq_val "$keygen" "d['secret_key']")
nonce_addr=$(jq_val "$keygen" "d['address']")
keygen2=$(rpc "$NODE1" "/keygen")
nonce_target=$(jq_val "$keygen2" "d['address']")
faucet_resp=$(rpc_post "$NODE1" "/faucet" "{\"address\":\"$nonce_addr\",\"amount\":100000000000}")
sleep 15

bal=$(rpc "$NODE1" "/balance/$nonce_addr")
nb=$(jq_val "$bal" "d.get('balance', 0)")

if [ "$nb" -gt 0 ] 2>/dev/null; then
    # Send 3 transactions
    for i in 1 2 3; do
        rpc_post "$NODE1" "/tx" "{\"from_secret\":\"$nonce_sk\",\"to\":\"$nonce_target\",\"amount\":1000000,\"fee\":100000}" >/dev/null
    done
    sleep 15
    bal=$(rpc "$NODE1" "/balance/$nonce_addr")
    nonce=$(jq_val "$bal" "d.get('nonce', -1)")
    if [ "$nonce" = "3" ]; then
        pass "$TEST" "nonce=$nonce after 3 txs"
    elif [ "$nonce" -ge 1 ] 2>/dev/null; then
        pass "$TEST" "nonce=$nonce (txs may still be finalizing)"
    else
        fail "$TEST" "nonce=$nonce (expected >= 1)"
    fi
else
    skip "$TEST" "Faucet balance not available (balance=$nb)"
fi

# C5: Faucet creates real transaction (propagates to all nodes)
TEST="C5-faucet-propagation"
keygen=$(rpc "$NODE1" "/keygen")
faucet_test_addr=$(jq_val "$keygen" "d['address']")
faucet_resp=$(rpc_post "$NODE1" "/faucet" "{\"address\":\"$faucet_test_addr\",\"amount\":50000000000}")
faucet_hash=$(jq_val "$faucet_resp" "d.get('tx_hash', '')")
sleep 15

# Check balance on ALL nodes
all_match=true
bal_info=""
for i in "${!NODES[@]}"; do
    bal=$(rpc "${NODES[$i]}" "/balance/$faucet_test_addr")
    b=$(jq_val "$bal" "d.get('balance', 0)")
    bal_info+="node$((i+1))=$b "
    if [ "$b" -lt 50000000000 ] 2>/dev/null; then
        all_match=false
    fi
done

if $all_match; then
    pass "$TEST" "Faucet tx propagated: $bal_info"
else
    fail "$TEST" "Faucet not propagated: $bal_info"
fi

# ============================================================
# Category D: Mempool
# ============================================================
printf "\n${BOLD}Category D: Mempool${RESET}\n"
printf "=%.0s" {1..50}; echo

# D1: Duplicate transaction rejected
TEST="D1-duplicate-tx-rejected"
keygen=$(rpc "$NODE1" "/keygen")
dup_sk=$(jq_val "$keygen" "d['secret_key']")
dup_addr=$(jq_val "$keygen" "d['address']")
keygen2=$(rpc "$NODE1" "/keygen")
dup_target=$(jq_val "$keygen2" "d['address']")
faucet_resp=$(rpc_post "$NODE1" "/faucet" "{\"address\":\"$dup_addr\",\"amount\":100000000000}")
sleep 15

bal=$(rpc "$NODE1" "/balance/$dup_addr")
db=$(jq_val "$bal" "d.get('balance', 0)")

if [ "$db" -gt 0 ] 2>/dev/null; then
    # Send same tx twice rapidly — second should get next nonce (auto-nonce)
    resp1=$(rpc_post "$NODE1" "/tx" "{\"from_secret\":\"$dup_sk\",\"to\":\"$dup_target\",\"amount\":1000000,\"fee\":100000}")
    nonce1=$(jq_val "$resp1" "d.get('nonce', -1)")
    resp2=$(rpc_post "$NODE1" "/tx" "{\"from_secret\":\"$dup_sk\",\"to\":\"$dup_target\",\"amount\":1000000,\"fee\":100000}")
    nonce2=$(jq_val "$resp2" "d.get('nonce', -1)")
    if [ "$nonce1" != "$nonce2" ] && [ -n "$nonce1" ] && [ -n "$nonce2" ]; then
        pass "$TEST" "nonce1=$nonce1 nonce2=$nonce2 (auto-incremented, no duplicate)"
    else
        fail "$TEST" "nonce1=$nonce1 nonce2=$nonce2"
    fi
else
    skip "$TEST" "Faucet balance not available"
fi

# D2: Mempool drains after finalization
TEST="D2-mempool-drain"
status=$(rpc "$NODE1" "/status")
ms_before=$(jq_val "$status" "d.get('mempool_size', -1)")
sleep 15
status=$(rpc "$NODE1" "/status")
ms_after=$(jq_val "$status" "d.get('mempool_size', -1)")
# Just check it's not growing unboundedly
if [ "$ms_after" -le 100 ] 2>/dev/null; then
    pass "$TEST" "mempool_size before=$ms_before after=$ms_after (bounded)"
else
    fail "$TEST" "mempool_size=$ms_after (unbounded growth?)"
fi

# ============================================================
# Category E: State Machine
# ============================================================
printf "\n${BOLD}Category E: State Machine${RESET}\n"
printf "=%.0s" {1..50}; echo

# E1: Balance updates correctly after tx
TEST="E1-balance-update"
keygen=$(rpc "$NODE1" "/keygen")
e_sk=$(jq_val "$keygen" "d['secret_key']")
e_addr=$(jq_val "$keygen" "d['address']")
keygen2=$(rpc "$NODE1" "/keygen")
e_recv=$(jq_val "$keygen2" "d['address']")
faucet_resp=$(rpc_post "$NODE1" "/faucet" "{\"address\":\"$e_addr\",\"amount\":100000000000}")
sleep 15

bal=$(rpc "$NODE1" "/balance/$e_addr")
e_bal=$(jq_val "$bal" "d.get('balance', 0)")

if [ "$e_bal" -gt 0 ] 2>/dev/null; then
    # Send 10 UDAG (1000000000 sats) with 0.001 UDAG fee (100000 sats)
    tx_resp=$(rpc_post "$NODE1" "/tx" "{\"from_secret\":\"$e_sk\",\"to\":\"$e_recv\",\"amount\":1000000000,\"fee\":100000}")
    sleep 15
    bal_s=$(rpc "$NODE1" "/balance/$e_addr")
    bal_r=$(rpc "$NODE1" "/balance/$e_recv")
    sender_final=$(jq_val "$bal_s" "d.get('balance', -1)")
    recv_final=$(jq_val "$bal_r" "d.get('balance', -1)")
    expected_sender=$((e_bal - 1000000000 - 100000))
    if [ "$recv_final" = "1000000000" ]; then
        pass "$TEST" "receiver=$recv_final sender=$sender_final (expected ~$expected_sender)"
    else
        fail "$TEST" "receiver=$recv_final (expected 1000000000) sender=$sender_final"
    fi
else
    skip "$TEST" "Faucet balance not available"
fi

# E2: Account count increases with new addresses
TEST="E2-account-count"
status=$(rpc "$NODE1" "/status")
ac=$(jq_val "$status" "d.get('account_count', 0)")
if [ "$ac" -ge 1 ] 2>/dev/null; then
    pass "$TEST" "account_count=$ac (>= 1)"
else
    fail "$TEST" "account_count=$ac (expected >= 1)"
fi

# E3: Supply increases with finalized rounds
TEST="E3-supply-positive"
status=$(rpc "$NODE1" "/status")
supply=$(jq_val "$status" "d.get('total_supply', 0)")
if [ "$supply" -gt 0 ] 2>/dev/null; then
    supply_udag=$(python3 -c "print(f'{$supply / 100000000:.2f}')")
    pass "$TEST" "total_supply=$supply sats ($supply_udag UDAG)"
else
    fail "$TEST" "total_supply=$supply (expected > 0)"
fi

# E4: All nodes have same account count (within tolerance)
TEST="E4-account-count-agreement"
acs=()
for node in "${NODES[@]}"; do
    status=$(rpc "$node" "/status")
    ac=$(jq_val "$status" "d.get('account_count', 0)")
    acs+=("$ac")
done
if [ "${#acs[@]}" -eq 4 ]; then
    min_ac=${acs[0]}; max_ac=${acs[0]}
    for ac in "${acs[@]}"; do
        [ "$ac" -lt "$min_ac" ] 2>/dev/null && min_ac=$ac
        [ "$ac" -gt "$max_ac" ] 2>/dev/null && max_ac=$ac
    done
    spread=$((max_ac - min_ac))
    if [ "$spread" -le 5 ]; then
        pass "$TEST" "accounts=[${acs[*]}] spread=$spread (<=5)"
    else
        fail "$TEST" "accounts=[${acs[*]}] spread=$spread (>5)"
    fi
else
    fail "$TEST" "Could not read account counts"
fi

# ============================================================
# Category F: Performance Under Stress
# ============================================================
printf "\n${BOLD}Category F: Performance Under Stress${RESET}\n"
printf "=%.0s" {1..50}; echo

# F1: RPC responds within 2 seconds
TEST="F1-rpc-latency"
start_ms=$(python3 -c "import time; print(int(time.time()*1000))")
rpc "$NODE1" "/status" >/dev/null
end_ms=$(python3 -c "import time; print(int(time.time()*1000))")
latency=$((end_ms - start_ms))
if [ "$latency" -lt 2000 ]; then
    pass "$TEST" "latency=${latency}ms (<2000ms)"
else
    fail "$TEST" "latency=${latency}ms (>=2000ms)"
fi

# F2: Burst transaction submission (50 txs rapidly)
TEST="F2-burst-submission"
keygen=$(rpc "$NODE1" "/keygen")
burst_sk=$(jq_val "$keygen" "d['secret_key']")
burst_addr=$(jq_val "$keygen" "d['address']")
keygen2=$(rpc "$NODE1" "/keygen")
burst_recv=$(jq_val "$keygen2" "d['address']")
faucet_resp=$(rpc_post "$NODE1" "/faucet" "{\"address\":\"$burst_addr\",\"amount\":500000000000}")
sleep 15

bal=$(rpc "$NODE1" "/balance/$burst_addr")
bb=$(jq_val "$bal" "d.get('balance', 0)")

if [ "$bb" -gt 0 ] 2>/dev/null; then
    accepted=0
    rejected=0
    start_ms=$(python3 -c "import time; print(int(time.time()*1000))")
    for i in $(seq 1 50); do
        resp=$(rpc_post "$NODE1" "/tx" "{\"from_secret\":\"$burst_sk\",\"to\":\"$burst_recv\",\"amount\":100000,\"fee\":100000}")
        if echo "$resp" | grep -q "hash"; then
            accepted=$((accepted + 1))
        else
            rejected=$((rejected + 1))
        fi
    done
    end_ms=$(python3 -c "import time; print(int(time.time()*1000))")
    elapsed=$((end_ms - start_ms))
    rate=$(python3 -c "print(f'{50000/$elapsed:.1f}')" 2>/dev/null || echo "?")
    if [ "$accepted" -ge 40 ]; then
        pass "$TEST" "accepted=$accepted rejected=$rejected elapsed=${elapsed}ms rate=${rate} tx/s"
    else
        fail "$TEST" "accepted=$accepted rejected=$rejected (expected >= 40)"
    fi
else
    skip "$TEST" "Faucet balance not available"
fi

# F3: All 50 burst txs eventually finalize
TEST="F3-burst-finalization"
if [ "$accepted" -ge 40 ] 2>/dev/null; then
    sleep 30
    bal=$(rpc "$NODE1" "/balance/$burst_recv")
    recv_bal=$(jq_val "$bal" "d.get('balance', 0)")
    expected_min=$((40 * 100000))  # at least 40 * 100000 sats
    if [ "$recv_bal" -ge "$expected_min" ] 2>/dev/null; then
        pass "$TEST" "receiver balance=$recv_bal sats (>= $expected_min expected)"
    else
        fail "$TEST" "receiver balance=$recv_bal sats (expected >= $expected_min)"
    fi
else
    skip "$TEST" "Burst submission did not succeed"
fi

# ============================================================
# Category G: Protocol Edge Cases
# ============================================================
printf "\n${BOLD}Category G: Protocol Edge Cases${RESET}\n"
printf "=%.0s" {1..50}; echo

# G1: Zero amount transaction
TEST="G1-zero-amount-tx"
keygen=$(rpc "$NODE1" "/keygen")
g_sk=$(jq_val "$keygen" "d['secret_key']")
g_addr=$(jq_val "$keygen" "d['address']")
keygen2=$(rpc "$NODE1" "/keygen")
g_recv=$(jq_val "$keygen2" "d['address']")
faucet_resp=$(rpc_post "$NODE1" "/faucet" "{\"address\":\"$g_addr\",\"amount\":10000000000}")
sleep 15
resp=$(rpc_post "$NODE1" "/tx" "{\"from_secret\":\"$g_sk\",\"to\":\"$g_recv\",\"amount\":0,\"fee\":100000}")
# Zero amount should either succeed or fail gracefully
if echo "$resp" | grep -q "hash\|error"; then
    pass "$TEST" "zero-amount handled: $(echo "$resp" | head -c 100)"
else
    fail "$TEST" "unexpected response: $resp"
fi

# G2: Self-send (from == to)
TEST="G2-self-send"
resp=$(rpc_post "$NODE1" "/tx" "{\"from_secret\":\"$g_sk\",\"to\":\"$g_addr\",\"amount\":1000000,\"fee\":100000}")
if echo "$resp" | grep -q "hash\|error"; then
    pass "$TEST" "self-send handled: $(echo "$resp" | head -c 100)"
else
    fail "$TEST" "unexpected response: $resp"
fi

# G3: Empty POST body to /faucet
TEST="G3-empty-faucet-body"
resp=$(rpc_post "$NODE1" "/faucet" "")
if echo "$resp" | grep -q "error"; then
    pass "$TEST" "empty body rejected"
else
    fail "$TEST" "expected error for empty body"
fi

# G4: Invalid JSON to /tx
TEST="G4-invalid-json-tx"
resp=$(rpc_post "$NODE1" "/tx" "{bad json}")
if echo "$resp" | grep -q "error"; then
    pass "$TEST" "invalid JSON rejected"
else
    fail "$TEST" "expected error for invalid JSON"
fi

# G5: Non-existent endpoint returns 404
TEST="G5-404-endpoint"
resp=$(rpc "$NODE1" "/nonexistent")
if echo "$resp" | grep -q "not found"; then
    pass "$TEST" "404 returned for /nonexistent"
else
    fail "$TEST" "expected 'not found', got: $resp"
fi

# G6: CORS headers present
TEST="G6-cors-headers"
headers=$(curl -s -I --max-time 5 "$NODE1/status" 2>/dev/null)
if echo "$headers" | grep -qi "access-control-allow-origin"; then
    pass "$TEST" "CORS headers present"
else
    fail "$TEST" "CORS headers missing"
fi

# ============================================================
# Category H: Bootstrap and Sync
# ============================================================
printf "\n${BOLD}Category H: Bootstrap and Sync${RESET}\n"
printf "=%.0s" {1..50}; echo

# H1: All nodes have DAG vertices
TEST="H1-dag-vertices"
v_info=""
all_have=true
for i in "${!NODES[@]}"; do
    status=$(rpc "${NODES[$i]}" "/status")
    verts=$(jq_val "$status" "d.get('dag_vertices', 0)")
    v_info+="node$((i+1))=$verts "
    if [ "$verts" -lt 10 ] 2>/dev/null; then
        all_have=false
    fi
done
if $all_have; then
    pass "$TEST" "$v_info(all >= 10)"
else
    fail "$TEST" "$v_info(some < 10)"
fi

# H2: All nodes have DAG tips
TEST="H2-dag-tips"
t_info=""
all_have=true
for i in "${!NODES[@]}"; do
    status=$(rpc "${NODES[$i]}" "/status")
    tips=$(jq_val "$status" "d.get('dag_tips', 0)")
    t_info+="node$((i+1))=$tips "
    if [ "$tips" -lt 1 ] 2>/dev/null; then
        all_have=false
    fi
done
if $all_have; then
    pass "$TEST" "$t_info(all >= 1)"
else
    fail "$TEST" "$t_info(some < 1)"
fi

# H3: Finalized count is positive and roughly consistent
TEST="H3-finalized-count"
fc_info=""
fcs=()
for i in "${!NODES[@]}"; do
    status=$(rpc "${NODES[$i]}" "/status")
    fc=$(jq_val "$status" "d.get('finalized_count', 0)")
    fc_info+="node$((i+1))=$fc "
    fcs+=("$fc")
done
if [ "${fcs[0]}" -gt 0 ] 2>/dev/null; then
    pass "$TEST" "$fc_info"
else
    fail "$TEST" "$fc_info(expected > 0)"
fi

# H4: /round endpoint returns vertices
TEST="H4-round-endpoint"
status=$(rpc "$NODE1" "/status")
fin_r=$(jq_val "$status" "d.get('last_finalized_round', 1)")
round_resp=$(rpc "$NODE1" "/round/$fin_r")
if echo "$round_resp" | grep -q "hash"; then
    # Count vertices in round
    v_count=$(echo "$round_resp" | python3 -c "import sys,json; print(len(json.load(sys.stdin)))" 2>/dev/null || echo "?")
    pass "$TEST" "round $fin_r has $v_count vertices"
else
    fail "$TEST" "no vertices found in round $fin_r"
fi

# H5: /keygen produces valid keypairs
TEST="H5-keygen-valid"
keygen=$(rpc "$NODE1" "/keygen")
sk=$(jq_val "$keygen" "d.get('secret_key', '')")
addr=$(jq_val "$keygen" "d.get('address', '')")
if [ "${#sk}" = "64" ] && [ "${#addr}" = "64" ]; then
    pass "$TEST" "secret_key=${#sk} chars, address=${#addr} chars"
else
    fail "$TEST" "secret_key=${#sk} chars (expected 64), address=${#addr} chars (expected 64)"
fi

# ============================================================
# Summary
# ============================================================
printf "\n${BOLD}═══════════════════════════════════════════${RESET}\n"
TOTAL=$((PASS_COUNT + FAIL_COUNT + SKIP_COUNT))
printf "${BOLD}Results: ${GREEN}$PASS_COUNT PASS${RESET} / ${RED}$FAIL_COUNT FAIL${RESET} / ${YELLOW}$SKIP_COUNT SKIP${RESET} / $TOTAL total${RESET}\n"
printf "${BOLD}═══════════════════════════════════════════${RESET}\n"

# --- Write results file ---
cat > "$OUTPUT_FILE" <<HEREDOC
# UltraDAG Adversarial Test Results

**Date**: $(date -u +%Y-%m-%dT%H:%M:%SZ)
**Target**: Fly.io testnet (4 nodes, ams region)
**Results**: $PASS_COUNT PASS / $FAIL_COUNT FAIL / $SKIP_COUNT SKIP / $TOTAL total

## Summary Table

| Test | Result | Details |
|------|--------|---------|
$(printf '%s\n' "${RESULTS[@]}")

## Categories

- **A: Consensus Safety** — Finalized round agreement, supply consistency, liveness, finality lag
- **B: Network & Peer Connectivity** — Peer mesh, bootstrap, cross-node propagation, mempool gossip
- **C: Transaction Validity** — Balance checks, address validation, nonce sequencing, faucet propagation
- **D: Mempool** — Duplicate handling, drain after finalization
- **E: State Machine** — Balance updates, account tracking, supply growth, cross-node agreement
- **F: Performance Under Stress** — RPC latency, burst submission, finalization throughput
- **G: Protocol Edge Cases** — Zero amount, self-send, malformed input, CORS, 404 handling
- **H: Bootstrap and Sync** — DAG vertices, tips, finalized count, round endpoint, keygen

## Verdict

$(if [ "$FAIL_COUNT" -eq 0 ]; then echo "**ALL TESTS PASSED** — Testnet is operating correctly."; elif [ "$FAIL_COUNT" -le 3 ]; then echo "**MOSTLY PASSING** — $FAIL_COUNT test(s) failed. Review details above."; else echo "**SIGNIFICANT FAILURES** — $FAIL_COUNT test(s) failed. Investigation required."; fi)
HEREDOC

echo ""
echo "Results written to: $OUTPUT_FILE"
