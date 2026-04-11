//! Cross-implementation compatibility test between the Rust bridge signing
//! code in `ultradag-coin::bridge` and the Solidity verifier in
//! `bridge/src/UDAGBridgeValidator.sol`.
//!
//! The two implementations never actually call each other at test time (no EVM
//! embedded in Rust), so the only way to validate they agree is to:
//!
//!   1. Fix a set of bridge inputs.
//!   2. Have the Rust side produce `solidity_message_hash()`, the validator
//!      Ethereum addresses, and the threshold signatures.
//!   3. Hardcode the resulting byte strings as "golden vectors" in BOTH this
//!      Rust test and in `bridge/test/BridgeRustCompat.t.sol`.
//!   4. The Rust test asserts the current Rust output matches the golden
//!      vectors byte-for-byte.
//!   5. The Solidity test passes those same golden vectors into
//!      `claimWithdrawal()` and asserts it succeeds.
//!
//! If either side drifts — a different EIP-191 prefix, a different ABI layout,
//! a different secp256k1 key derivation — exactly one of the two tests fails
//! and you get immediate feedback on which side is broken. Both tests use
//! deterministic inputs, so there is no nondeterminism between CI runs.
//!
//! The signatures are produced via RFC 6979 (deterministic ECDSA via k256's
//! `sign_prehash_recoverable`), so they are byte-for-byte stable across
//! machines and across k256 patch releases.

use ultradag_coin::address::Address;
use ultradag_coin::bridge::{
    derive_secp_key_from_ed25519, eth_address_from_secp_key, sign_for_bridge,
    BridgeAttestation, BridgeProof, SignedBridgeAttestation,
};
use std::collections::HashSet;

// ─── Fixed inputs (also hardcoded in the Solidity test) ────────────────

const SENDER_BYTES: [u8; 20] = [0x01; 20];
const RECIPIENT_BYTES: [u8; 20] = [0x02; 20];
const BRIDGE_CONTRACT: [u8; 20] = [0x03; 20];
const AMOUNT: u64 = 100_000_000; // 1 UDAG in 8-decimal sats
const NONCE: u64 = 42;
const CHAIN_ID: u64 = 421614; // Arbitrum Sepolia
const CREATION_ROUND: u64 = 0;

// Deterministic validator seeds — fed to `derive_secp_key_from_ed25519`.
// Real validators would use independently-generated secp256k1 keys in
// production; this test exercises the testnet-derivation path.
const VALIDATOR_SEEDS: [[u8; 32]; 3] = [
    [0x10; 32],
    [0x20; 32],
    [0x30; 32],
];

/// Pretty-print `bytes` as a lowercase hex string (no `0x` prefix, no separator).
fn to_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{:02x}", b));
    }
    out
}

/// Build the fixed attestation used by both sides.
fn fixed_attestation() -> BridgeAttestation {
    BridgeAttestation::new_with_contract(
        Address(SENDER_BYTES),
        RECIPIENT_BYTES,
        AMOUNT,
        NONCE,
        CHAIN_ID,
        BRIDGE_CONTRACT,
        CREATION_ROUND,
    )
}

/// Build the three (secp256k1 key, Ethereum address) pairs from the fixed seeds.
fn fixed_validator_keys() -> Vec<(k256::ecdsa::SigningKey, [u8; 20])> {
    VALIDATOR_SEEDS
        .iter()
        .map(|seed| {
            let sk = derive_secp_key_from_ed25519(seed)
                .expect("deterministic test seed must derive a valid scalar");
            let addr = eth_address_from_secp_key(&sk);
            (sk, addr)
        })
        .collect()
}

// ═══════════════════════════════════════════════════════════════════════
//  Capture mode
// ═══════════════════════════════════════════════════════════════════════
//
// Run with `cargo test -p ultradag-coin --test bridge_solidity_compat \
//           -- --nocapture capture_golden_vectors` to print the values that
// should be hardcoded into the golden constants below AND into
// `bridge/test/BridgeRustCompat.t.sol`.
//
// This is a one-time (or on-drift) maintenance task — regular CI runs exercise
// the `golden_vectors_self_consistency` test below, which is a hard equality
// check against the hardcoded values.

