// SPDX-License-Identifier: BUSL-1.1
pragma solidity ^0.8.24;

import "forge-std/Script.sol";
import "../src/UDAGToken.sol";
import "../src/UDAGBridgeValidator.sol";
import "@openzeppelin/contracts/governance/TimelockController.sol";

/// @notice Deploy UltraDAG token and bridge to Arbitrum
/// @dev Run with:
///   forge script script/Deploy.s.sol:DeployScript --rpc-url $RPC_URL \
///     --private-key $DEPLOYER_KEY --broadcast --verify -vvvv
///
/// Environment variables required:
///   RPC_URL: Arbitrum RPC endpoint
///   DEPLOYER_KEY: Deployer private key
///   GOVERNOR_KEY: Governor/admin private key (can be same as deployer)
contract DeployScript is Script {
    // Configuration
    uint256 public constant REQUIRED_SIGNERS = 3;
    uint256 public constant MIN_DELAY = 1 days; // Timelock delay
    
    // Genesis allocations (in sats, 8 decimals)
    uint256 public constant DEV_ALLOCATION = 1_050_000 * 10 ** 8; // 5%
    uint256 public constant TREASURY_ALLOCATION = 2_100_000 * 10 ** 8; // 10%
    
    // Deployed contract addresses
    address public tokenAddress;
    address public bridgeAddress;
    address public timelockAddress;

    function run() external {
        // Load configuration from environment
        address governor = vm.envAddress("GOVERNOR_KEY");
        address devAddress = vm.envAddress("DEV_ADDRESS");
        address treasuryAddress = vm.envAddress("TREASURY_ADDRESS");

        vm.startBroadcast();

        // Step 1: Deploy TimelockController
        timelockAddress = address(new TimelockController(
            MIN_DELAY,
            new address[](0), // No proposers initially
            new address[](0), // No executors initially
            governor // Admin
        ));

        console.log("TimelockController deployed:", timelockAddress);

        // Step 2: Deploy UDAG Token
        tokenAddress = address(new UDAGToken(timelockAddress));
        console.log("UDAGToken deployed:", tokenAddress);

        // Step 3: Deploy UDAG Bridge (Validator Federation - no relayers needed!)
        bridgeAddress = address(new UDAGBridgeValidator(
            tokenAddress,
            timelockAddress
        ));
        console.log("UDAGBridgeValidator deployed:", bridgeAddress);
        
        // Step 4: Grant bridge MINTER_ROLE and BURNER_ROLE
        UDAGToken(tokenAddress).grantRole(
            UDAGToken(tokenAddress).MINTER_ROLE(),
            bridgeAddress
        );
        UDAGToken(tokenAddress).grantRole(
            UDAGToken(tokenAddress).BURNER_ROLE(),
            bridgeAddress
        );
        console.log("Bridge granted MINTER_ROLE and BURNER_ROLE");
        
        // Step 5: Mint genesis allocations
        UDAGToken(tokenAddress).mint(devAddress, DEV_ALLOCATION);
        console.log("Minted dev allocation:", DEV_ALLOCATION, "to", devAddress);
        
        UDAGToken(tokenAddress).mint(treasuryAddress, TREASURY_ALLOCATION);
        console.log("Minted treasury allocation:", TREASURY_ALLOCATION, "to", treasuryAddress);
        
        // Step 6: Finalize genesis (revoke admin mint role)
        UDAGToken(tokenAddress).finalizeGenesis();
        console.log("Genesis finalized - admin mint role revoked");
        
        // Step 7: Configure Timelock roles
        // Grant proposer role to timelock itself (for self-scheduling)
        TimelockController(payable(timelockAddress)).grantRole(
            TimelockController(payable(timelockAddress)).PROPOSER_ROLE(),
            timelockAddress
        );
        
        // Grant executor role to timelock itself
        TimelockController(payable(timelockAddress)).grantRole(
            TimelockController(payable(timelockAddress)).EXECUTOR_ROLE(),
            timelockAddress
        );
        
        // Grant proposer role to governor (can propose changes)
        TimelockController(payable(timelockAddress)).grantRole(
            TimelockController(payable(timelockAddress)).PROPOSER_ROLE(),
            governor
        );
        
        console.log("Timelock configured with proposer and executor roles");
        
        vm.stopBroadcast();
        
        // Output deployment summary
        console.log("\n========================================");
        console.log("       DEPLOYMENT SUMMARY");
        console.log("========================================");
        console.log("Network:", block.chainid);
        console.log("UDAGToken:", tokenAddress);
        console.log("UDAGBridge:", bridgeAddress);
        console.log("TimelockController:", timelockAddress);
        console.log("Governor:", governor);
        console.log("Dev Address:", devAddress);
        console.log("Treasury Address:", treasuryAddress);
        console.log("Relayers:");
        for (uint256 i = 0; i < relayers.length; i++) {
            console.log("  ", i, ":", relayers[i]);
        }
        console.log("Required Signatures:", REQUIRED_SIGNERS);
        console.log("Timelock Delay:", MIN_DELAY, "seconds");
        console.log("========================================\n");
        
        // Save deployment artifacts
        _saveDeploymentArtifacts(governor, devAddress, treasuryAddress, relayers);
    }
    
    function _saveDeploymentArtifacts(
        address governor,
        address devAddress,
        address treasuryAddress,
        address[] memory relayers
    ) internal {
        // Create deployment output file
        string memory deploymentInfo = string.concat(
            '{"network":', vm.toString(block.chainid),
            ',"token":"', vm.toString(tokenAddress), '"',
            ',"bridge":"', vm.toString(bridgeAddress), '"',
            ',"timelock":"', vm.toString(timelockAddress), '"',
            ',"governor":"', vm.toString(governor), '"',
            ',"devAddress":"', vm.toString(devAddress), '"',
            ',"treasuryAddress":"', vm.toString(treasuryAddress), '"',
            ',"relayers":[',
            _addressesToJson(relayers),
            '],"requiredSigners":', vm.toString(REQUIRED_SIGNERS),
            ',"timelockDelay":', vm.toString(MIN_DELAY), '}'
        );
        
        // Write to file (for CI/CD integration)
        vm.writeJson(deploymentInfo, "deployment-output.json");
        console.log("Deployment artifacts saved to deployment-output.json");
    }
    
    function _addressesToJson(address[] memory addrs) internal pure returns (string memory) {
        string memory result = "";
        for (uint256 i = 0; i < addrs.length; i++) {
            if (i > 0) {
                result = string.concat(result, ",");
            }
            result = string.concat(result, '"', vm.toString(addrs[i]), '"');
        }
        return result;
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
