//! Regression tests for GHSA-hf8w-rcvm-rgqr.
//!
//! RegisterNameTx used to short-circuit `verify_signature()` when a
//! fee_payer was present, making the owner's signature optional. That
//! let any funded attacker register any name to any victim address by
//! forging the `from` field (which is just a free-form byte string
//! without its signature). The fix requires the owner's signature
//! unconditionally; fee_payer only authorizes the fee debit.

use ultradag_coin::address::{SecretKey, Signature};
use ultradag_coin::constants::COIN;
use ultradag_coin::state::StateEngine;
use ultradag_coin::tx::name_registry::RegisterNameTx;
use ultradag_coin::tx::smart_account::FeePayer;

fn engine_with_funded(addrs: &[(&SecretKey, u64)]) -> StateEngine {
    let mut engine = StateEngine::new_with_genesis();
    for (sk, bal) in addrs {
        engine.faucet_credit(&sk.address(), *bal).unwrap();
    }
    engine
}

fn owner_signed_sponsored_tx(
    owner: &SecretKey,
    sponsor: &SecretKey,
    name: &str,
    fee: u64,
    owner_nonce: u64,
    sponsor_nonce: u64,
) -> RegisterNameTx {
    let mut tx = RegisterNameTx {
        from: owner.address(),
        name: name.to_string(),
        duration_years: 1,
        fee,
        nonce: owner_nonce,
        pub_key: owner.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
        fee_payer: None,
    };
    let signable = tx.signable_bytes();
    tx.signature = owner.sign(&signable);
    // Fee_payer signs the same payload; engine verifies it on apply.
    tx.fee_payer = Some(FeePayer {
        address: sponsor.address(),
        pub_key: sponsor.verifying_key().to_bytes(),
        signature: sponsor.sign(&signable),
        nonce: sponsor_nonce,
    });
    tx
}

#[test]
fn sponsored_registration_rejects_forged_from_without_owner_signature() {
    // Exact reporter PoC: attacker with balance, victim with none. Attacker
    // crafts a tx claiming `from = victim` with a zero owner signature,
    // then attaches a valid fee_payer (themselves). The old code accepted
    // this because `verify_signature()` returned true whenever fee_payer
    // was present. The fix must reject it at the signature layer.
    let victim = SecretKey::generate();
    let attacker = SecretKey::generate();
    let engine = engine_with_funded(&[(&attacker, 10_000 * COIN)]);
    assert_eq!(engine.balance(&victim.address()), 0);

    let mut tx = RegisterNameTx {
        from: victim.address(),
        name: "hijack".to_string(),
        duration_years: 1,
        fee: 0,
        nonce: 0,
        pub_key: victim.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
        fee_payer: None,
    };
    let signable = tx.signable_bytes();
    tx.fee_payer = Some(FeePayer {
        address: attacker.address(),
        pub_key: attacker.verifying_key().to_bytes(),
        signature: attacker.sign(&signable),
        nonce: 0,
    });

    assert!(
        !tx.verify_signature(),
        "GHSA-hf8w-rcvm-rgqr: forged-from sponsored tx must fail signature verification"
    );
}

#[test]
fn sponsored_registration_rejects_mismatched_pubkey() {
    // Variant: attacker sets from=victim but supplies their OWN pub_key,
    // signs with their own key (so ed25519 verification against the pub_key
    // succeeds). The address derived from pub_key must match `from`, else
    // the check rejects the tx.
    let victim = SecretKey::generate();
    let attacker = SecretKey::generate();
    let signable_key = SecretKey::generate();

    let mut tx = RegisterNameTx {
        from: victim.address(),
        name: "poc".to_string(),
        duration_years: 1,
        fee: 1000 * COIN,
        nonce: 0,
        pub_key: signable_key.verifying_key().to_bytes(), // not victim's key
        signature: Signature([0u8; 64]),
        fee_payer: None,
    };
    let signable = tx.signable_bytes();
    tx.signature = signable_key.sign(&signable);
    tx.fee_payer = Some(FeePayer {
        address: attacker.address(),
        pub_key: attacker.verifying_key().to_bytes(),
        signature: attacker.sign(&signable),
        nonce: 0,
    });

    assert!(
        !tx.verify_signature(),
        "pub_key address mismatch with `from` must fail verification"
    );
}

#[test]
fn sponsored_registration_accepts_owner_signed_tx() {
    // Legitimate meta-tx flow: owner signs intent, sponsor signs envelope.
    // Owner has zero UDAG; sponsor pays the fee. Both signatures valid.
    let owner = SecretKey::generate();
    let sponsor = SecretKey::generate();
    let mut engine = engine_with_funded(&[(&sponsor, 10_000 * COIN)]);
    assert_eq!(engine.balance(&owner.address()), 0);

    let tx = owner_signed_sponsored_tx(&owner, &sponsor, "alice", 100 * COIN, 0, 0);
    assert!(tx.verify_signature(), "owner-signed sponsored tx must verify");
    engine
        .apply_register_name_tx(&tx)
        .expect("owner-signed sponsored tx must apply");
    assert_eq!(engine.resolve_name("alice"), Some(owner.address()));
    // Fee debited from sponsor, not owner.
    assert!(engine.balance(&sponsor.address()) < 10_000 * COIN);
    assert_eq!(engine.balance(&owner.address()), 0);
}

#[test]
fn non_sponsored_registration_still_requires_owner_signature() {
    // Sanity: the non-sponsored path remains unchanged — zero-signature
    // tx must still be rejected.
    let owner = SecretKey::generate();
    let mut tx = RegisterNameTx {
        from: owner.address(),
        name: "bob".to_string(),
        duration_years: 1,
        fee: 100 * COIN,
        nonce: 0,
        pub_key: owner.verifying_key().to_bytes(),
        signature: Signature([0u8; 64]),
        fee_payer: None,
    };
    assert!(!tx.verify_signature(), "unsigned tx must fail");
    tx.signature = owner.sign(&tx.signable_bytes());
    assert!(tx.verify_signature(), "owner-signed tx must verify");
}
