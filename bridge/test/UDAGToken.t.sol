// SPDX-License-Identifier: BUSL-1.1
pragma solidity ^0.8.24;

import "forge-std/Test.sol";
import "../src/UDAGToken.sol";

contract UDAGTokenTest is Test {
    UDAGToken public token;
    address public admin = address(0xA);
    address public minter = address(0xB);
    address public user = address(0xC);

    function setUp() public {
        vm.prank(admin);
        token = new UDAGToken(admin);
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
        vm.expectRevert("UDAG: exceeds max supply");
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
}
