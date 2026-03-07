#!/usr/bin/env bash
# 8hr_monitor_pro_final.sh — UltraDAG testnet monitor (stable & informative)

set -euo pipefail

# Config
LOGFILE="${LOGFILE:-$HOME/Projects/15_UltraDAG/TESTNET_8HR_LOG.jsonl}"
DURATION=$((8 * 3600))          # 8 hours
INTERVAL="${1:-60}"             # seconds, default 60s
MAX_LOG_LINES=20000             # rotate if exceeded

# Colors
[ -t 1 ] && {
    RED='\033[0;31m' GREEN='\033[0;32m' YELLOW='\033[1;33m' RESET='\033[0m'
} || { RED='' GREEN='' YELLOW='' RESET=''; }

info()  { echo -e "${GREEN}$*${RESET}"; }
warn()  { echo -e "${YELLOW}WARN: $*${RESET}" >&2; }
err()   { echo -e "${RED}ERROR: $*${RESET}" >&2; exit 1; }

# Rotate log
[ -f "$LOGFILE" ] && [ $(wc -l < "$LOGFILE") -gt $MAX_LOG_LINES ] && {
    BACKUP="${LOGFILE}.$(date +%Y%m%d-%H%M).bak"
    mv "$LOGFILE" "$BACKUP"
    info "Log rotated → $BACKUP"
}

echo "Starting monitor at $(date '+%Y-%m-%d %H:%M:%S %Z'). Interval: ${INTERVAL}s. Log: $LOGFILE"
echo "" > "$LOGFILE"

START=$(date +%s)
CHECKS=0
PREV_LAG=0
PREV_VERTS=0
PREV_SUPPLY=0
STABLE_VERT_COUNT=0   # track how many consecutive stable vertex checks

get_node_status() {
    local n=$1 url="https://ultradag-node-${n}.fly.dev/status"
    local status=$(curl -s --max-time 10 --retry 2 --retry-delay 2 "$url" 2>/dev/null)

    [ -z "$status" ] || echo "$status" | grep -qi "error\|html" && {
        echo '{"reachable":false,"error":"unreachable"}'
        return 1
    }

    echo "$status" | python3 -c '
import sys,json
try:
    d = json.load(sys.stdin)
    print(json.dumps({
        "reachable": True,
        "dag_round": d.get("dag_round", 0),
        "last_finalized_round": d.get("last_finalized_round", 0),
        "lag": d.get("dag_round", 0) - d.get("last_finalized_round", 0),
        "total_supply": d.get("total_supply", 0),
        "peer_count": d.get("peer_count", d.get("peers", 0)),
        "dag_vertices": d.get("dag_vertices", 0),
        "dag_tips": d.get("dag_tips", 0),
        "mempool_size": d.get("mempool_size", 0),
        "account_count": d.get("account_count", 0),
        "validator_count": d.get("validator_count", 0)
    }))
except:
    print(json.dumps({"reachable": False, "error": "parse failed"}))
' 2>/dev/null || echo '{"reachable":false,"error":"python failed"}'
}

