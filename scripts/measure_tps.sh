#!/bin/bash
# TPS measurement script for UltraDAG testnet
# Measures transactions per second by tracking finalized vertices over time

TESTNET_URL="${1:-https://ultradag.fly.dev}"
DURATION="${2:-60}"  # seconds to measure
INTERVAL="${3:-5}"   # sampling interval in seconds

echo "=== UltraDAG TPS Measurement ==="
echo "Node: $TESTNET_URL"
echo "Duration: ${DURATION}s"
echo "Sample interval: ${INTERVAL}s"
echo ""

# Get initial state
initial_response=$(curl -s "$TESTNET_URL/status")
initial_round=$(echo "$initial_response" | jq -r '.dag_round')
initial_vertices=$(echo "$initial_response" | jq -r '.dag_vertices')

echo "Initial state:"
echo "  Round: $initial_round"
echo "  Total vertices: $initial_vertices"
echo ""

# Wait for measurement period
echo "Measuring for ${DURATION} seconds..."
sleep "$DURATION"

# Get final state
final_response=$(curl -s "$TESTNET_URL/status")
final_round=$(echo "$final_response" | jq -r '.dag_round')
final_vertices=$(echo "$final_response" | jq -r '.dag_vertices')
finalized_round=$(echo "$final_response" | jq -r '.finalized_round')

echo ""
echo "Final state:"
echo "  Round: $final_round"
echo "  Total vertices: $final_vertices"
echo "  Finalized round: $finalized_round"
echo ""

# Calculate metrics
rounds_produced=$((final_round - initial_round))
vertices_produced=$((final_vertices - initial_vertices))
rounds_per_sec=$(echo "scale=2; $rounds_produced / $DURATION" | bc)
vertices_per_sec=$(echo "scale=2; $vertices_produced / $DURATION" | bc)

# Get transaction count by querying recent vertices
echo "Sampling recent vertices to count transactions..."
tx_count=0
sample_size=20

for i in $(seq 1 $sample_size); do
    round=$((final_round - i))
    if [ $round -ge 0 ]; then
        vertex_response=$(curl -s "$TESTNET_URL/vertex/$round" 2>/dev/null)
        if [ $? -eq 0 ]; then
            vertex_tx=$(echo "$vertex_response" | jq -r 'if type=="array" then .[0].tx_count else .tx_count end' 2>/dev/null)
            if [ "$vertex_tx" != "null" ] && [ -n "$vertex_tx" ]; then
                tx_count=$((tx_count + vertex_tx))
            fi
        fi
    fi
done

avg_tx_per_vertex=$(echo "scale=2; $tx_count / $sample_size" | bc)
estimated_tps=$(echo "scale=2; $avg_tx_per_vertex * $vertices_per_sec" | bc)

echo ""
echo "=== RESULTS ==="
echo "Time period: ${DURATION}s"
echo "Rounds produced: $rounds_produced"
echo "Vertices produced: $vertices_produced"
echo "Rounds/sec: $rounds_per_sec"
echo "Vertices/sec: $vertices_per_sec"
echo ""
echo "Transaction sampling (last $sample_size vertices):"
echo "  Total transactions: $tx_count"
echo "  Avg tx/vertex: $avg_tx_per_vertex"
echo ""
echo "Estimated TPS: $estimated_tps tx/sec"
echo ""
