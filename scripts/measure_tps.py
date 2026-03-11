#!/usr/bin/env python3
"""
TPS measurement tool for UltraDAG testnet.
Measures transactions per second by tracking finalized vertices over time.
"""

import requests
import time
import sys
import json
from datetime import datetime

def get_status(node_url):
    """Get node status from /status endpoint."""
    try:
        response = requests.get(f"{node_url}/status", timeout=5)
        response.raise_for_status()
        return response.json()
    except Exception as e:
        print(f"Error fetching status: {e}")
        return None

def get_vertex(node_url, round_num):
    """Get vertex data for a specific round."""
    try:
        response = requests.get(f"{node_url}/vertex/{round_num}", timeout=5)
        response.raise_for_status()
        data = response.json()
        # Handle both single vertex and array responses
        if isinstance(data, list):
            return data[0] if data else None
        return data
    except:
        return None

def measure_tps(node_url, duration=60, sample_interval=5):
    """
    Measure TPS over a time period.
    
    Args:
        node_url: Base URL of the node
        duration: How long to measure (seconds)
        sample_interval: How often to sample (seconds)
    """
    print("=" * 60)
    print("UltraDAG TPS Measurement")
    print("=" * 60)
    print(f"Node: {node_url}")
    print(f"Duration: {duration}s")
    print(f"Sample interval: {sample_interval}s")
    print(f"Started at: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}")
    print()
    
    # Get initial state
    print("Fetching initial state...")
    initial = get_status(node_url)
    if not initial:
        print("Failed to get initial status")
        return
    
    initial_round = initial.get('dag_round', 0)
    initial_vertices = initial.get('dag_vertices', 0)
    initial_finalized = initial.get('finalized_round', 0)
    
    print(f"Initial state:")
    print(f"  DAG round: {initial_round}")
    print(f"  Total vertices: {initial_vertices}")
    print(f"  Finalized round: {initial_finalized}")
    print()
    
    # Sample periodically
    samples = []
    start_time = time.time()
    next_sample = start_time + sample_interval
    
    print(f"Measuring (sampling every {sample_interval}s)...")
    
    while time.time() - start_time < duration:
        if time.time() >= next_sample:
            status = get_status(node_url)
            if status:
                elapsed = time.time() - start_time
                samples.append({
                    'time': elapsed,
                    'dag_round': status.get('dag_round', 0),
                    'dag_vertices': status.get('dag_vertices', 0),
                    'finalized_round': status.get('finalized_round', 0),
                })
                print(f"  [{elapsed:.1f}s] Round: {status.get('dag_round')}, "
                      f"Vertices: {status.get('dag_vertices')}, "
                      f"Finalized: {status.get('finalized_round')}")
            next_sample += sample_interval
        time.sleep(0.5)
    
    # Get final state
    print()
    print("Fetching final state...")
    final = get_status(node_url)
    if not final:
        print("Failed to get final status")
        return
    
    final_round = final.get('dag_round', 0)
    final_vertices = final.get('dag_vertices', 0)
    final_finalized = final.get('finalized_round', 0)
    actual_duration = time.time() - start_time
    
    print(f"Final state:")
    print(f"  DAG round: {final_round}")
    print(f"  Total vertices: {final_vertices}")
    print(f"  Finalized round: {final_finalized}")
    print()
    
    # Calculate metrics
    rounds_produced = final_round - initial_round
    vertices_produced = final_vertices - initial_vertices
    rounds_finalized = final_finalized - initial_finalized
    
    rounds_per_sec = rounds_produced / actual_duration
    vertices_per_sec = vertices_produced / actual_duration
    finalized_per_sec = rounds_finalized / actual_duration
    
    # Sample recent vertices to count transactions
    print("Sampling recent vertices to count transactions...")
    sample_size = min(50, rounds_produced)
    tx_count = 0
    sampled = 0
    
    for i in range(sample_size):
        round_num = final_round - i - 1
        if round_num >= initial_round:
            vertex = get_vertex(node_url, round_num)
            if vertex and 'tx_count' in vertex:
                tx_count += vertex['tx_count']
                sampled += 1
    
    avg_tx_per_vertex = tx_count / sampled if sampled > 0 else 0
    estimated_tps = avg_tx_per_vertex * vertices_per_sec
    
    # Print results
    print()
    print("=" * 60)
    print("RESULTS")
    print("=" * 60)
    print(f"Measurement duration: {actual_duration:.1f}s")
    print()
    print("Consensus metrics:")
    print(f"  Rounds produced: {rounds_produced}")
    print(f"  Vertices produced: {vertices_produced}")
    print(f"  Rounds finalized: {rounds_finalized}")
    print(f"  Rounds/sec: {rounds_per_sec:.2f}")
    print(f"  Vertices/sec: {vertices_per_sec:.2f}")
    print(f"  Finalized rounds/sec: {finalized_per_sec:.2f}")
    print()
    print(f"Transaction sampling (last {sampled} vertices):")
    print(f"  Total transactions: {tx_count}")
    print(f"  Avg tx/vertex: {avg_tx_per_vertex:.2f}")
    print()
    print(f"Estimated TPS: {estimated_tps:.2f} tx/sec")
    print()
    
    # Calculate finality lag
    finality_lag = final_round - final_finalized
    print(f"Finality lag: {finality_lag} rounds")
    
    if finality_lag > 10:
        print("  ⚠️  High finality lag detected")
    elif finality_lag > 5:
        print("  ⚠️  Moderate finality lag")
    else:
        print("  ✅ Low finality lag (healthy)")
    
    print()
    print("=" * 60)

if __name__ == "__main__":
    node_url = sys.argv[1] if len(sys.argv) > 1 else "https://ultradag.fly.dev"
    duration = int(sys.argv[2]) if len(sys.argv) > 2 else 60
    sample_interval = int(sys.argv[3]) if len(sys.argv) > 3 else 5
    
    measure_tps(node_url, duration, sample_interval)
