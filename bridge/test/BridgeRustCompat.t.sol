// SPDX-License-Identifier: BUSL-1.1
pragma solidity ^0.8.24;

import "forge-std/Test.sol";
import "../src/UDAGToken.sol";
import "../src/UDAGBridgeValidator.sol";

/// @title Rust ↔ Solidity bridge compatibility test
/// @notice Asserts that the ABI layout of `_computeWithdrawalHash` matches,
///         byte-for-byte, the `BridgeAttestation::solidity_message_hash()`
///         implementation in the Rust crate.
///
///         The actual cross-implementation golden vectors live in
///         `crates/ultradag-coin/tests/bridge_solidity_compat.rs`. That file
///         is the authority: it both self-asserts against a set of captured
///         expected outputs AND captures them on demand. This file mirrors
///         only the inputs + the expected message hash, and drives the
///         deployed Solidity contract against them.
///
///         If this test starts failing, exactly one of the following has
///         drifted:
///           • `BridgeAttestation::solidity_message_hash()` — Rust side
///             (9-slot manual ABI encoding in `bridge/mod.rs`).
///           • `UDAGBridgeValidator::_computeWithdrawalHash()` — Solidity
///             side (`keccak256(abi.encode(...))` in `UDAGBridgeValidator.sol`).
///
///         The Rust-side `golden_vectors_self_consistency` test should fail
///         at the same time, pointing to which side actually changed.
///
/// @dev Why we use `vm.etch` + a fixed bridge address:
///
///      The Solidity `_computeWithdrawalHash` includes `address(this)` and
///      `block.chainid` in the hash. The Rust-captured golden hash was
///      computed against a specific bridge contract address
///      (`BRIDGE_CONTRACT = [0x03; 20]`) and chain ID (`421614` =
///      Arbitrum Sepolia). To reproduce that hash from a live Solidity
///      contract we need `address(this) == [0x03; 20]` and
///      `block.chainid == 421614`. We achieve both via `vm.etch` (deploy
///      the bridge bytecode at the fixed address) and `vm.chainId`.
///      `getWithdrawalHash` is a pure function of `block.chainid`,
///      `address(this)`, and its calldata arguments — it does not touch
///      the bridge's storage — so the etched contract does not need to
///      be initialised.
contract BridgeRustCompatTest is Test {
    // ─── Fixed inputs (must match the Rust test byte-for-byte) ────────
    bytes20 constant SENDER      = bytes20(hex"0101010101010101010101010101010101010101");
    address constant RECIPIENT   = 0x0202020202020202020202020202020202020202;
    uint256 constant AMOUNT      = 100_000_000;  // 1 UDAG in 8-decimal sats
    uint256 constant NONCE       = 42;
    uint256 constant RUST_CHAIN_ID = 421614;     // Arbitrum Sepolia

    // The Rust test captured its golden message hash with this bridge
    // contract address hardcoded into the attestation. We replicate the
    // deployment at exactly this address via `vm.etch`.
    address constant RUST_BRIDGE_CONTRACT = 0x0303030303030303030303030303030303030303;

    // ─── Golden message hash (captured from the Rust side) ────────────
    //
    // Regeneration:
    //   cargo test -p ultradag-coin --test bridge_solidity_compat \
    //     -- --ignored --nocapture capture_golden_vectors
    //
    // Then paste the new value here AND into the Rust test's
    // EXPECTED_MESSAGE_HASH_HEX constant.
    bytes32 constant EXPECTED_MESSAGE_HASH =
        0x1931be052fdf4b7d366afefa26634aeaf9fe45c5640ddfc970115e1664d60734;

    function test_messageHashLayoutMatchesRust() public {
        // Pin the chain ID to the Rust-captured value.
        vm.chainId(RUST_CHAIN_ID);

        // Deploy a fresh bridge at whatever address foundry assigns, then
        // copy its runtime bytecode to the fixed Rust-captured address.
        // The token instance doesn't need anything special — we're only
        // going to call `getWithdrawalHash`, which doesn't touch the
        // token.
        address scratchGovernor = address(0xFEED);
        vm.startPrank(scratchGovernor);
        uint256 scratchNonce = vm.getNonce(scratchGovernor);
        address predictedBridge = vm.computeCreateAddress(scratchGovernor, scratchNonce + 1);
        UDAGToken scratchToken = new UDAGToken(
            scratchGovernor,
            predictedBridge,
            address(0),
            0
        );
        UDAGBridgeValidator scratchBridge = new UDAGBridgeValidator(
            address(scratchToken),
            scratchGovernor
        );
        require(address(scratchBridge) == predictedBridge, "scratch mismatch");
        vm.stopPrank();

        // Etch the scratch bridge's runtime code onto the fixed address.
        // Storage at that address remains empty, but `getWithdrawalHash`
        // reads nothing from storage — only `block.chainid`,
        // `address(this)`, and the calldata arguments — so the etched
        // contract's hash output will match what the Rust side produced.
        vm.etch(RUST_BRIDGE_CONTRACT, address(scratchBridge).code);

        UDAGBridgeValidator fixedBridge = UDAGBridgeValidator(payable(RUST_BRIDGE_CONTRACT));
        bytes32 actual = fixedBridge.getWithdrawalHash(SENDER, RECIPIENT, AMOUNT, NONCE);

        assertEq(
            actual,
            EXPECTED_MESSAGE_HASH,
            "Solidity message hash drifted from Rust-generated golden vector"
        );
    }

    /// @notice Sanity: under a DIFFERENT chain ID, the same inputs must
    ///         produce a DIFFERENT hash. This catches the failure mode
    ///         where chain-ID separation silently drops out of the
    ///         encoding.
    function test_messageHashIncludesChainId() public {
        vm.chainId(RUST_CHAIN_ID);
        address scratchGovernor = address(0xFEED);
        vm.startPrank(scratchGovernor);
        uint256 scratchNonce = vm.getNonce(scratchGovernor);
        address predictedBridge = vm.computeCreateAddress(scratchGovernor, scratchNonce + 1);
        UDAGToken scratchToken = new UDAGToken(scratchGovernor, predictedBridge, address(0), 0);
        UDAGBridgeValidator scratchBridge = new UDAGBridgeValidator(address(scratchToken), scratchGovernor);
        require(address(scratchBridge) == predictedBridge, "scratch mismatch");
        vm.stopPrank();
        vm.etch(RUST_BRIDGE_CONTRACT, address(scratchBridge).code);

        UDAGBridgeValidator fixedBridge = UDAGBridgeValidator(payable(RUST_BRIDGE_CONTRACT));
        bytes32 h1 = fixedBridge.getWithdrawalHash(SENDER, RECIPIENT, AMOUNT, NONCE);

        // Change only the chain ID; the hash must move.
        vm.chainId(RUST_CHAIN_ID + 1);
        bytes32 h2 = fixedBridge.getWithdrawalHash(SENDER, RECIPIENT, AMOUNT, NONCE);

        assertTrue(h1 != h2, "chain-ID separation missing from message hash");
    }

    /// @notice Sanity: the bridge-contract address must be mixed into the
    ///         hash too. An attacker must not be able to replay a
    ///         withdrawal claim against a different deployment of the
    ///         same contract on the same chain.
    function test_messageHashIncludesBridgeAddress() public {
        vm.chainId(RUST_CHAIN_ID);
        address scratchGovernor = address(0xFEED);
        vm.startPrank(scratchGovernor);
        uint256 scratchNonce = vm.getNonce(scratchGovernor);
        address predictedBridge = vm.computeCreateAddress(scratchGovernor, scratchNonce + 1);
        UDAGToken scratchToken = new UDAGToken(scratchGovernor, predictedBridge, address(0), 0);
        UDAGBridgeValidator scratchBridge = new UDAGBridgeValidator(address(scratchToken), scratchGovernor);
        require(address(scratchBridge) == predictedBridge, "scratch mismatch");
        vm.stopPrank();

        // Etch the bridge at two different fixed addresses and compare
        // their hashes for identical inputs.
        address addr_a = 0x0303030303030303030303030303030303030303;
        address addr_b = 0x0404040404040404040404040404040404040404;

        vm.etch(addr_a, address(scratchBridge).code);
        vm.etch(addr_b, address(scratchBridge).code);

        bytes32 hash_a = UDAGBridgeValidator(payable(addr_a))
            .getWithdrawalHash(SENDER, RECIPIENT, AMOUNT, NONCE);
        bytes32 hash_b = UDAGBridgeValidator(payable(addr_b))
            .getWithdrawalHash(SENDER, RECIPIENT, AMOUNT, NONCE);

        assertTrue(hash_a != hash_b, "deployment separation missing from message hash");
    }
}
