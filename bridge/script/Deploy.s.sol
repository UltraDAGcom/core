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
///   DEPLOYER_KEY: Deployer private key (for signing transactions)
///   GOVERNOR_ADDRESS: Governor/admin address (timelock or multisig)
///   DEV_ADDRESS: Developer allocation recipient
///   TREASURY_ADDRESS: Treasury allocation recipient
contract DeployScript is Script {
    // Configuration
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
        address governor = vm.envAddress("GOVERNOR_ADDRESS");
        address devAddress = vm.envAddress("DEV_ADDRESS");
        address treasuryAddress = vm.envAddress("TREASURY_ADDRESS");
        
        // Get deployer address from private key
        uint256 deployerKey = vm.envUint("DEPLOYER_KEY");
        address deployer = vm.addr(deployerKey);

        // Zero address checks
        require(governor != address(0), "Deploy: zero governor");
        require(devAddress != address(0), "Deploy: zero dev address");
        require(treasuryAddress != address(0), "Deploy: zero treasury");

        vm.startBroadcast(deployerKey);

        // Step 1: Deploy TimelockController
        // Governor (EOA/multisig) will have TIMELOCK_ADMIN_ROLE
        timelockAddress = address(new TimelockController(
            MIN_DELAY,
            new address[](0), // No proposers initially
            new address[](0), // No executors initially
            governor // Admin
        ));

        console.log("TimelockController deployed:", timelockAddress);

        // Step 2: Deploy UDAG Token
        // Pass deployer as genesisMinter so it can mint genesis allocations
        tokenAddress = address(new UDAGToken(timelockAddress, address(0), deployer));
        console.log("UDAGToken deployed:", tokenAddress);

        // Step 3: Deploy UDAG Bridge (Validator Federation - no relayers needed!)
        bridgeAddress = address(new UDAGBridgeValidator(
            tokenAddress,
            timelockAddress
        ));
        console.log("UDAGBridgeValidator deployed:", bridgeAddress);

        // Step 4: Grant bridge MINTER_ROLE only (deposits lock tokens via transferFrom)
        UDAGToken(tokenAddress).grantRole(
            UDAGToken(tokenAddress).MINTER_ROLE(),
            bridgeAddress
        );
        console.log("Bridge granted MINTER_ROLE (for minting on withdrawal claims)");
        
        // Step 5: Mint genesis allocations (deployer has MINTER_ROLE)
        UDAGToken(tokenAddress).mint(devAddress, DEV_ALLOCATION);
        console.log("Minted dev allocation:", DEV_ALLOCATION, "to", devAddress);

        UDAGToken(tokenAddress).mint(treasuryAddress, TREASURY_ALLOCATION);
        console.log("Minted treasury allocation:", TREASURY_ALLOCATION, "to", treasuryAddress);

        // Step 6: Finalize genesis (revoke MINTER_ROLE from deployer)
        UDAGToken(tokenAddress).finalizeGenesis();
        console.log("Genesis finalized - deployer MINTER_ROLE revoked");
        
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
        console.log("Timelock Delay:", MIN_DELAY, "seconds");
        console.log("========================================\n");

        // Save deployment artifacts
        _saveDeploymentArtifacts(governor, devAddress, treasuryAddress);
    }

    function _saveDeploymentArtifacts(
        address governor,
        address devAddress,
        address treasuryAddress
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
