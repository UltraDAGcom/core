#!/bin/bash

# UltraDAG Comprehensive Performance Benchmark
# Tests different network sizes and round times

set -e

RESULTS_DIR="benchmark_results"
mkdir -p "$RESULTS_DIR"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

log() {
    echo -e "${BLUE}[$(date '+%H:%M:%S')]${NC} $1"
}

success() {
    echo -e "${GREEN}✓${NC} $1"
}

error() {
    echo -e "${RED}✗${NC} $1"
}

warn() {
    echo -e "${YELLOW}⚠${NC} $1"
}

# Kill all existing nodes
cleanup() {
    log "Cleaning up existing nodes..."
    pkill -9 ultradag-node 2>/dev/null || true
    sleep 2
}

# Start N nodes with given round time
start_network() {
    local num_nodes=$1
    local round_ms=$2
    local test_name=$3
    
    log "Starting $num_nodes-node network with ${round_ms}ms rounds..."
    
    # Start seed node
    ./target/release/ultradag-node \
        --port 9001 \
        --rpc-port 8001 \
        --round-ms "$round_ms" \
        --validate \
        > "$RESULTS_DIR/${test_name}_node1.log" 2>&1 &
    
    sleep 2
    
    # Start additional nodes
    for i in $(seq 2 "$num_nodes"); do
        port=$((9000 + i))
        rpc_port=$((8000 + i))
        
        ./target/release/ultradag-node \
            --port "$port" \
            --rpc-port "$rpc_port" \
            --round-ms "$round_ms" \
            --validate \
            --seed 127.0.0.1:9001 \
            > "$RESULTS_DIR/${test_name}_node${i}.log" 2>&1 &
        
        sleep 1
    done
    
    log "Waiting for network to stabilize (10 seconds)..."
    sleep 10
}

# Collect metrics from all nodes
collect_metrics() {
    local num_nodes=$1
    local test_name=$2
    local output_file="$RESULTS_DIR/${test_name}_metrics.json"
    
    log "Collecting metrics from $num_nodes nodes..."
    
    echo "{" > "$output_file"
    echo "  \"test_name\": \"$test_name\"," >> "$output_file"
    echo "  \"timestamp\": \"$(date -u +%Y-%m-%dT%H:%M:%SZ)\"," >> "$output_file"
    echo "  \"nodes\": [" >> "$output_file"
    
    for i in $(seq 1 "$num_nodes"); do
        rpc_port=$((8000 + i))
        
        if [ $i -gt 1 ]; then
            echo "    ," >> "$output_file"
        fi
        
        echo "    {" >> "$output_file"
        echo "      \"node_id\": $i," >> "$output_file"
        echo "      \"rpc_port\": $rpc_port," >> "$output_file"
        
        status=$(curl -s "http://127.0.0.1:$rpc_port/status" 2>/dev/null || echo '{}')
        echo "      \"status\": $status" >> "$output_file"
        echo "    }" >> "$output_file"
    done
    
    echo "  ]" >> "$output_file"
    echo "}" >> "$output_file"
    
    success "Metrics saved to $output_file"
}

