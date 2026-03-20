// SPDX-License-Identifier: BUSL-1.1
pragma solidity ^0.8.24;

import "forge-std/Test.sol";
import "../src/UDAGToken.sol";
import "../src/UDAGBridge.sol";

contract UDAGBridgeTest is Test {
    UDAGToken public token;
    UDAGBridge public bridge;

    address public governor = address(0x600);
    address public user = address(0xBEEF);

    uint256 public relayerKey1 = 0x1;
    uint256 public relayerKey2 = 0x2;
    uint256 public relayerKey3 = 0x3;
    uint256 public relayerKey4 = 0x4;
    uint256 public relayerKey5 = 0x5;

    address public relayer1;
    address public relayer2;
    address public relayer3;
    address public relayer4;
    address public relayer5;

    bytes20 public nativeAddr = bytes20(hex"aabbccddee00112233445566778899aabbccddee");

    function setUp() public {
        relayer1 = vm.addr(relayerKey1);
        relayer2 = vm.addr(relayerKey2);
        relayer3 = vm.addr(relayerKey3);
        relayer4 = vm.addr(relayerKey4);
        relayer5 = vm.addr(relayerKey5);

        // Deploy token
        vm.prank(governor);
        token = new UDAGToken(governor);

        // Deploy bridge with 3-of-5 multi-sig
        address[] memory relayerList = new address[](5);
        relayerList[0] = relayer1;
        relayerList[1] = relayer2;
        relayerList[2] = relayer3;
        relayerList[3] = relayer4;
        relayerList[4] = relayer5;

        vm.prank(governor);
        bridge = new UDAGBridge(address(token), governor, relayerList, 3);

        // Grant bridge MINTER and BURNER roles
        vm.startPrank(governor);
        token.grantRole(token.MINTER_ROLE(), address(bridge));
        token.grantRole(token.BURNER_ROLE(), address(bridge));
        vm.stopPrank();

        // Mint some tokens to user for testing
        vm.prank(governor);
        token.mint(user, 10_000 * 10 ** 8);
    }

    // ─── Phase 1: Bridge inactive ───

    function test_bridgeStartsInactive() public view {
        assertFalse(bridge.bridgeActive());
    }

    function test_cannotBridgeWhenInactive() public {
        vm.prank(user);
        token.approve(address(bridge), 100 * 10 ** 8);

        vm.prank(user);
        vm.expectRevert(UDAGBridge.BridgeNotActive.selector);
        bridge.bridgeToNative(nativeAddr, 100 * 10 ** 8);
    }

    // ─── Activation ───

    function test_governorCanActivate() public {
        vm.prank(governor);
        bridge.activateBridge();
        assertTrue(bridge.bridgeActive());
    }

    function test_nonGovernorCannotActivate() public {
        vm.prank(user);
        vm.expectRevert(UDAGBridge.NotGovernor.selector);
        bridge.activateBridge();
    }

    function test_cannotActivateTwice() public {
        vm.prank(governor);
        bridge.activateBridge();

        vm.prank(governor);
        vm.expectRevert("already active");
        bridge.activateBridge();
    }

    // ─── Bridge: Arbitrum → Native (escrow model) ───

    function test_bridgeToNative() public {
        vm.prank(governor);
        bridge.activateBridge();

        uint256 amount = 100 * 10 ** 8;
        uint256 userBalBefore = token.balanceOf(user);
        uint256 bridgeBalBefore = token.balanceOf(address(bridge));

        vm.startPrank(user);
        token.approve(address(bridge), amount);
        bridge.bridgeToNative(nativeAddr, amount);
        vm.stopPrank();

        // Tokens moved from user to bridge (escrowed)
        assertEq(token.balanceOf(user), userBalBefore - amount);
        assertEq(token.balanceOf(address(bridge)), bridgeBalBefore + amount);
        assertEq(bridge.nonce(), 1);
    }

    function test_bridgeToNativeEmitsEvent() public {
        vm.prank(governor);
        bridge.activateBridge();

        uint256 amount = 50 * 10 ** 8;
        vm.startPrank(user);
        token.approve(address(bridge), amount);

        vm.expectEmit(true, true, true, true);
        emit UDAGBridge.BridgeToNative(user, nativeAddr, amount, 0);
        bridge.bridgeToNative(nativeAddr, amount);
        vm.stopPrank();
    }

    function test_bridgeToNativeRejectsZeroAmount() public {
        vm.prank(governor);
        bridge.activateBridge();

        vm.prank(user);
        vm.expectRevert(UDAGBridge.InvalidAmount.selector);
        bridge.bridgeToNative(nativeAddr, 0);
    }

    function test_bridgeToNativeRejectsZeroRecipient() public {
        vm.prank(governor);
        bridge.activateBridge();

        vm.prank(user);
        vm.expectRevert(UDAGBridge.InvalidRecipient.selector);
        bridge.bridgeToNative(bytes20(0), 100);
    }

    function test_bridgeToNativeRejectsTooLarge() public {
        uint256 tooMuch = bridge.MAX_BRIDGE_PER_TX() + 1 * 10 ** 8; // 100,001 UDAG
        vm.startPrank(governor);
        bridge.activateBridge();
        token.mint(user, tooMuch);
        vm.stopPrank();

        vm.startPrank(user);
        token.approve(address(bridge), tooMuch);
        vm.expectRevert(UDAGBridge.AmountTooLarge.selector);
        bridge.bridgeToNative(nativeAddr, tooMuch);
        vm.stopPrank();
    }

    // ─── Complete bridge-to-native (relayer confirms delivery) ───

    function test_completeBridgeToNative() public {
        vm.prank(governor);
        bridge.activateBridge();

        uint256 amount = 100 * 10 ** 8;

        // Escrow tokens
        vm.startPrank(user);
        token.approve(address(bridge), amount);
        bridge.bridgeToNative(nativeAddr, amount);
        vm.stopPrank();

        uint256 bridgeNonce = 0;
        uint256 bridgeBal = token.balanceOf(address(bridge));
        assertEq(bridgeBal, amount);

        // Relayers sign completion
        bytes memory sigs = _createCompletionSignatures(bridgeNonce, user, nativeAddr, amount);
        bridge.completeBridgeToNative(bridgeNonce, sigs);

        // Escrowed tokens burned
        assertEq(token.balanceOf(address(bridge)), 0);

        // Request marked completed
        (,,,, bool completed, bool refunded) = bridge.bridgeRequests(bridgeNonce);
        assertTrue(completed);
        assertFalse(refunded);
    }

    function test_completeBridgeToNativeCannotDouble() public {
        vm.prank(governor);
        bridge.activateBridge();

        uint256 amount = 100 * 10 ** 8;
        vm.startPrank(user);
        token.approve(address(bridge), amount);
        bridge.bridgeToNative(nativeAddr, amount);
        vm.stopPrank();

        bytes memory sigs = _createCompletionSignatures(0, user, nativeAddr, amount);
        bridge.completeBridgeToNative(0, sigs);

        vm.expectRevert("already completed");
        bridge.completeBridgeToNative(0, sigs);
    }

    // ─── Refund after timeout ───

    function test_refundBridgeAfterTimeout() public {
        vm.prank(governor);
        bridge.activateBridge();

        uint256 amount = 100 * 10 ** 8;
        vm.startPrank(user);
        token.approve(address(bridge), amount);
        bridge.bridgeToNative(nativeAddr, amount);
        vm.stopPrank();

        uint256 userBalAfterEscrow = token.balanceOf(user);

        // Cannot refund before timeout
        vm.prank(user);
        vm.expectRevert("too early");
        bridge.refundBridge(0);

        // Fast forward past timeout
        vm.warp(block.timestamp + 7 days + 1);

        vm.prank(user);
        bridge.refundBridge(0);

        // Tokens returned to user
        assertEq(token.balanceOf(user), userBalAfterEscrow + amount);
        assertEq(token.balanceOf(address(bridge)), 0);

        // Request marked refunded
        (,,,, bool completed, bool refunded) = bridge.bridgeRequests(0);
        assertFalse(completed);
        assertTrue(refunded);
    }

    function test_refundBridgeOnlySender() public {
        vm.prank(governor);
        bridge.activateBridge();

        uint256 amount = 100 * 10 ** 8;
        vm.startPrank(user);
        token.approve(address(bridge), amount);
        bridge.bridgeToNative(nativeAddr, amount);
        vm.stopPrank();

        vm.warp(block.timestamp + 7 days + 1);

        vm.prank(address(0xDEAD));
        vm.expectRevert("not sender");
        bridge.refundBridge(0);
    }

    function test_cannotRefundAfterCompletion() public {
        vm.prank(governor);
        bridge.activateBridge();

        uint256 amount = 100 * 10 ** 8;
        vm.startPrank(user);
        token.approve(address(bridge), amount);
        bridge.bridgeToNative(nativeAddr, amount);
        vm.stopPrank();

        bytes memory sigs = _createCompletionSignatures(0, user, nativeAddr, amount);
        bridge.completeBridgeToNative(0, sigs);

        vm.warp(block.timestamp + 7 days + 1);

        vm.prank(user);
        vm.expectRevert("already completed");
        bridge.refundBridge(0);
    }

    // ─── Bridge: Native → Arbitrum ───

    function test_bridgeFromNative() public {
        vm.prank(governor);
        bridge.activateBridge();

        uint256 amount = 200 * 10 ** 8;
        bytes32 nativeTxHash = keccak256("native_tx_1");
        uint256 bridgeNonce = 0;

        bytes memory sigs = _createRelayerSignatures(
            nativeAddr, user, amount, nativeTxHash, bridgeNonce
        );

        uint256 balBefore = token.balanceOf(user);
        bridge.bridgeFromNative(nativeAddr, user, amount, nativeTxHash, bridgeNonce, sigs);
        assertEq(token.balanceOf(user), balBefore + amount);
    }

    function test_bridgeFromNativeRejectsZeroRecipient() public {
        vm.prank(governor);
        bridge.activateBridge();

        uint256 amount = 100 * 10 ** 8;
        bytes32 nativeTxHash = keccak256("native_tx_zero");

        bytes memory sigs = _createRelayerSignatures(
            nativeAddr, address(0), amount, nativeTxHash, 0
        );

        vm.expectRevert("zero recipient");
        bridge.bridgeFromNative(nativeAddr, address(0), amount, nativeTxHash, 0, sigs);
    }

    function test_bridgeFromNativeRejectsReplay() public {
        vm.prank(governor);
        bridge.activateBridge();

        uint256 amount = 100 * 10 ** 8;
        bytes32 nativeTxHash = keccak256("native_tx_2");
        uint256 bridgeNonce = 1;

        bytes memory sigs = _createRelayerSignatures(
            nativeAddr, user, amount, nativeTxHash, bridgeNonce
        );

        bridge.bridgeFromNative(nativeAddr, user, amount, nativeTxHash, bridgeNonce, sigs);

        vm.expectRevert(UDAGBridge.NonceAlreadyProcessed.selector);
        bridge.bridgeFromNative(nativeAddr, user, amount, nativeTxHash, bridgeNonce, sigs);
    }

    function test_bridgeFromNativeRejectsInsufficientSigs() public {
        vm.prank(governor);
        bridge.activateBridge();

        // Only 2 signatures, need 3
        bytes memory sigs = _createPartialSignatures(
            nativeAddr, user, 100, keccak256("tx"), 0, 2
        );

        vm.expectRevert(UDAGBridge.InsufficientSignatures.selector);
        bridge.bridgeFromNative(nativeAddr, user, 100, keccak256("tx"), 0, sigs);
    }

    // ─── Pause/Unpause ───

    function test_relayerCanPause() public {
        vm.prank(governor);
        bridge.activateBridge();

        vm.prank(relayer1);
        bridge.pause();
        assertTrue(bridge.paused());
    }

    function test_nonRelayerCannotPause() public {
        vm.prank(user);
        vm.expectRevert(UDAGBridge.NotRelayer.selector);
        bridge.pause();
    }

    function test_governorCanUnpause() public {
        vm.prank(governor);
        bridge.activateBridge();

        vm.prank(relayer1);
        bridge.pause();

        vm.prank(governor);
        bridge.unpause();
        assertFalse(bridge.paused());
    }

    function test_cannotBridgeWhenPaused() public {
        vm.prank(governor);
        bridge.activateBridge();

        vm.prank(relayer1);
        bridge.pause();

        vm.startPrank(user);
        token.approve(address(bridge), 100);
        vm.expectRevert(UDAGBridge.BridgePausedError.selector);
        bridge.bridgeToNative(nativeAddr, 100);
        vm.stopPrank();
    }

    // ─── Daily Volume Cap ───

    function test_dailyVolumeCap() public {
        vm.startPrank(governor);
        bridge.activateBridge();
        token.mint(user, bridge.DAILY_VOLUME_CAP());
        vm.stopPrank();

        // Bridge up to the cap
        uint256 perTx = bridge.MAX_BRIDGE_PER_TX();
        uint256 rounds = bridge.DAILY_VOLUME_CAP() / perTx;

        vm.startPrank(user);
        token.approve(address(bridge), bridge.DAILY_VOLUME_CAP() + perTx);

        for (uint256 i = 0; i < rounds; i++) {
            bridge.bridgeToNative(nativeAddr, perTx);
        }

        // Next should fail
        vm.expectRevert(UDAGBridge.DailyCapExceeded.selector);
        bridge.bridgeToNative(nativeAddr, perTx);
        vm.stopPrank();
    }

    function test_dailyVolumeResetsAfterDay() public {
        vm.startPrank(governor);
        bridge.activateBridge();
        token.mint(user, bridge.MAX_BRIDGE_PER_TX() * 2);
        vm.stopPrank();

        vm.startPrank(user);
        token.approve(address(bridge), bridge.MAX_BRIDGE_PER_TX() * 2);
        bridge.bridgeToNative(nativeAddr, bridge.MAX_BRIDGE_PER_TX());
        vm.stopPrank();

        // Fast forward 1 day
        vm.warp(block.timestamp + 1 days + 1);

        vm.startPrank(user);
        bridge.bridgeToNative(nativeAddr, bridge.MAX_BRIDGE_PER_TX());
        vm.stopPrank();
    }

    // ─── Relayer Management ───

    function test_relayerCount() public view {
        assertEq(bridge.relayerCount(), 5);
        assertEq(bridge.requiredSignatures(), 3);
    }

    function test_addRelayer() public {
        address newRelayer = address(0xABC);
        vm.prank(governor);
        bridge.addRelayer(newRelayer);
        assertEq(bridge.relayerCount(), 6);
        assertTrue(bridge.isRelayer(newRelayer));
    }

    function test_addRelayerRejectsZeroAddress() public {
        vm.prank(governor);
        vm.expectRevert("zero relayer");
        bridge.addRelayer(address(0));
    }

    function test_removeRelayer() public {
        vm.prank(governor);
        bridge.removeRelayer(relayer5);
        assertEq(bridge.relayerCount(), 4);
        assertFalse(bridge.isRelayer(relayer5));
    }

    // ─── Two-Step Governor Transfer ───

    function test_twoStepGovernorTransfer() public {
        address newGov = address(0x999);

        vm.prank(governor);
        bridge.proposeGovernor(newGov);
        assertEq(bridge.pendingGovernor(), newGov);

        // Old governor still in control
        assertEq(bridge.governor(), governor);

        // New governor accepts
        vm.prank(newGov);
        bridge.acceptGovernance();
        assertEq(bridge.governor(), newGov);
        assertEq(bridge.pendingGovernor(), address(0));
    }

    function test_nonPendingCannotAcceptGovernance() public {
        address newGov = address(0x999);

        vm.prank(governor);
        bridge.proposeGovernor(newGov);

        vm.prank(user);
        vm.expectRevert("not pending governor");
        bridge.acceptGovernance();
    }

    function test_proposeGovernorRejectsZero() public {
        vm.prank(governor);
        vm.expectRevert("zero address");
        bridge.proposeGovernor(address(0));
    }

    // ─── Constructor Validation ───

    function test_constructorRejectsDuplicateRelayer() public {
        address[] memory dupes = new address[](3);
        dupes[0] = relayer1;
        dupes[1] = relayer2;
        dupes[2] = relayer1; // duplicate

        vm.expectRevert("duplicate relayer");
        new UDAGBridge(address(token), governor, dupes, 2);
    }

    function test_constructorRejectsZeroRelayer() public {
        address[] memory bad = new address[](3);
        bad[0] = relayer1;
        bad[1] = address(0); // zero address
        bad[2] = relayer2;

        vm.expectRevert("zero relayer");
        new UDAGBridge(address(token), governor, bad, 2);
    }

    // ─── Genesis Finalization (UDAGToken) ───

    function test_finalizeGenesis() public {
        vm.startPrank(governor);
        assertFalse(token.genesisFinalized());
        token.finalizeGenesis();
        assertTrue(token.genesisFinalized());

        // Admin no longer has MINTER_ROLE
        assertFalse(token.hasRole(token.MINTER_ROLE(), governor));

        // Cannot mint anymore from governor
        vm.expectRevert();
        token.mint(user, 1);
        vm.stopPrank();
    }

    function test_cannotFinalizeGenesisTwice() public {
        vm.startPrank(governor);
        token.finalizeGenesis();

        vm.expectRevert("already finalized");
        token.finalizeGenesis();
        vm.stopPrank();
    }

    // ─── Burn Allowance Check (C1) ───

    function test_burnRequiresAllowance() public {
        // Test the C1 fix: BURNER_ROLE holder must have allowance to burn other's tokens
        uint256 amount = 100 * 10 ** 8;
        address burner = address(0xBBB);

        // Governor is the admin who can grant roles
        vm.startPrank(governor);
        token.grantRole(token.BURNER_ROLE(), burner);
        vm.stopPrank();

        // No allowance from user to burner — should revert
        vm.prank(burner);
        vm.expectRevert();
        token.burn(user, amount);

        // Now approve and it should work
        vm.prank(user);
        token.approve(burner, amount);

        vm.prank(burner);
        token.burn(user, amount);
        assertEq(token.balanceOf(user), 10_000 * 10 ** 8 - amount);
    }

    // ─── Helpers ───

    function _createRelayerSignatures(
        bytes20 nativeSender,
        address recipient,
        uint256 amount,
        bytes32 nativeTxHash,
        uint256 bridgeNonce
    ) internal view returns (bytes memory) {
        bytes32 innerHash = keccak256(
            abi.encode(
                block.chainid, address(bridge), nativeSender, recipient, amount, nativeTxHash, bridgeNonce
            )
        );
        bytes32 messageHash = keccak256(abi.encodePacked("\x19Ethereum Signed Message:\n32", innerHash));

        // Sort signers by address (required by contract)
        uint256[] memory keys = new uint256[](3);
        keys[0] = relayerKey1;
        keys[1] = relayerKey2;
        keys[2] = relayerKey3;
        _sortKeysByAddress(keys);

        bytes memory sigs;
        for (uint256 i = 0; i < 3; i++) {
            (uint8 v, bytes32 r, bytes32 s) = vm.sign(keys[i], messageHash);
            sigs = abi.encodePacked(sigs, r, s, v);
        }
        return sigs;
    }

    function _createPartialSignatures(
        bytes20 nativeSender,
        address recipient,
        uint256 amount,
        bytes32 nativeTxHash,
        uint256 bridgeNonce,
        uint256 count
    ) internal view returns (bytes memory) {
        bytes32 innerHash = keccak256(
            abi.encode(
                block.chainid, address(bridge), nativeSender, recipient, amount, nativeTxHash, bridgeNonce
            )
        );
        bytes32 messageHash = keccak256(abi.encodePacked("\x19Ethereum Signed Message:\n32", innerHash));

        uint256[] memory keys = new uint256[](count);
        for (uint256 i = 0; i < count; i++) {
            keys[i] = i + 1;
        }
        _sortKeysByAddress(keys);

        bytes memory sigs;
        for (uint256 i = 0; i < count; i++) {
            (uint8 v, bytes32 r, bytes32 s) = vm.sign(keys[i], messageHash);
            sigs = abi.encodePacked(sigs, r, s, v);
        }
        return sigs;
    }

    function _createCompletionSignatures(
        uint256 bridgeNonce,
        address sender,
        bytes20 nativeRecipient,
        uint256 amount
    ) internal view returns (bytes memory) {
        bytes32 innerHash = keccak256(
            abi.encode(
                "completeBridgeToNative",
                block.chainid,
                address(bridge),
                bridgeNonce,
                sender,
                nativeRecipient,
                amount
            )
        );
        bytes32 messageHash = keccak256(abi.encodePacked("\x19Ethereum Signed Message:\n32", innerHash));

        uint256[] memory keys = new uint256[](3);
        keys[0] = relayerKey1;
        keys[1] = relayerKey2;
        keys[2] = relayerKey3;
        _sortKeysByAddress(keys);

        bytes memory sigs;
        for (uint256 i = 0; i < 3; i++) {
            (uint8 v, bytes32 r, bytes32 s) = vm.sign(keys[i], messageHash);
            sigs = abi.encodePacked(sigs, r, s, v);
        }
        return sigs;
    }

    function _sortKeysByAddress(uint256[] memory keys) internal pure {
        for (uint256 i = 0; i < keys.length; i++) {
            for (uint256 j = i + 1; j < keys.length; j++) {
                if (vm.addr(keys[i]) > vm.addr(keys[j])) {
                    (keys[i], keys[j]) = (keys[j], keys[i]);
                }
            }
        }
    }
}
