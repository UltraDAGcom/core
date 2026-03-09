#!/bin/bash
# ============================================================
# UltraDAG Maximum Throughput Benchmark
# ============================================================
# Saturates the mempool, measures actual drain rate.
# Usage: ./scripts/max_throughput.sh [NUM_TXS] [CONCURRENCY] [RPC_URL] [ROUND_S]
# ============================================================
set -euo pipefail

NUM_TXS=${1:-9000}
CONCURRENCY=${2:-100}
RPC_URL=${3:-"https://ultradag-node-1.fly.dev"}
ROUND_S=${4:-5}

SENDER_SK="3f8efc5578b787786b059127f795a9ec4736880aa38aad974387e8a3423b15bd"
SENDER_ADDR="5c23f1097d7887ca6dbc2455e71ef2836772cde4ae58d3d9cf4be07b7d9b3f3f"
RECIPIENT="ba2b4e32f49d409b913f9845276533c67297967ab169e6ad86944d0ff3830b70"
TMPDIR=$(mktemp -d)
POLL_LOG="$TMPDIR/poll.csv"

cleanup() { rm -rf "$TMPDIR"; }
trap cleanup EXIT

echo "============================================================"
echo "  UltraDAG MAXIMUM Throughput Benchmark"
echo "============================================================"
echo "  Target:       $RPC_URL"
echo "  Transactions: $NUM_TXS"
echo "  Concurrency:  $CONCURRENCY"
echo "  Round time:   ${ROUND_S}s"
echo "  MAX_TXS/BLK:  10,000"
echo "  MAX_MEMPOOL:   10,000"
echo "============================================================"
echo ""

# ---- Step 1: Fund wallet ----
echo "[1/4] Funding wallet with 10,000,000 UDAG..."
# Each tx costs 100000 (amount) + 100000 (fee) = 200000 sats = 0.002 UDAG
# 9000 txs * 200000 = 1,800,000,000 sats = 18 UDAG  (plenty of headroom with 10M)
FUND_AMOUNT=1000000000000000  # 10,000,000 UDAG in sats
curl -sf -X POST "$RPC_URL/faucet" \
    -H "Content-Type: application/json" \
    -d "{\"address\":\"$SENDER_ADDR\",\"amount\":$FUND_AMOUNT}" > /dev/null
BALANCE=$(curl -sf "$RPC_URL/balance/$SENDER_ADDR" | python3 -c "import sys,json; print(json.load(sys.stdin)['balance'])")
echo "  Balance: $(python3 -c "print(f'{$BALANCE / 100_000_000:,.2f}')") UDAG"
echo ""

# ---- Step 2: Record baseline ----
echo "[2/4] Recording baseline..."
INIT_STATUS=$(curl -sf "$RPC_URL/status")
INIT_ROUND=$(echo "$INIT_STATUS" | python3 -c "import sys,json; print(json.load(sys.stdin)['dag_round'])")
INIT_FINALIZED=$(echo "$INIT_STATUS" | python3 -c "import sys,json; print(json.load(sys.stdin)['finalized_count'])")
echo "  Round:     $INIT_ROUND"
echo "  Finalized: $INIT_FINALIZED"
echo ""

# ---- Step 3: Blast 9000 transactions ----
echo "[3/4] Submitting $NUM_TXS transactions ($CONCURRENCY concurrent)..."

SUBMIT_START=$(python3 -c "import time; print(time.time())")

# Use xargs for maximum parallelism
seq 1 $NUM_TXS | xargs -P "$CONCURRENCY" -I{} \
    curl -sf -o "$TMPDIR/tx_{}.json" \
    -X POST "$RPC_URL/tx" \
    -H "Content-Type: application/json" \
    -d "{\"from_secret\":\"$SENDER_SK\",\"to\":\"$RECIPIENT\",\"amount\":100000,\"fee\":100000}" \
    2>/dev/null || true

SUBMIT_END=$(python3 -c "import time; print(time.time())")
SUBMIT_DUR=$(python3 -c "print(f'{$SUBMIT_END - $SUBMIT_START:.2f}')")