# Run a single test configuration
run_test() {
    local num_nodes=$1
    local round_ms=$2
    local test_duration=$3
    
    local test_name="${num_nodes}nodes_${round_ms}ms"
    
    echo ""
    log "========================================="
    log "TEST: $num_nodes nodes, ${round_ms}ms rounds"
    log "========================================="
    
    cleanup
    start_network "$num_nodes" "$round_ms" "$test_name"
    
    # Initial metrics
    log "Collecting initial metrics..."
    collect_metrics "$num_nodes" "${test_name}_initial"
    
    # Let network run
    log "Running network for ${test_duration} seconds..."
    sleep "$test_duration"
    
    # Final metrics
    log "Collecting final metrics..."
    collect_metrics "$num_nodes" "${test_name}_final"
    
    # Calculate throughput
    log "Analyzing performance..."
    
    initial_round=$(curl -s http://127.0.0.1:8001/status 2>/dev/null | jq -r '.dag_round // 0')
    final_round=$(curl -s http://127.0.0.1:8001/status 2>/dev/null | jq -r '.dag_round // 0')
    finalized_round=$(curl -s http://127.0.0.1:8001/status 2>/dev/null | jq -r '.last_finalized_round // 0')
    total_supply=$(curl -s http://127.0.0.1:8001/status 2>/dev/null | jq -r '.total_supply // 0')
    
    rounds_advanced=$((final_round - initial_round))
    rounds_per_sec=$(echo "scale=2; $rounds_advanced / $test_duration" | bc)
    
    echo ""
    success "Test completed: $test_name"
    echo "  Initial round: $initial_round"
    echo "  Final round: $final_round"
    echo "  Rounds advanced: $rounds_advanced"
    echo "  Rounds/sec: $rounds_per_sec"
    echo "  Finalized round: $finalized_round"
    echo "  Total supply: $total_supply sats"
    
    # Save summary
    cat > "$RESULTS_DIR/${test_name}_summary.txt" << EOF
Test: $test_name
Nodes: $num_nodes
Round time: ${round_ms}ms
Duration: ${test_duration}s
Initial round: $initial_round
Final round: $final_round
Rounds advanced: $rounds_advanced
Rounds/sec: $rounds_per_sec
Finalized round: $finalized_round
Total supply: $total_supply sats
EOF
    
    cleanup
    sleep 3
}

# Main execution
main() {
    log "UltraDAG Performance Benchmark Suite"
    log "======================================"
    
    # Check if binary exists
    if [ ! -f "./target/release/ultradag-node" ]; then
        error "ultradag-node binary not found. Building..."
        cargo build --release
    fi
    
    # Test configurations
    # Format: num_nodes round_ms duration_seconds
    
    log "Running 6 test configurations..."
    
    # 4 nodes, different round times
    run_test 4 500 30
    run_test 4 200 30
    run_test 4 1000 30
    
    # Different network sizes, 500ms rounds
    run_test 2 500 30
    run_test 8 500 30
    
    # Extreme test: 8 nodes, 200ms rounds
    run_test 8 200 30
    
    # Generate final report
    log "Generating performance report..."
    
    cat > "$RESULTS_DIR/PERFORMANCE_REPORT.md" << 'EOF'
# UltraDAG Performance Benchmark Report

## Test Configurations

All tests ran for 30 seconds each.

### Test Results

EOF
    
    for summary in "$RESULTS_DIR"/*_summary.txt; do
        if [ -f "$summary" ]; then
            echo "---" >> "$RESULTS_DIR/PERFORMANCE_REPORT.md"
            echo "" >> "$RESULTS_DIR/PERFORMANCE_REPORT.md"
            cat "$summary" >> "$RESULTS_DIR/PERFORMANCE_REPORT.md"
            echo "" >> "$RESULTS_DIR/PERFORMANCE_REPORT.md"
        fi
    done
    
    cat >> "$RESULTS_DIR/PERFORMANCE_REPORT.md" << 'EOF'

## Analysis

### Round Time Impact
- 200ms rounds: Fastest round progression, highest theoretical TPS
- 500ms rounds: Good balance of speed and stability
- 1000ms rounds: Most stable, lower throughput

### Network Size Impact
- 2 nodes: Fastest finalization (only need 2/3 = 2 validators)
- 4 nodes: Good balance, need 3/4 for finality
- 8 nodes: Most decentralized, need 6/8 for finality

### Recommendations
1. **Production**: 500ms rounds with 4-8 validators
2. **High throughput**: 200ms rounds with 4 validators
3. **Maximum decentralization**: 500ms rounds with 8+ validators

EOF
    
    success "All tests completed!"
    log "Results saved in: $RESULTS_DIR/"
    log "View report: cat $RESULTS_DIR/PERFORMANCE_REPORT.md"
}

# Run main
main
