// SPDX-License-Identifier: BUSL-1.1
pragma solidity ^0.8.24;

import "forge-std/Test.sol";
import "../src/UDAGToken.sol";
import "../src/UDAGBridgeValidator.sol";

/// @notice Handler contract driven by foundry's stateful fuzzer.
///
/// The fuzzer picks random functions on this handler and drives the
/// live bridge through arbitrary sequences of validator-set edits,
/// deposits, pauses, and rate-limit day-rollovers. The `invariant_*`
/// functions in the test contract then assert properties that must
/// hold after every sequence.
///
/// `claimWithdrawal` is deliberately NOT exposed here: constructing a
/// correctly-sorted threshold signature blob in Solidity against a
/// dynamic validator set is non-trivial and duplicates the Rust-side
/// test. The unit tests in UDAGBridgeValidator.t.sol cover
/// claimWithdrawal behaviour in depth; what this handler adds is
/// state-machine coverage for everything AROUND the claim path.
contract BridgeHandler is Test {
    UDAGToken public token;
    UDAGBridgeValidator public bridge;
    address public governor;

    // Ghost state shadowed against the contract's real state.
    uint256 public ghost_totalDeposited;
    uint256 public ghost_depositCount;

    // Ghost validator set — mirrors `bridge.validators[]` and
    // `bridge.isValidator`. Handler is the only mutator, so if the
    // ghost set ever disagrees with the real set, our invariants
    // catch it.
    address[] public ghostValidators;
    mapping(address => bool) public isGhostValidator;

    // Monotonicity tracking for `depositNonceCounter`.
    uint256 public lastObservedNonce;

    constructor(UDAGToken _token, UDAGBridgeValidator _bridge, address _governor) {
        token = _token;
        bridge = _bridge;
        governor = _governor;

        // Pre-fund the handler with 1M UDAG so `deposit` can fire even
        // without the mint path being exercised. `deal(..., true)`
        // updates totalSupply, so the token's supply invariant remains
        // internally consistent.
        deal(address(token), address(this), 1_000_000 * 10 ** 8, true);
        token.approve(address(bridge), type(uint256).max);
    }

    // ─── Actions ───────────────────────────────────────────────────────

    /// @notice Add a validator. The fuzzer provides an arbitrary
    ///         address — the handler filters out zero, already-registered,
    ///         and over-capacity cases before calling.
    function addValidator(address v) external {
        if (v == address(0)) return;
        if (isGhostValidator[v]) return;
        if (ghostValidators.length >= bridge.MAX_VALIDATORS()) return;

        vm.prank(governor);
        try bridge.addValidator(v) {
            ghostValidators.push(v);
            isGhostValidator[v] = true;
        } catch {}
    }

    /// @notice Remove a validator at a fuzzer-chosen index. Bounds to
    ///         a valid index; skips when at or below MIN_VALIDATORS.
    function removeValidator(uint256 idx) external {
        if (ghostValidators.length <= bridge.MIN_VALIDATORS()) return;
        idx = bound(idx, 0, ghostValidators.length - 1);
        address v = ghostValidators[idx];

        vm.prank(governor);
        try bridge.removeValidator(v) {
            // Mirror the contract's swap-and-pop in the ghost array.
            ghostValidators[idx] = ghostValidators[ghostValidators.length - 1];
            ghostValidators.pop();
            isGhostValidator[v] = false;
        } catch {}
    }

    /// @notice Lock UDAG in escrow to bridge to the native chain.
    function deposit(uint256 amount, bytes20 recipient) external {
        if (!bridge.bridgeEnabled()) return;
        if (bridge.paused()) return;
        if (recipient == bytes20(0)) return;

        uint256 lo = bridge.MIN_AMOUNT();
        uint256 hi = bridge.MAX_AMOUNT();
        amount = bound(amount, lo, hi);

        uint256 bal = token.balanceOf(address(this));
        if (bal < amount) return;

        // Day-scoped cap: skip if we'd blow the daily ceiling. The
        // contract would revert, and invariants don't care about
        // reverted calls — skipping keeps the fuzzer productive.
        uint256 dayIdx = block.timestamp / 1 days;
        // Solidity can't read the private mapping directly, but we can
        // use the view helper the contract exposes.
        if (bridge.getDailyDepositRemaining() < amount) return;

        try bridge.deposit(recipient, amount) {
            ghost_totalDeposited += amount;
            ghost_depositCount++;
        } catch {}
        // Silence unused-var warning while documenting the day index
        // being scoped to the current block.
        dayIdx;
    }

    function pause() external {
        vm.prank(governor);
        try bridge.pause() {} catch {}
    }

    function unpause() external {
        vm.prank(governor);
        try bridge.unpause() {} catch {}
    }

    function setThreshold(uint256 newThreshold) external {
        uint256 n = ghostValidators.length;
        if (n == 0) return;
        newThreshold = bound(newThreshold, 1, n);
        vm.prank(governor);
        try bridge.setThreshold(newThreshold) {} catch {}
    }

    /// @notice Advance time by one day so daily rate-limit counters
    ///         roll over. Without this, every deposit series would
    ///         eventually saturate the day's 500k cap and the fuzzer
    ///         would stall.
    function warpOneDay() external {
        vm.warp(block.timestamp + 1 days + 1);
    }

    // ─── Views for invariant assertions ────────────────────────────────

    function ghostValidatorCount() external view returns (uint256) {
        return ghostValidators.length;
    }

    function allGhostValidators() external view returns (address[] memory) {
        return ghostValidators;
    }

    /// @notice Track whether the deposit nonce ever goes backwards.
    ///         Called by invariant checks — cheaper than scanning.
    function recordNonce() external {
        uint256 current = bridge.depositNonceCounter();
        if (current < lastObservedNonce) {
            revert("deposit nonce went backwards");
        }
        lastObservedNonce = current;
    }
}

