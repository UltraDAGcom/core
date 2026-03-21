// SPDX-License-Identifier: BUSL-1.1
pragma solidity ^0.8.24;

import "forge-std/Script.sol";
import "../src/UDAGBridge.sol";

/// @notice Activate the bridge after deployment and testing
/// @dev Run with:
///   forge script script/ActivateBridge.s.sol:ActivateBridgeScript --rpc-url $RPC_URL \
///     --private-key $GOVERNOR_KEY --broadcast --verify -vvvv
///
/// IMPORTANT: Only run this after:
/// 1. Bridge is deployed and tested on testnet
/// 2. Relayer infrastructure is operational
/// 3. External audit is complete
/// 4. Ready for production use
contract ActivateBridgeScript is Script {
    function run() external {
        address bridgeAddress = vm.envAddress("BRIDGE_ADDRESS");
        
        console.log("Activating bridge at:", bridgeAddress);
        
        UDAGBridge bridge = UDAGBridge(bridgeAddress);
        
        // Verify caller is governor
        require(bridge.governor() == msg.sender, "Caller is not governor");
        
        // Verify bridge is not already active
        require(!bridge.bridgeActive(), "Bridge is already active");
        
        vm.startBroadcast();
        
        // Activate the bridge
        bridge.activateBridge();
        
        vm.stopBroadcast();
        
        console.log("Bridge activated successfully!");
        console.log("Bridge is now accepting deposits and withdrawals");
        
        // Output activation summary
        console.log("\n========================================");
        console.log("       ACTIVATION SUMMARY");
        console.log("========================================");
        console.log("Bridge:", bridgeAddress);
        console.log("Status: ACTIVE");
        console.log("Timestamp:", block.timestamp);
        console.log("Block:", block.number);
        console.log("========================================\n");
    }
}
