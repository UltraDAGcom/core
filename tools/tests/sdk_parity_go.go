// Cross-SDK parity helper — Go SDK.
//
// Computes signable_bytes for all transaction types using the Go SDK
// and prints hex output in SDK_PARITY:<TYPE>:<hex> format.
//
// Usage: cd sdk/go && go run ../../tools/tests/sdk_parity_go.go <secret_seed_hex> <from_address_hex> <public_key_hex>

package main

import (
	"encoding/hex"
	"fmt"
	"os"

	"github.com/ultradag/sdk-go/ultradag"
)

func main() {
	if len(os.Args) != 4 {
		fmt.Fprintf(os.Stderr, "Usage: %s <secret_seed_hex> <from_address_hex> <public_key_hex>\n", os.Args[0])
		os.Exit(1)
	}

	fromAddressHex := os.Args[2]

	fromBytes, err := hex.DecodeString(fromAddressHex)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Invalid from_address hex: %v\n", err)
		os.Exit(1)
	}

	var fromAddr [32]byte
	copy(fromAddr[:], fromBytes)

	var toAddr [32]byte
	for i := range toAddr {
		toAddr[i] = 0x02
	}

	// Shared parameters (must match Rust test)
	var amount uint64 = 1_000_000_000
	var fee uint64 = 10_000
	var nonce uint64 = 42

	// Transfer
	transfer := ultradag.TransferSignableBytes(fromAddr, toAddr, amount, fee, nonce, nil)
	fmt.Printf("SDK_PARITY:TRANSFER:%s\n", hex.EncodeToString(transfer))

	// Stake
	stake := ultradag.StakeSignableBytes(fromAddr, amount, nonce)
	fmt.Printf("SDK_PARITY:STAKE:%s\n", hex.EncodeToString(stake))

	// Delegate
	delegate := ultradag.DelegateSignableBytes(fromAddr, toAddr, amount, nonce)
	fmt.Printf("SDK_PARITY:DELEGATE:%s\n", hex.EncodeToString(delegate))

	// Vote (proposal_id=7, approve=true, fee=10000, nonce=42)
	vote := ultradag.VoteSignableBytes(fromAddr, 7, true, fee, nonce)
	fmt.Printf("SDK_PARITY:VOTE:%s\n", hex.EncodeToString(vote))

	// Unstake
	unstake := ultradag.UnstakeSignableBytes(fromAddr, nonce)
	fmt.Printf("SDK_PARITY:UNSTAKE:%s\n", hex.EncodeToString(unstake))

	// Undelegate
	undelegate := ultradag.UndelegateSignableBytes(fromAddr, nonce)
	fmt.Printf("SDK_PARITY:UNDELEGATE:%s\n", hex.EncodeToString(undelegate))

	// SetCommission (commission_percent=15, nonce=42)
	setCommission := ultradag.SetCommissionSignableBytes(fromAddr, 15, nonce)
	fmt.Printf("SDK_PARITY:SET_COMMISSION:%s\n", hex.EncodeToString(setCommission))
}