#[test]
#[ignore = "capture-only; run manually when updating golden vectors"]
fn capture_golden_vectors() {
    let att = fixed_attestation();
    let msg_hash = att.solidity_message_hash();

    println!("─── SOLIDITY MESSAGE HASH ─────────────────────");
    println!("messageHash: 0x{}", to_hex(&msg_hash));
    println!();

    let keys = fixed_validator_keys();

    println!("─── VALIDATOR ADDRESSES ───────────────────────");
    for (i, (_sk, addr)) in keys.iter().enumerate() {
        println!("validator[{}]: 0x{}", i, to_hex(addr));
    }
    println!();

    println!("─── RAW SIGNATURES ────────────────────────────");
    let mut signed: Vec<SignedBridgeAttestation> = Vec::new();
    for (i, (sk, addr)) in keys.iter().enumerate() {
        let sig = sign_for_bridge(&msg_hash, sk).expect("signing must succeed");
        println!("signature[{}]: 0x{}", i, to_hex(&sig));
        let dummy_dag_address = Address([(i as u8).wrapping_add(0xa0); 20]);
        signed.push(SignedBridgeAttestation::new(
            att.clone(),
            dummy_dag_address,
            *addr,
            sig,
        ));
    }
    println!();

    let proof = BridgeProof::new(att, signed);
    let encoded = proof.encode_signatures();

    println!("─── ENCODED SIGNATURE BLOB (sorted by address) ");
    println!("length: {} bytes ({} signatures × 65)", encoded.len(), encoded.len() / 65);
    println!("encoded: 0x{}", to_hex(&encoded));
}

// ═══════════════════════════════════════════════════════════════════════
//  Golden vectors (locked-in expected values)
// ═══════════════════════════════════════════════════════════════════════
//
// These MUST match the constants in `bridge/test/BridgeRustCompat.t.sol`.
// If you change any of the fixed inputs above, re-run `capture_golden_vectors`
// and paste the new hex into both this file and the Solidity test.

const EXPECTED_MESSAGE_HASH_HEX: &str =
    "1931be052fdf4b7d366afefa26634aeaf9fe45c5640ddfc970115e1664d60734";

// Validator addresses in the order they are derived from VALIDATOR_SEEDS
// (i.e. NOT the sorted-by-address order that the encoded blob uses).
const EXPECTED_VALIDATOR_ADDRESSES_HEX: [&str; 3] = [
    "d184f584858b5b57d1de097f93f9f792be60c6fe",
    "9f836d149b9690b66e6238c9ac679f462c3a2c38",
    "b3881fb59fde3949ba17be6f16a8eb810b9d87e2",
];

// Raw signatures in the same VALIDATOR_SEEDS order as the addresses above.
// These are EIP-191-prefixed ECDSA-recoverable sigs (r || s || v, 65 bytes).
const EXPECTED_SIGNATURES_HEX: [&str; 3] = [
    "ae2eb3ee02dfd02fa1cbf6602d6e2d9e018daf48b86451ebefb7c6f88483349a22c1613eb09582deab2fad4ae54652c6a4caf746da68f43e6fb102bc237ebf541c",
    "0d1b95c870d33c135f5be3cc994158b4ce89a0dbeeefcc225cb95d0f06555cba2d1c566d04421f73dfe84ecc8e5c10ff964e0dd1bf15f8d25365fabe2a39f9871c",
    "c82c59024a3b3621dd7bb1ea503f0bb22b894fa1f5dffdaa0e317bd7f7e38d9059256995a95e899ce81d2cc5df0a1033016fbf612c72776e60b9554f0fa2f38a1b",
];

