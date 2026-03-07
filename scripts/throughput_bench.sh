#!/bin/bash
# ============================================================
# UltraDAG Throughput Benchmark
# ============================================================
# Measures actual network throughput (finalization rate),
# not submission speed.
#
# Usage: ./scripts/throughput_bench.sh [NUM_TXS] [CONCURRENCY] [RPC_URL]
# ============================================================

set -euo pipefail

NUM_TXS=${1:-1000}
CONCURRENCY=${2:-50}
RPC_URL=${3:-"https://ultradag-node-1.fly.dev"}

SENDER_SK="3f8efc5578b787786b059127f795a9ec4736880aa38aad974387e8a3423b15bd"
SENDER_ADDR="5c23f1097d7887ca6dbc2455e71ef2836772cde4ae58d3d9cf4be07b7d9b3f3f"
RECIPIENT="ba2b4e32f49d409b913f9845276533c67297967ab169e6ad86944d0ff3830b70"
ROUND_DURATION=5  # seconds (default --round-ms 5000)
TMPDIR=$(mktemp -d)

cleanup() { rm -rf "$TMPDIR"; }
trap cleanup EXIT

echo "============================================================"
echo "  UltraDAG Throughput Benchmark"
echo "============================================================"
echo "  Target:       $RPC_URL"
echo "  Transactions: $NUM_TXS"
echo "  Concurrency:  $CONCURRENCY"
echo "  Round time:   ${ROUND_DURATION}s"
echo "============================================================"
echo ""

# ---- Step 1: Fund wallet with 100,000 UDAG ----
echo "[1/5] Funding wallet with 100,000 UDAG..."
FUND_AMOUNT=10000000000000  # 100,000 UDAG in sats
curl -sf -X POST "$RPC_URL/faucet" \
    -H "Content-Type: application/json" \
    -d "{\"address\":\"$SENDER_ADDR\",\"amount\":$FUND_AMOUNT}" > /dev/null

BALANCE=$(curl -sf "$RPC_URL/balance/$SENDER_ADDR" | python3 -c "import sys,json; print(json.load(sys.stdin)['balance'])")
echo "  Balance: $(python3 -c "print(f'{$BALANCE / 100_000_000:.2f}')") UDAG"
echo ""

# ---- Step 2: Record initial state ----
echo "[2/5] Recording initial state..."
INIT_STATUS=$(curl -sf "$RPC_URL/status")
INIT_ROUND=$(echo "$INIT_STATUS" | python3 -c "import sys,json; print(json.load(sys.stdin)['dag_round'])")
INIT_FINALIZED=$(echo "$INIT_STATUS" | python3 -c "import sys,json; print(json.load(sys.stdin)['finalized_count'])")
INIT_MEMPOOL=$(echo "$INIT_STATUS" | python3 -c "import sys,json; print(json.load(sys.stdin)['mempool_size'])")
echo "  Round:     $INIT_ROUND"
echo "  Finalized: $INIT_FINALIZED"
echo "  Mempool:   $INIT_MEMPOOL"
echo ""

# ---- Step 3: Generate curl commands file ----
echo "[3/5] Submitting $NUM_TXS transactions ($CONCURRENCY parallel)..."

# Create a file with one curl command per line for xargs
for i in $(seq 1 $NUM_TXS); do
    echo "$i"
done > "$TMPDIR/tx_ids.txt"

SUBMIT_START=$(python3 -c "import time; print(time.time())")

# Submit using xargs for controlled parallelism
# Each curl call hits the atomic /tx endpoint which assigns nonces safely
cat "$TMPDIR/tx_ids.txt" | xargs -P "$CONCURRENCY" -I{} \
    curl -sf -o "$TMPDIR/tx_{}.json" -w '' \
    -X POST "$RPC_URL/tx" \
    -H "Content-Type: application/json" \
    -d "{\"from_secret\":\"$SENDER_SK\",\"to\":\"$RECIPIENT\",\"amount\":100000,\"fee\":100000}" \
    2>/dev/null || true