# Count results
SUCCESS=$(find "$TMPDIR" -name 'tx_*.json' -exec python3 -c "
import json,sys
try:
    d=json.load(open(sys.argv[1]))
    print('ok' if 'hash' in d else 'fail')
except:
    print('fail')
" {} \; 2>/dev/null | grep -c ok || echo 0)
TOTAL_FILES=$(find "$TMPDIR" -name 'tx_*.json' | wc -l | tr -d ' ')
FAIL=$((TOTAL_FILES - SUCCESS))

SUBMIT_TPS=$(python3 -c "d=$SUBMIT_END-$SUBMIT_START; print(f'{$SUCCESS/d:.1f}' if d>0 else 'inf')")
echo "  Accepted: $SUCCESS / $NUM_TXS in ${SUBMIT_DUR}s ($SUBMIT_TPS tx/s submission)"
if [ "$FAIL" -gt 0 ]; then
    echo "  Failed: $FAIL"
    # Show sample errors
    find "$TMPDIR" -name 'tx_*.json' | head -3 | while read f; do
        python3 -c "
import json
try:
    d=json.load(open('$f'))
    if 'error' in d: print(f'    -> {d[\"error\"]}')
except: pass" 2>/dev/null
    done
fi

# Check mempool immediately
POST_STATUS=$(curl -sf "$RPC_URL/status")
POST_ROUND=$(echo "$POST_STATUS" | python3 -c "import sys,json; print(json.load(sys.stdin)['dag_round'])")
POST_MEMPOOL=$(echo "$POST_STATUS" | python3 -c "import sys,json; print(json.load(sys.stdin)['mempool_size'])")
POST_FINALIZED=$(echo "$POST_STATUS" | python3 -c "import sys,json; print(json.load(sys.stdin)['finalized_count'])")
echo ""
echo "  Post-submission state:"
echo "    Round:     $POST_ROUND (advanced $((POST_ROUND - INIT_ROUND)) during submission)"
echo "    Mempool:   $POST_MEMPOOL"
echo "    Finalized: $POST_FINALIZED"
echo ""

# ---- Step 4: Poll drain rate at 500ms intervals ----
echo "[4/4] Measuring drain rate (polling every 500ms)..."
echo "timestamp,elapsed_ms,round,finalized,mempool" > "$POLL_LOG"

DRAIN_START=$(python3 -c "import time; print(time.time())")
PEAK_MEMPOOL=$POST_MEMPOOL
MAX_WAIT=300
ELAPSED_S=0

while true; do
    NOW=$(python3 -c "import time; print(time.time())")
    ELAPSED_MS=$(python3 -c "print(int(($NOW - $DRAIN_START) * 1000))")
    ELAPSED_S=$(python3 -c "print(f'{$NOW - $DRAIN_START:.1f}')")

    STATUS=$(curl -sf "$RPC_URL/status" 2>/dev/null || echo '{}')
    R=$(echo "$STATUS" | python3 -c "import sys,json; print(json.load(sys.stdin).get('dag_round',0))")
    F=$(echo "$STATUS" | python3 -c "import sys,json; print(json.load(sys.stdin).get('finalized_count',0))")
    M=$(echo "$STATUS" | python3 -c "import sys,json; print(json.load(sys.stdin).get('mempool_size',0))")

    echo "$NOW,$ELAPSED_MS,$R,$F,$M" >> "$POLL_LOG"

    DRAINED=$((PEAK_MEMPOOL - M))
    if [ "$PEAK_MEMPOOL" -gt 0 ]; then
        PCT=$((DRAINED * 100 / PEAK_MEMPOOL))
    else
        PCT=100
    fi
    printf "\r  [%3d%%] round=%s fin=%s mempool=%4s  (%ss)" "$PCT" "$R" "$F" "$M" "$ELAPSED_S"

    if [ "$M" -eq 0 ] && [ "$DRAINED" -gt 0 ]; then
        echo ""
        echo "  Mempool fully drained!"
        break
    fi

    # Safety timeout
    NOW_INT=$(python3 -c "print(int($NOW - $DRAIN_START))")
    if [ "$NOW_INT" -ge "$MAX_WAIT" ]; then
        echo ""
        echo "  TIMEOUT after ${MAX_WAIT}s (mempool=$M remaining)"
        break
    fi

    # 500ms sleep
    python3 -c "import time; time.sleep(0.5)"
done

DRAIN_END=$(python3 -c "import time; print(time.time())")
DRAIN_DUR=$(python3 -c "print(f'{$DRAIN_END - $DRAIN_START:.2f}')")

# ---- Final state ----
FINAL_STATUS=$(curl -sf "$RPC_URL/status")
FINAL_ROUND=$(echo "$FINAL_STATUS" | python3 -c "import sys,json; print(json.load(sys.stdin)['dag_round'])")
FINAL_FIN=$(echo "$FINAL_STATUS" | python3 -c "import sys,json; print(json.load(sys.stdin)['finalized_count'])")
FINAL_MEMPOOL=$(echo "$FINAL_STATUS" | python3 -c "import sys,json; print(json.load(sys.stdin)['mempool_size'])")
FINAL_SUPPLY=$(echo "$FINAL_STATUS" | python3 -c "import sys,json; print(json.load(sys.stdin)['total_supply'])")

ROUNDS_ELAPSED=$((FINAL_ROUND - INIT_ROUND))
FIN_ELAPSED=$((FINAL_FIN - INIT_FINALIZED))

# Throughput calculations
WALL_TPS=$(python3 -c "
dur = $DRAIN_END - $SUBMIT_START
print(f'{$SUCCESS / dur:.2f}' if dur > 0 else 'N/A')
")
DRAIN_TPS=$(python3 -c "
dur = $DRAIN_END - $DRAIN_START
print(f'{$PEAK_MEMPOOL / dur:.2f}' if dur > 0 else 'N/A')
")
ROUNDS_TPS=$(python3 -c "
rounds = $ROUNDS_ELAPSED
dur = rounds * $ROUND_S
print(f'{$SUCCESS / dur:.2f}' if dur > 0 else 'N/A')
")
TX_PER_ROUND=$(python3 -c "
print(f'{$SUCCESS / $ROUNDS_ELAPSED:.1f}' if $ROUNDS_ELAPSED > 0 else 'N/A')
")

# Verification
RECV_BAL=$(curl -sf "$RPC_URL/balance/$RECIPIENT" | python3 -c "import sys,json; print(json.load(sys.stdin)['balance'])")
EXPECTED=$((SUCCESS * 100000))

echo ""
echo "============================================================"
echo "  RESULTS (round_ms = $((ROUND_S * 1000)))"
echo "============================================================"
echo ""
echo "  Submission:"
echo "    Accepted:             $SUCCESS / $NUM_TXS"
echo "    Submit time:          ${SUBMIT_DUR}s"
echo "    Submit rate:          $SUBMIT_TPS tx/s"
echo "    Peak mempool:         $PEAK_MEMPOOL"
echo ""
echo "  Drain:"
echo "    Drain time:           ${DRAIN_DUR}s"
echo "    Rounds to drain:      $ROUNDS_ELAPSED"
echo "    Finalized rounds:     $FIN_ELAPSED"
echo ""
echo "  Throughput:"
echo "    tx/round:             $TX_PER_ROUND"
echo "    Wall clock TPS:       $WALL_TPS tx/s  (submit + drain)"
echo "    Drain TPS:            $DRAIN_TPS tx/s  (drain only)"
echo "    Rounds-based TPS:     $ROUNDS_TPS tx/s  ($SUCCESS / ($ROUNDS_ELAPSED x ${ROUND_S}s))"
echo ""
echo "  Verification:"
echo "    Recipient balance:    $RECV_BAL sats"
echo "    Expected:             $EXPECTED sats"
echo "    Match:                $(python3 -c "print('YES' if $RECV_BAL == $EXPECTED else 'NO')")"
echo ""
echo "  Network:"
echo "    Final round:          $FINAL_ROUND"
echo "    Final finalized:      $FINAL_FIN"
echo "    Remaining mempool:    $FINAL_MEMPOOL"
echo "    Supply:               $(python3 -c "print(f'{$FINAL_SUPPLY / 100_000_000:,.2f}')") UDAG"
echo ""
echo "============================================================"

# Save poll log for analysis
echo ""
echo "  Poll log saved: $POLL_LOG"
echo "  Sample:"
head -5 "$POLL_LOG"
echo "  ..."
tail -3 "$POLL_LOG"