// Encoded blob is the concatenation of the three signatures above in
// ASCENDING-by-Ethereum-address order, which is:
//   validator[1] (0x9f83...) → validator[2] (0xb388...) → validator[0] (0xd184...)
// This is exactly what the Solidity contract's _verifyThresholdSignatures
// requires.
const EXPECTED_ENCODED_HEX: &str = "0d1b95c870d33c135f5be3cc994158b4ce89a0dbeeefcc225cb95d0f06555cba2d1c566d04421f73dfe84ecc8e5c10ff964e0dd1bf15f8d25365fabe2a39f9871cc82c59024a3b3621dd7bb1ea503f0bb22b894fa1f5dffdaa0e317bd7f7e38d9059256995a95e899ce81d2cc5df0a1033016fbf612c72776e60b9554f0fa2f38a1bae2eb3ee02dfd02fa1cbf6602d6e2d9e018daf48b86451ebefb7c6f88483349a22c1613eb09582deab2fad4ae54652c6a4caf746da68f43e6fb102bc237ebf541c";

#[test]
fn golden_vectors_self_consistency() {
    // Skip cleanly until the golden vectors have been populated.
    if EXPECTED_MESSAGE_HASH_HEX.starts_with("__") {
        eprintln!(
            "golden vectors not yet captured — run `capture_golden_vectors` \
             with --ignored and paste the output."
        );
        return;
    }

    let att = fixed_attestation();
    let msg_hash = att.solidity_message_hash();
    assert_eq!(
        to_hex(&msg_hash),
        EXPECTED_MESSAGE_HASH_HEX,
        "solidity_message_hash drifted — Rust side changed its ABI encoding \
         or hash construction since the golden vector was recorded."
    );

    let keys = fixed_validator_keys();
    for (i, (_sk, addr)) in keys.iter().enumerate() {
        assert_eq!(
            to_hex(addr),
            EXPECTED_VALIDATOR_ADDRESSES_HEX[i],
            "validator[{}] Ethereum address drifted — \
             derive_secp_key_from_ed25519 or eth_address_from_secp_key changed.",
            i
        );
    }

    let mut signed: Vec<SignedBridgeAttestation> = Vec::new();
    for (i, (sk, addr)) in keys.iter().enumerate() {
        let sig = sign_for_bridge(&msg_hash, sk).expect("signing must succeed");
        assert_eq!(
            to_hex(&sig),
            EXPECTED_SIGNATURES_HEX[i],
            "signature[{}] drifted — sign_for_bridge changed its prefix, \
             nonce derivation, or recovery-id convention.",
            i
        );
        let dummy_dag_address = Address([(i as u8).wrapping_add(0xa0); 20]);
        signed.push(SignedBridgeAttestation::new(
            att.clone(),
            dummy_dag_address,
            *addr,
            sig,
        ));
    }

    // Encoded blob must match what the Solidity test submits, byte-for-byte.
    let proof = BridgeProof::new(att, signed);
    let encoded = proof.encode_signatures();
    assert_eq!(
        to_hex(&encoded),
        EXPECTED_ENCODED_HEX,
        "encoded signature blob drifted — BridgeProof::encode_signatures \
         ordering or length changed."
    );
}

// ═══════════════════════════════════════════════════════════════════════
//  Sanity: the Rust side verifies its own signatures
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn rust_side_verifies_its_own_signatures() {
    let att = fixed_attestation();
    let msg_hash = att.solidity_message_hash();
    let keys = fixed_validator_keys();

    let mut active: HashSet<Address> = HashSet::new();
    let mut signed: Vec<SignedBridgeAttestation> = Vec::new();
    for (i, (sk, addr)) in keys.iter().enumerate() {
        let dag_addr = Address([(i as u8).wrapping_add(0xa0); 20]);
        active.insert(dag_addr);
        let sig = sign_for_bridge(&msg_hash, sk).expect("signing must succeed");
        let s = SignedBridgeAttestation::new(att.clone(), dag_addr, *addr, sig);
        assert!(
            s.verify_signature(),
            "SignedBridgeAttestation::verify_signature failed for validator {}",
            i
        );
        signed.push(s);
    }

    let proof = BridgeProof::new(att, signed);
    // threshold = 3 matches MIN_VALIDATORS=3, 3-of-3 BFT safety for this test
    proof.verify(3, &active).expect("bridge proof must verify");
}