SUBMIT_END=$(python3 -c "import time; print(time.time())")
SUBMIT_DURATION=$(python3 -c "print(f'{$SUBMIT_END - $SUBMIT_START:.2f}')")

# Count results
SUCCESS=$(ls "$TMPDIR"/tx_*.json 2>/dev/null | while read f; do
    python3 -c "import json; d=json.load(open('$f')); print('ok' if 'hash' in d else 'fail')" 2>/dev/null
done | grep -c "ok" || echo 0)
TOTAL_FILES=$(ls "$TMPDIR"/tx_*.json 2>/dev/null | wc -l | tr -d ' ')
FAIL=$((TOTAL_FILES - SUCCESS))

SUBMIT_TPS=$(python3 -c "d=$SUBMIT_END-$SUBMIT_START; print(f'{$SUCCESS/d:.1f}' if d > 0 else 'inf')")
echo "  Submitted: $SUCCESS/$NUM_TXS in ${SUBMIT_DURATION}s ($SUBMIT_TPS tx/s)"
if [ "$FAIL" -gt 0 ]; then
    echo "  Failed: $FAIL"
    # Show first few errors
    ls "$TMPDIR"/tx_*.json 2>/dev/null | head -5 | while read f; do
        python3 -c "
import json
try:
    d=json.load(open('$f'))
    if 'error' in d: print(f'    Error: {d[\"error\"]}')
except: pass" 2>/dev/null
    done
fi
echo ""

# ---- Step 4: Poll until mempool drains ----
echo "[4/5] Waiting for finalization (polling /status every second)..."
echo ""

POST_SUBMIT=$(curl -sf "$RPC_URL/status")
POST_ROUND=$(echo "$POST_SUBMIT" | python3 -c "import sys,json; print(json.load(sys.stdin)['dag_round'])")
POST_MEMPOOL=$(echo "$POST_SUBMIT" | python3 -c "import sys,json; print(json.load(sys.stdin)['mempool_size'])")
echo "  After submission: round=$POST_ROUND mempool=$POST_MEMPOOL"

POLL_START=$(python3 -c "import time; print(time.time())")
MAX_WAIT=300  # 5 minutes max
ELAPSED=0

while true; do
    sleep 1
    ELAPSED=$((ELAPSED + 1))

    STATUS=$(curl -sf "$RPC_URL/status" 2>/dev/null || echo '{}')
    CUR_ROUND=$(echo "$STATUS" | python3 -c "import sys,json; print(json.load(sys.stdin).get('dag_round', 0))")
    CUR_FINALIZED=$(echo "$STATUS" | python3 -c "import sys,json; print(json.load(sys.stdin).get('finalized_count', 0))")
    CUR_MEMPOOL=$(echo "$STATUS" | python3 -c "import sys,json; print(json.load(sys.stdin).get('mempool_size', 0))")

    # Progress bar
    if [ "$POST_MEMPOOL" -gt 0 ]; then
        PROCESSED=$((POST_MEMPOOL - CUR_MEMPOOL))
        PCT=$((PROCESSED * 100 / POST_MEMPOOL))
    else
        PCT=100
    fi
    printf "\r  [%3d%%] round=%s finalized=%s mempool=%s (%ds)" \
        "$PCT" "$CUR_ROUND" "$CUR_FINALIZED" "$CUR_MEMPOOL" "$ELAPSED"

    if [ "$CUR_MEMPOOL" -eq 0 ]; then
        echo ""
        echo "  Mempool drained!"
        break
    fi

    if [ "$ELAPSED" -ge "$MAX_WAIT" ]; then
        echo ""
        echo "  TIMEOUT after ${MAX_WAIT}s (mempool still has $CUR_MEMPOOL txs)"
        break
    fi
done

POLL_END=$(python3 -c "import time; print(time.time())")

# ---- Step 5: Calculate results ----
echo ""
echo "[5/5] Calculating throughput..."
echo ""

