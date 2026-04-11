//! Regression test: the four protocol address constants in constants.rs
//! (founder/IDO/Ecosystem/Reserve) must match the canonical bech32m addresses
//! provided at the April 2026 tokenomics update. If someone edits the byte
//! arrays in constants.rs, this test catches the drift by round-tripping
//! through bech32m.
//!
//! Canonical sources:
//!   @founder    udag1dps6ypxmdj7qajqv86u6vhe7rzgx5ldm9jh2h0  (2026-04-11,
//!               rotated from the previous founder — key generated offline
//!               via examples/mainnet_keygen.rs)
//!   @liquidity  udag1rvdfs928eu7trrc33wj2edwctdkt08gdkmhppx  (2026-04-10)
//!   @ecosystem  udag17z5yull0zrhrmkvw6337f3hdh3rfs7mgnhmvfz  (2026-04-10)
//!   @reserve    udag1rs22h8y2ack0285efhe4g57hm8kr8z7a4gkxp8  (2026-04-10)

use ultradag_coin::address::Address;
use ultradag_coin::constants::{
    dev_address, ecosystem_address, ido_address, reserve_address,
    DEV_ADDRESS_BYTES, ECOSYSTEM_ADDRESS_BYTES, IDO_ADDRESS_BYTES, RESERVE_ADDRESS_BYTES,
};

/// The canonical bech32m form of each protocol address must decode to the
/// hardcoded 20-byte constant in constants.rs.
#[test]
fn protocol_address_constants_match_canonical_bech32m() {
    let cases = [
        (
            "FOUNDER",
            "udag1dps6ypxmdj7qajqv86u6vhe7rzgx5ldm9jh2h0",
            DEV_ADDRESS_BYTES,
        ),
        (
            "IDO",
            "udag1rvdfs928eu7trrc33wj2edwctdkt08gdkmhppx",
            IDO_ADDRESS_BYTES,
        ),
        (
            "ECOSYSTEM",
            "udag17z5yull0zrhrmkvw6337f3hdh3rfs7mgnhmvfz",
            ECOSYSTEM_ADDRESS_BYTES,
        ),
        (
            "RESERVE",
            "udag1rs22h8y2ack0285efhe4g57hm8kr8z7a4gkxp8",
            RESERVE_ADDRESS_BYTES,
        ),
    ];
    for (label, canonical, const_bytes) in &cases {
        let decoded = Address::from_bech32(canonical)
            .unwrap_or_else(|| panic!("{label}: canonical bech32m failed to decode: {canonical}"));
        assert_eq!(
            decoded.0, *const_bytes,
            "{label}: constant bytes don't match canonical bech32m {canonical}"
        );
    }
}

/// The accessor functions must return the hardcoded constants (no env var override
/// is set in this test process, so the fallback path is exercised).
#[test]
fn protocol_address_accessors_return_constants() {
    assert_eq!(dev_address().0, DEV_ADDRESS_BYTES);
    assert_eq!(ido_address().0, IDO_ADDRESS_BYTES);
    assert_eq!(ecosystem_address().0, ECOSYSTEM_ADDRESS_BYTES);
    assert_eq!(reserve_address().0, RESERVE_ADDRESS_BYTES);
}
