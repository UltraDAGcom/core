// SPDX-License-Identifier: BUSL-1.1
pragma solidity ^0.8.24;

import "./UDAGToken.sol";

/// @title UltraDAG Validator Federation Bridge
/// @notice Bridge secured by UltraDAG validator set (2/3 threshold).
/// @dev No external relayers! DAG validators sign attestations as part of consensus.
contract UDAGBridgeValidator {
    UDAGToken public immutable token;
    address public governor;
    bool public paused;
    
    // Validator set management
    mapping(address => bool) public isValidator;
    address[] public validators;
    uint256 public threshold; // 2/3 of validator count
    
    // Nonce tracking (prevent replay)
    uint256 public nonce;
    mapping(uint256 => bool) public usedNonces;
    
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
    
    // ─── Errors ───
    
    error BridgePaused();
    error AmountTooLarge();
    error InvalidRecipient();
    error NotGovernor();
    error InvalidSignature();
    error InsufficientSignatures();
    error NonceAlreadyUsed();
    error InvalidValidatorSet();
    
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
    /// @dev Validators will sign attestation on native chain.
    function deposit(bytes20 nativeRecipient, uint256 amount) external whenNotPaused {
        if (nativeRecipient == bytes20(0)) revert InvalidRecipient();
        if (amount == 0 || amount > MAX_DEPOSIT) revert AmountTooLarge();
        
        // Escrow tokens
        token.transferFrom(msg.sender, address(this), amount);
        
        uint256 depositNonce = nonce++;
        
        emit DepositMade(msg.sender, nativeRecipient, amount, depositNonce);
    }
    
    // ─── Bridge: Native → Arbitrum ───
    
    /// @notice Claim withdrawal on Arbitrum with validator signatures.
    /// @dev Requires 2/3+ validator signatures (BFT threshold).
    /// @param sender Original sender on native chain
    /// @param recipient Recipient on Arbitrum
    /// @param amount Amount to mint
    /// @param depositNonce Unique nonce for this withdrawal
    /// @param signatures Concatenated signatures (65 bytes each: r, s, v)
    /// @param messageHash Hash of the message that was signed
    function claimWithdrawal(
        bytes20 sender,
        address recipient,
        uint256 amount,
        uint256 depositNonce,
        bytes calldata signatures,
        bytes32 messageHash
    ) external whenNotPaused {
        if (recipient == address(0)) revert InvalidRecipient();
        if (amount == 0 || amount > MAX_DEPOSIT) revert AmountTooLarge();
        if (usedNonces[depositNonce]) revert NonceAlreadyUsed();
        
        // Verify threshold signatures from validators
        _verifyThresholdSignatures(messageHash, signatures, amount);
        
        // Mark nonce as used
        usedNonces[depositNonce] = true;
        
        // Mint tokens to recipient
        token.mint(recipient, amount);
        
        emit WithdrawalClaimed(sender, recipient, amount, depositNonce);
    }
    
    // ─── Validator Management ───
    
    /// @notice Add a new validator to the set.
    function addValidator(address validator) external onlyGovernor {
        require(validator != address(0), "zero address");
        require(!isValidator[validator], "already validator");
        
        isValidator[validator] = true;
        validators.push(validator);
        
        // Update threshold to 2/3
        _updateThreshold();
        
        emit ValidatorAdded(validator);
    }
    
    /// @notice Remove a validator from the set.
    function removeValidator(address validator) external onlyGovernor {
        require(isValidator[validator], "not validator");
        
        isValidator[validator] = false;
        
        // Remove from array
        for (uint256 i = 0; i < validators.length; i++) {
            if (validators[i] == validator) {
                validators[i] = validators[validators.length - 1];
                validators.pop();
                break;
            }
        }
        
        // Update threshold to 2/3
        _updateThreshold();
        
        emit ValidatorRemoved(validator);
    }
    
    /// @notice Update threshold to 2/3 of validator count.
    function _updateThreshold() internal {
        uint256 validatorCount = validators.length;
        if (validatorCount < 3) {
            // Allow adding validators until we have at least 3
            threshold = validatorCount;
        } else {
            // Threshold = ceil(2/3 * validatorCount)
            threshold = (2 * validatorCount + 2) / 3;
        }
        
        emit ThresholdUpdated(threshold);
    }
    
    /// @notice Set custom threshold (for testing/emergency).
    function setThreshold(uint256 newThreshold) external onlyGovernor {
        require(newThreshold > 0 && newThreshold <= validators.length, "invalid threshold");
        threshold = newThreshold;
        emit ThresholdUpdated(newThreshold);
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
    
    function getValidatorCount() external view returns (uint256) {
        return validators.length;
    }
    
    function getAllValidators() external view returns (address[] memory) {
        return validators;
    }
    
    function getThreshold() external view returns (uint256) {
        return threshold;
    }
    
    // ─── Internal ───
    
    /// @notice Verify threshold signatures from validators.
    function _verifyThresholdSignatures(
        bytes32 messageHash,
        bytes calldata signatures,
        uint256 /* amount */
    ) internal view {
        uint256 sigCount = signatures.length / 65;
        if (sigCount < threshold) revert InsufficientSignatures();
        
        address lastSigner = address(0);
        uint256 validCount = 0;
        
        for (uint256 i = 0; i < sigCount; i++) {
            bytes32 r;
            bytes32 s;
            uint8 v;
            uint256 offset = i * 65;
            
            assembly {
                r := calldataload(add(signatures.offset, offset))
                s := calldataload(add(signatures.offset, add(offset, 32)))
                v := byte(0, calldataload(add(signatures.offset, add(offset, 64))))
            }
            
            // Reject malleable signatures (EIP-2)
            require(uint256(s) <= 0x7FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF5D576E7357A4501DDFE92F46681B20A0, "malleable sig");
            
            address signer = ecrecover(messageHash, v, r, s);
            if (signer == address(0)) revert InvalidSignature();
            
            // Verify signer is a validator
            if (!isValidator[signer]) continue;
            
            // Enforce unique, sorted signers (prevent double-counting)
            require(signer > lastSigner, "signatures not sorted");
            lastSigner = signer;
            validCount++;
        }
        
        if (validCount < threshold) revert InsufficientSignatures();
    }
    
    /// @notice Hash message for signing (native chain → Arbitrum).
    function getMessageHash(
        bytes20 sender,
        address recipient,
        uint256 amount,
        uint256 depositNonce,
        uint256 chainId
    ) public pure returns (bytes32) {
        return keccak256(abi.encode(
            "claimWithdrawal",
            chainId,
            sender,
            recipient,
            amount,
            depositNonce
        ));
    }
}