FINAL_STATUS=$(curl -sf "$RPC_URL/status")
FINAL_ROUND=$(echo "$FINAL_STATUS" | python3 -c "import sys,json; print(json.load(sys.stdin)['dag_round'])")
FINAL_FINALIZED=$(echo "$FINAL_STATUS" | python3 -c "import sys,json; print(json.load(sys.stdin)['finalized_count'])")
FINAL_SUPPLY=$(echo "$FINAL_STATUS" | python3 -c "import sys,json; print(json.load(sys.stdin)['total_supply'])")
FINAL_MEMPOOL=$(echo "$FINAL_STATUS" | python3 -c "import sys,json; print(json.load(sys.stdin)['mempool_size'])")

ROUNDS_ELAPSED=$((FINAL_ROUND - INIT_ROUND))
FINALIZED_ELAPSED=$((FINAL_FINALIZED - INIT_FINALIZED))
FINALIZATION_TIME=$(python3 -c "print(f'{$POLL_END - $SUBMIT_START:.2f}')")
WALL_TIME=$((ROUNDS_ELAPSED * ROUND_DURATION))

# Throughput = transactions / (rounds_elapsed * round_duration)
THROUGHPUT=$(python3 -c "
rounds = $ROUNDS_ELAPSED
duration = rounds * $ROUND_DURATION
txs = $SUCCESS
if duration > 0:
    print(f'{txs / duration:.2f}')
else:
    print('N/A')
")

# Also calculate observed wall-clock throughput
WALL_TPS=$(python3 -c "
duration = $POLL_END - $SUBMIT_START
if duration > 0:
    print(f'{$SUCCESS / duration:.2f}')
else:
    print('N/A')
")

# Per-round throughput
TX_PER_ROUND=$(python3 -c "
if $ROUNDS_ELAPSED > 0:
    print(f'{$SUCCESS / $ROUNDS_ELAPSED:.1f}')
else:
    print('N/A')
")

RECV_BAL=$(curl -sf "$RPC_URL/balance/$RECIPIENT" | python3 -c "import sys,json; print(json.load(sys.stdin)['balance'])")

echo "============================================================"
echo "  RESULTS"
echo "============================================================"
echo ""
echo "  Submission:"
echo "    Accepted:          $SUCCESS / $NUM_TXS"
echo "    Submit duration:   ${SUBMIT_DURATION}s"
echo "    Submit rate:       $SUBMIT_TPS tx/s"
echo ""
echo "  Finalization:"
echo "    Rounds elapsed:    $ROUNDS_ELAPSED (${WALL_TIME}s at ${ROUND_DURATION}s/round)"
echo "    Rounds finalized:  $FINALIZED_ELAPSED"
echo "    Finalization time: ${FINALIZATION_TIME}s (wall clock)"
echo "    Remaining mempool: $FINAL_MEMPOOL"
echo ""
echo "  Throughput:"
echo "    tx/round:          $TX_PER_ROUND"
echo "    tx/s (by rounds):  $THROUGHPUT"
echo "    tx/s (wall clock): $WALL_TPS"
echo ""
echo "  Verification:"
echo "    Recipient balance: $(python3 -c "print(f'{$RECV_BAL / 100_000_000:.4f}')") UDAG ($RECV_BAL sats)"
echo "    Expected:          $(python3 -c "print(f'{$SUCCESS * 100000 / 100_000_000:.4f}')") UDAG ($((SUCCESS * 100000)) sats)"
echo "    Match:             $(python3 -c "print('YES' if $RECV_BAL == $SUCCESS * 100000 else 'NO')")"
echo ""
echo "  Network State:"
echo "    Supply:            $(python3 -c "print(f'{$FINAL_SUPPLY / 100_000_000:.2f}')") UDAG"
echo "    Round:             $FINAL_ROUND"
echo "    Finalized:         $FINAL_FINALIZED"
echo ""
echo "============================================================"
