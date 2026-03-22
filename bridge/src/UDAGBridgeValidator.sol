// SPDX-License-Identifier: BUSL-1.1
pragma solidity ^0.8.24;

import "./UDAGToken.sol";
import "@openzeppelin/contracts/utils/ReentrancyGuard.sol";
import "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";

/// @title UltraDAG Validator Federation Bridge - Hardened
/// @notice Bridge secured by UltraDAG validator set (2/3 threshold).
/// @dev No external relayers! DAG validators sign attestations as part of consensus.
///      Includes reentrancy guard, internal hash computation, and emergency controls.
contract UDAGBridgeValidator is ReentrancyGuard {
    using SafeERC20 for IERC20;
    
    UDAGToken public immutable token;
    address public governor;
    address public pendingGovernor;
    bool public paused;
    
    // ─── Validator Set Management ───
    mapping(address => bool) public isValidator;
    address[] public validators;
    uint256 public threshold; // 2/3 of validator count
    uint256 public constant MIN_VALIDATORS = 3; // Safety minimum
    uint256 public constant MAX_VALIDATORS = 100; // Prevent DoS via gas exhaustion
    bool public bridgeEnabled; // Bridge operations disabled until minimum validators set
    
    // ─── Replay Protection ───
    uint256 public nonce;
    mapping(uint256 => bool) public usedNonces;
    
    // ─── Rate Limiting ───
    uint256 public constant MAX_DEPOSIT = 100_000 * 10 ** 8; // 100K UDAG per tx
    uint256 public constant DAILY_WITHDRAWAL_LIMIT = 500_000 * 10 ** 8; // 500K UDAG/day
    mapping(uint256 => uint256) public dailyWithdrawalVolume; // date → volume
    
    // ─── Events ───
    event DepositMade(
        address indexed sender,
        bytes20 indexed nativeRecipient,
        uint256 amount,
        uint256 indexed nonce
    );

    event WithdrawalClaimed(
        bytes20 indexed sender,
        address indexed recipient,
        uint256 amount,
        uint256 indexed nonce
    );

    event ValidatorAdded(address indexed validator);
    event ValidatorRemoved(address indexed validator);
    event ThresholdUpdated(uint256 newThreshold);
    event EmergencyWithdrawal(address indexed token, address indexed to, uint256 amount);
    event BridgePaused(address indexed pausedBy);
    event BridgeUnpaused(address indexed unpausedBy);
    event PendingGovernorSet(address indexed currentGovernor, address indexed pendingGovernor);
    event GovernorAccepted(address indexed oldGovernor, address indexed newGovernor);
    event BridgeMigration(address indexed newBridge, uint256 amount);
    event ETHReceived(address indexed sender, uint256 amount);
    event BridgeEnabled();
    
    // ─── Errors ───
    error BridgeIsPaused();
    error AmountTooLarge();
    error InvalidRecipient();
    error NotGovernor();
    error InvalidSignature();
    error InsufficientSignatures();
    error NonceAlreadyUsed();
    error InvalidValidatorSet();
    error DailyLimitExceeded();
    error MinValidatorsNotMet();
    error SignatureNotSorted();
    error MalleableSignature();
    error NotPendingGovernor();
    error NotPaused();
    
    // ─── Modifiers ───
    modifier onlyGovernor() {
        if (msg.sender != governor) revert NotGovernor();
        _;
    }
    
    modifier whenNotPaused() {
        if (paused) revert BridgeIsPaused();
        _;
    }
    
    // ─── Constructor ───
    constructor(address _token, address _governor) {
        if (_token == address(0) || _governor == address(0)) revert InvalidRecipient();
        token = UDAGToken(_token);
        governor = _governor;
    }
    
    // ─── Bridge: Arbitrum → Native (Deposit) ───

    /// @notice Deposit UDAG for bridging to native chain.
    /// @dev Validators will sign attestation on native chain.
    ///      Bridge must be enabled (minimum validators set).
    function deposit(bytes20 nativeRecipient, uint256 amount)
        external
        whenNotPaused
        nonReentrant
    {
        if (!bridgeEnabled) revert InvalidValidatorSet();
        if (nativeRecipient == bytes20(0)) revert InvalidRecipient();
        if (amount == 0 || amount > MAX_DEPOSIT) revert AmountTooLarge();

        // Transfer tokens into bridge escrow using SafeERC20
        IERC20(address(token)).safeTransferFrom(msg.sender, address(this), amount);

        uint256 depositNonce = nonce++;

        emit DepositMade(msg.sender, nativeRecipient, amount, depositNonce);
    }
    
    // ─── Bridge: Native → Arbitrum (Claim Withdrawal) ───
    
    /// @notice Claim withdrawal on Arbitrum with validator signatures.
    /// @dev Requires 2/3+ validator signatures (BFT threshold).
    ///      Message hash is computed internally to prevent forgery.
    ///      Bridge must be enabled (minimum validators set).
    /// @param sender Original sender on native chain
    /// @param recipient Recipient on Arbitrum
    /// @param amount Amount to mint
    /// @param depositNonce Unique nonce for this withdrawal
    /// @param signatures Concatenated signatures (65 bytes each: r, s, v)
    function claimWithdrawal(
        bytes20 sender,
        address recipient,
        uint256 amount,
        uint256 depositNonce,
        bytes calldata signatures
    )
        external
        whenNotPaused
        nonReentrant
    {
        if (!bridgeEnabled) revert InvalidValidatorSet();
        if (recipient == address(0)) revert InvalidRecipient();
        if (amount == 0 || amount > MAX_DEPOSIT) revert AmountTooLarge();
        if (usedNonces[depositNonce]) revert NonceAlreadyUsed();

        // Rate limiting: check daily withdrawal volume
        uint256 today = block.timestamp / 1 days;
        if (dailyWithdrawalVolume[today] + amount > DAILY_WITHDRAWAL_LIMIT) {
            revert DailyLimitExceeded();
        }

        // ⚠️ CRITICAL: Compute message hash internally - NEVER trust user-provided hash
        bytes32 messageHash = _getMessageHashInternal(
            sender,
            recipient,
            amount,
            depositNonce
        );

        // Verify threshold signatures from validators
        _verifyThresholdSignatures(messageHash, signatures);

        // Mark nonce as used AND update daily volume
        usedNonces[depositNonce] = true;
        // Safe: DAILY_WITHDRAWAL_LIMIT (500K UDAG = 5e13) << type(uint256).max (1.1e77)
        // Overflow would require 2.2e63 days of max withdrawals (~6e60 years)
        unchecked {
            dailyWithdrawalVolume[today] += amount;
        }
        
        // Mint tokens to recipient
        token.mint(recipient, amount);

        emit WithdrawalClaimed(sender, recipient, amount, depositNonce);
    }
    
    // ─── Internal: Message Hash Computation ───
    
    /// @notice Compute message hash internally (prevents forgery attacks).
    function _getMessageHashInternal(
        bytes20 sender,
        address recipient,
        uint256 amount,
        uint256 depositNonce
    ) internal view returns (bytes32) {
        return keccak256(abi.encode(
            "claimWithdrawal",
            block.chainid,        // Domain separation by chain
            address(this),        // Bridge address for replay protection across deployments
            sender,
            recipient,
            amount,
            depositNonce
        ));
    }
    
    /// @notice Public view function for off-chain signature generation.
    /// @dev Validators must sign the returned hash using eth_sign (EIP-191),
    ///      which automatically adds "\x19Ethereum Signed Message:\n32" prefix.
    function getMessageHash(
        bytes20 sender,
        address recipient,
        uint256 amount,
        uint256 depositNonce
    ) external view returns (bytes32) {
        return _getMessageHashInternal(sender, recipient, amount, depositNonce);
    }
    
    // ─── Signature Verification ───
    
    /// @notice Verify threshold signatures from validators.
    /// @dev Enforces: unique signers, sorted order, validator membership.
    ///      Uses EIP-191 personal sign prefix for ecrecover.
    ///      Validators must sign using eth_sign (which adds the prefix automatically).
    function _verifyThresholdSignatures(
        bytes32 messageHash,
        bytes calldata signatures
    ) internal view {
        // Validate signature length (each signature is exactly 65 bytes)
        if (signatures.length % 65 != 0) revert InvalidSignature();

        uint256 sigCount = signatures.length / 65;
        if (sigCount < threshold) revert InsufficientSignatures();
        if (sigCount > validators.length) revert InvalidSignature();

        // EIP-191 personal sign prefix
        bytes32 ethSignedMessageHash = keccak256(
            abi.encodePacked("\x19Ethereum Signed Message:\n32", messageHash)
        );

        address lastSigner = address(0);
        uint256 validCount = 0;

        for (uint256 i = 0; i < sigCount; i++) {
            // Extract signature components safely
            bytes32 r;
            bytes32 s;
            uint8 v;
            uint256 offset = i * 65;

            assembly {
                r := calldataload(add(signatures.offset, offset))
                s := calldataload(add(signatures.offset, add(offset, 32)))
                v := byte(0, calldataload(add(signatures.offset, add(offset, 64))))
            }

            // M5: Reject malleable signatures (s-value upper half)
            if (uint256(s) > 0x7FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF5D576E7357A4501DDFE92F46681B20A0) revert MalleableSignature();

            // Reconstruct signer address from EIP-191 prefixed hash
            address signer;
            if (v == 27 || v == 28) {
                signer = ecrecover(ethSignedMessageHash, v, r, s);
            } else if (v == 0 || v == 1) {
                // EIP-155 chain-specific v value - normalize to 27/28
                signer = ecrecover(ethSignedMessageHash, v + 27, r, s);
            } else {
                revert InvalidSignature();
            }

            if (signer == address(0)) revert InvalidSignature();

            // Verify signer is an active validator
            if (!isValidator[signer]) continue;

            // Enforce unique, sorted signers to prevent double-counting attacks
            if (signer <= lastSigner) revert SignatureNotSorted();
            lastSigner = signer;
            validCount++;
        }

        if (validCount < threshold) revert InsufficientSignatures();
    }
    
    // ─── Validator Management ───

    /// @notice Add a new validator to the set.
    /// @dev Bridge is automatically enabled when minimum validators reached.
    function addValidator(address validator) external onlyGovernor whenNotPaused {
        if (validator == address(0)) revert InvalidRecipient();
        if (isValidator[validator]) revert InvalidValidatorSet();
        if (validators.length >= MAX_VALIDATORS) revert InvalidValidatorSet();

        isValidator[validator] = true;
        validators.push(validator);

        _updateThreshold();
        
        // Enable bridge operations when minimum validators reached
        if (!bridgeEnabled && validators.length >= MIN_VALIDATORS) {
            bridgeEnabled = true;
            emit BridgeEnabled();
        }
        
        emit ValidatorAdded(validator);
    }
    
    /// @notice Remove a validator from the set.
    /// @dev Enforces MIN_VALIDATORS to prevent centralization attacks.
    function removeValidator(address validator) external onlyGovernor whenNotPaused {
        if (!isValidator[validator]) revert InvalidValidatorSet();
        if (validators.length <= MIN_VALIDATORS) revert MinValidatorsNotMet();
        
        isValidator[validator] = false;
        
        // Efficient removal: swap with last element then pop
        for (uint256 i = 0; i < validators.length; i++) {
            if (validators[i] == validator) {
                validators[i] = validators[validators.length - 1];
                validators.pop();
                break;
            }
        }
        
        _updateThreshold();
        emit ValidatorRemoved(validator);
    }
    
    /// @notice Update threshold to strictly >2/3 of validator count.
    /// @dev Formula: floor(2n/3) + 1, which gives:
    ///      n=3: 3 (unanimous), n=4: 3, n=5: 4, n=21: 15
    function _updateThreshold() internal {
        uint256 validatorCount = validators.length;
        if (validatorCount < MIN_VALIDATORS) {
            threshold = validatorCount; // Allow bootstrapping
        } else {
            // Strict BFT: floor(2n/3) + 1 ensures threshold > 2n/3
            threshold = (2 * validatorCount) / 3 + 1;
        }
        emit ThresholdUpdated(threshold);
    }
    
    /// @notice Set custom threshold (emergency/testing only).
    /// @dev Enforces BFT minimum of ceil(2n/3) when enough validators exist.
    function setThreshold(uint256 newThreshold) external onlyGovernor whenNotPaused {
        if (newThreshold == 0 || newThreshold > validators.length) revert InvalidValidatorSet();
        // Enforce BFT minimum when validator count is at or above MIN_VALIDATORS
        if (validators.length >= MIN_VALIDATORS) {
            uint256 bftMinimum = (2 * validators.length) / 3 + 1;
            require(newThreshold >= bftMinimum, "Below BFT minimum");
        }
        threshold = newThreshold;
        emit ThresholdUpdated(newThreshold);
    }
    
    // ─── Admin Functions ───

    function pause() external onlyGovernor {
        paused = true;
        emit BridgePaused(msg.sender);
    }

    function unpause() external onlyGovernor {
        paused = false;
        emit BridgeUnpaused(msg.sender);
    }
    
    /// @notice Initiate governor transfer (2-step).
    /// @dev New governor must call acceptGovernor() to complete the transfer.
    function setGovernor(address newGovernor) external onlyGovernor {
        if (newGovernor == address(0)) revert InvalidRecipient();
        pendingGovernor = newGovernor;
        emit PendingGovernorSet(governor, newGovernor);
    }

    /// @notice Accept governor role. Only callable by pendingGovernor.
    function acceptGovernor() external {
        if (msg.sender != pendingGovernor) revert NotPendingGovernor();
        address old = governor;
        governor = pendingGovernor;
        pendingGovernor = address(0);
        emit GovernorAccepted(old, governor);
    }
    
    // ─── Bridge Migration ───

    /// @notice Migrate escrowed UDAG to a new bridge contract.
    /// @dev Only callable by governor when paused (safety measure).
    ///      Intended to be called via timelock for operational safety.
    function migrateToNewBridge(address newBridge, uint256 amount) external onlyGovernor nonReentrant {
        if (!paused) revert NotPaused();
        if (newBridge == address(0)) revert InvalidRecipient();

        IERC20(address(token)).safeTransfer(newBridge, amount);
        emit BridgeMigration(newBridge, amount);
    }

    // ─── Emergency Recovery ───
    
    /// @notice Emergency withdrawal of stuck ERC20 tokens.
    /// @dev Should be gated by timelock/multisig in production.
    ///      Only for tokens OTHER than the managed UDAG token.
    ///      Uses SafeERC20 for compatibility with non-standard tokens (e.g., USDT).
    function emergencyWithdrawERC20(
        address tokenAddress,
        address to,
        uint256 amount
    ) external onlyGovernor {
        if (tokenAddress == address(token)) revert InvalidRecipient(); // Prevent draining managed token
        if (to == address(0)) revert InvalidRecipient();

        IERC20(tokenAddress).safeTransfer(to, amount);
        emit EmergencyWithdrawal(tokenAddress, to, amount);
    }
    
    /// @notice Emergency withdrawal of native ETH (if any).
    function emergencyWithdrawETH(address payable to) external onlyGovernor nonReentrant {
        if (to == address(0)) revert InvalidRecipient();
        uint256 balance = address(this).balance;
        if (balance == 0) return;
        
        // Use call instead of transfer for forward compatibility
        (bool success, ) = to.call{value: balance}("");
        if (!success) revert InvalidRecipient();
        
        emit EmergencyWithdrawal(address(0), to, balance);
    }
    
    // ─── View Functions ───
    
    function getValidatorCount() external view returns (uint256) {
        return validators.length;
    }
    
    function getAllValidators() external view returns (address[] memory) {
        return validators;
    }
    
    function getThreshold() external view returns (uint256) {
        return threshold;
    }
    
    function isNonceUsed(uint256 _nonce) external view returns (bool) {
        return usedNonces[_nonce];
    }
    
    function getDailyWithdrawalVolume(uint256 date) external view returns (uint256) {
        return dailyWithdrawalVolume[date];
    }
    
    // ─── Receive ETH (prevent accidental sends) ───
    receive() external payable {
        emit ETHReceived(msg.sender, msg.value);
    }
}