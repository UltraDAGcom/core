/**
 * Cross-SDK parity helper — JavaScript/TypeScript SDK.
 *
 * Computes signable_bytes for all transaction types using the JS SDK
 * and prints hex output in SDK_PARITY:<TYPE>:<hex> format.
 *
 * Usage: tsx tools/tests/sdk_parity_js.ts <secret_seed_hex> <from_address_hex> <public_key_hex>
 */

import {
  transferSignableBytes,
  stakeSignableBytes,
  unstakeSignableBytes,
  delegateSignableBytes,
  undelegateSignableBytes,
  setCommissionSignableBytes,
  voteSignableBytes,
} from "../../sdk/javascript/src/transactions.ts";

function hexToBytes(hex: string): Uint8Array {
  const bytes = new Uint8Array(hex.length / 2);
  for (let i = 0; i < hex.length; i += 2) {
    bytes[i / 2] = parseInt(hex.substring(i, i + 2), 16);
  }
  return bytes;
}

function bytesToHex(bytes: Uint8Array): string {
  return Array.from(bytes)
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
}

function main() {
  const args = process.argv.slice(2);
  if (args.length !== 3) {
    console.error(
      `Usage: tsx ${process.argv[1]} <secret_seed_hex> <from_address_hex> <public_key_hex>`
    );
    process.exit(1);
  }

  const [_secretSeedHex, fromAddressHex, _publicKeyHex] = args;
  const fromAddr = hexToBytes(fromAddressHex);
  const toAddr = new Uint8Array(32).fill(0x02);

  // Shared parameters (must match Rust test)
  const amount = 1_000_000_000n;
  const fee = 10_000n;
  const nonce = 42n;

  // Transfer
  const transfer = transferSignableBytes(fromAddr, toAddr, amount, fee, nonce);
  console.log(`SDK_PARITY:TRANSFER:${bytesToHex(transfer)}`);

  // Stake
  const stake = stakeSignableBytes(fromAddr, amount, nonce);
  console.log(`SDK_PARITY:STAKE:${bytesToHex(stake)}`);

  // Delegate
  const delegate = delegateSignableBytes(fromAddr, toAddr, amount, nonce);
  console.log(`SDK_PARITY:DELEGATE:${bytesToHex(delegate)}`);

  // Vote (proposal_id=7, approve=true, fee=10000, nonce=42)
  const vote = voteSignableBytes(fromAddr, 7n, true, fee, nonce);
  console.log(`SDK_PARITY:VOTE:${bytesToHex(vote)}`);

  // Unstake
  const unstake = unstakeSignableBytes(fromAddr, nonce);
  console.log(`SDK_PARITY:UNSTAKE:${bytesToHex(unstake)}`);

  // Undelegate
  const undelegate = undelegateSignableBytes(fromAddr, nonce);
  console.log(`SDK_PARITY:UNDELEGATE:${bytesToHex(undelegate)}`);

  // SetCommission (commission_percent=15, nonce=42)
  const setCommission = setCommissionSignableBytes(fromAddr, 15, nonce);
  console.log(`SDK_PARITY:SET_COMMISSION:${bytesToHex(setCommission)}`);
}

main();
