// SPDX-License-Identifier: BUSL-1.1
pragma solidity ^0.8.24;

import "@openzeppelin/contracts/token/ERC20/ERC20.sol";
import "@openzeppelin/contracts/token/ERC20/extensions/ERC20Permit.sol";
import "@openzeppelin/contracts/access/AccessControl.sol";
import "@openzeppelin/contracts/security/Pausable.sol";
import "@openzeppelin/contracts/security/ReentrancyGuard.sol";

/// @title UltraDAG Token (UDAG) - Hardened Version
/// @notice ERC-20 representation of UDAG on Arbitrum with enhanced security controls.
/// @dev 8 decimals to match the native chain (1 UDAG = 100,000,000 sats).
///      The bridge contract holds MINTER_ROLE and BURNER_ROLE.
///      Implements emergency pause, reentrancy guard, and improved role management.
contract UDAGToken is ERC20, ERC20Permit, AccessControl, Pausable, ReentrancyGuard {
    
    // ─── Role Definitions ───
    bytes32 public constant MINTER_ROLE = keccak256("MINTER_ROLE");
    bytes32 public constant BURNER_ROLE = keccak256("BURNER_ROLE");
    bytes32 public constant PAUSER_ROLE = keccak256("PAUSER_ROLE");
    
    // ─── Supply Configuration ───
    /// @notice Maximum supply: 21,000,000 UDAG with 8 decimal places.
    uint256 public constant MAX_SUPPLY = 21_000_000 * 10 ** 8;

    // ─── Genesis State ───
    bool public genesisFinalized;
    address public bridgeAddress; // Track authorized bridge for monitoring

    // ─── Events ───
    event GenesisFinalized(uint256 indexed totalSupply, address indexed finalizedBy);
    event BridgeUpdated(address indexed oldBridge, address indexed newBridge);
    event EmergencyPause(address indexed pausedBy, string reason);
    event EmergencyUnpause(address indexed unpausedBy);
    event AdminRoleRenounced(address indexed formerAdmin);

    /// @notice Constructor - sets up initial roles and configuration
    /// @param admin Address that will hold DEFAULT_ADMIN_ROLE initially
    /// @param initialBridge Address of the bridge contract (can be updated later)
    /// @dev Bridge receives MINTER_ROLE for minting on claims.
    ///      Bridge does NOT receive BURNER_ROLE - deposits use transferFrom() to lock tokens.
    constructor(
        address admin,
        address initialBridge
    )
        ERC20("UltraDAG", "UDAG")
        ERC20Permit("UltraDAG")
    {
        require(admin != address(0), "UDAG: admin cannot be zero");
        require(initialBridge != address(0), "UDAG: bridge cannot be zero");

        _grantRole(DEFAULT_ADMIN_ROLE, admin);
        _grantRole(MINTER_ROLE, admin);
        _grantRole(PAUSER_ROLE, admin); // Admin can pause initially

        bridgeAddress = initialBridge;
        // Bridge only needs MINTER_ROLE for minting on withdrawal claims
        // Deposits use transferFrom() to lock tokens - no burn needed
        _grantRole(MINTER_ROLE, initialBridge);

        emit BridgeUpdated(address(0), initialBridge);
    }

    /// @notice Override decimals to match native chain (8 decimals)
    function decimals() public pure override returns (uint8) {
        return 8;
    }

    // ─── Pause Control (Pausable) ───
    
    /// @notice Pause all token transfers and minting/burning
    /// @dev Only callable by PAUSER_ROLE. Useful during emergencies.
    function pause() external onlyRole(PAUSER_ROLE) whenNotPaused {
        _pause();
        emit EmergencyPause(msg.sender, "Emergency pause triggered");
    }

    /// @notice Resume token operations after emergency
    /// @dev Only callable by DEFAULT_ADMIN_ROLE to ensure careful review
    function unpause() external onlyRole(DEFAULT_ADMIN_ROLE) whenPaused {
        _unpause();
        emit EmergencyUnpause(msg.sender);
    }

    // ─── Minting Logic ───

    /// @notice Mint tokens. Only callable by addresses with MINTER_ROLE.
    /// @dev Enforces MAX_SUPPLY ceiling and pause protection.
    ///      Used by bridge for native-to-Arbitrum transfers.
    function mint(address to, uint256 amount) 
        external 
        onlyRole(MINTER_ROLE) 
        whenNotPaused 
        nonReentrant 
    {
        require(to != address(0), "UDAG: mint to zero address");
        require(amount > 0, "UDAG: mint amount must be positive");
        require(totalSupply() + amount <= MAX_SUPPLY, "UDAG: would exceed max supply");
        
        _mint(to, amount);
    }

    // ─── Burning Logic ───

    /// @notice Burn tokens from an address. Only callable by BURNER_ROLE.
    /// @dev Used by bridge when tokens move from Arbitrum → native chain.
    ///      Requires approval if burning from another address.
    function burn(address from, uint256 amount) 
        external 
        onlyRole(BURNER_ROLE) 
        whenNotPaused 
        nonReentrant 
    {
        require(from != address(0), "UDAG: burn from zero address");
        require(amount > 0, "UDAG: burn amount must be positive");
        
        if (from != msg.sender) {
            _spendAllowance(from, msg.sender, amount);
        }
        _burn(from, amount);
    }

    /// @notice Burn tokens from caller. Anyone can burn their own tokens.
    /// @dev Paused during emergencies for consistency with other token operations
    function burnSelf(uint256 amount) external whenNotPaused nonReentrant {
        require(amount > 0, "UDAG: burn amount must be positive");
        _burn(msg.sender, amount);
    }

    // ─── Genesis Finalization ───

    /// @notice Finalize genesis minting. After this, admin minter role is revoked.
    /// @dev Should be called after minting dev allocation + treasury.
    ///      Admin retains DEFAULT_ADMIN_ROLE for emergency functions.
    function finalizeGenesis() external onlyRole(DEFAULT_ADMIN_ROLE) whenNotPaused {
        require(!genesisFinalized, "UDAG: already finalized");
        
        genesisFinalized = true;
        _revokeRole(MINTER_ROLE, msg.sender); // Revoke from caller only
        
        emit GenesisFinalized(totalSupply(), msg.sender);
    }

    // ─── Bridge Management ───

    /// @notice Update the authorized bridge address
    /// @dev Only DEFAULT_ADMIN_ROLE. Transfers roles from old bridge to new.
    function updateBridge(address newBridge) external onlyRole(DEFAULT_ADMIN_ROLE) whenNotPaused {
        require(newBridge != address(0), "UDAG: new bridge cannot be zero");
        require(newBridge != bridgeAddress, "UDAG: same bridge address");
        
        address oldBridge = bridgeAddress;
        
        // Revoke roles from old bridge
        _revokeRole(MINTER_ROLE, oldBridge);
        _revokeRole(BURNER_ROLE, oldBridge);
        
        // Grant roles to new bridge
        _grantRole(MINTER_ROLE, newBridge);
        _grantRole(BURNER_ROLE, newBridge);
        
        bridgeAddress = newBridge;
        emit BridgeUpdated(oldBridge, newBridge);
    }

    // ─── Administrative Functions ───

    /// @notice Allow admin to voluntarily renounce DEFAULT_ADMIN_ROLE
    /// @dev IRREVERSIBLE. Use only when ready to fully decentralize.
    function renounceAdminRole() external onlyRole(DEFAULT_ADMIN_ROLE) {
        // Prevent renouncing if genesis not finalized (safety check)
        require(genesisFinalized, "UDAG: finalize genesis first");
        
        _revokeRole(DEFAULT_ADMIN_ROLE, msg.sender);
        _revokeRole(PAUSER_ROLE, msg.sender); // Also remove pause ability
        
        emit AdminRoleRenounced(msg.sender);
    }

    /// @notice Grant PAUSER_ROLE to a new address
    /// @dev Separates emergency pause capability from full admin powers
    function grantPauserRole(address account) external onlyRole(DEFAULT_ADMIN_ROLE) {
        require(account != address(0), "UDAG: zero address");
        _grantRole(PAUSER_ROLE, account);
    }

    /// @notice Revoke PAUSER_ROLE from an address
    function revokePauserRole(address account) external onlyRole(DEFAULT_ADMIN_ROLE) {
        _revokeRole(PAUSER_ROLE, account);
    }

    // ─── View Functions for Monitoring ───

    /// @notice Check if an address has minting privileges
    function isMinter(address account) external view returns (bool) {
        return hasRole(MINTER_ROLE, account);
    }

    /// @notice Check if an address has burning privileges
    function isBurner(address account) external view returns (bool) {
        return hasRole(BURNER_ROLE, account);
    }

    /// @notice Check if contract is currently paused
    function isPaused() external view returns (bool) {
        return paused();
    }

    /// @notice Get remaining mintable supply
    function remainingSupply() external view returns (uint256) {
        return MAX_SUPPLY - totalSupply();
    }

    // ─── Override _beforeTokenTransfer for pause enforcement ───
    /// @dev Pausable already hooks into _beforeTokenTransfer, 
    ///      but we explicitly note the integration here for auditors
    function _beforeTokenTransfer(
        address from,
        address to,
        uint256 amount
    ) internal override(ERC20) whenNotPaused {
        super._beforeTokenTransfer(from, to, amount);
    }

    // ─── Support for EIP-712 Domain (inherited from ERC20Permit) ───
    // No changes needed - OpenZeppelin handles domain separation correctly
}