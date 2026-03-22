// SPDX-License-Identifier: BUSL-1.1
pragma solidity ^0.8.24;

import "forge-std/Test.sol";
import "../src/UDAGToken.sol";

contract UDAGTokenTest is Test {
    UDAGToken public token;
    address public admin = address(0xA);
    address public minter = address(0xB);
    address public user = address(0xC);
    address public bridge = address(0xD);
    address public genesisMinter = address(0xE);

    function setUp() public {
        vm.prank(admin);
        token = new UDAGToken(admin, bridge, genesisMinter);
    }

    function test_name() public view {
        assertEq(token.name(), "UltraDAG");
    }

    function test_symbol() public view {
        assertEq(token.symbol(), "UDAG");
    }

    function test_decimals() public view {
        assertEq(token.decimals(), 8);
    }

    function test_maxSupply() public view {
        assertEq(token.MAX_SUPPLY(), 21_000_000 * 10 ** 8);
    }

    function test_adminCanMint() public {
        vm.prank(admin);
        token.mint(user, 1000 * 10 ** 8);
        assertEq(token.balanceOf(user), 1000 * 10 ** 8);
    }

    function test_mintRespectsMaxSupply() public {
        vm.startPrank(admin);
        token.mint(user, token.MAX_SUPPLY());
        vm.expectRevert("UDAG: would exceed max supply");
        token.mint(user, 1);
        vm.stopPrank();
    }

    function test_nonMinterCannotMint() public {
        vm.prank(user);
        vm.expectRevert();
        token.mint(user, 100);
    }

    function test_burnRequiresRole() public {
        vm.prank(admin);
        token.mint(user, 1000);

        vm.prank(user);
        vm.expectRevert();
        token.burn(user, 500);
    }

    function test_burnSelfWorks() public {
        vm.prank(admin);
        token.mint(user, 1000);

        vm.prank(user);
        token.burnSelf(500);
        assertEq(token.balanceOf(user), 500);
    }

    function test_grantMinterRole() public {
        vm.startPrank(admin);
        token.grantRole(token.MINTER_ROLE(), minter);
        vm.stopPrank();

        vm.prank(minter);
        token.mint(user, 100);
        assertEq(token.balanceOf(user), 100);
    }

    function test_grantBurnerRole() public {
        vm.startPrank(admin);
        token.mint(user, 1000);
        token.grantRole(token.BURNER_ROLE(), minter);
        vm.stopPrank();

        // Burner must have allowance to burn from another address (C1 fix)
        vm.prank(user);
        token.approve(minter, 500);

        vm.prank(minter);
        token.burn(user, 500);
        assertEq(token.balanceOf(user), 500);
    }

    function test_totalSupplyTracking() public {
        vm.prank(admin);
        token.mint(user, 1000);
        assertEq(token.totalSupply(), 1000);

        vm.prank(user);
        token.burnSelf(300);
        assertEq(token.totalSupply(), 700);
    }

    function test_genesisAllocation() public {
        // Dev allocation: 1,050,000 UDAG
        uint256 devAlloc = 1_050_000 * 10 ** 8;
        // Treasury: 2,100,000 UDAG
        uint256 treasury = 2_100_000 * 10 ** 8;

        vm.startPrank(admin);
        token.mint(address(0xDE), devAlloc);
        token.mint(address(0xDA0), treasury);
        vm.stopPrank();

        assertEq(token.totalSupply(), devAlloc + treasury);
        assertEq(token.totalSupply(), 3_150_000 * 10 ** 8);
    }

    // ─── M7: New comprehensive tests ───

    /// @notice After finalizeGenesis, MINTER_ROLE is revoked from admin and genesisMinter
    function test_finalizeGenesis() public {
        vm.prank(admin);
        token.finalizeGenesis();

        assertTrue(token.genesisFinalized());
        // Both admin and genesisMinter should lose MINTER_ROLE
        assertFalse(token.hasRole(token.MINTER_ROLE(), admin));
        assertFalse(token.hasRole(token.MINTER_ROLE(), genesisMinter));
        // Bridge should still have MINTER_ROLE
        assertTrue(token.hasRole(token.MINTER_ROLE(), bridge));
    }

    /// @notice After finalizeGenesis, nobody can grant MINTER_ROLE (role admin is dead)
    function test_cannotGrantMinterAfterGenesis() public {
        bytes32 minterRole = token.MINTER_ROLE();

        vm.prank(admin);
        token.finalizeGenesis();

        // Admin tries to grant MINTER_ROLE to an arbitrary address -- should revert
        // because the role admin for MINTER_ROLE is now bytes32(type(uint256).max)
        vm.prank(admin);
        vm.expectRevert();
        token.grantRole(minterRole, address(0xF00));
    }

    /// @notice updateBridge transfers MINTER_ROLE from old to new bridge
    function test_updateBridge() public {
        address newBridge = address(0xF1);

        // Bridge should have MINTER_ROLE initially
        assertTrue(token.hasRole(token.MINTER_ROLE(), bridge));

        vm.prank(admin);
        token.updateBridge(newBridge);

        // Old bridge loses MINTER_ROLE, new bridge gets it
        assertFalse(token.hasRole(token.MINTER_ROLE(), bridge));
        assertTrue(token.hasRole(token.MINTER_ROLE(), newBridge));
        assertEq(token.bridgeAddress(), newBridge);
    }

    /// @notice updateBridge reverts after genesis finalization (H5 fix)
    function test_updateBridgeLockedAfterGenesis() public {
        // finalize genesis
        vm.prank(admin);
        token.finalizeGenesis();

        // try to update bridge - should revert
        vm.prank(admin);
        vm.expectRevert("UDAG: bridge updates locked after genesis");
        token.updateBridge(address(0x999));
    }

    /// @notice Pause blocks all token transfers
    function test_pauseBlocksTransfers() public {
        // Mint some tokens
        vm.prank(admin);
        token.mint(user, 1000);

        // Grant PAUSER_ROLE to admin (already granted in constructor)
        vm.prank(admin);
        token.pause();

        // Transfer should revert when paused
        vm.prank(user);
        vm.expectRevert();
        token.transfer(address(0xF2), 500);

        // Mint should also revert when paused
        vm.prank(admin);
        vm.expectRevert();
        token.mint(user, 100);
    }
}
