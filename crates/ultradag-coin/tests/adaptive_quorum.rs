//! Regression tests for adaptive quorum threshold (auto-heal on validator dropout).
//!
//! The network must maintain liveness when some configured validators go offline,
//! as long as enough remain to form a 2/3 quorum of the *active* set.
//!
//! Security property preserved: only validators who have cryptographically
//! proven activity (signed vertices within the liveness window) count toward
//! the quorum base. Phantom registrations cannot shrink the quorum.

use ultradag_coin::consensus::validator_set::{ValidatorSet, LIVENESS_WINDOW_ROUNDS};
use ultradag_coin::address::Address;

fn addr(n: u8) -> Address {
    let mut a = [0u8; 20];
    a[0] = n;
    Address(a)
}

#[test]
fn adaptive_quorum_uses_configured_when_no_liveness_data() {
    let mut vs = ValidatorSet::new(1);
    vs.set_configured_validators(5);
    for i in 1..=5 {
        vs.register(addr(i));
    }
    // No vertices produced yet — should fall back to static quorum.
    assert_eq!(vs.adaptive_quorum_threshold(100), 4); // ceil(2*5/3) = 4
}

#[test]
fn adaptive_quorum_shrinks_when_validators_go_offline() {
    let mut vs = ValidatorSet::new(1);
    vs.set_configured_validators(5);
    for i in 1..=5 {
        vs.register(addr(i));
    }

    // All 5 produced recently.
    for i in 1..=5 {
        vs.record_production(addr(i), 1000);
    }
    assert_eq!(vs.adaptive_quorum_threshold(1000), 4); // ceil(2*5/3) = 4

    // Jump ahead 600 rounds — only 2 validators produced recently.
    vs.record_production(addr(1), 1600);
    vs.record_production(addr(2), 1600);
    // validators 3, 4, 5 last produced at 1000, now outside 500-round window.
    assert_eq!(vs.active_validator_count(1600), 2);
    assert_eq!(vs.adaptive_quorum_threshold(1600), 2); // ceil(2*2/3) = 2
}

#[test]
fn adaptive_quorum_never_exceeds_configured() {
    let mut vs = ValidatorSet::new(1);
    vs.set_configured_validators(3);
    // Register 10 — but only 3 configured.
    for i in 1..=10 {
        vs.register(addr(i));
        vs.record_production(addr(i), 100);
    }
    // Active = 10, but configured caps it at 3.
    assert_eq!(vs.adaptive_quorum_threshold(100), 2); // ceil(2*3/3) = 2
}

#[test]
fn adaptive_quorum_falls_back_to_static_below_min_validators() {
    let mut vs = ValidatorSet::new(2);
    vs.set_configured_validators(5);
    for i in 1..=5 {
        vs.register(addr(i));
    }
    // Only 1 active — below min_validators=2.
    // Falls back to static quorum (ceil(2*5/3)=4) rather than MAX,
    // preserving safety: you need the full static quorum until liveness
    // data shows enough validators producing.
    vs.record_production(addr(1), 100);
    assert_eq!(vs.adaptive_quorum_threshold(100), 4);
}

#[test]
fn phantom_validators_cannot_inflate_quorum_via_adaptive_path() {
    let mut vs = ValidatorSet::new(1);
    vs.set_configured_validators(3);
    // Real validators produce.
    vs.record_production(addr(1), 100);
    vs.record_production(addr(2), 100);
    vs.record_production(addr(3), 100);
    // Phantom validators register but never produce.
    for i in 4..=20 {
        vs.register(addr(i));
    }
    // Active count still 3 (the ones who produced).
    assert_eq!(vs.active_validator_count(100), 3);
    assert_eq!(vs.adaptive_quorum_threshold(100), 2);
}

#[test]
fn liveness_window_boundary() {
    let mut vs = ValidatorSet::new(1);
    vs.set_configured_validators(3);
    vs.register(addr(1));
    vs.register(addr(2));
    vs.register(addr(3));
    vs.record_production(addr(1), 100);
    vs.record_production(addr(2), 100);
    vs.record_production(addr(3), 100);

    // Exactly at the window boundary.
    let at_boundary = 100 + LIVENESS_WINDOW_ROUNDS;
    assert_eq!(vs.active_validator_count(at_boundary), 3);

    // One round past the window.
    assert_eq!(vs.active_validator_count(at_boundary + 1), 0);
    // Falls back to static quorum.
    assert_eq!(vs.adaptive_quorum_threshold(at_boundary + 1), 2); // static = ceil(2*3/3) = 2
}

#[test]
fn validator_returning_restores_quorum() {
    let mut vs = ValidatorSet::new(1);
    vs.set_configured_validators(5);
    for i in 1..=5 {
        vs.register(addr(i));
    }

    // All produced at round 100.
    for i in 1..=5 {
        vs.record_production(addr(i), 100);
    }

    // Fast-forward: only 2 active at round 1000.
    vs.record_production(addr(1), 1000);
    vs.record_production(addr(2), 1000);
    assert_eq!(vs.adaptive_quorum_threshold(1000), 2);

    // Validator 3 comes back at round 1100.
    vs.record_production(addr(3), 1100);
    // Now 3 active (1, 2, 3 — all within 500 rounds of 1100).
    assert_eq!(vs.active_validator_count(1100), 3);
    assert_eq!(vs.adaptive_quorum_threshold(1100), 2); // ceil(2*3/3) = 2

    // 4 and 5 come back.
    vs.record_production(addr(4), 1200);
    vs.record_production(addr(5), 1200);
    assert_eq!(vs.active_validator_count(1200), 5);
    assert_eq!(vs.adaptive_quorum_threshold(1200), 4); // back to full 2/3 of 5
}
