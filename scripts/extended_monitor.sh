#!/bin/bash
# Extended 24-48 hour testnet monitoring script
# Logs node status every 5 minutes and checks for issues

# Get script directory and set log file path
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LOG_FILE="$SCRIPT_DIR/extended_monitor.log"
DURATION_HOURS=${1:-24}  # Default 24 hours, can override
INTERVAL_SECONDS=300      # 5 minutes

echo "=== UltraDAG Extended Monitoring Started ===" | tee -a "$LOG_FILE"
echo "Duration: $DURATION_HOURS hours" | tee -a "$LOG_FILE"
echo "Interval: $INTERVAL_SECONDS seconds ($(($INTERVAL_SECONDS / 60)) minutes)" | tee -a "$LOG_FILE"
echo "Start time: $(date)" | tee -a "$LOG_FILE"
echo "" | tee -a "$LOG_FILE"

END_TIME=$(($(date +%s) + ($DURATION_HOURS * 3600)))
SAMPLE=0

while [ $(date +%s) -lt $END_TIME ]; do
    SAMPLE=$((SAMPLE + 1))
    TIMESTAMP=$(date '+%Y-%m-%d %H:%M:%S')
    
    echo "[$TIMESTAMP] Sample #$SAMPLE" | tee -a "$LOG_FILE"
    
    # Check all 4 nodes
    for n in 1 2 3 4; do
        STATUS=$(curl -s --max-time 10 "https://ultradag-node-$n.fly.dev/status" 2>/dev/null)
        
        if [ -z "$STATUS" ]; then
            echo "  Node $n: CRASHED or UNREACHABLE ❌" | tee -a "$LOG_FILE"
        else
            # Parse and log status
            echo "$STATUS" | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    dag = d['dag_round']
    fin = d.get('last_finalized_round', 0)
    lag = dag - fin
    peers = d.get('peer_count', 0)
    supply = d.get('total_supply', 0) / 100000000
    
    # Check for issues
    issues = []
    if lag > 10:
        issues.append('HIGH_LAG')
    if peers < 3:
        issues.append('LOW_PEERS')
    
    status_icon = '✅' if not issues else '⚠️'
    issues_str = ' [' + ','.join(issues) + ']' if issues else ''
    
    print(f'  Node $n: round={dag:4d} fin={fin:4d} lag={lag:2d} peers={peers:2d} supply={supply:,.0f}{issues_str} {status_icon}')
except Exception as e:
    print(f'  Node $n: Parse error - {e}')
" | tee -a "$LOG_FILE"
        fi
    done
    
    # Check vertex density for recent rounds
    CURRENT_ROUND=$(curl -s "https://ultradag-node-1.fly.dev/status" 2>/dev/null | python3 -c "import sys,json; print(json.load(sys.stdin)['dag_round'])" 2>/dev/null)
    
    if [ -n "$CURRENT_ROUND" ]; then
        PREV_ROUND=$((CURRENT_ROUND - 1))
        VERTICES=$(curl -s "https://ultradag-node-1.fly.dev/round/$PREV_ROUND" 2>/dev/null | python3 -c "import sys,json; print(len(json.load(sys.stdin)))" 2>/dev/null)
        
        if [ -n "$VERTICES" ]; then
            DENSITY_ICON="✅"
            [ "$VERTICES" -lt 3 ] && DENSITY_ICON="⚠️"
            echo "  Round $PREV_ROUND: $VERTICES vertices $DENSITY_ICON" | tee -a "$LOG_FILE"
        fi
    fi
    
    echo "" | tee -a "$LOG_FILE"
    
    # Sleep until next sample
    sleep $INTERVAL_SECONDS
done

echo "=== Monitoring Complete ===" | tee -a "$LOG_FILE"
echo "End time: $(date)" | tee -a "$LOG_FILE"
echo "Total samples: $SAMPLE" | tee -a "$LOG_FILE"
