// SPDX-License-Identifier: BUSL-1.1
pragma solidity ^0.8.24;

import "forge-std/Script.sol";
import "../src/UDAGBridgeValidator.sol";

/// @notice Configure the Validator Federation Bridge after deployment
/// @dev Run with:
///   forge script script/ConfigureBridge.s.sol:ConfigureBridgeScript --rpc-url $RPC_URL \
///     --private-key $GOVERNOR_KEY --broadcast --verify -vvvv
///
/// Environment variables required:
///   BRIDGE_ADDRESS: Bridge contract address
///   VALIDATOR_ADDRESSES: Comma-separated validator addresses to add
contract ConfigureBridgeScript is Script {
    function run() external {
        address bridgeAddress = vm.envAddress("BRIDGE_ADDRESS");
        string memory validatorAddressesStr = vm.envString("VALIDATOR_ADDRESSES");

        console.log("Configuring Validator Federation Bridge at:", bridgeAddress);

        UDAGBridgeValidator bridge = UDAGBridgeValidator(bridgeAddress);

        // Verify caller is governor
        require(bridge.governor() == msg.sender, "Caller is not governor");

        vm.startBroadcast();

        // Parse and add validators
        bytes memory addressesBytes = bytes(validatorAddressesStr);
        uint256 addrStart = 0;
        uint256 validatorCount = 0;

        for (uint256 i = 0; i <= addressesBytes.length; i++) {
            if (i == addressesBytes.length || addressesBytes[i] == ',') {
                if (i > addrStart) {
                    string memory addrStr = substring(validatorAddressesStr, addrStart, i);
                    address validator = vm.parseAddress(addrStr);
                    bridge.addValidator(validator);
                    console.log("Validator added:", validator);
                    validatorCount++;
                }
                addrStart = i + 1;
            }
        }

        vm.stopBroadcast();

        console.log("\n========================================");
        console.log("    BRIDGE CONFIGURATION COMPLETE");
        console.log("========================================");
        console.log("Bridge:", bridgeAddress);
        console.log("Validators added:", validatorCount);
        console.log("Threshold:", bridge.threshold(), "of", validatorCount);
        console.log("Status: READY");
        console.log("========================================\n");
    }

    function substring(string memory str, uint256 startIndex, uint256 endIndex) 
        internal pure returns (string memory) 
    {
        bytes memory strBytes = bytes(str);
        require(endIndex >= startIndex, "Invalid indices");
        require(endIndex <= strBytes.length, "End index out of bounds");
        
        bytes memory result = new bytes(endIndex - startIndex);
        for (uint256 i = startIndex; i < endIndex; i++) {
            result[i - startIndex] = strBytes[i];
        }
        return string(result);
    }
}
