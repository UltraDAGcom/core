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
    
    function setUp() public {
        validator1 = vm.addr(validatorKey1);
        validator2 = vm.addr(validatorKey2);
        validator3 = vm.addr(validatorKey3);
        validator4 = vm.addr(validatorKey4);
        
        vm.prank(governor);
        token = new UDAGToken(governor);
        
        vm.prank(governor);
        bridge = new UDAGBridgeValidator(address(token), governor);
        
        vm.startPrank(governor);
        bridge.addValidator(validator1);
        bridge.addValidator(validator2);
        bridge.addValidator(validator3);
        bridge.addValidator(validator4);
        token.grantRole(token.MINTER_ROLE(), address(bridge));
        vm.stopPrank();
        
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
        
        bytes32 messageHash = bridge.getMessageHash(nativeAddr, user, amount, depositNonce, block.chainid);
        
        // Sign with only 2 validators (below threshold of 3)
        (uint8 v1, bytes32 r1, bytes32 s1) = vm.sign(validatorKey1, messageHash);
        (uint8 v2, bytes32 r2, bytes32 s2) = vm.sign(validatorKey2, messageHash);
        
        bytes memory signatures = abi.encodePacked(r1, s1, v1, r2, s2, v2);
        
        vm.prank(user);
        vm.expectRevert(UDAGBridgeValidator.InsufficientSignatures.selector);
        bridge.claimWithdrawal(nativeAddr, user, amount, depositNonce, signatures, messageHash);
    }
    
    function _testClaim(uint256 numSigners) internal {
        uint256 amount = 100 * 10 ** 8;
        uint256 depositNonce = 0;
        
        bytes32 messageHash = bridge.getMessageHash(nativeAddr, user, amount, depositNonce, block.chainid);
        
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
        
        // Sign in sorted order
        for (uint256 i = 0; i < numSigners; i++) {
            (uint8 v, bytes32 r, bytes32 s) = vm.sign(keys[i], messageHash);
            signatures = abi.encodePacked(signatures, r, s, v);
        }
        
        vm.prank(user);
        bridge.claimWithdrawal(nativeAddr, user, amount, depositNonce, signatures, messageHash);
        
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
        vm.expectRevert(UDAGBridgeValidator.BridgePaused.selector);
        bridge.deposit(nativeAddr, 100 * 10 ** 8);
    }
}
