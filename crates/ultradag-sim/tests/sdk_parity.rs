//! Cross-SDK signable_bytes parity test.
//!
//! Verifies that the canonical Rust signable_bytes for each transaction type
//! matches the expected byte layout documented in all 4 SDKs (JS, Python, Go, Rust SDK).
//!
//! This test computes the canonical hex for deterministic inputs and prints it
//! in a machine-readable format.  The companion shell script
//! `tools/tests/sdk-parity-check.sh` runs the Python, JS, and Go SDKs with the
//! same inputs and compares the hex output against the Rust canonical values.

use ultradag_coin::address::{Address, SecretKey, Signature};
use ultradag_coin::tx::transaction::TransferTx;
use ultradag_coin::tx::stake::{StakeTx, UnstakeTx};
use ultradag_coin::tx::delegate::{DelegateTx, UndelegateTx, SetCommissionTx};
use ultradag_coin::governance::transactions::{CreateProposalTx, VoteTx};
use ultradag_coin::governance::ProposalType;

/// Deterministic secret key seed used across all SDKs.
const SECRET_SEED: [u8; 32] = [0x01; 32];

/// Deterministic recipient / validator address used across all SDKs.
const TO_ADDRESS: [u8; 32] = [0x02; 32];

/// Amount: 1 UDAG = 1_000_000_000 sats (note: COIN = 100_000_000, so 10 * COIN would be
/// 10 UDAG.  The spec says 1_000_000_000 which is 10 UDAG in the UltraDAG unit system,
/// but we use the exact value specified in the task).
const AMOUNT: u64 = 1_000_000_000;

/// Fee: 10_000 sats (MIN_FEE_SATS).
const FEE: u64 = 10_000;

/// Nonce: 42.
const NONCE: u64 = 42;

fn secret_key() -> SecretKey {
    SecretKey::from_bytes(SECRET_SEED)
}

