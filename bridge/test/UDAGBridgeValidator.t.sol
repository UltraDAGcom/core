// SPDX-License-Identifier: BUSL-1.1
pragma solidity ^0.8.24;

import "forge-std/Test.sol";
import "../src/UDAGToken.sol";
import "../src/UDAGBridgeValidator.sol";

contract UDAGBridgeValidatorTest is Test {
    UDAGToken public token;
    UDAGBridgeValidator public bridge;
    
    address public governor = address(0x600);
    address public user = address(0xBEEF);
    bytes20 public nativeAddr = bytes20(hex"aabbccddee00112233445566778899aabbccddee");
    
    uint256 public validatorKey1 = 0x1;
    uint256 public validatorKey2 = 0x2;
    uint256 public validatorKey3 = 0x3;
    uint256 public validatorKey4 = 0x4;
    
    address public validator1;
    address public validator2;
    address public validator3;
    address public validator4;
    
    address public tempBridge = address(0x700);

    function setUp() public {
        validator1 = vm.addr(validatorKey1);
        validator2 = vm.addr(validatorKey2);
        validator3 = vm.addr(validatorKey3);
        validator4 = vm.addr(validatorKey4);

        // Deploy token: governor as admin + genesisMinter, tempBridge as initial bridge
        vm.prank(governor);
        token = new UDAGToken(governor, tempBridge, governor);

        // Deploy bridge
        vm.prank(governor);
        bridge = new UDAGBridgeValidator(address(token), governor);

        vm.startPrank(governor);
        // Update the token's bridge address to the real bridge
        token.updateBridge(address(bridge));
        bridge.addValidator(validator1);
        bridge.addValidator(validator2);
        bridge.addValidator(validator3);
        bridge.addValidator(validator4);
        vm.stopPrank();

        // Mint tokens for user (governor still has MINTER_ROLE as genesisMinter)
        vm.prank(governor);
        token.mint(user, 10_000 * 10 ** 8);

        vm.prank(user);
        token.approve(address(bridge), 10_000 * 10 ** 8);
    }
    
    function test_depositWorks() public {
        vm.prank(user);
        bridge.deposit(nativeAddr, 100 * 10 ** 8);
        assertEq(token.balanceOf(address(bridge)), 100 * 10 ** 8);
    }
    
    function test_claimWithdrawalWithThresholdSignatures() public {
        _testClaim(3); // 3 signatures (threshold)
    }
    
    function test_cannotClaimWithInsufficientSignatures() public {
        uint256 amount = 100 * 10 ** 8;
        uint256 depositNonce = 0;

        bytes32 messageHash = bridge.getMessageHash(nativeAddr, user, amount, depositNonce);
        // EIP-191 prefix for signing (eth_sign adds this automatically)
        bytes32 ethSignedHash = keccak256(abi.encodePacked("\x19Ethereum Signed Message:\n32", messageHash));

        // Sign with only 2 validators (below threshold of 3)
        (uint8 v1, bytes32 r1, bytes32 s1) = vm.sign(validatorKey1, ethSignedHash);
        (uint8 v2, bytes32 r2, bytes32 s2) = vm.sign(validatorKey2, ethSignedHash);

        bytes memory signatures = abi.encodePacked(r1, s1, v1, r2, s2, v2);

        vm.prank(user);
        vm.expectRevert(UDAGBridgeValidator.InsufficientSignatures.selector);
        bridge.claimWithdrawal(nativeAddr, user, amount, depositNonce, signatures);
    }
    
    function _testClaim(uint256 numSigners) internal {
        uint256 amount = 100 * 10 ** 8;
        uint256 depositNonce = 0;

        bytes32 messageHash = bridge.getMessageHash(nativeAddr, user, amount, depositNonce);
        // EIP-191 prefix for signing (eth_sign adds this automatically)
        bytes32 ethSignedHash = keccak256(abi.encodePacked("\x19Ethereum Signed Message:\n32", messageHash));

        // Sign in order of validator address (ascending)
        bytes memory signatures;
        uint256[] memory keys = new uint256[](numSigners);
        address[] memory addrs = new address[](numSigners);

        // Store keys and addresses
        for (uint256 i = 0; i < numSigners; i++) {
            keys[i] = i + 1; // validatorKey1, validatorKey2, etc.
            addrs[i] = vm.addr(keys[i]);
        }

        // Sort by address (bubble sort for simplicity)
        for (uint256 i = 0; i < numSigners - 1; i++) {
            for (uint256 j = 0; j < numSigners - i - 1; j++) {
                if (addrs[j] > addrs[j + 1]) {
                    (addrs[j], addrs[j + 1]) = (addrs[j + 1], addrs[j]);
                    (keys[j], keys[j + 1]) = (keys[j + 1], keys[j]);
                }
            }
        }

        // Sign in sorted order using EIP-191 prefixed hash
        for (uint256 i = 0; i < numSigners; i++) {
            (uint8 v, bytes32 r, bytes32 s) = vm.sign(keys[i], ethSignedHash);
            signatures = abi.encodePacked(signatures, r, s, v);
        }

        vm.prank(user);
        bridge.claimWithdrawal(nativeAddr, user, amount, depositNonce, signatures);

        assertEq(token.balanceOf(user), 10_100 * 10 ** 8);
    }
    
    function test_validatorManagement() public {
        assertEq(bridge.getValidatorCount(), 4);
        assertEq(bridge.getThreshold(), 3);
        
        vm.prank(governor);
        bridge.addValidator(address(0x999));
        assertEq(bridge.getValidatorCount(), 5);
        assertEq(bridge.getThreshold(), 4);
        
        vm.prank(governor);
        bridge.removeValidator(validator4);
        assertEq(bridge.getValidatorCount(), 4);
        assertEq(bridge.getThreshold(), 3);
    }
    
    function test_pause() public {
        vm.prank(governor);
        bridge.pause();
        assertTrue(bridge.paused());

        vm.prank(user);
        vm.expectRevert(UDAGBridgeValidator.BridgeIsPaused.selector);
        bridge.deposit(nativeAddr, 100 * 10 ** 8);
    }

    // ─── H5/M7: New comprehensive tests ───

    /// @notice Duplicate signatures from same validator should revert (signer == lastSigner)
    function test_claimWithDuplicateSignatures() public {
        uint256 amount = 100 * 10 ** 8;
        uint256 depositNonce = 0;

        bytes32 messageHash = bridge.getMessageHash(nativeAddr, user, amount, depositNonce);
        bytes32 ethSignedHash = keccak256(abi.encodePacked("\x19Ethereum Signed Message:\n32", messageHash));

        // Get sorted keys for 3 signers
        uint256[] memory keys = new uint256[](3);
        address[] memory addrs = new address[](3);
        keys[0] = validatorKey1; addrs[0] = vm.addr(validatorKey1);
        keys[1] = validatorKey2; addrs[1] = vm.addr(validatorKey2);
        keys[2] = validatorKey3; addrs[2] = vm.addr(validatorKey3);
        _sortKeysByAddr(keys, addrs);

        // Sign first two normally, then duplicate the second signer (same address => signer <= lastSigner)
        bytes memory signatures;
        {
            (uint8 v0, bytes32 r0, bytes32 s0) = vm.sign(keys[0], ethSignedHash);
            signatures = abi.encodePacked(r0, s0, v0);
        }
        {
            (uint8 v1, bytes32 r1, bytes32 s1) = vm.sign(keys[1], ethSignedHash);
            // Append this signer twice: second occurrence triggers signer <= lastSigner
            signatures = abi.encodePacked(signatures, r1, s1, v1, r1, s1, v1);
        }

        vm.prank(user);
        vm.expectRevert(UDAGBridgeValidator.SignatureNotSorted.selector);
        bridge.claimWithdrawal(nativeAddr, user, amount, depositNonce, signatures);
    }

    /// @notice Non-validator signatures should not count toward threshold
    function test_claimWithNonValidatorSignature() public {
        uint256 amount = 100 * 10 ** 8;
        uint256 depositNonce = 0;

        bytes32 messageHash = bridge.getMessageHash(nativeAddr, user, amount, depositNonce);
        bytes32 ethSignedHash = keccak256(abi.encodePacked("\x19Ethereum Signed Message:\n32", messageHash));

        // Use a non-validator key (0xBBBB is not a validator)
        uint256 nonValidatorKey = 0xBBBB;

        // Collect keys + addrs: 2 validators + 1 non-validator = only 2 valid sigs (below threshold 3)
        uint256[] memory keys = new uint256[](3);
        address[] memory addrs = new address[](3);
        keys[0] = validatorKey1;
        keys[1] = validatorKey2;
        keys[2] = nonValidatorKey;
        addrs[0] = vm.addr(validatorKey1);
        addrs[1] = vm.addr(validatorKey2);
        addrs[2] = vm.addr(nonValidatorKey);

        // Sort by address
        for (uint256 i = 0; i < 2; i++) {
            for (uint256 j = 0; j < 2 - i; j++) {
                if (addrs[j] > addrs[j + 1]) {
                    (addrs[j], addrs[j + 1]) = (addrs[j + 1], addrs[j]);
                    (keys[j], keys[j + 1]) = (keys[j + 1], keys[j]);
                }
            }
        }

        bytes memory signatures;
        for (uint256 i = 0; i < 3; i++) {
            (uint8 v, bytes32 r, bytes32 s) = vm.sign(keys[i], ethSignedHash);
            signatures = abi.encodePacked(signatures, r, s, v);
        }

        vm.prank(user);
        vm.expectRevert(UDAGBridgeValidator.InsufficientSignatures.selector);
        bridge.claimWithdrawal(nativeAddr, user, amount, depositNonce, signatures);
    }

    /// @notice Exceeding daily withdrawal limit should revert
    function test_dailyWithdrawalLimit() public {
        // MAX_DEPOSIT is 100K UDAG. Daily limit is 500K UDAG.
        // Claim 5 x 100K = 500K to hit the daily limit, then try 1 more.
        uint256 maxDeposit = 100_000 * 10 ** 8;

        for (uint256 i = 0; i < 5; i++) {
            _claimAmount(maxDeposit, i);
        }

        // 6th claim should exceed daily limit
        uint256 smallAmount = 1 * 10 ** 8;
        bytes memory sigs = _buildSortedSignatures(smallAmount, 5);
        vm.prank(user);
        vm.expectRevert(UDAGBridgeValidator.DailyLimitExceeded.selector);
        bridge.claimWithdrawal(nativeAddr, user, smallAmount, 5, sigs);
    }

    /// @notice Replaying same nonce should revert
    function test_nonceReplay() public {
        _claimAmount(100 * 10 ** 8, 0);

        // Replay the same nonce
        bytes memory sigs = _buildSortedSignatures(100 * 10 ** 8, 0);
        vm.prank(user);
        vm.expectRevert(UDAGBridgeValidator.NonceAlreadyUsed.selector);
        bridge.claimWithdrawal(nativeAddr, user, 100 * 10 ** 8, 0, sigs);
    }

    /// @notice Two-step governor transfer works correctly
    function test_twoStepGovernorTransfer() public {
        address newGov = address(0x999);

        vm.prank(governor);
        bridge.setGovernor(newGov);
        assertEq(bridge.pendingGovernor(), newGov);
        // Governor hasn't changed yet
        assertEq(bridge.governor(), governor);

        vm.prank(newGov);
        bridge.acceptGovernor();
        assertEq(bridge.governor(), newGov);
        assertEq(bridge.pendingGovernor(), address(0));
    }

    /// @notice Setting threshold below BFT minimum should revert
    function test_setThresholdBelowBFT() public {
        // With 4 validators, BFT minimum is floor(2*4/3)+1 = 3
        vm.prank(governor);
        vm.expectRevert("Below BFT minimum");
        bridge.setThreshold(2);
    }

    /// @notice Migration moves escrowed tokens to new bridge
    function test_migrateToNewBridge() public {
        // Deposit tokens first
        vm.prank(user);
        bridge.deposit(nativeAddr, 100 * 10 ** 8);
        assertEq(token.balanceOf(address(bridge)), 100 * 10 ** 8);

        address newBridge = address(0xBEEF1);

        vm.startPrank(governor);
        bridge.pause();
        bridge.migrateToNewBridge(newBridge, 100 * 10 ** 8);
        vm.stopPrank();

        assertEq(token.balanceOf(address(bridge)), 0);
        assertEq(token.balanceOf(newBridge), 100 * 10 ** 8);
    }

    /// @notice Malleable signature (high s-value) should be rejected
    function test_malleableSignatureRejected() public {
        uint256 amount = 100 * 10 ** 8;

        bytes32 messageHash = bridge.getMessageHash(nativeAddr, user, amount, 0);
        bytes32 ethSignedHash = keccak256(abi.encodePacked("\x19Ethereum Signed Message:\n32", messageHash));

        // Build malleable sig from first sorted key
        bytes memory malleableSig = _buildMalleableSig(ethSignedHash);

        vm.prank(user);
        vm.expectRevert(UDAGBridgeValidator.MalleableSignature.selector);
        bridge.claimWithdrawal(nativeAddr, user, amount, 0, malleableSig);
    }

    /// @notice Helper: build a signature blob where the first sig has a flipped (high) s-value
    function _buildMalleableSig(bytes32 ethSignedHash) internal view returns (bytes memory) {
        uint256[] memory keys = new uint256[](3);
        address[] memory addrs = new address[](3);
        keys[0] = validatorKey1; addrs[0] = vm.addr(validatorKey1);
        keys[1] = validatorKey2; addrs[1] = vm.addr(validatorKey2);
        keys[2] = validatorKey3; addrs[2] = vm.addr(validatorKey3);
        _sortKeysByAddr(keys, addrs);

        // Sign with first key and flip s to high half (malleable)
        (uint8 v0, bytes32 r0, bytes32 s0) = vm.sign(keys[0], ethSignedHash);
        uint256 secp256k1n = 0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD0364141;
        bytes memory sigs = abi.encodePacked(r0, bytes32(secp256k1n - uint256(s0)), v0 == 27 ? uint8(28) : uint8(27));

        // Append remaining valid sigs
        for (uint256 i = 1; i < 3; i++) {
            (uint8 v, bytes32 r, bytes32 s) = vm.sign(keys[i], ethSignedHash);
            sigs = abi.encodePacked(sigs, r, s, v);
        }
        return sigs;
    }

    /// @notice Bridge should be disabled before minimum validators are added
    function test_bridgeDisabledBeforeMinValidators() public {
        // Deploy a fresh bridge with no validators
        vm.prank(governor);
        UDAGBridgeValidator freshBridge = new UDAGBridgeValidator(address(token), governor);

        assertFalse(freshBridge.bridgeEnabled());

        // Try to deposit — should revert
        vm.prank(user);
        vm.expectRevert(UDAGBridgeValidator.InvalidValidatorSet.selector);
        freshBridge.deposit(nativeAddr, 100 * 10 ** 8);
    }

    /// @notice Deposit above MAX_DEPOSIT should revert
    function test_maxDepositLimit() public {
        uint256 maxDeposit = 100_000 * 10 ** 8;

        vm.prank(user);
        vm.expectRevert(UDAGBridgeValidator.AmountTooLarge.selector);
        bridge.deposit(nativeAddr, maxDeposit + 1);
    }

    // ─── Internal helpers for new tests ───

    /// @notice Sort keys and addrs arrays in-place by address ascending
    function _sortKeysByAddr(uint256[] memory keys, address[] memory addrs) internal pure {
        uint256 n = keys.length;
        for (uint256 i = 0; i < n - 1; i++) {
            for (uint256 j = 0; j < n - i - 1; j++) {
                if (addrs[j] > addrs[j + 1]) {
                    (addrs[j], addrs[j + 1]) = (addrs[j + 1], addrs[j]);
                    (keys[j], keys[j + 1]) = (keys[j + 1], keys[j]);
                }
            }
        }
    }

    /// @notice Sign a hash with sorted keys and return concatenated signatures
    function _signSorted(uint256[] memory sortedKeys, bytes32 ethSignedHash) internal pure returns (bytes memory) {
        bytes memory signatures;
        for (uint256 i = 0; i < sortedKeys.length; i++) {
            (uint8 v, bytes32 r, bytes32 s) = vm.sign(sortedKeys[i], ethSignedHash);
            signatures = abi.encodePacked(signatures, r, s, v);
        }
        return signatures;
    }

    /// @notice Build sorted threshold signatures for a given amount and nonce
    function _buildSortedSignatures(uint256 amount, uint256 depositNonce) internal view returns (bytes memory) {
        bytes32 messageHash = bridge.getMessageHash(nativeAddr, user, amount, depositNonce);
        bytes32 ethSignedHash = keccak256(abi.encodePacked("\x19Ethereum Signed Message:\n32", messageHash));

        uint256[] memory keys = new uint256[](3);
        address[] memory addrs = new address[](3);
        keys[0] = validatorKey1; addrs[0] = vm.addr(validatorKey1);
        keys[1] = validatorKey2; addrs[1] = vm.addr(validatorKey2);
        keys[2] = validatorKey3; addrs[2] = vm.addr(validatorKey3);

        _sortKeysByAddr(keys, addrs);
        return _signSorted(keys, ethSignedHash);
    }

    /// @notice Claim a specific amount with valid sorted signatures
    function _claimAmount(uint256 amount, uint256 depositNonce) internal {
        bytes memory sigs = _buildSortedSignatures(amount, depositNonce);
        vm.prank(user);
        bridge.claimWithdrawal(nativeAddr, user, amount, depositNonce, sigs);
    }
}
