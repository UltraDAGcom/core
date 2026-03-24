// SPDX-License-Identifier: BUSL-1.1
pragma solidity ^0.8.24;

import "./UDAGToken.sol";
import "@openzeppelin/contracts/utils/ReentrancyGuard.sol";
import "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";

/// @title UltraDAG Validator Federation Bridge
/// @notice Bridge secured by UltraDAG validator set (strict >2/3 BFT threshold).
/// @dev No external relayers — DAG validators sign attestations as part of consensus.
///
///      Bridge model:
///        • Deposit  (Arbitrum → native): user locks UDAG in escrow via transferFrom.
///        • Withdraw (native → Arbitrum): validators attest, bridge mints UDAG to recipient.
///
///      Security measures:
///        • Reentrancy guard on all state-mutating external functions.
///        • Internal hash computation (never trust user-provided hashes).
///        • EIP-191 personal sign prefix for ecrecover.
///        • Strict signature ordering with no gaps — every signature must recover
///          to a valid validator; non-validator signers cause a revert, not a skip.
///        • Rate limiting (per-tx and daily caps) on deposits and withdrawals.
///        • Two-step governor transfer.
///        • Bridge auto-enables once MIN_VALIDATORS are registered.
contract UDAGBridgeValidator is ReentrancyGuard {
    using SafeERC20 for IERC20;
    UDAGToken public immutable token;
    address   public governor;
    address   public pendingGovernor;
    bool      public paused;

    // ─── Validator Set Management ────────────────────────────────────────
    mapping(address => bool) public isValidator;
    address[] public validators;
    uint256   public threshold;

    uint256 public constant MIN_VALIDATORS = 3;
    uint256 public constant MAX_VALIDATORS = 100;
    bool    public bridgeEnabled;

    // ─── Replay Protection ──────────────────────────────────────────────
    /// @dev Monotonic counter used to assign deposit nonces.
    uint256 public depositNonceCounter;
    /// @dev Guards withdrawal replay: each native-chain nonce may only be claimed once.
    mapping(uint256 => bool) public usedWithdrawalNonces;

    // ─── Rate Limiting ──────────────────────────────────────────────────
    uint256 public constant MIN_AMOUNT       = 1 * 10 ** 8;        // 1 UDAG
    uint256 public constant MAX_AMOUNT       = 100_000 * 10 ** 8;  // 100K UDAG per tx
    uint256 public constant DAILY_LIMIT      = 500_000 * 10 ** 8;  // 500K UDAG per day

    mapping(uint256 => uint256) public dailyWithdrawalVolume; // day-index → volume
    mapping(uint256 => uint256) public dailyDepositVolume;    // day-index → volume

    // ─── Events ─────────────────────────────────────────────────────────
    event DepositMade(
        address indexed sender,
        bytes20 indexed nativeRecipient,
        uint256 amount,
        uint256 indexed depositNonce,
        uint256 sourceChainId
    );

    event WithdrawalClaimed(
        bytes20 indexed nativeSender,
        address indexed recipient,
        uint256 amount,
        uint256 indexed withdrawalNonce
    );

    event ValidatorAdded(address indexed validator);
    event ValidatorRemoved(address indexed validator);
    event ThresholdUpdated(uint256 newThreshold);
    event EmergencyWithdrawal(address indexed tokenAddr, address indexed to, uint256 amount);
    event BridgePaused(address indexed pausedBy);
    event BridgeUnpaused(address indexed unpausedBy);
    event PendingGovernorSet(address indexed currentGovernor, address indexed pendingGovernor);
    event GovernorAccepted(address indexed oldGovernor, address indexed newGovernor);
    event BridgeMigration(address indexed newBridge, uint256 amount);
    event ETHReceived(address indexed sender, uint256 amount);
    event BridgeEnabled();

    // ─── Errors ─────────────────────────────────────────────────────────
    error ZeroAddress(string param);
    error BridgeIsPaused();
    error BridgeNotPaused();
    error BridgeNotEnabled();
    error AmountBelowMinimum(uint256 amount, uint256 minimum);
    error AmountAboveMaximum(uint256 amount, uint256 maximum);
    error DailyLimitExceeded(uint256 requested, uint256 remaining);
    error NonceAlreadyUsed(uint256 nonce);
    error NotGovernor();
    error NotPendingGovernor();
    error ValidatorAlreadyRegistered(address validator);
    error ValidatorNotRegistered(address validator);
    error ValidatorSetFull();
    error BelowMinValidators(uint256 current, uint256 minimum);
    error BelowBFTMinimum(uint256 proposed, uint256 bftMinimum);
    error InvalidSignatureLength(uint256 length);
    error TooFewSignatures(uint256 provided, uint256 required);
    error TooManySignatures(uint256 provided, uint256 validatorCount);
    error MalleableSignature(uint256 index);
    error InvalidVValue(uint256 index, uint8 v);
    error RecoveredZeroAddress(uint256 index);
    error SignerNotValidator(uint256 index, address signer);
    error SignersNotSorted(uint256 index, address current, address previous);
    error CannotDrainManagedToken();
    error ETHTransferFailed();

    // ─── Modifiers ──────────────────────────────────────────────────────
    modifier onlyGovernor() {
        if (msg.sender != governor) revert NotGovernor();
        _;
    }

    modifier whenNotPaused() {
        if (paused) revert BridgeIsPaused();
        _;
    }

    modifier whenPaused() {
        if (!paused) revert BridgeNotPaused();
        _;
    }

    // ─── Constructor ────────────────────────────────────────────────────
    constructor(address _token, address _governor) {
        if (_token == address(0))    revert ZeroAddress("token");
        if (_governor == address(0)) revert ZeroAddress("governor");
        token    = UDAGToken(_token);
        governor = _governor;
    }

    // ═══════════════════════════════════════════════════════════════════
    //  BRIDGE OPERATIONS
    // ═══════════════════════════════════════════════════════════════════

    /// @notice Lock UDAG in escrow for bridging to the native chain.
    /// @param nativeRecipient 20-byte address on the native UDAG chain.
    /// @param amount          Amount of UDAG (8-decimal) to bridge.
    function deposit(bytes20 nativeRecipient, uint256 amount)
        external
        whenNotPaused
        nonReentrant
    {
        if (!bridgeEnabled)               revert BridgeNotEnabled();
        if (nativeRecipient == bytes20(0)) revert ZeroAddress("nativeRecipient");
        _validateAmount(amount);

        uint256 today = block.timestamp / 1 days;
        uint256 usedToday = dailyDepositVolume[today];
        if (usedToday + amount > DAILY_LIMIT) {
            revert DailyLimitExceeded(amount, DAILY_LIMIT - usedToday);
        }

        IERC20(address(token)).safeTransferFrom(msg.sender, address(this), amount);

        // Safe: DAILY_LIMIT (5e13) << type(uint256).max
        unchecked { dailyDepositVolume[today] = usedToday + amount; }

        uint256 thisNonce = depositNonceCounter++;

        emit DepositMade(msg.sender, nativeRecipient, amount, thisNonce, block.chainid);
    }

    /// @notice Claim a withdrawal on Arbitrum backed by validator attestations.
    /// @param nativeSender    Original sender on the native chain.
    /// @param recipient       Recipient on Arbitrum.
    /// @param amount          Amount to mint.
    /// @param withdrawalNonce Unique nonce for this withdrawal (assigned on native chain).
    /// @param signatures      Concatenated 65-byte ECDSA signatures (r‖s‖v), sorted by
    ///                        recovered signer address ascending. Every signature must
    ///                        recover to a registered validator.
    function claimWithdrawal(
        bytes20 nativeSender,
        address recipient,
        uint256 amount,
        uint256 withdrawalNonce,
        bytes calldata signatures
    )
        external
        whenNotPaused
        nonReentrant
    {
        if (!bridgeEnabled)                    revert BridgeNotEnabled();
        if (recipient == address(0))           revert ZeroAddress("recipient");
        if (usedWithdrawalNonces[withdrawalNonce]) revert NonceAlreadyUsed(withdrawalNonce);
        _validateAmount(amount);

        uint256 today = block.timestamp / 1 days;
        uint256 usedToday = dailyWithdrawalVolume[today];
        if (usedToday + amount > DAILY_LIMIT) {
            revert DailyLimitExceeded(amount, DAILY_LIMIT - usedToday);
        }

        bytes32 messageHash = _computeWithdrawalHash(
            nativeSender, recipient, amount, withdrawalNonce
        );

        _verifyThresholdSignatures(messageHash, signatures);

        usedWithdrawalNonces[withdrawalNonce] = true;
        unchecked { dailyWithdrawalVolume[today] = usedToday + amount; }

        token.mint(recipient, amount);

        emit WithdrawalClaimed(nativeSender, recipient, amount, withdrawalNonce);
    }

    // ═══════════════════════════════════════════════════════════════════
    //  MESSAGE HASH
    // ═══════════════════════════════════════════════════════════════════

    /// @dev Internal hash — never expose raw hashes to callers for signing.
    function _computeWithdrawalHash(
        bytes20 nativeSender,
        address recipient,
        uint256 amount,
        uint256 withdrawalNonce
    ) internal view returns (bytes32) {
        return keccak256(abi.encode(
            "UDAGBridge::claimWithdrawal",  // Domain tag
            block.chainid,                   // Chain separation
            address(this),                   // Deployment separation
            nativeSender,
            recipient,
            amount,
            withdrawalNonce
        ));
    }

    /// @notice Off-chain helper: returns the hash validators must sign.
    /// @dev Validators sign this hash using eth_sign (which prepends EIP-191 prefix).
    function getWithdrawalHash(
        bytes20 nativeSender,
        address recipient,
        uint256 amount,
        uint256 withdrawalNonce
    ) external view returns (bytes32) {
        return _computeWithdrawalHash(nativeSender, recipient, amount, withdrawalNonce);
    }

    // ═══════════════════════════════════════════════════════════════════
    //  SIGNATURE VERIFICATION
    // ═══════════════════════════════════════════════════════════════════

    /// @dev Verify that `signatures` contains at least `threshold` valid
    ///      validator signatures over `messageHash`.
    ///
    ///      CRITICAL INVARIANTS:
    ///        1. Every signature must recover to a registered validator — non-validators
    ///           cause a revert, not a skip. This prevents an attacker from inserting
    ///           garbage signatures to create gaps in the sorted-order check.
    ///        2. Recovered addresses must be strictly ascending (no duplicates).
    ///        3. Malleable signatures (high-s) are rejected.
    function _verifyThresholdSignatures(
        bytes32 messageHash,
        bytes calldata signatures
    ) internal view {
        uint256 len = signatures.length;
        if (len == 0 || len % 65 != 0) revert InvalidSignatureLength(len);

        uint256 sigCount = len / 65;
        if (sigCount < threshold)        revert TooFewSignatures(sigCount, threshold);
        if (sigCount > validators.length) revert TooManySignatures(sigCount, validators.length);

        // EIP-191 personal sign prefix
        bytes32 ethSignedHash = keccak256(
            abi.encodePacked("\x19Ethereum Signed Message:\n32", messageHash)
        );

        address lastSigner;

        for (uint256 i; i < sigCount; ++i) {
            (bytes32 r, bytes32 s, uint8 v) = _extractSignature(signatures, i);

            // Reject malleable signatures (s in upper half of the curve)
            if (uint256(s) > 0x7FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF5D576E7357A4501DDFE92F46681B20A0) {
                revert MalleableSignature(i);
            }

            // Normalise v
            if (v == 0 || v == 1) v += 27;
            if (v != 27 && v != 28) revert InvalidVValue(i, v);

            address signer = ecrecover(ethSignedHash, v, r, s);
            if (signer == address(0)) revert RecoveredZeroAddress(i);

            // CRITICAL: every signer MUST be a validator — no skipping
            if (!isValidator[signer]) revert SignerNotValidator(i, signer);

            // Strictly ascending order — prevents double-counting
            if (signer <= lastSigner) revert SignersNotSorted(i, signer, lastSigner);
            lastSigner = signer;
        }
        // If we reach here, sigCount >= threshold and all are unique valid validators.
    }

    /// @dev Extract (r, s, v) from a concatenated signature blob at position `index`.
    function _extractSignature(bytes calldata signatures, uint256 index)
        internal
        pure
        returns (bytes32 r, bytes32 s, uint8 v)
    {
        uint256 offset = index * 65;
        assembly {
            r := calldataload(add(signatures.offset, offset))
            s := calldataload(add(signatures.offset, add(offset, 32)))
            v := byte(0, calldataload(add(signatures.offset, add(offset, 64))))
        }
    }

    // ═══════════════════════════════════════════════════════════════════
    //  VALIDATOR MANAGEMENT
    // ═══════════════════════════════════════════════════════════════════

    /// @notice Register a new validator. Auto-enables the bridge at MIN_VALIDATORS.
    function addValidator(address validator) external onlyGovernor whenNotPaused {
        if (validator == address(0))  revert ZeroAddress("validator");
        if (isValidator[validator])   revert ValidatorAlreadyRegistered(validator);
        if (validators.length >= MAX_VALIDATORS) revert ValidatorSetFull();

        isValidator[validator] = true;
        validators.push(validator);
        _updateThreshold();

        if (!bridgeEnabled && validators.length >= MIN_VALIDATORS) {
            bridgeEnabled = true;
            emit BridgeEnabled();
        }

        emit ValidatorAdded(validator);
    }

    /// @notice Remove a validator. Cannot drop below MIN_VALIDATORS.
    function removeValidator(address validator) external onlyGovernor whenNotPaused {
        if (!isValidator[validator]) revert ValidatorNotRegistered(validator);
        if (validators.length <= MIN_VALIDATORS) {
            revert BelowMinValidators(validators.length, MIN_VALIDATORS);
        }

        isValidator[validator] = false;

        // Swap-and-pop removal
        uint256 len = validators.length;
        for (uint256 i; i < len; ++i) {
            if (validators[i] == validator) {
                validators[i] = validators[len - 1];
                validators.pop();
                break;
            }
        }

        _updateThreshold();
        emit ValidatorRemoved(validator);
    }

    /// @dev Recalculate threshold: floor(2n/3) + 1 for n ≥ MIN_VALIDATORS.
    function _updateThreshold() internal {
        uint256 n = validators.length;
        threshold = (n < MIN_VALIDATORS) ? n : (2 * n) / 3 + 1;
        emit ThresholdUpdated(threshold);
    }

    /// @notice Override threshold (must still satisfy BFT minimum).
    function setThreshold(uint256 newThreshold) external onlyGovernor whenNotPaused {
        uint256 n = validators.length;
        if (newThreshold == 0 || newThreshold > n) {
            revert BelowMinValidators(newThreshold, 1);
        }
        if (n >= MIN_VALIDATORS) {
            uint256 bftMin = (2 * n) / 3 + 1;
            if (newThreshold < bftMin) revert BelowBFTMinimum(newThreshold, bftMin);
        }
        threshold = newThreshold;
        emit ThresholdUpdated(newThreshold);
    }

    // ═══════════════════════════════════════════════════════════════════
    //  GOVERNANCE
    // ═══════════════════════════════════════════════════════════════════

    function pause() external onlyGovernor {
        paused = true;
        emit BridgePaused(msg.sender);
    }

    function unpause() external onlyGovernor {
        paused = false;
        emit BridgeUnpaused(msg.sender);
    }

    /// @notice Begin two-step governor transfer.
    function setGovernor(address newGovernor) external onlyGovernor {
        if (newGovernor == address(0)) revert ZeroAddress("newGovernor");
        pendingGovernor = newGovernor;
        emit PendingGovernorSet(governor, newGovernor);
    }

    /// @notice Complete governor transfer.
    function acceptGovernor() external {
        if (msg.sender != pendingGovernor) revert NotPendingGovernor();
        address old = governor;
        governor = msg.sender;
        pendingGovernor = address(0);
        emit GovernorAccepted(old, msg.sender);
    }

    // ═══════════════════════════════════════════════════════════════════
    //  MIGRATION & EMERGENCY
    // ═══════════════════════════════════════════════════════════════════

    /// @notice Migrate escrowed UDAG to a new bridge. Only when paused.
    function migrateToNewBridge(address newBridge, uint256 amount)
        external
        onlyGovernor
        whenPaused
        nonReentrant
    {
        if (newBridge == address(0)) revert ZeroAddress("newBridge");
        IERC20(address(token)).safeTransfer(newBridge, amount);
        emit BridgeMigration(newBridge, amount);
    }

    /// @notice Recover accidentally-sent ERC-20 tokens (NOT the managed UDAG token).
    /// @dev For UDAG recovery, use migrateToNewBridge (requires pause).
    function emergencyWithdrawERC20(
        address tokenAddress,
        address to,
        uint256 amount
    ) external onlyGovernor nonReentrant {
        if (tokenAddress == address(token)) revert CannotDrainManagedToken();
        if (to == address(0)) revert ZeroAddress("to");
        IERC20(tokenAddress).safeTransfer(to, amount);
        emit EmergencyWithdrawal(tokenAddress, to, amount);
    }

    /// @notice Recover accidentally-sent ETH.
    function emergencyWithdrawETH(address payable to) external onlyGovernor nonReentrant {
        if (to == address(0)) revert ZeroAddress("to");
        uint256 bal = address(this).balance;
        if (bal == 0) return;
        (bool ok, ) = to.call{value: bal}("");
        if (!ok) revert ETHTransferFailed();
        emit EmergencyWithdrawal(address(0), to, bal);
    }

    // ═══════════════════════════════════════════════════════════════════
    //  VIEW HELPERS
    // ═══════════════════════════════════════════════════════════════════

    function getValidatorCount() external view returns (uint256) {
        return validators.length;
    }

    function getAllValidators() external view returns (address[] memory) {
        return validators;
    }

    function getThreshold() external view returns (uint256) {
        return threshold;
    }

    function isWithdrawalNonceUsed(uint256 _nonce) external view returns (bool) {
        return usedWithdrawalNonces[_nonce];
    }

    function getDailyWithdrawalVolume() external view returns (uint256) {
        return dailyWithdrawalVolume[block.timestamp / 1 days];
    }

    function getDailyDepositVolume() external view returns (uint256) {
        return dailyDepositVolume[block.timestamp / 1 days];
    }

    function getDailyWithdrawalRemaining() external view returns (uint256) {
        uint256 used = dailyWithdrawalVolume[block.timestamp / 1 days];
        return used >= DAILY_LIMIT ? 0 : DAILY_LIMIT - used;
    }

    function getDailyDepositRemaining() external view returns (uint256) {
        uint256 used = dailyDepositVolume[block.timestamp / 1 days];
        return used >= DAILY_LIMIT ? 0 : DAILY_LIMIT - used;
    }

    // ═══════════════════════════════════════════════════════════════════
    //  INTERNALS
    // ═══════════════════════════════════════════════════════════════════

    /// @dev Shared validation for deposit and withdrawal amounts.
    function _validateAmount(uint256 amount) internal pure {
        if (amount < MIN_AMOUNT) revert AmountBelowMinimum(amount, MIN_AMOUNT);
        if (amount > MAX_AMOUNT) revert AmountAboveMaximum(amount, MAX_AMOUNT);
    }

    /// @dev Accept ETH so it can be recovered via emergencyWithdrawETH.
    receive() external payable {
        emit ETHReceived(msg.sender, msg.value);
    }
}
