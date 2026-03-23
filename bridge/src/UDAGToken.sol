// SPDX-License-Identifier: BUSL-1.1
pragma solidity ^0.8.24;

import "@openzeppelin/contracts/token/ERC20/ERC20.sol";
import "@openzeppelin/contracts/token/ERC20/extensions/ERC20Permit.sol";
import "@openzeppelin/contracts/access/AccessControl.sol";
import "@openzeppelin/contracts/utils/Pausable.sol";

/// @title UltraDAG Token (UDAG)
/// @notice ERC-20 representation of UDAG on Arbitrum, purely bridge-driven.
/// @dev 8 decimals to match the native chain (1 UDAG = 100,000,000 sats).
///
///      All UDAG originates on the native chain via emission. The only way
///      ERC-20 UDAG enters Arbitrum is through the bridge (bridge mints on
///      withdrawal claim). There is no genesis minting.
///
///      Bridge model (escrow + mint):
///        - Deposit  (Arbitrum -> native): bridge calls transferFrom() to lock tokens in escrow.
///        - Withdraw (native -> Arbitrum): bridge calls mint() to create tokens for the claimant.
///
///      Role lockdown:
///        - MINTER_ROLE admin is set to a dead role in the constructor, so no one
///          can ever grant new MINTER_ROLE except via the internal updateBridge().
///        - A timelock-gated bridge migration path remains available so a compromised
///          bridge can be replaced without redeploying the token.
///        - renounceAdminRole() is the final decentralisation step and is irreversible.
contract UDAGToken is ERC20, ERC20Permit, AccessControl, Pausable {

    // ─── Role Definitions ───────────────────────────────────────────────
    bytes32 public constant MINTER_ROLE = keccak256("MINTER_ROLE");
    bytes32 public constant PAUSER_ROLE = keccak256("PAUSER_ROLE");

    /// @dev A role that no address will ever hold. Used to permanently lock
    ///      MINTER_ROLE grants via the standard AccessControl admin mechanism.
    bytes32 private constant DEAD_ROLE = bytes32(type(uint256).max);

    // ─── Supply Configuration ───────────────────────────────────────────
    /// @notice Maximum supply: 21,000,000 UDAG with 8 decimal places.
    uint256 public constant MAX_SUPPLY = 21_000_000 * 10 ** 8;

    // ─── Bridge State ───────────────────────────────────────────────────
    address public bridgeAddress;

    // ─── Bridge Migration Timelock ──────────────────────────────────────
    /// @notice Minimum delay between proposing and executing a bridge migration.
    uint256 public constant BRIDGE_MIGRATION_DELAY = 2 days;

    struct BridgeMigration {
        address newBridge;
        uint256 executableAfter; // timestamp after which the migration can execute
    }
    BridgeMigration public pendingBridgeMigration;

    // ─── Events ─────────────────────────────────────────────────────────
    event BridgeUpdated(address indexed oldBridge, address indexed newBridge);
    event BridgeMigrationProposed(address indexed newBridge, uint256 executableAfter, address indexed proposedBy);
    event BridgeMigrationCancelled(address indexed cancelledBridge, address indexed cancelledBy);
    event EmergencyPause(address indexed pausedBy, string reason);
    event EmergencyUnpause(address indexed unpausedBy);
    event AdminRoleRenounced(address indexed formerAdmin);

    // ─── Errors ─────────────────────────────────────────────────────────
    error ZeroAddress(string param);
    error MintAmountZero();
    error BurnAmountZero();
    error ExceedsMaxSupply(uint256 requested, uint256 remaining);
    error SameBridgeAddress();
    error NoPendingMigration();
    error MigrationTimelockNotElapsed(uint256 executableAfter, uint256 currentTime);
    error MigrationAlreadyPending();

    /// @notice Constructor - sets up initial roles and configuration.
    /// @param admin  Address that will hold DEFAULT_ADMIN_ROLE (should be a timelock / multisig).
    /// @param bridge Address of the bridge contract (sole minter).
    constructor(
        address admin,
        address bridge
    )
        ERC20("UltraDAG", "UDAG")
        ERC20Permit("UltraDAG")
    {
        if (admin == address(0))  revert ZeroAddress("admin");
        if (bridge == address(0)) revert ZeroAddress("bridge");

        // Admin roles
        _grantRole(DEFAULT_ADMIN_ROLE, admin);
        _grantRole(PAUSER_ROLE, admin);

        // Bridge is the sole minter
        _grantRole(MINTER_ROLE, bridge);

        // Permanently lock MINTER_ROLE: set its admin to a role nobody holds.
        // After this, no one can call grantRole(MINTER_ROLE, ...) via AccessControl.
        // Bridge migration uses _grantRole/_revokeRole internally to bypass this.
        _setRoleAdmin(MINTER_ROLE, DEAD_ROLE);

        bridgeAddress = bridge;

        emit BridgeUpdated(address(0), bridge);
    }

    // ─── ERC-20 Overrides ───────────────────────────────────────────────

    /// @notice 8 decimals to match the native UDAG chain.
    function decimals() public pure override returns (uint8) {
        return 8;
    }

    /// @dev Enforces pause on all token movements (transfers, mints, burns).
    function _update(
        address from,
        address to,
        uint256 value
    ) internal override(ERC20) whenNotPaused {
        super._update(from, to, value);
    }

    // ─── Pause Control ──────────────────────────────────────────────────

    /// @notice Pause all token transfers and minting.
    /// @param reason Human-readable reason for the pause (emitted in the event).
    function pause(string calldata reason) external onlyRole(PAUSER_ROLE) whenNotPaused {
        _pause();
        emit EmergencyPause(msg.sender, reason);
    }

    /// @notice Resume token operations after an emergency.
    /// @dev Only DEFAULT_ADMIN_ROLE to ensure careful review before unpausing.
    function unpause() external onlyRole(DEFAULT_ADMIN_ROLE) whenPaused {
        _unpause();
        emit EmergencyUnpause(msg.sender);
    }

    // ─── Minting ────────────────────────────────────────────────────────

    /// @notice Mint tokens. Only callable by MINTER_ROLE (the bridge).
    /// @dev Used by the bridge for native -> Arbitrum withdrawal claims.
    function mint(address to, uint256 amount)
        external
        onlyRole(MINTER_ROLE)
        whenNotPaused
    {
        if (to == address(0)) revert ZeroAddress("to");
        if (amount == 0)      revert MintAmountZero();

        uint256 remaining = MAX_SUPPLY - totalSupply();
        if (amount > remaining) revert ExceedsMaxSupply(amount, remaining);

        _mint(to, amount);
    }

    // ─── Burning ────────────────────────────────────────────────────────

    /// @notice Burn tokens from the caller. Anyone can burn their own tokens.
    function burnSelf(uint256 amount) external whenNotPaused {
        if (amount == 0) revert BurnAmountZero();
        _burn(msg.sender, amount);
    }

    // ─── Bridge Management ──────────────────────────────────────────────
    //
    //  Two-step timelock migration via proposeBridgeMigration()
    //  + executeBridgeMigration() for safety.

    /// @notice Propose a bridge migration (timelock-gated).
    /// @dev Starts a BRIDGE_MIGRATION_DELAY countdown. Can be cancelled by admin.
    function proposeBridgeMigration(address newBridge)
        external
        onlyRole(DEFAULT_ADMIN_ROLE)
        whenNotPaused
    {
        if (newBridge == address(0))     revert ZeroAddress("newBridge");
        if (newBridge == bridgeAddress)  revert SameBridgeAddress();
        if (pendingBridgeMigration.newBridge != address(0)) revert MigrationAlreadyPending();

        uint256 executableAfter = block.timestamp + BRIDGE_MIGRATION_DELAY;
        pendingBridgeMigration = BridgeMigration({
            newBridge: newBridge,
            executableAfter: executableAfter
        });

        emit BridgeMigrationProposed(newBridge, executableAfter, msg.sender);
    }

    /// @notice Execute a previously proposed bridge migration after the timelock.
    /// @dev Temporarily resets MINTER_ROLE admin to DEFAULT_ADMIN_ROLE so we can
    ///      revoke/grant, then re-locks it to the dead role.
    function executeBridgeMigration()
        external
        onlyRole(DEFAULT_ADMIN_ROLE)
        whenNotPaused
    {
        BridgeMigration memory migration = pendingBridgeMigration;

        if (migration.newBridge == address(0)) revert NoPendingMigration();
        if (block.timestamp < migration.executableAfter) {
            revert MigrationTimelockNotElapsed(migration.executableAfter, block.timestamp);
        }

        address oldBridge = bridgeAddress;

        // Temporarily unlock MINTER_ROLE for role transfer
        _setRoleAdmin(MINTER_ROLE, DEFAULT_ADMIN_ROLE);
        _revokeRole(MINTER_ROLE, oldBridge);
        _grantRole(MINTER_ROLE, migration.newBridge);
        // Re-lock MINTER_ROLE permanently
        _setRoleAdmin(MINTER_ROLE, DEAD_ROLE);

        bridgeAddress = migration.newBridge;

        // Clear pending migration
        delete pendingBridgeMigration;

        emit BridgeUpdated(oldBridge, migration.newBridge);
    }

    /// @notice Cancel a pending bridge migration.
    function cancelBridgeMigration() external onlyRole(DEFAULT_ADMIN_ROLE) {
        address pending = pendingBridgeMigration.newBridge;
        if (pending == address(0)) revert NoPendingMigration();

        delete pendingBridgeMigration;

        emit BridgeMigrationCancelled(pending, msg.sender);
    }

    // ─── Administrative Functions ───────────────────────────────────────

    /// @notice Irreversibly renounce DEFAULT_ADMIN_ROLE (full decentralisation).
    /// @dev Also removes PAUSER_ROLE from the caller.
    function renounceAdminRole() external onlyRole(DEFAULT_ADMIN_ROLE) {
        _revokeRole(DEFAULT_ADMIN_ROLE, msg.sender);
        _revokeRole(PAUSER_ROLE, msg.sender);

        emit AdminRoleRenounced(msg.sender);
    }

    /// @notice Grant PAUSER_ROLE to a new address (e.g., monitoring bot).
    function grantPauserRole(address account) external onlyRole(DEFAULT_ADMIN_ROLE) {
        if (account == address(0)) revert ZeroAddress("account");
        _grantRole(PAUSER_ROLE, account);
    }

    /// @notice Revoke PAUSER_ROLE from an address.
    function revokePauserRole(address account) external onlyRole(DEFAULT_ADMIN_ROLE) {
        _revokeRole(PAUSER_ROLE, account);
    }

    // ─── View Functions ─────────────────────────────────────────────────

    /// @notice Check if an address has minting privileges.
    function isMinter(address account) external view returns (bool) {
        return hasRole(MINTER_ROLE, account);
    }

    /// @notice Whether the contract is currently paused.
    function isPaused() external view returns (bool) {
        return paused();
    }

    /// @notice Remaining tokens that can be minted before hitting MAX_SUPPLY.
    function remainingSupply() external view returns (uint256) {
        return MAX_SUPPLY - totalSupply();
    }
}