/// @notice Invariant tests for UDAGBridgeValidator.
///
/// The bridge has much richer state than the token: a validator array
/// + mapping that must stay in sync, a BFT threshold that tracks the
/// validator count, rate limits that roll over by day, monotonic
/// nonce counters, and a pausable / enablable / migratable state
/// machine. Each of these invariants checks a property that no single
/// unit test can cover because they depend on arbitrary sequences of
/// state transitions.
contract BridgeInvariantTest is Test {
    UDAGToken public token;
    UDAGBridgeValidator public bridge;
    BridgeHandler public handler;

    address public governor = address(0xBEEF);

    function setUp() public {
        // Deploy the token + bridge with CREATE-nonce prediction so
        // the token's MINTER_ROLE can be set at construction time.
        uint256 govNonce = vm.getNonce(governor);
        address predictedBridge = vm.computeCreateAddress(governor, govNonce + 1);

        vm.startPrank(governor);
        token = new UDAGToken(governor, predictedBridge, address(0), 0);
        bridge = new UDAGBridgeValidator(address(token), governor);
        require(address(bridge) == predictedBridge, "bridge address mismatch");
        vm.stopPrank();

        handler = new BridgeHandler(token, bridge, governor);

        // Restrict the fuzzer's universe: only the handler's exported
        // functions are callable. This keeps the state transitions
        // inside our ghost-tracked wrappers.
        targetContract(address(handler));
    }

    // ─── Validator set consistency ─────────────────────────────────────

    /// @notice Every address in the ghost set must be a validator on
    ///         the bridge, and the ghost size must match the real size.
    ///         Catches any bug where the contract's `isValidator`
    ///         mapping and `validators` array drift apart.
    function invariant_validatorSetInSync() public view {
        address[] memory ghosts = handler.allGhostValidators();
        assertEq(
            ghosts.length,
            bridge.getValidatorCount(),
            "ghost validator count != real validator count"
        );
        for (uint256 i = 0; i < ghosts.length; i++) {
            assertTrue(
                bridge.isValidator(ghosts[i]),
                "ghost-registered validator not flagged in isValidator mapping"
            );
        }
    }

    /// @notice Threshold bounds:
    ///           - Always in [0, n] (where n = getValidatorCount()).
    ///             n == 0 is the initial state before any validator
    ///             is added, where threshold is still its default 0.
    ///           - Once at or above MIN_VALIDATORS, threshold must
    ///             be at least `floor(2n/3) + 1` — the BFT minimum
    ///             that `_updateThreshold` and `setThreshold` both
    ///             enforce.
    ///
    ///         Subtle: below MIN_VALIDATORS, `_updateThreshold` sets
    ///         threshold = n, BUT the governor is also free to call
    ///         `setThreshold(anywhere-in-[1,n])` in that regime. So
    ///         the strongest provable property below MIN is simply
    ///         `1 <= threshold <= n` (or `0 == n == threshold` for
    ///         the pristine state).
    function invariant_thresholdRespectsFormula() public view {
        uint256 n = bridge.getValidatorCount();
        uint256 t = bridge.getThreshold();
        assertLe(t, n, "threshold exceeds validator count");

        if (n == 0) {
            assertEq(t, 0, "threshold non-zero with no validators");
            return;
        }

        if (n >= bridge.MIN_VALIDATORS()) {
            uint256 bftMin = (2 * n) / 3 + 1;
            assertGe(t, bftMin, "threshold fell below BFT minimum");
        } else {
            // Below MIN_VALIDATORS: threshold can be anywhere in [1, n]
            // because setThreshold is allowed to set it to any value
            // in that range without the BFT floor kicking in.
            assertGe(t, 1, "threshold dropped to zero with n > 0");
        }
    }

    // ─── Bridge enablement is monotonic ────────────────────────────────

    /// @notice Once the bridge auto-enables (at MIN_VALIDATORS), there
    ///         is no code path that sets `bridgeEnabled` back to false.
    ///         This invariant asserts: if we are enabled now, we have
    ///         at least MIN_VALIDATORS. The inverse ("if we had enough
    ///         validators in the past we're still enabled") is
    ///         automatically true because removeValidator enforces
    ///         `validators.length > MIN_VALIDATORS` — you physically
    ///         cannot drop below MIN once enabled.
    function invariant_bridgeEnabledImpliesMinValidators() public view {
        if (bridge.bridgeEnabled()) {
            assertGe(
                bridge.getValidatorCount(),
                bridge.MIN_VALIDATORS(),
                "bridge enabled with fewer than MIN_VALIDATORS"
            );
        }
    }

    // ─── Rate limit accounting ─────────────────────────────────────────

    /// @notice Daily deposit volume must never exceed DAILY_LIMIT.
    ///         The contract enforces this in-line; this invariant
    ///         catches any future regression where the enforcement
    ///         gets weakened or routed around.
    function invariant_dailyDepositVolumeUnderCap() public view {
        // Deposit volume for the current day.
        uint256 dailyVol = bridge.getDailyDepositVolume();
        assertLe(
            dailyVol,
            bridge.DAILY_LIMIT(),
            "daily deposit volume exceeds DAILY_LIMIT"
        );
    }

    /// @notice Same for withdrawals. claimWithdrawal isn't exposed
    ///         in the handler, so the withdrawal counter can only go
    ///         up via other tests; this invariant still holds
    ///         trivially in this fuzz run but documents the property
    ///         for future expansion.
    function invariant_dailyWithdrawalVolumeUnderCap() public view {
        uint256 dailyVol = bridge.getDailyWithdrawalVolume();
        assertLe(dailyVol, bridge.DAILY_LIMIT());
    }

    // ─── Escrow accounting ─────────────────────────────────────────────

    /// @notice Token balance held in escrow by the bridge must equal
    ///         the ghost sum of all successful deposits. If any code
    ///         path accepted a deposit without transferring tokens, or
    ///         transferred tokens without recording it, these diverge.
    function invariant_escrowMatchesDeposits() public view {
        assertEq(
            token.balanceOf(address(bridge)),
            handler.ghost_totalDeposited(),
            "bridge escrow balance != sum of successful deposits"
        );
    }

    // ─── Deposit nonce monotonicity ────────────────────────────────────

    /// @notice The deposit nonce counter must only ever grow. This is
    ///         the replay-protection foundation — if it ever decreased
    ///         or reset, a replayed deposit attestation could be
    ///         accepted on the destination chain.
    function invariant_depositNonceMonotonic() public view {
        assertGe(
            bridge.depositNonceCounter(),
            handler.lastObservedNonce(),
            "deposit nonce went backwards"
        );
    }

    /// @notice The real nonce counter must equal the ghost deposit
    ///         count — every successful deposit bumps the counter by
    ///         exactly one.
    function invariant_nonceEqualsDepositCount() public view {
        assertEq(
            bridge.depositNonceCounter(),
            handler.ghost_depositCount(),
            "depositNonceCounter drifted from ghost_depositCount"
        );
    }

    // ─── Governance invariants ─────────────────────────────────────────

    /// @notice Governor must never become the zero address. The
    ///         constructor rejects zero, `setGovernor` rejects zero,
    ///         and `acceptGovernor` cannot be called without a
    ///         previous set. The handler doesn't call setGovernor, so
    ///         this holds trivially — but it's a cheap sentinel
    ///         against future refactors.
    function invariant_governorIsNonZero() public view {
        assertTrue(bridge.governor() != address(0));
    }
}