fn from_address() -> Address {
    secret_key().address()
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

// ---------------------------------------------------------------------------
// Transfer
// ---------------------------------------------------------------------------

#[test]
fn canonical_transfer_signable_bytes() {
    let sk = secret_key();
    let from = from_address();
    let to = Address(TO_ADDRESS);

    let tx = TransferTx {
        from,
        to,
        amount: AMOUNT,
        fee: FEE,
        nonce: NONCE,
        pub_key: sk.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
        memo: None,
    };

    let signable = tx.signable_bytes();
    let hex = to_hex(&signable);

    // Print in machine-readable format for the shell script
    println!("SDK_PARITY:TRANSFER:{}", hex);

    // Verify structure: NETWORK_ID(19) + "transfer"(8) + from(32) + to(32) + amount(8) + fee(8) + nonce(8) = 115
    assert_eq!(signable.len(), 115, "Transfer signable_bytes length mismatch");

    // Verify the signature round-trips
    let mut signed_tx = tx;
    signed_tx.signature = sk.sign(&signed_tx.signable_bytes());
    assert!(signed_tx.verify_signature(), "Transfer signature verification failed");
}

// ---------------------------------------------------------------------------
// Stake
// ---------------------------------------------------------------------------

#[test]
fn canonical_stake_signable_bytes() {
    let sk = secret_key();
    let from = from_address();

    let tx = StakeTx {
        from,
        amount: AMOUNT,
        nonce: NONCE,
        pub_key: sk.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
    };

    let signable = tx.signable_bytes();
    let hex = to_hex(&signable);

    println!("SDK_PARITY:STAKE:{}", hex);

    // NETWORK_ID(19) + "stake"(5) + from(32) + amount(8) + nonce(8) = 72
    assert_eq!(signable.len(), 72, "Stake signable_bytes length mismatch");

    let mut signed_tx = tx;
    signed_tx.signature = sk.sign(&signed_tx.signable_bytes());
    assert!(signed_tx.verify_signature(), "Stake signature verification failed");
}

// ---------------------------------------------------------------------------
// Delegate
// ---------------------------------------------------------------------------

#[test]
fn canonical_delegate_signable_bytes() {
    let sk = secret_key();
    let from = from_address();
    let validator = Address(TO_ADDRESS);

    let tx = DelegateTx {
        from,
        validator,
        amount: AMOUNT,
        nonce: NONCE,
        pub_key: sk.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
    };

    let signable = tx.signable_bytes();
    let hex = to_hex(&signable);

    println!("SDK_PARITY:DELEGATE:{}", hex);

    // NETWORK_ID(19) + "delegate"(8) + from(32) + validator(32) + amount(8) + nonce(8) = 107
    assert_eq!(signable.len(), 107, "Delegate signable_bytes length mismatch");

    let mut signed_tx = tx;
    signed_tx.signature = sk.sign(&signed_tx.signable_bytes());
    assert!(signed_tx.verify_signature(), "Delegate signature verification failed");
}

// ---------------------------------------------------------------------------
// Vote
// ---------------------------------------------------------------------------

#[test]
fn canonical_vote_signable_bytes() {
    let sk = secret_key();
    let from = from_address();

    let tx = VoteTx {
        from,
        proposal_id: 7,
        vote: true,
        fee: FEE,
        nonce: NONCE,
        pub_key: sk.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
    };

    let signable = tx.signable_bytes();
    let hex = to_hex(&signable);

    println!("SDK_PARITY:VOTE:{}", hex);

    // NETWORK_ID(19) + "vote"(4) + from(32) + proposal_id(8) + vote(1) + fee(8) + nonce(8) = 80
    assert_eq!(signable.len(), 80, "Vote signable_bytes length mismatch");

    let mut signed_tx = tx;
    signed_tx.signature = sk.sign(&signed_tx.signable_bytes());
    assert!(signed_tx.verify_signature(), "Vote signature verification failed");
}

// ---------------------------------------------------------------------------
// Unstake
// ---------------------------------------------------------------------------

#[test]
fn canonical_unstake_signable_bytes() {
    let sk = secret_key();
    let from = from_address();

    let tx = UnstakeTx {
        from,
        nonce: NONCE,
        pub_key: sk.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
    };

    let signable = tx.signable_bytes();
    let hex = to_hex(&signable);

    println!("SDK_PARITY:UNSTAKE:{}", hex);

    // NETWORK_ID(19) + "unstake"(7) + from(32) + nonce(8) = 66
    assert_eq!(signable.len(), 66, "Unstake signable_bytes length mismatch");
}

// ---------------------------------------------------------------------------
// Undelegate
// ---------------------------------------------------------------------------

#[test]
fn canonical_undelegate_signable_bytes() {
    let sk = secret_key();
    let from = from_address();

    let tx = UndelegateTx {
        from,
        nonce: NONCE,
        pub_key: sk.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
    };

    let signable = tx.signable_bytes();
    let hex = to_hex(&signable);

    println!("SDK_PARITY:UNDELEGATE:{}", hex);

    // NETWORK_ID(19) + "undelegate"(10) + from(32) + nonce(8) = 69
    assert_eq!(signable.len(), 69, "Undelegate signable_bytes length mismatch");
}

// ---------------------------------------------------------------------------
// SetCommission
// ---------------------------------------------------------------------------

#[test]
fn canonical_set_commission_signable_bytes() {
    let sk = secret_key();
    let from = from_address();

    let tx = SetCommissionTx {
        from,
        commission_percent: 15,
        nonce: NONCE,
        pub_key: sk.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
    };

    let signable = tx.signable_bytes();
    let hex = to_hex(&signable);

    println!("SDK_PARITY:SET_COMMISSION:{}", hex);

    // NETWORK_ID(19) + "set_commission"(14) + from(32) + commission(1) + nonce(8) = 74
    assert_eq!(signable.len(), 74, "SetCommission signable_bytes length mismatch");
}

// ---------------------------------------------------------------------------
// Print from_address for SDK scripts (they need it to build signable bytes)
// ---------------------------------------------------------------------------

#[test]
fn print_derived_from_address() {
    let sk = secret_key();
    let from = from_address();
    let pub_key = sk.verifying_key().to_bytes();

    println!("SDK_PARITY:FROM_ADDRESS:{}", to_hex(&from.0));
    println!("SDK_PARITY:PUBLIC_KEY:{}", to_hex(&pub_key));
    println!("SDK_PARITY:SECRET_SEED:{}", to_hex(&SECRET_SEED));
}
