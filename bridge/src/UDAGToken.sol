// SPDX-License-Identifier: BUSL-1.1
pragma solidity ^0.8.24;

import "@openzeppelin/contracts/token/ERC20/ERC20.sol";
import "@openzeppelin/contracts/token/ERC20/extensions/ERC20Permit.sol";
import "@openzeppelin/contracts/access/AccessControl.sol";

/// @title UltraDAG Token (UDAG)
/// @notice ERC-20 representation of UDAG on Arbitrum.
/// @dev 8 decimals to match the native chain (1 UDAG = 100,000,000 sats).
///      The bridge contract holds MINTER_ROLE and BURNER_ROLE.
contract UDAGToken is ERC20, ERC20Permit, AccessControl {
    bytes32 public constant MINTER_ROLE = keccak256("MINTER_ROLE");
    bytes32 public constant BURNER_ROLE = keccak256("BURNER_ROLE");

    /// @notice Maximum supply: 21,000,000 UDAG with 8 decimal places.
    uint256 public constant MAX_SUPPLY = 21_000_000 * 10 ** 8;

    constructor(address admin) ERC20("UltraDAG", "UDAG") ERC20Permit("UltraDAG") {
        _grantRole(DEFAULT_ADMIN_ROLE, admin);
        _grantRole(MINTER_ROLE, admin);
    }

    function decimals() public pure override returns (uint8) {
        return 8;
    }

    /// @notice Mint tokens. Only callable by addresses with MINTER_ROLE.
    /// @dev Enforces MAX_SUPPLY ceiling. Used by bridge for native-to-Arbitrum transfers
    ///      and by admin for initial genesis allocation minting.
    function mint(address to, uint256 amount) external onlyRole(MINTER_ROLE) {
        require(totalSupply() + amount <= MAX_SUPPLY, "UDAG: exceeds max supply");
        _mint(to, amount);
    }

    /// @notice Burn tokens from an address. Only callable by addresses with BURNER_ROLE.
    /// @dev Used by bridge when tokens are bridged from Arbitrum to native chain.
    ///      The caller (bridge contract) must have approval or be the token holder.
    function burn(address from, uint256 amount) external onlyRole(BURNER_ROLE) {
        if (from != msg.sender) {
            _spendAllowance(from, msg.sender, amount);
        }
        _burn(from, amount);
    }

    /// @notice Burn tokens from caller. Anyone can burn their own tokens.
    function burnSelf(uint256 amount) external {
        _burn(msg.sender, amount);
    }

    // ─── Genesis Finalization ───

    bool public genesisFinalized;

    /// @notice Finalize genesis minting. After this, only the bridge can mint.
    /// @dev Should be called after minting dev allocation + treasury.
    function finalizeGenesis() external onlyRole(DEFAULT_ADMIN_ROLE) {
        require(!genesisFinalized, "already finalized");
        genesisFinalized = true;
        _revokeRole(MINTER_ROLE, msg.sender);
        emit GenesisFinalized(totalSupply());
    }

    event GenesisFinalized(uint256 totalSupply);
}