while true; do
    NOW=$(date +%s)
    ELAPSED=$((NOW - START))
    [ $ELAPSED -ge $DURATION ] && {
        info "Monitor complete (8 hours). Total checks: $CHECKS"
        break
    }

    CHECKS=$((CHECKS + 1))
    HOURS=$((ELAPSED / 3600))
    MINS=$(((ELAPSED % 3600) / 60))
    TS=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

    ENTRY="{\"ts\":\"$TS\",\"elapsed_min\":$((ELAPSED/60)),\"check\":$CHECKS"

    ALL_REACHABLE=true
    SUMMARY=""
    MAX_LAG=0

    for n in 1 2 3 4; do
        node_data=$(get_node_status $n)
        ENTRY="$ENTRY,\"node${n}\":${node_data}"

        if echo "$node_data" | grep -q '"reachable":false'; then
            ALL_REACHABLE=false
            SUMMARY="${SUMMARY} N${n}:UNREACHABLE |"
        else
            dag=$(echo "$node_data" | python3 -c "import sys,json; print(json.load(sys.stdin).get('dag_round',0))" 2>/dev/null || echo 0)
            fin=$(echo "$node_data" | python3 -c "import sys,json; print(json.load(sys.stdin).get('last_finalized_round',0))" 2>/dev/null || echo 0)
            lag=$((dag - fin))
            verts=$(echo "$node_data" | python3 -c "import sys,json; print(json.load(sys.stdin).get('dag_vertices',0))" 2>/dev/null || echo 0)
            peers=$(echo "$node_data" | python3 -c "import sys,json; print(json.load(sys.stdin).get('peer_count',0))" 2>/dev/null || echo 0)
            supply=$(echo "$node_data" | python3 -c "import sys,json; print(json.load(sys.stdin).get('total_supply',0))" 2>/dev/null || echo 0)

            SUMMARY="${SUMMARY} N${n}:dag=${dag} fin=${fin} lag=${lag} v=${verts} p=${peers}"

            MAX_LAG=$(( MAX_LAG > lag ? MAX_LAG : lag ))

            # Trend & alerts
            if [ $CHECKS -gt 1 ]; then
                [ $lag -gt $((PREV_LAG + 10)) ] && warn "Lag spike! Node $n: $lag (prev $PREV_LAG)"
                if [ $verts -eq $PREV_VERTS ] && [ $PREV_VERTS -gt 5000 ]; then
                    STABLE_VERT_COUNT=$((STABLE_VERT_COUNT + 1))
                    [ $STABLE_VERT_COUNT -ge 3 ] && info "Pruning active — vertices stable for $STABLE_VERT_COUNT checks (~$verts)"
                else
                    STABLE_VERT_COUNT=0
                fi
                [ $supply -lt $PREV_SUPPLY ] && warn "Supply decreased! $supply < $PREV_SUPPLY"
                [ $supply -gt $((PREV_SUPPLY + 10000000000)) ] && warn "Large supply jump! +$(( (supply - PREV_SUPPLY)/100000000 )) UDAG"
            fi

            PREV_LAG=$lag
            PREV_VERTS=$verts
            PREV_SUPPLY=$supply
        fi

        SUMMARY="${SUMMARY} |"
    done

    ENTRY="$ENTRY,\"all_reachable\":$ALL_REACHABLE,\"max_lag\":$MAX_LAG}"
    echo "$ENTRY" >> "$LOGFILE"

    # Print summary
    if $ALL_REACHABLE; then
        echo -e "[${HOURS}h${MINS}m] Check $CHECKS |$SUMMARY max_lag=${MAX_LAG}"
    else
        echo -e "${RED}[${HOURS}h${MINS}m] Check $CHECKS |$SUMMARY UNREACHABLE${RESET}"
    fi

    # Progress
    echo -n "Waiting ${INTERVAL}s "
    for ((i=0; i<INTERVAL; i+=5)); do echo -n "."; sleep 5; done
    echo ""
done

# Final stats
echo ""
info "Final summary (last 50 checks):"
tail -n 50 "$LOGFILE" | python3 -c '
import sys, json, statistics
data = []
for line in sys.stdin:
    if line.strip():
        try:
            d = json.loads(line)
            if "node1" in d and d["node1"].get("reachable", False):
                data.append(d["node1"])
        except:
            pass
if not data:
    print("No valid data")
    sys.exit(0)
lags = [d["lag"] for d in data]
verts = [d["dag_vertices"] for d in data]
supply_start = data[0]["total_supply"]
supply_end = data[-1]["total_supply"]
print(f"Avg lag (node1): {statistics.mean(lags):.1f}")
print(f"Max vertices: {max(verts)}")
print(f"Supply growth: {(supply_end - supply_start)/1e8:.2f} UDAG")
' 2>/dev/null || echo "Final stats unavailable"