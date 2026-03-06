#!/usr/bin/env python3
"""
UltraDAG Throughput Test
Measures actual transaction processing speed by sending transactions
in controlled batches with proper nonce management
"""

import requests
import time
import json
from concurrent.futures import ThreadPoolExecutor, as_completed

NODE_URL = "http://127.0.0.1:8001"
VALIDATOR_SECRET = "7fab8b10b9a1e8f1b4e7f6788eafdb5531c66299671e8ceba13e0cbde36c7f87"
RECEIVER = "0000000000000000000000000000000000000000000000000000000000000001"

# Test configuration
NUM_TXS = 200
BATCH_SIZE = 10  # Send 10 at a time
BATCH_DELAY = 0.1  # Wait 100ms between batches
AMOUNT = 1000
FEE = 100

def send_transaction():
    """Send a single transaction"""
    try:
        response = requests.post(
            f"{NODE_URL}/tx",
            json={
                "from_secret": VALIDATOR_SECRET,
                "to": RECEIVER,
                "amount": AMOUNT,
                "fee": FEE
            },
            timeout=5
        )
        
        if response.status_code == 200:
            return True, response.json()
        else:
            return False, response.text
    except Exception as e:
        return False, str(e)

def get_status():
    """Get network status"""
    try:
        response = requests.get(f"{NODE_URL}/status", timeout=5)
        return response.json()
    except Exception as e:
        return {"error": str(e)}

def main():
    print("=== UltraDAG Throughput Test ===")
    print(f"Node: {NODE_URL}")
    print(f"Total transactions: {NUM_TXS}")
    print(f"Batch size: {BATCH_SIZE}")
    print(f"Batch delay: {BATCH_DELAY}s")
    print()

    # Get initial status
    print("=== Initial Network Status ===")
    status = get_status()
    print(f"DAG Round: {status.get('dag_round', 'N/A')}")
    print(f"Finalized Round: {status.get('last_finalized_round', 'N/A')}")
    print(f"Total Supply: {status.get('total_supply', 0) / 100_000_000:.2f} UDAG")
    print(f"Mempool: {status.get('mempool_size', 0)} txs")
    print(f"DAG Vertices: {status.get('dag_vertices', 0)}")
    print()

    # Send transactions in batches
    print(f"=== Sending {NUM_TXS} transactions ===")
    start_time = time.time()
    
    successful = 0
    failed = 0
    errors = {}
    latencies = []
    
    for batch_num in range(0, NUM_TXS, BATCH_SIZE):
        batch_size = min(BATCH_SIZE, NUM_TXS - batch_num)
        
        # Send batch concurrently
        batch_start = time.time()
        with ThreadPoolExecutor(max_workers=batch_size) as executor:
            futures = [executor.submit(send_transaction) for _ in range(batch_size)]
            
            for future in as_completed(futures):
                success, result = future.result()
                if success:
                    successful += 1
                else:
                    failed += 1
                    error_msg = str(result)[:80]
                    errors[error_msg] = errors.get(error_msg, 0) + 1
        
        batch_time = time.time() - batch_start
        latencies.append(batch_time / batch_size)
        
        # Progress update
        if (batch_num + batch_size) % 50 == 0 or batch_num + batch_size >= NUM_TXS:
            print(f"  Progress: {batch_num + batch_size}/{NUM_TXS} sent ({successful} successful, {failed} failed)")
        
        # Small delay between batches
        if batch_num + batch_size < NUM_TXS:
            time.sleep(BATCH_DELAY)
    
    end_time = time.time()
    duration = end_time - start_time
    
    print()
    print("=== Results ===")
    print(f"Total time: {duration:.2f}s")
    print(f"Successful: {successful}")
    print(f"Failed: {failed}")
    print(f"Success rate: {(successful / NUM_TXS * 100):.1f}%")
    
    if successful > 0:
        print(f"Submission throughput: {successful / duration:.2f} TPS")
        avg_latency = sum(latencies) / len(latencies) * 1000
        print(f"Average latency: {avg_latency:.2f}ms")
    
    if errors:
        print()
        print("Top errors:")
        for error, count in sorted(errors.items(), key=lambda x: x[1], reverse=True)[:3]:
            print(f"  {count}x: {error}")
    
    # Wait for processing
    print()
    print("=== Waiting 15 seconds for DAG processing ===")
    time.sleep(15)
    
    # Get final status
    print()
    print("=== Final Network Status ===")
    status = get_status()
    print(f"DAG Round: {status.get('dag_round', 'N/A')}")
    print(f"Finalized Round: {status.get('last_finalized_round', 'N/A')}")
    print(f"Total Supply: {status.get('total_supply', 0) / 100_000_000:.2f} UDAG")
    print(f"Mempool: {status.get('mempool_size', 0)} txs")
    print(f"DAG Vertices: {status.get('dag_vertices', 0)}")
    print(f"Finalized Count: {status.get('finalized_count', 0)}")
    
    # Calculate processing throughput
    if successful > 0:
        total_time_with_processing = time.time() - start_time
        print()
        print(f"Total time (including processing): {total_time_with_processing:.2f}s")
        print(f"End-to-end throughput: {successful / total_time_with_processing:.2f} TPS")
    
    print()
    print("=== Throughput test complete ===")

if __name__ == "__main__":
    main()
