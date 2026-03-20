// SPDX-License-Identifier: BUSL-1.1
pragma solidity ^0.8.24;

import "./UDAGToken.sol";

/// @title UltraDAG Bridge
/// @notice Bridges UDAG between Arbitrum and the UltraDAG native chain.
/// @dev Phase 1: bridge is inactive (pre-mainnet). Phase 2: bridge activates after mainnet launch.
///      Arbitrum→Native: user escrows ERC-20 in bridge, relayers confirm native delivery, bridge burns.
///      Native→Arbitrum: user locks on native, relayers mint ERC-20 here.
contract UDAGBridge {
    // ─── State ───

    UDAGToken public immutable token;
    address public governor; // Should be a TimelockController for production deployments
    address public pendingGovernor;
    bool public bridgeActive;
    bool public paused;
    uint256 public nonce; // Monotonic nonce for Arbitrum→Native transfers

    // Relayer multi-sig
    address[] public relayers;
    mapping(address => bool) public isRelayer;
    uint256 public requiredSignatures; // e.g., 3-of-5

    // Processed native→Arbitrum nonces (prevents replay)
    mapping(uint256 => bool) public processedNonces;

    // Rate limiting
    uint256 public constant MAX_BRIDGE_PER_TX = 100_000 * 10 ** 8; // 100,000 UDAG per tx
    uint256 public constant DAILY_VOLUME_CAP = 500_000 * 10 ** 8; // 500,000 UDAG per day
    uint256 public dailyVolume;
    uint256 public dailyVolumeResetTime;

    // Escrow model for Arbitrum→Native bridge requests
    uint256 public constant REFUND_TIMEOUT = 7 days;

    struct BridgeRequest {
        address sender;
        bytes20 nativeRecipient;
        uint256 amount;
        uint256 timestamp;
        bool completed;
        bool refunded;
    }

    mapping(uint256 => BridgeRequest) public bridgeRequests;

    // ─── Events ───

    /// @notice Emitted when tokens are escrowed for bridging to UltraDAG native chain.
    event BridgeToNative(
        address indexed sender,
        bytes20 indexed nativeRecipient,
        uint256 amount,
        uint256 indexed bridgeNonce
    );

    /// @notice Emitted when tokens are minted from a native chain bridge-lock event.
    event BridgeFromNative(
        bytes20 indexed nativeSender,
        address indexed recipient,
        uint256 amount,
        bytes32 nativeTxHash,
        uint256 indexed bridgeNonce
    );

    event BridgeActivated(uint256 timestamp);
    event BridgePaused(address by);
    event BridgeUnpaused(address by);
    event RelayerAdded(address relayer);
    event RelayerRemoved(address relayer);
    event ThresholdChanged(uint256 oldThreshold, uint256 newThreshold);
    event GovernorProposed(address indexed newGovernor);
    event GovernorChanged(address indexed oldGovernor, address indexed newGovernor);
    event BridgeRefunded(uint256 indexed bridgeNonce, address indexed sender, uint256 amount);
    event BridgeCompleted(uint256 indexed bridgeNonce);

    // ─── Errors ───

    error BridgeNotActive();
    error BridgePausedError();
    error AmountTooLarge();
    error DailyCapExceeded();
    error InvalidRecipient();
    error InvalidAmount();
    error NonceAlreadyProcessed();
    error InsufficientSignatures();
    error InvalidSignature();
    error NotGovernor();
    error NotRelayer();

    // ─── Modifiers ───

    modifier onlyGovernor() {
        if (msg.sender != governor) revert NotGovernor();
        _;
    }

    modifier whenActive() {
        if (!bridgeActive) revert BridgeNotActive();
        if (paused) revert BridgePausedError();
        _;
    }

    // ─── Constructor ───

    /// @param _governor SHOULD be a TimelockController address for production deployments
    ///        to enforce a delay on governance actions and prevent instant malicious changes.
    constructor(address _token, address _governor, address[] memory _relayers, uint256 _requiredSignatures) {
        require(_requiredSignatures > 0 && _requiredSignatures <= _relayers.length, "invalid threshold");
        token = UDAGToken(_token);
        governor = _governor;
        requiredSignatures = _requiredSignatures;
        for (uint256 i = 0; i < _relayers.length; i++) {
            require(_relayers[i] != address(0), "zero relayer");
            require(!isRelayer[_relayers[i]], "duplicate relayer");
            relayers.push(_relayers[i]);
            isRelayer[_relayers[i]] = true;
        }
        dailyVolumeResetTime = block.timestamp;
    }

    // ─── Bridge: Arbitrum → Native (escrow model) ───

    /// @notice Escrow UDAG tokens in the bridge for bridging to the UltraDAG native chain.
    /// @param nativeRecipient The 20-byte UltraDAG address (bech32m decoded).
    /// @param amount Amount in sats (8 decimals). Must have approved this contract.
    function bridgeToNative(bytes20 nativeRecipient, uint256 amount) external whenActive {
        if (nativeRecipient == bytes20(0)) revert InvalidRecipient();
        if (amount == 0) revert InvalidAmount();
        if (amount > MAX_BRIDGE_PER_TX) revert AmountTooLarge();
        _checkAndUpdateDailyVolume(amount);

        // Escrow tokens in bridge contract (requires prior approval)
        token.transferFrom(msg.sender, address(this), amount);

        uint256 currentNonce = nonce++;
        bridgeRequests[currentNonce] = BridgeRequest({
            sender: msg.sender,
            nativeRecipient: nativeRecipient,
            amount: amount,
            timestamp: block.timestamp,
            completed: false,
            refunded: false
        });

        emit BridgeToNative(msg.sender, nativeRecipient, amount, currentNonce);
    }

    /// @notice Complete a bridge-to-native request after confirming native-side delivery.
    /// @dev Burns the escrowed tokens. Requires relayer multi-sig confirmation.
    function completeBridgeToNative(uint256 bridgeNonce, bytes calldata signatures) external whenActive {
        BridgeRequest storage req = bridgeRequests[bridgeNonce];
        require(req.sender != address(0), "request not found");
        require(!req.completed, "already completed");
        require(!req.refunded, "already refunded");

        // Verify relayer signatures over the bridge nonce completion
        bytes32 messageHash = keccak256(
            abi.encodePacked(
                "\x19Ethereum Signed Message:\n32",
                keccak256(
                    abi.encode(
                        "completeBridgeToNative",
                        block.chainid,
                        address(this),
                        bridgeNonce,
                        req.sender,
                        req.nativeRecipient,
                        req.amount
                    )
                )
            )
        );

        _verifySignatures(messageHash, signatures);

        req.completed = true;
        token.burn(address(this), req.amount);
        emit BridgeCompleted(bridgeNonce);
    }

    /// @notice Refund escrowed tokens if the bridge request was never completed.
    /// @dev Only the original sender can call, after REFUND_TIMEOUT has elapsed.
    function refundBridge(uint256 bridgeNonce) external {
        BridgeRequest storage req = bridgeRequests[bridgeNonce];
        require(req.sender == msg.sender, "not sender");
        require(!req.completed, "already completed");
        require(!req.refunded, "already refunded");
        require(block.timestamp >= req.timestamp + REFUND_TIMEOUT, "too early");

        req.refunded = true;
        token.transfer(msg.sender, req.amount);
        emit BridgeRefunded(bridgeNonce, msg.sender, req.amount);
    }

    // ─── Bridge: Native → Arbitrum ───

    /// @notice Complete a bridge transfer from UltraDAG native to Arbitrum.
    /// @dev Only succeeds if enough relayers have signed the attestation.
    /// @param nativeSender The UltraDAG address that locked funds.
    /// @param recipient The Arbitrum address to receive tokens.
    /// @param amount Amount in sats (8 decimals).
    /// @param nativeTxHash The transaction hash on UltraDAG native chain.
    /// @param bridgeNonce The bridge nonce from the native chain.
    /// @param signatures Concatenated relayer signatures (65 bytes each: r, s, v).
    function bridgeFromNative(
        bytes20 nativeSender,
        address recipient,
        uint256 amount,
        bytes32 nativeTxHash,
        uint256 bridgeNonce,
        bytes calldata signatures
    ) external whenActive {
        require(recipient != address(0), "zero recipient");
        if (amount == 0) revert InvalidAmount();
        if (amount > MAX_BRIDGE_PER_TX) revert AmountTooLarge();
        if (processedNonces[bridgeNonce]) revert NonceAlreadyProcessed();
        _checkAndUpdateDailyVolume(amount);

        // Verify relayer signatures
        bytes32 messageHash = keccak256(
            abi.encodePacked(
                "\x19Ethereum Signed Message:\n32",
                keccak256(
                    abi.encode(
                        block.chainid,
                        address(this),
                        nativeSender,
                        recipient,
                        amount,
                        nativeTxHash,
                        bridgeNonce
                    )
                )
            )
        );

        _verifySignatures(messageHash, signatures);

        processedNonces[bridgeNonce] = true;
        token.mint(recipient, amount);

        emit BridgeFromNative(nativeSender, recipient, amount, nativeTxHash, bridgeNonce);
    }

    // ─── Admin Functions ───

    /// @notice Activate the bridge. Can only be called once by governor.
    function activateBridge() external onlyGovernor {
        require(!bridgeActive, "already active");
        bridgeActive = true;
        emit BridgeActivated(block.timestamp);
    }

    /// @notice Emergency pause. Any relayer can pause.
    function pause() external {
        if (!isRelayer[msg.sender] && msg.sender != governor) revert NotRelayer();
        paused = true;
        emit BridgePaused(msg.sender);
    }

    /// @notice Unpause. Only governor.
    function unpause() external onlyGovernor {
        paused = false;
        emit BridgeUnpaused(msg.sender);
    }

    function addRelayer(address relayer) external onlyGovernor {
        require(relayer != address(0), "zero relayer");
        require(!isRelayer[relayer], "already relayer");
        relayers.push(relayer);
        isRelayer[relayer] = true;
        emit RelayerAdded(relayer);
    }

    function removeRelayer(address relayer) external onlyGovernor {
        require(isRelayer[relayer], "not relayer");
        isRelayer[relayer] = false;
        // Remove from array
        for (uint256 i = 0; i < relayers.length; i++) {
            if (relayers[i] == relayer) {
                relayers[i] = relayers[relayers.length - 1];
                relayers.pop();
                break;
            }
        }
        require(relayers.length >= requiredSignatures, "would break threshold");
        emit RelayerRemoved(relayer);
    }

    function setThreshold(uint256 newThreshold) external onlyGovernor {
        require(newThreshold > 0 && newThreshold <= relayers.length, "invalid threshold");
        emit ThresholdChanged(requiredSignatures, newThreshold);
        requiredSignatures = newThreshold;
    }

    /// @notice Propose a new governor. Must be accepted by the new governor.
    function proposeGovernor(address newGovernor) external onlyGovernor {
        require(newGovernor != address(0), "zero address");
        pendingGovernor = newGovernor;
        emit GovernorProposed(newGovernor);
    }

    /// @notice Accept governance. Only the pending governor can call.
    function acceptGovernance() external {
        require(msg.sender == pendingGovernor, "not pending governor");
        emit GovernorChanged(governor, msg.sender);
        governor = msg.sender;
        pendingGovernor = address(0);
    }

    // ─── View Functions ───

    function getRelayers() external view returns (address[] memory) {
        return relayers;
    }

    function relayerCount() external view returns (uint256) {
        return relayers.length;
    }

    // ─── Internal ───

    function _checkAndUpdateDailyVolume(uint256 amount) internal {
        if (block.timestamp >= dailyVolumeResetTime + 1 days) {
            dailyVolume = 0;
            dailyVolumeResetTime = block.timestamp;
        }
        if (dailyVolume + amount > DAILY_VOLUME_CAP) revert DailyCapExceeded();
        dailyVolume += amount;
    }

    function _verifySignatures(bytes32 messageHash, bytes calldata signatures) internal view {
        uint256 sigCount = signatures.length / 65;
        if (sigCount < requiredSignatures) revert InsufficientSignatures();

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
            if (!isRelayer[signer]) continue; // Skip non-relayer signatures
            require(signer > lastSigner, "signatures not sorted"); // Enforce unique, sorted signers
            lastSigner = signer;
            validCount++;
        }

        if (validCount < requiredSignatures) revert InsufficientSignatures();
    }
}
