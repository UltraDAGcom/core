#!/usr/bin/env python3
"""
UltraDAG Simple Load Test
Measures transaction throughput with concurrent requests
"""

import asyncio
import aiohttp
import time
from datetime import datetime

NODE_URL = "http://127.0.0.1:8001"
VALIDATOR_SECRET = "7fab8b10b9a1e8f1b4e7f6788eafdb5531c66299671e8ceba13e0cbde36c7f87"
RECEIVER = "0000000000000000000000000000000000000000000000000000000000000001"

NUM_TXS = 500
CONCURRENT = 50
AMOUNT = 1000
FEE = 100

async def get_balance(session, address):
    """Get balance and nonce for an address"""
    async with session.get(f"{NODE_URL}/balance/{address}") as response:
        return await response.json()

async def send_transaction(session, tx_num):
    """Send a single transaction"""
    try:
        async with session.post(
            f"{NODE_URL}/tx",
            json={
                "from_secret": VALIDATOR_SECRET,
                "to": RECEIVER,
                "amount": AMOUNT,
                "fee": FEE
            },
            timeout=aiohttp.ClientTimeout(total=10)
        ) as response:
            if response.status == 200:
                return True, await response.json()
            else:
                text = await response.text()
                return False, text
    except Exception as e:
        return False, str(e)

async def get_status(session):
    """Get network status"""
    async with session.get(f"{NODE_URL}/status") as response:
        return await response.json()

async def main():
    print("=== UltraDAG Load Test ===")
    print(f"Node: {NODE_URL}")
    print(f"Transactions: {NUM_TXS}")
    print(f"Concurrent requests: {CONCURRENT}")
    print()

    async with aiohttp.ClientSession() as session:
        # Get initial status
        print("=== Initial Network Status ===")
        status = await get_status(session)
        print(f"DAG Round: {status['dag_round']}")
        print(f"Finalized Round: {status.get('last_finalized_round', 'N/A')}")
        print(f"Total Supply: {status['total_supply'] / 100_000_000:.2f} UDAG")
        print(f"Mempool: {status['mempool_size']} txs")
        print()

        # Send transactions
        print(f"=== Sending {NUM_TXS} transactions ===")
        start_time = time.time()
        
        successful = 0
        failed = 0
        errors = {}
        
        # Send in batches
        for batch_start in range(0, NUM_TXS, CONCURRENT):
            batch_size = min(CONCURRENT, NUM_TXS - batch_start)
            tasks = [send_transaction(session, batch_start + i) for i in range(batch_size)]
            results = await asyncio.gather(*tasks)
            
            for success, result in results:
                if success:
                    successful += 1
                    if successful % 50 == 0:
                        print(f"  Progress: {successful}/{NUM_TXS} successful")
                else:
                    failed += 1
                    error_msg = str(result)[:50]
                    errors[error_msg] = errors.get(error_msg, 0) + 1
        
        end_time = time.time()
        duration = end_time - start_time
        
        print()
        print("=== Results ===")
        print(f"Total time: {duration:.2f}s")
        print(f"Successful: {successful}")
        print(f"Failed: {failed}")
        print(f"Success rate: {(successful / NUM_TXS * 100):.1f}%")
        print(f"Throughput: {successful / duration:.2f} TPS")
        
        if errors:
            print()
            print("Top errors:")
            for error, count in sorted(errors.items(), key=lambda x: x[1], reverse=True)[:5]:
                print(f"  {count}x: {error}")
        
        # Wait for processing
        print()
        print("=== Waiting 10 seconds for processing ===")
        await asyncio.sleep(10)
        
        # Get final status
        print()
        print("=== Final Network Status ===")
        status = await get_status(session)
        print(f"DAG Round: {status['dag_round']}")
        print(f"Finalized Round: {status.get('last_finalized_round', 'N/A')}")
        print(f"Total Supply: {status['total_supply'] / 100_000_000:.2f} UDAG")
        print(f"Mempool: {status['mempool_size']} txs")
        print(f"DAG Vertices: {status['dag_vertices']}")
        print()
        
        print("=== Load test complete ===")

if __name__ == "__main__":
    asyncio.run(main())
