// SPDX-License-Identifier: BUSL-1.1
pragma solidity ^0.8.24;

import "./UDAGToken.sol";

/// @title UltraDAG Bridge (Optimistic - No Relayers!)
/// @notice Simple optimistic bridge with 7-day challenge period.
/// @dev No relayers needed! Users wait 7 days to withdraw.
contract UDAGBridgeOptimistic {
    UDAGToken public immutable token;
    address public governor;
    bool public paused;
    
    uint256 public constant CHALLENGE_PERIOD = 7 days;
    uint256 public nonce;
    
    struct Deposit {
        address sender;
        bytes20 nativeRecipient;
        uint256 amount;
        uint256 timestamp;
        bool claimed;
    }
    
    mapping(uint256 => Deposit) public deposits;
    mapping(uint256 => bool) public withdrawalNonces; // Prevent replay
    
    // ─── Events ───
    
    event DepositMade(
        address indexed sender,
        bytes20 indexed nativeRecipient,
        uint256 amount,
        uint256 indexed depositNonce
    );
    
    event WithdrawalClaimed(
        address indexed sender,
        uint256 amount,
        uint256 indexed depositNonce
    );
    
    // ─── Errors ───
    
    error BridgePaused();
    error AmountTooLarge();
    error InvalidRecipient();
    error NotGovernor();
    error AlreadyClaimed();
    error ChallengePeriodNotPassed();
    
    // ─── Modifiers ───
    
    modifier onlyGovernor() {
        if (msg.sender != governor) revert NotGovernor();
        _;
    }
    
    modifier whenNotPaused() {
        if (paused) revert BridgePaused();
        _;
    }
    
    // ─── Constants ───
    
    uint256 public constant MAX_DEPOSIT = 100_000 * 10 ** 8; // 100K UDAG
    
    // ─── Constructor ───
    
    constructor(address _token, address _governor) {
        token = UDAGToken(_token);
        governor = _governor;
    }
    
    // ─── Bridge: Arbitrum → Native ───
    
    /// @notice Deposit UDAG for bridging to native chain.
    /// @dev User must wait 7 days before claiming on native side.
    function deposit(bytes20 nativeRecipient, uint256 amount) external whenNotPaused {
        if (nativeRecipient == bytes20(0)) revert InvalidRecipient();
        if (amount == 0 || amount > MAX_DEPOSIT) revert AmountTooLarge();
        
        // Escrow tokens
        token.transferFrom(msg.sender, address(this), amount);
        
        uint256 depositNonce = nonce++;
        deposits[depositNonce] = Deposit({
            sender: msg.sender,
            nativeRecipient: nativeRecipient,
            amount: amount,
            timestamp: block.timestamp,
            claimed: false
        });
        
        emit DepositMade(msg.sender, nativeRecipient, amount, depositNonce);
    }
    
    // ─── Bridge: Native → Arbitrum ───
    
    /// @notice Claim withdrawal after 7-day challenge period.
    /// @dev No relayers needed! Optimistic verification.
    function claimWithdrawal(uint256 depositNonce) external whenNotPaused {
        Deposit storage deposit = deposits[depositNonce];
        
        if (deposit.sender == address(0)) revert InvalidRecipient();
        if (deposit.claimed) revert AlreadyClaimed();
        if (deposit.sender != msg.sender) revert NotGovernor();
        if (block.timestamp < deposit.timestamp + CHALLENGE_PERIOD) {
            revert ChallengePeriodNotPassed();
        }
        
        deposit.claimed = true;
        
        // Mint tokens directly (bridge must have MINTER_ROLE)
        token.mint(deposit.sender, deposit.amount);
        
        emit WithdrawalClaimed(deposit.sender, deposit.amount, depositNonce);
    }
    
    // ─── Admin Functions ───
    
    function pause() external onlyGovernor {
        paused = true;
    }
    
    function unpause() external onlyGovernor {
        paused = false;
    }
    
    function setGovernor(address newGovernor) external onlyGovernor {
        if (newGovernor == address(0)) revert InvalidRecipient();
        governor = newGovernor;
    }
    
    // ─── View Functions ───
    
    function getDeposit(uint256 depositNonce) external view returns (Deposit memory) {
        return deposits[depositNonce];
    }
    
    function canClaim(uint256 depositNonce) external view returns (bool) {
        Deposit storage deposit = deposits[depositNonce];
        return !deposit.claimed && 
               block.timestamp >= deposit.timestamp + CHALLENGE_PERIOD;
    }
    
    function timeUntilClaimable(uint256 depositNonce) external view returns (uint256) {
        Deposit storage deposit = deposits[depositNonce];
        uint256 claimableAt = deposit.timestamp + CHALLENGE_PERIOD;
        if (block.timestamp >= claimableAt) return 0;
        return claimableAt - block.timestamp;
    }
}
