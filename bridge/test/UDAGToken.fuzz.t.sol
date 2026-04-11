// SPDX-License-Identifier: BUSL-1.1
pragma solidity ^0.8.24;

import "forge-std/Test.sol";
import "../src/UDAGToken.sol";

/// @notice Property-based fuzz tests for UDAGToken.
///
/// Unit tests in UDAGToken.t.sol cover fixed inputs; these tests cover
/// the full input space for each property-level assertion. Foundry's
/// default fuzzer runs 256 random inputs per test function; increase via
/// `forge test --fuzz-runs N` when validating a behaviour change.
///
/// The fuzzer will shrink any failure to its minimal reproducing input
/// and report both the original and shrunk case.
contract UDAGTokenFuzzTest is Test {
    UDAGToken public token;
    address public admin   = address(0xA11);
    address public bridge  = address(0xB12);

    function setUp() public {
        // Default setup — no genesis allocation, bridge is sole minter.
        vm.prank(admin);
        token = new UDAGToken(admin, bridge, address(0), 0);
    }

    // ─── Constructor / genesis allocation fuzz ─────────────────────────

    /// @notice Any valid genesis allocation (0..MAX_GENESIS_ALLOCATION)
    ///         paired with a non-zero recipient must deploy successfully
    ///         and credit the recipient with exactly that amount. The
    ///         total supply immediately after deployment equals the
    ///         allocation.
    function testFuzz_genesisAllocation_withinCap(uint256 amount, address recipient) public {
        amount = bound(amount, 1, token.MAX_GENESIS_ALLOCATION());
        vm.assume(recipient != address(0));

        UDAGToken t = new UDAGToken(admin, bridge, recipient, amount);

        assertEq(t.genesisAllocation(), amount);
        assertEq(t.genesisRecipient(), recipient);
        assertEq(t.balanceOf(recipient), amount);
        assertEq(t.totalSupply(), amount);
        assertEq(t.remainingSupply(), t.MAX_SUPPLY() - amount);
    }

    /// @notice Any genesis allocation strictly above the 12% cap must
    ///         revert with `GenesisAllocationTooLarge`.
    function testFuzz_genesisAllocation_aboveCap(uint256 over) public {
        over = bound(over, 1, type(uint96).max);
        uint256 amount = token.MAX_GENESIS_ALLOCATION() + over;

        vm.expectRevert(
            abi.encodeWithSelector(
                UDAGToken.GenesisAllocationTooLarge.selector,
                amount,
                token.MAX_GENESIS_ALLOCATION()
            )
        );
        new UDAGToken(admin, bridge, address(0xBEEF), amount);
    }

    /// @notice A zero allocation paired with a zero recipient is the
    ///         no-pre-mine path — it must deploy and leave supply at
    ///         zero, regardless of the recipient argument value.
    function testFuzz_genesisAllocation_zeroIsAlwaysSafe(address recipient) public {
        UDAGToken t = new UDAGToken(admin, bridge, recipient, 0);
        assertEq(t.totalSupply(), 0);
        assertEq(t.genesisAllocation(), 0);
        // When allocation is zero the recipient field is informational
        // only — no tokens were minted anywhere.
        assertEq(t.balanceOf(recipient), 0);
    }

    /// @notice A non-zero allocation with a zero recipient must always
    ///         revert, regardless of the allocation value.
    function testFuzz_genesisAllocation_zeroRecipientReverts(uint256 amount) public {
        amount = bound(amount, 1, token.MAX_GENESIS_ALLOCATION());
        vm.expectRevert(UDAGToken.GenesisRecipientRequired.selector);
        new UDAGToken(admin, bridge, address(0), amount);
    }

    // ─── MAX_SUPPLY invariant under fuzzed bridge mints ────────────────

    /// @notice After genesis, the bridge can mint any amount up to
    ///         (MAX_SUPPLY - genesisAllocation), but never more. One
    ///         additional sat past the cap must revert.
    function testFuzz_bridgeMint_respectsCap(uint256 genesis, uint256 mintAmount, address user) public {
        genesis = bound(genesis, 0, token.MAX_GENESIS_ALLOCATION());
        vm.assume(user != address(0));

        UDAGToken t;
        if (genesis == 0) {
            t = new UDAGToken(admin, bridge, address(0), 0);
        } else {
            t = new UDAGToken(admin, bridge, address(0xBEEF), genesis);
        }

        uint256 remaining = t.remainingSupply();
        mintAmount = bound(mintAmount, 1, remaining);

        vm.prank(bridge);
        t.mint(user, mintAmount);

        assertEq(t.balanceOf(user), mintAmount);
        assertEq(t.totalSupply(), genesis + mintAmount);
        assertLe(t.totalSupply(), t.MAX_SUPPLY());

        // One more sat past the cap must always revert. Cache
        // `remainingSupply()` BEFORE arming `vm.expectRevert`, otherwise
        // the subsequent external view call counts as "the next call"
        // and expectRevert matches against it (foundry footgun).
        uint256 overshoot = t.remainingSupply() + 1;
        uint256 currentRemaining = overshoot - 1;
        vm.prank(bridge);
        vm.expectRevert(
            abi.encodeWithSelector(
                UDAGToken.ExceedsMaxSupply.selector,
                overshoot,
                currentRemaining
            )
        );
        t.mint(user, overshoot);
    }

    // ─── Transfer / approve fuzz ───────────────────────────────────────

    /// @notice Transferring any amount up to the sender's balance must
    ///         conserve total supply, credit the recipient, debit the
    ///         sender, and NOT revert.
    function testFuzz_transferConservesSupply(
        address recipient,
        uint256 amount
    ) public {
        vm.assume(recipient != address(0));
        vm.assume(recipient != address(this));

        // Mint a fixed well-known amount to this contract via the bridge.
        uint256 seed = 1_000_000 * 10 ** 8;
        vm.prank(bridge);
        token.mint(address(this), seed);

        amount = bound(amount, 0, seed);
        uint256 supplyBefore = token.totalSupply();
        uint256 recipBefore = token.balanceOf(recipient);

        token.transfer(recipient, amount);

        assertEq(token.totalSupply(), supplyBefore, "supply changed on transfer");
        assertEq(token.balanceOf(address(this)), seed - amount);
        assertEq(token.balanceOf(recipient), recipBefore + amount);
    }

    /// @notice Burning any amount up to the caller's balance must reduce
    ///         totalSupply by exactly that amount and debit the caller.
    function testFuzz_burnSelfReducesSupply(uint256 mintAmount, uint256 burnAmount) public {
        mintAmount = bound(mintAmount, 1, token.MAX_SUPPLY());
        vm.prank(bridge);
        token.mint(address(this), mintAmount);

        burnAmount = bound(burnAmount, 1, mintAmount);
        uint256 supplyBefore = token.totalSupply();

        token.burnSelf(burnAmount);

        assertEq(token.totalSupply(), supplyBefore - burnAmount);
        assertEq(token.balanceOf(address(this)), mintAmount - burnAmount);
        // Burn expands the remaining mintable cap back out — another
        // `burnAmount` of headroom is now available.
        assertEq(token.remainingSupply(), token.MAX_SUPPLY() - (mintAmount - burnAmount));
    }

    // ─── Only bridge can mint ──────────────────────────────────────────

    /// @notice Any non-bridge address attempting to mint must revert,
    ///         regardless of the amount or the target.
    function testFuzz_onlyBridgeCanMint(address caller, address to, uint256 amount) public {
        vm.assume(caller != bridge);
        vm.assume(to != address(0));
        amount = bound(amount, 1, token.MAX_SUPPLY());

        vm.prank(caller);
        vm.expectRevert();
        token.mint(to, amount);
    }

    // ─── Bridge migration timelock cannot be shortened ─────────────────

    /// @notice executeBridgeMigration must revert whenever the current
    ///         block.timestamp is strictly earlier than the timelock
    ///         executableAfter stamp. No amount of admin privilege lets
    ///         you bypass the delay.
    function testFuzz_bridgeMigrationTimelockHolds(uint256 timeSkip) public {
        address newBridge = address(0xDEAD);

        vm.prank(admin);
        token.proposeBridgeMigration(newBridge);

        uint256 delay = token.BRIDGE_MIGRATION_DELAY();
        // Bounded skip that is still strictly less than the delay.
        timeSkip = bound(timeSkip, 0, delay - 1);
        vm.warp(block.timestamp + timeSkip);

        vm.prank(admin);
        vm.expectRevert(); // MigrationTimelockNotElapsed
        token.executeBridgeMigration();
    }

    /// @notice Exactly at-or-after the timelock delay, migration
    ///         succeeds and MINTER_ROLE transfers to the new bridge.
    function testFuzz_bridgeMigrationExecutesAfterDelay(uint256 overshoot) public {
        address newBridge = address(0xDEAD);

        vm.prank(admin);
        token.proposeBridgeMigration(newBridge);

        overshoot = bound(overshoot, 0, 365 days);
        vm.warp(block.timestamp + token.BRIDGE_MIGRATION_DELAY() + overshoot);

        vm.prank(admin);
        token.executeBridgeMigration();

        assertEq(token.bridgeAddress(), newBridge);
        assertTrue(token.hasRole(token.MINTER_ROLE(), newBridge));
        assertFalse(token.hasRole(token.MINTER_ROLE(), bridge));
    }
}
