// SPDX-License-Identifier: BUSL-1.1
pragma solidity ^0.8.24;

import "forge-std/Test.sol";
import "../src/UDAGToken.sol";

/// @notice Handler contract used by UDAGTokenInvariantTest.
///
/// Foundry's invariant fuzzer picks random functions on this contract
/// to call, with random arguments, building up a sequence of state
/// transitions. After each sequence the test contract's `invariant_*`
/// functions are evaluated against the state. Anything we want the
/// fuzzer to be able to do to the token goes here; anything we DON'T
/// want it to do (e.g. deploy a new token) is simply not exposed.
contract Handler is Test {
    UDAGToken public token;
    address public admin;
    address public bridge;

    // Track everyone who ever held a non-zero balance so the invariant
    // test can sum balances for conservation checks.
    address[] public holders;
    mapping(address => bool) internal seenHolder;

    // Ghost variables: shadow state that should always agree with the
    // real contract's state.
    uint256 public ghost_totalMinted;
    uint256 public ghost_totalBurned;

    constructor(UDAGToken _token, address _admin, address _bridge) {
        token = _token;
        admin = _admin;
        bridge = _bridge;
        _trackHolder(_admin);
        _trackHolder(_bridge);
        _trackHolder(address(this));
    }

    function _trackHolder(address who) internal {
        if (who == address(0) || seenHolder[who]) return;
        seenHolder[who] = true;
        holders.push(who);
    }

    function allHolders() external view returns (address[] memory) {
        return holders;
    }

    // ─── Actions the fuzzer may take ───────────────────────────────────

    function mint(address to, uint256 amount) external {
        vm.assume(to != address(0));
        uint256 remaining = token.remainingSupply();
        if (remaining == 0) return;
        amount = bound(amount, 1, remaining);

        vm.prank(bridge);
        try token.mint(to, amount) {
            ghost_totalMinted += amount;
            _trackHolder(to);
        } catch {
            // Ignore reverts — not a property violation, just the
            // fuzzer picking disallowed inputs (paused, etc.). The
            // real property checks happen in `invariant_*`.
        }
    }

    function transfer(uint256 fromIdx, uint256 toIdx, uint256 amount) external {
        if (holders.length == 0) return;
        address from = holders[bound(fromIdx, 0, holders.length - 1)];
        address to = holders[bound(toIdx, 0, holders.length - 1)];
        if (from == address(0) || to == address(0)) return;

        uint256 bal = token.balanceOf(from);
        if (bal == 0) return;
        amount = bound(amount, 0, bal);

        vm.prank(from);
        try token.transfer(to, amount) {
            _trackHolder(to);
        } catch {}
    }

    function burnSelf(uint256 fromIdx, uint256 amount) external {
        if (holders.length == 0) return;
        address from = holders[bound(fromIdx, 0, holders.length - 1)];
        if (from == address(0)) return;

        uint256 bal = token.balanceOf(from);
        if (bal == 0) return;
        amount = bound(amount, 1, bal);

        vm.prank(from);
        try token.burnSelf(amount) {
            ghost_totalBurned += amount;
        } catch {}
    }

    function pause(string calldata reason) external {
        vm.prank(admin);
        try token.pause(reason) {} catch {}
    }

    function unpause() external {
        vm.prank(admin);
        try token.unpause() {} catch {}
    }
}

/// @notice Invariant tests for UDAGToken.
///
/// Invariants are assertions that must hold for ANY sequence of
/// operations the fuzzer can produce. They are much more powerful than
/// unit tests or stateless fuzz tests because they catch bugs that only
/// manifest after a specific sequence of state transitions.
///
/// Tuning: foundry's defaults (256 runs × 500 calls/run) are fine for
/// local dev. Bump with `forge test --invariant-runs 10000 --invariant-depth 500`
/// on CI / before a release.
contract UDAGTokenInvariantTest is Test {
    UDAGToken public token;
    Handler public handler;

    address public admin  = address(0xAA1);
    address public bridge = address(0xBB2);

    function setUp() public {
        vm.prank(admin);
        token = new UDAGToken(admin, bridge, address(0), 0);
        handler = new Handler(token, admin, bridge);

        // Restrict the fuzzer's universe: it may only call functions
        // on `handler`, not on the token directly. This keeps the state
        // transitions inside our carefully-crafted wrappers that track
        // ghost variables.
        targetContract(address(handler));
    }

    /// @notice The cap is sacred: total supply must never exceed
    ///         MAX_SUPPLY under any sequence of operations.
    function invariant_totalSupplyUnderCap() public view {
        assertLe(token.totalSupply(), token.MAX_SUPPLY());
    }

    /// @notice Conservation of tokens. For every address the handler
    ///         has ever touched, sum balances and confirm they equal
    ///         totalSupply. If any code path forgot to debit a sender
    ///         or double-credited a recipient, this catches it.
    function invariant_balancesSumToTotalSupply() public view {
        address[] memory hs = handler.allHolders();
        uint256 sum = 0;
        for (uint256 i = 0; i < hs.length; i++) {
            sum += token.balanceOf(hs[i]);
        }
        assertEq(sum, token.totalSupply(), "balances do not sum to totalSupply");
    }

    /// @notice The bridge address, genesis allocation, and genesis
    ///         recipient never change on their own. Bridge changes
    ///         require a 2-day timelock and happen only via
    ///         `executeBridgeMigration`, which the handler does not
    ///         expose. Genesis fields are constructor-immutables.
    function invariant_immutablesUnchanged() public view {
        assertEq(token.bridgeAddress(), bridge);
        assertEq(token.genesisAllocation(), 0);
        assertEq(token.genesisRecipient(), address(0));
    }

    /// @notice MINTER_ROLE can never be granted via the standard
    ///         AccessControl path — its admin is locked to a dead role.
    ///         The handler cannot exploit any normal grantRole call to
    ///         create a new minter.
    function invariant_bridgeIsSoleMinter() public view {
        assertTrue(token.hasRole(token.MINTER_ROLE(), bridge));
        // The admin must never accidentally also be a minter.
        assertFalse(token.hasRole(token.MINTER_ROLE(), admin));
    }

    /// @notice Ghost accounting: whatever has been minted minus whatever
    ///         has been burned equals the current totalSupply. This
    ///         catches any code path where tokens appear or disappear
    ///         outside of `mint` and `burnSelf`.
    function invariant_mintMinusBurnEqualsSupply() public view {
        assertEq(
            handler.ghost_totalMinted() - handler.ghost_totalBurned(),
            token.totalSupply(),
            "mint/burn accounting drifted from totalSupply"
        );
    }

    /// @notice Remaining supply plus current supply equals the cap.
    ///         Trivial to state but catches arithmetic bugs in the
    ///         `remainingSupply()` view function.
    function invariant_remainingPlusCurrentEqualsCap() public view {
        assertEq(
            token.remainingSupply() + token.totalSupply(),
            token.MAX_SUPPLY()
        );
    }
}
