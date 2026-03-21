// SPDX-License-Identifier: BUSL-1.1
pragma solidity ^0.8.24;

import "forge-std/Test.sol";
import "../src/UDAGToken.sol";
import "../src/UDAGBridgeOptimistic.sol";

contract UDAGBridgeOptimisticTest is Test {
    UDAGToken public token;
    UDAGBridgeOptimistic public bridge;
    
    address public governor = address(0x600);
    address public user = address(0xBEEF);
    bytes20 public nativeAddr = bytes20(hex"aabbccddee00112233445566778899aabbccddee");
    
    function setUp() public {
        // Deploy token with governor as admin (has DEFAULT_ADMIN_ROLE and MINTER_ROLE)
        vm.prank(governor);
        token = new UDAGToken(governor);
        
        // Deploy optimistic bridge (NO RELAYERS!)
        vm.prank(governor);
        bridge = new UDAGBridgeOptimistic(address(token), governor);
        
        // Grant bridge MINTER_ROLE and BURNER_ROLE
        vm.prank(governor);
        token.grantRole(token.MINTER_ROLE(), address(bridge));
        vm.prank(governor);
        token.grantRole(token.BURNER_ROLE(), address(bridge));
        
        // Mint tokens to user for testing
        vm.prank(governor);
        token.mint(user, 10_000 * 10 ** 8);
        
        // User approves bridge
        vm.prank(user);
        token.approve(address(bridge), 10_000 * 10 ** 8);
    }
    
    // ─── Basic Functionality ───
    
    function test_depositWorks() public {
        vm.startPrank(user);
        token.approve(address(bridge), 100 * 10 ** 8);
        bridge.deposit(nativeAddr, 100 * 10 ** 8);
        vm.stopPrank();
        
        // Verify deposit created
        UDAGBridgeOptimistic.Deposit memory deposit = bridge.getDeposit(0);
        assertEq(deposit.sender, user);
        assertEq(deposit.nativeRecipient, nativeAddr);
        assertEq(deposit.amount, 100 * 10 ** 8);
        assertEq(deposit.claimed, false);
        
        // Verify tokens escrowed
        assertEq(token.balanceOf(address(bridge)), 100 * 10 ** 8);
    }
    
    function test_cannotClaimBeforeChallengePeriod() public {
        vm.startPrank(user);
        token.approve(address(bridge), 100 * 10 ** 8);
        bridge.deposit(nativeAddr, 100 * 10 ** 8);
        vm.stopPrank();
        
        // Try to claim immediately (should fail)
        vm.prank(user);
        vm.expectRevert(UDAGBridgeOptimistic.ChallengePeriodNotPassed.selector);
        bridge.claimWithdrawal(0);
    }
    
    function test_canClaimAfterChallengePeriod() public {
        // Make deposit
        vm.startPrank(user);
        token.approve(address(bridge), 100 * 10 ** 8);
        bridge.deposit(nativeAddr, 100 * 10 ** 8);
        vm.stopPrank();
        
        // Fast forward 7 days
        vm.warp(block.timestamp + 7 days + 1);
        
        // Claim should work
        vm.prank(user);
        bridge.claimWithdrawal(0);
        
        // Verify tokens minted
        assertEq(token.balanceOf(user), 10_000 * 10 ** 8); // Original + minted
        
        // Verify deposit marked claimed
        UDAGBridgeOptimistic.Deposit memory deposit = bridge.getDeposit(0);
        assertEq(deposit.claimed, true);
    }
    
    function test_cannotClaimTwice() public {
        // Make deposit
        vm.startPrank(user);
        token.approve(address(bridge), 100 * 10 ** 8);
        bridge.deposit(nativeAddr, 100 * 10 ** 8);
        vm.stopPrank();
        
        // Fast forward 7 days
        vm.warp(block.timestamp + 7 days + 1);
        
        // First claim works
        vm.prank(user);
        bridge.claimWithdrawal(0);
        
        // Second claim fails
        vm.prank(user);
        vm.expectRevert(UDAGBridgeOptimistic.AlreadyClaimed.selector);
        bridge.claimWithdrawal(0);
    }
    
    // ─── Security ───
    
    function test_cannotDepositZero() public {
        vm.startPrank(user);
        token.approve(address(bridge), 1);
        vm.expectRevert(UDAGBridgeOptimistic.AmountTooLarge.selector);
        bridge.deposit(nativeAddr, 0);
        vm.stopPrank();
    }
    
    function test_cannotDepositTooLarge() public {
        vm.startPrank(user);
        token.approve(address(bridge), 200_000 * 10 ** 8);
        vm.expectRevert(UDAGBridgeOptimistic.AmountTooLarge.selector);
        bridge.deposit(nativeAddr, 200_000 * 10 ** 8);
        vm.stopPrank();
    }
    
    function test_cannotDepositInvalidRecipient() public {
        vm.startPrank(user);
        token.approve(address(bridge), 100 * 10 ** 8);
        vm.expectRevert(UDAGBridgeOptimistic.InvalidRecipient.selector);
        bridge.deposit(bytes20(0), 100 * 10 ** 8);
        vm.stopPrank();
    }
    
    function test_onlySenderCanClaim() public {
        // Make deposit
        vm.startPrank(user);
        token.approve(address(bridge), 100 * 10 ** 8);
        bridge.deposit(nativeAddr, 100 * 10 ** 8);
        vm.stopPrank();
        
        // Fast forward 7 days
        vm.warp(block.timestamp + 7 days + 1);
        
        // Someone else tries to claim (should fail)
        vm.prank(address(0x123));
        vm.expectRevert(UDAGBridgeOptimistic.NotGovernor.selector);
        bridge.claimWithdrawal(0);
    }
    
    // ─── Admin Functions ───
    
    function test_governorCanPause() public {
        vm.prank(governor);
        bridge.pause();
        assertTrue(bridge.paused());
    }
    
    function test_cannotDepositWhenPaused() public {
        vm.prank(governor);
        bridge.pause();
        
        vm.startPrank(user);
        token.approve(address(bridge), 100 * 10 ** 8);
        vm.expectRevert(UDAGBridgeOptimistic.BridgePaused.selector);
        bridge.deposit(nativeAddr, 100 * 10 ** 8);
        vm.stopPrank();
    }
    
    function test_nonGovernorCannotPause() public {
        vm.prank(user);
        vm.expectRevert(UDAGBridgeOptimistic.NotGovernor.selector);
        bridge.pause();
    }
    
    // ─── View Functions ───
    
    function test_canClaimView() public {
        // Before deposit
        assertFalse(bridge.canClaim(0));
        
        // Make deposit
        vm.startPrank(user);
        token.approve(address(bridge), 100 * 10 ** 8);
        bridge.deposit(nativeAddr, 100 * 10 ** 8);
        vm.stopPrank();
        
        // During challenge period
        assertFalse(bridge.canClaim(0));
        
        // After challenge period
        vm.warp(block.timestamp + 7 days + 1);
        assertTrue(bridge.canClaim(0));
    }
    
    function test_timeUntilClaimable() public {
        // Make deposit
        vm.startPrank(user);
        token.approve(address(bridge), 100 * 10 ** 8);
        bridge.deposit(nativeAddr, 100 * 10 ** 8);
        vm.stopPrank();
        
        // Check time remaining
        uint256 timeRemaining = bridge.timeUntilClaimable(0);
        assertEq(timeRemaining, 7 days);
        
        // Fast forward 3 days
        vm.warp(block.timestamp + 3 days);
        timeRemaining = bridge.timeUntilClaimable(0);
        assertEq(timeRemaining, 4 days);
        
        // After challenge period
        vm.warp(block.timestamp + 7 days + 1);
        timeRemaining = bridge.timeUntilClaimable(0);
        assertEq(timeRemaining, 0);
    }
}
