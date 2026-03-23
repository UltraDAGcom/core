// SPDX-License-Identifier: BUSL-1.1
pragma solidity ^0.8.24;

import "forge-std/Test.sol";
import "../src/UDAGToken.sol";

contract UDAGTokenTest is Test {
    UDAGToken public token;
    address public admin = address(0xA);
    address public user = address(0xC);
    address public bridge = address(0xD);

    function setUp() public {
        vm.prank(admin);
        token = new UDAGToken(admin, bridge);
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

    function test_bridgeCanMint() public {
        vm.prank(bridge);
        token.mint(user, 1000 * 10 ** 8);
        assertEq(token.balanceOf(user), 1000 * 10 ** 8);
    }

    function test_adminCannotMint() public {
        vm.prank(admin);
        vm.expectRevert();
        token.mint(user, 1000 * 10 ** 8);
    }

    function test_mintRespectsMaxSupply() public {
        vm.startPrank(bridge);
        token.mint(user, token.MAX_SUPPLY());
        vm.expectRevert(
            abi.encodeWithSelector(UDAGToken.ExceedsMaxSupply.selector, 1, 0)
        );
        token.mint(user, 1);
        vm.stopPrank();
    }

    function test_nonMinterCannotMint() public {
        vm.prank(user);
        vm.expectRevert();
        token.mint(user, 100);
    }

    function test_burnSelfWorks() public {
        vm.prank(bridge);
        token.mint(user, 1000);

        vm.prank(user);
        token.burnSelf(500);
        assertEq(token.balanceOf(user), 500);
    }

    function test_totalSupplyTracking() public {
        vm.prank(bridge);
        token.mint(user, 1000);
        assertEq(token.totalSupply(), 1000);

        vm.prank(user);
        token.burnSelf(300);
        assertEq(token.totalSupply(), 700);
    }

    function test_zeroSupplyAtDeploy() public view {
        // No genesis minting -- supply starts at zero
        assertEq(token.totalSupply(), 0);
    }

    // ─── MINTER_ROLE is locked from the start ───

    /// @notice MINTER_ROLE admin is a dead role -- nobody can grant it
    function test_cannotGrantMinterRole() public {
        bytes32 minterRole = token.MINTER_ROLE();

        // Admin tries to grant MINTER_ROLE to an arbitrary address -- should revert
        // because the role admin for MINTER_ROLE is a dead role
        vm.prank(admin);
        vm.expectRevert();
        token.grantRole(minterRole, address(0xF00));
    }

    /// @notice Only bridge has MINTER_ROLE, admin does not
    function test_onlyBridgeHasMinterRole() public view {
        assertTrue(token.hasRole(token.MINTER_ROLE(), bridge));
        assertFalse(token.hasRole(token.MINTER_ROLE(), admin));
    }

    // ─── Bridge Migration ───

    /// @notice Propose + execute bridge migration transfers MINTER_ROLE
    function test_bridgeMigration() public {
        address newBridge = address(0xF1);

        // Bridge should have MINTER_ROLE initially
        assertTrue(token.hasRole(token.MINTER_ROLE(), bridge));

        // Propose migration
        vm.prank(admin);
        token.proposeBridgeMigration(newBridge);

        // Fast-forward past timelock
        vm.warp(block.timestamp + token.BRIDGE_MIGRATION_DELAY() + 1);

        // Execute migration
        vm.prank(admin);
        token.executeBridgeMigration();

        // Old bridge loses MINTER_ROLE, new bridge gets it
        assertFalse(token.hasRole(token.MINTER_ROLE(), bridge));
        assertTrue(token.hasRole(token.MINTER_ROLE(), newBridge));
        assertEq(token.bridgeAddress(), newBridge);
    }

    /// @notice Pause blocks all token transfers
    function test_pauseBlocksTransfers() public {
        // Mint some tokens via bridge
        vm.prank(bridge);
        token.mint(user, 1000);

        // Pause
        vm.prank(admin);
        token.pause("test pause");

        // Transfer should revert when paused
        vm.prank(user);
        vm.expectRevert();
        token.transfer(address(0xF2), 500);

        // Mint should also revert when paused
        vm.prank(bridge);
        vm.expectRevert();
        token.mint(user, 100);
    }

    /// @notice renounceAdminRole is irreversible
    function test_renounceAdminRole() public {
        vm.prank(admin);
        token.renounceAdminRole();

        assertFalse(token.hasRole(token.DEFAULT_ADMIN_ROLE(), admin));
        assertFalse(token.hasRole(token.PAUSER_ROLE(), admin));
    }

    /// @notice Constructor rejects zero addresses
    function test_constructorRejectsZeroAdmin() public {
        vm.expectRevert(
            abi.encodeWithSelector(UDAGToken.ZeroAddress.selector, "admin")
        );
        new UDAGToken(address(0), bridge);
    }

    function test_constructorRejectsZeroBridge() public {
        vm.expectRevert(
            abi.encodeWithSelector(UDAGToken.ZeroAddress.selector, "bridge")
        );
        new UDAGToken(admin, address(0));
    }

    /// @notice isMinter view function works
    function test_isMinter() public view {
        assertTrue(token.isMinter(bridge));
        assertFalse(token.isMinter(admin));
        assertFalse(token.isMinter(user));
    }

    /// @notice remainingSupply tracks correctly
    function test_remainingSupply() public {
        assertEq(token.remainingSupply(), token.MAX_SUPPLY());

        vm.prank(bridge);
        token.mint(user, 1000);

        assertEq(token.remainingSupply(), token.MAX_SUPPLY() - 1000);
    }
}
