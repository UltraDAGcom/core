// SPDX-License-Identifier: BUSL-1.1
pragma solidity ^0.8.24;

import "@openzeppelin/contracts/token/ERC20/ERC20.sol";
import "@openzeppelin/contracts/token/ERC20/extensions/ERC20Permit.sol";
import "@openzeppelin/contracts/access/AccessControl.sol";
import "@openzeppelin/contracts/utils/Pausable.sol";

/// @title UltraDAG Token (UDAG)
/// @notice ERC-20 representation of UDAG on Arbitrum with enhanced security controls.
/// @dev 8 decimals to match the native chain (1 UDAG = 100,000,000 sats).
///
///      Bridge model (escrow + mint):
///        • Deposit  (Arbitrum → native): bridge calls transferFrom() to lock tokens in escrow.
///        • Withdraw (native → Arbitrum): bridge calls mint() to create tokens for the claimant.
///
///      No burn-by-role is needed under this model. Users may still burn their own
///      tokens voluntarily via burnSelf().
///
///      Post-genesis role lockdown:
///        • finalizeGenesis() makes MINTER_ROLE permanently un-grantable except
///          for the bridge, which retains its existing grant.
///        • A timelock-gated bridge migration path remains available post-genesis
///          so a compromised bridge can be replaced without redeploying the token.
///        • renounceAdminRole() is the final decentralisation step and is irreversible.
contract UDAGToken is ERC20, ERC20Permit, AccessControl, Pausable {

    // ─── Role Definitions ───────────────────────────────────────────────
    bytes32 public constant MINTER_ROLE = keccak256("MINTER_ROLE");
    bytes32 public constant PAUSER_ROLE = keccak256("PAUSER_ROLE");

    // ─── Supply Configuration ───────────────────────────────────────────
    /// @notice Maximum supply: 21,000,000 UDAG with 8 decimal places.
    uint256 public constant MAX_SUPPLY = 21_000_000 * 10 ** 8;

    // ─── Genesis State ──────────────────────────────────────────────────
    bool public genesisFinalized;
    address public bridgeAddress;
    address public genesisMinter;

    // ─── Bridge Migration Timelock ──────────────────────────────────────
    /// @notice Minimum delay between proposing and executing a bridge migration.
    uint256 public constant BRIDGE_MIGRATION_DELAY = 2 days;

    struct BridgeMigration {
        address newBridge;
        uint256 executableAfter; // timestamp after which the migration can execute
    }
    BridgeMigration public pendingBridgeMigration;

    // ─── Events ─────────────────────────────────────────────────────────
    event GenesisFinalized(uint256 indexed totalSupply, address indexed finalizedBy);
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
    error GenesisAlreadyFinalized();
    error GenesisNotFinalized();
    error SameBridgeAddress();
    error NoPendingMigration();
    error MigrationTimelockNotElapsed(uint256 executableAfter, uint256 currentTime);
    error MigrationBridgeMismatch();
    error MigrationAlreadyPending();

    /// @notice Constructor - sets up initial roles and configuration.
    /// @param admin       Address that will hold DEFAULT_ADMIN_ROLE (should be a timelock / multisig).
    /// @param initialBridge Address of the bridge contract.
    /// @param genesisMinter_ Address that can mint genesis allocations (deployer EOA, temporary).
    constructor(
        address admin,
        address initialBridge,
        address genesisMinter_
    )
        ERC20("UltraDAG", "UDAG")
        ERC20Permit("UltraDAG")
    {
        if (admin == address(0))          revert ZeroAddress("admin");
        if (initialBridge == address(0))  revert ZeroAddress("initialBridge");
        if (genesisMinter_ == address(0)) revert ZeroAddress("genesisMinter");

        // Admin roles
        _grantRole(DEFAULT_ADMIN_ROLE, admin);
        _grantRole(PAUSER_ROLE, admin);

        // Minting roles (genesis minter is temporary)
        _grantRole(MINTER_ROLE, admin);
        _grantRole(MINTER_ROLE, genesisMinter_);
        _grantRole(MINTER_ROLE, initialBridge);

        genesisMinter = genesisMinter_;
        bridgeAddress = initialBridge;

        emit BridgeUpdated(address(0), initialBridge);
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

    /// @notice Mint tokens. Only callable by MINTER_ROLE.
    /// @dev Used by the bridge for native → Arbitrum withdrawal claims,
    ///      and by admin/genesisMinter for pre-genesis allocations.
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

    // ─── Genesis Finalization ───────────────────────────────────────────

    /// @notice Finalize genesis minting.
    /// @dev Revokes MINTER_ROLE from admin and genesisMinter, then permanently
    ///      locks MINTER_ROLE so no new grants can ever be made.
    ///      The bridge retains its existing MINTER_ROLE grant.
    function finalizeGenesis() external onlyRole(DEFAULT_ADMIN_ROLE) whenNotPaused {
        if (genesisFinalized) revert GenesisAlreadyFinalized();

        genesisFinalized = true;

        _revokeRole(MINTER_ROLE, msg.sender);
        _revokeRole(MINTER_ROLE, genesisMinter);

        // Set MINTER_ROLE's admin to a role nobody holds, permanently preventing
        // new grants. The bridge keeps its existing grant.
        _setRoleAdmin(MINTER_ROLE, bytes32(type(uint256).max));

        emit GenesisFinalized(totalSupply(), msg.sender);
    }

    // ─── Bridge Management ──────────────────────────────────────────────
    //
    //  Pre-genesis  : updateBridge() for immediate swaps (deployment flexibility).
    //  Post-genesis : two-step timelock migration via proposeBridgeMigration()
    //                 + executeBridgeMigration() for safety.

    /// @notice Immediately update the bridge address. Only available before genesis.
    function updateBridge(address newBridge) external onlyRole(DEFAULT_ADMIN_ROLE) whenNotPaused {
        if (genesisFinalized)            revert GenesisAlreadyFinalized();
        if (newBridge == address(0))     revert ZeroAddress("newBridge");
        if (newBridge == bridgeAddress)  revert SameBridgeAddress();

        address oldBridge = bridgeAddress;

        _revokeRole(MINTER_ROLE, oldBridge);
        _grantRole(MINTER_ROLE, newBridge);

        bridgeAddress = newBridge;
        emit BridgeUpdated(oldBridge, newBridge);
    }

    /// @notice Propose a post-genesis bridge migration (timelock-gated).
    /// @dev Starts a BRIDGE_MIGRATION_DELAY countdown. Can be cancelled by admin.
    function proposeBridgeMigration(address newBridge)
        external
        onlyRole(DEFAULT_ADMIN_ROLE)
        whenNotPaused
    {
        if (!genesisFinalized)           revert GenesisNotFinalized();
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
    /// @dev The MINTER_ROLE admin is locked post-genesis, so we cannot use
    ///      _grantRole/_revokeRole directly. Instead we transfer the bridge's
    ///      minter capability by low-level role slot manipulation:
    ///        1. Temporarily reset MINTER_ROLE admin to DEFAULT_ADMIN_ROLE.
    ///        2. Revoke from old bridge, grant to new bridge.
    ///        3. Re-lock MINTER_ROLE admin.
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
        _setRoleAdmin(MINTER_ROLE, bytes32(type(uint256).max));

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
    /// @dev Only callable after genesis is finalized. Also removes PAUSER_ROLE.
    function renounceAdminRole() external onlyRole(DEFAULT_ADMIN_ROLE) {
        if (!genesisFinalized) revert GenesisNotFinalized();

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
