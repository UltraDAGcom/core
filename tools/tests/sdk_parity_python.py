#!/usr/bin/env python3
"""
Cross-SDK parity helper — Python SDK.

Computes signable_bytes for all transaction types using the Python SDK
and prints hex output in SDK_PARITY:<TYPE>:<hex> format.

Usage: python3 sdk_parity_python.py <secret_seed_hex> <from_address_hex> <public_key_hex>
"""

import sys
import os

# Add the Python SDK to the path
SDK_DIR = os.path.join(os.path.dirname(__file__), "..", "..", "sdk", "python")
sys.path.insert(0, SDK_DIR)

from ultradag.transactions import (
    transfer_signable_bytes,
    stake_signable_bytes,
    unstake_signable_bytes,
    delegate_signable_bytes,
    undelegate_signable_bytes,
    set_commission_signable_bytes,
    vote_signable_bytes,
)


def main():
    if len(sys.argv) != 4:
        print(f"Usage: {sys.argv[0]} <secret_seed_hex> <from_address_hex> <public_key_hex>",
              file=sys.stderr)
        sys.exit(1)

    _secret_seed_hex = sys.argv[1]
    from_address_hex = sys.argv[2]
    _public_key_hex = sys.argv[3]

    from_addr = bytes.fromhex(from_address_hex)
    to_addr = bytes(32 * [0x02])

    # Shared parameters (must match Rust test)
    amount = 1_000_000_000
    fee = 10_000
    nonce = 42

    # Transfer
    transfer = transfer_signable_bytes(from_addr, to_addr, amount, fee, nonce)
    print(f"SDK_PARITY:TRANSFER:{transfer.hex()}")

    # Stake
    stake = stake_signable_bytes(from_addr, amount, nonce)
    print(f"SDK_PARITY:STAKE:{stake.hex()}")

    # Delegate
    delegate = delegate_signable_bytes(from_addr, to_addr, amount, nonce)
    print(f"SDK_PARITY:DELEGATE:{delegate.hex()}")

    # Vote (proposal_id=7, approve=True, fee=10000, nonce=42)
    vote = vote_signable_bytes(from_addr, 7, True, fee, nonce)
    print(f"SDK_PARITY:VOTE:{vote.hex()}")

    # Unstake
    unstake = unstake_signable_bytes(from_addr, nonce)
    print(f"SDK_PARITY:UNSTAKE:{unstake.hex()}")

    # Undelegate
    undelegate = undelegate_signable_bytes(from_addr, nonce)
    print(f"SDK_PARITY:UNDELEGATE:{undelegate.hex()}")

    # SetCommission (commission_percent=15, nonce=42)
    set_commission = set_commission_signable_bytes(from_addr, 15, nonce)
    print(f"SDK_PARITY:SET_COMMISSION:{set_commission.hex()}")


if __name__ == "__main__":
    main()
