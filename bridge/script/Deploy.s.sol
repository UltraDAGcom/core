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
contract DeployScript is Script {
    // Configuration
    uint256 public constant MIN_DELAY = 1 days; // Timelock delay

    // Deployed contract addresses
    address public tokenAddress;
    address public bridgeAddress;
    address public timelockAddress;

    function run() external {
        // Load configuration from environment
        address governor = vm.envAddress("GOVERNOR_ADDRESS");

        // Get deployer address from private key
        uint256 deployerKey = vm.envUint("DEPLOYER_KEY");
        address deployer = vm.addr(deployerKey);

        // Zero address checks
        require(governor != address(0), "Deploy: zero governor");

        vm.startBroadcast(deployerKey);

        // Step 1: Deploy TimelockController
        address[] memory proposers = new address[](1);
        proposers[0] = governor;
        address[] memory executors = new address[](1);
        executors[0] = governor;

        timelockAddress = address(new TimelockController(
            MIN_DELAY,
            proposers,
            executors,
            governor // Admin
        ));

        console.log("TimelockController deployed:", timelockAddress);

        // Step 2: Deploy UDAG Bridge (Validator Federation)
        // We need the bridge address first so the token can grant MINTER_ROLE to it.
        // Use CREATE2 or deploy bridge first with a temp token, then redeploy.
        // Simpler approach: deploy bridge first, then token with bridge address.

        // Deploy a placeholder token for bridge constructor (bridge needs a token address)
        // Actually, the bridge constructor just stores the token address, so we can
        // predict the token address or use a two-step approach.

        // Two-step: deploy bridge with a temp token, deploy real token with bridge,
        // then... bridge.token is immutable. So we must know the bridge address first.

        // Better approach: compute the bridge address via CREATE nonce prediction.
        // deployer nonce after timelock deploy = current + 1
        // token will be deployed at nonce + 1, bridge at nonce + 2
        // OR: deploy bridge first (needs token address), deploy token second (needs bridge address)
        // This is a chicken-and-egg. The solution: deploy token with deployer as temp bridge,
        // then deploy real bridge, then migrate the bridge address via proposeBridgeMigration.
        //
        // But proposeBridgeMigration has a 2-day timelock! For deployment we need something faster.
        //
        // Simplest correct approach: deploy token with deployer as admin, use a deterministic
        // address for the bridge via CREATE2, or just accept that we deploy token first with
        // a temporary bridge, then deploy real bridge, then do an immediate bridge migration.
        //
        // Actually, since MINTER_ROLE admin is locked in constructor, we cannot use grantRole.
        // But executeBridgeMigration() temporarily unlocks it. The deployer would need to wait
        // 2 days for the timelock though.
        //
        // The pragmatic solution: compute the bridge address ahead of time using CREATE nonce.
        // Deployer's next nonce after timelock = creates token, nonce after token = creates bridge.

        // Nonce-based address prediction:
        // After timelock deploy, deployer nonce has incremented by 1.
        // Next deploy (token) will be at nonce N, bridge at nonce N+1.
        // We need bridge address for token constructor, so predict it.
        uint256 deployerNonce = vm.getNonce(deployer);
        // Token will be deployed at current nonce, bridge at nonce+1
        address predictedBridge = vm.computeCreateAddress(deployer, deployerNonce + 1);

        // Step 2: Deploy UDAG Token with predicted bridge address
        tokenAddress = address(new UDAGToken(timelockAddress, predictedBridge));
        console.log("UDAGToken deployed:", tokenAddress);

        // Step 3: Deploy UDAG Bridge (Validator Federation)
        bridgeAddress = address(new UDAGBridgeValidator(
            tokenAddress,
            timelockAddress
        ));
        console.log("UDAGBridgeValidator deployed:", bridgeAddress);

        // Verify prediction was correct
        require(bridgeAddress == predictedBridge, "Deploy: bridge address prediction failed");
        console.log("Bridge address prediction verified");

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
        console.log("Timelock Delay:", MIN_DELAY, "seconds");
        console.log("========================================\n");

        // Save deployment artifacts
        _saveDeploymentArtifacts(governor);
    }

    function _saveDeploymentArtifacts(address governor) internal {
        // Create deployment output file
        string memory deploymentInfo = string.concat(
            '{"network":', vm.toString(block.chainid),
            ',"token":"', vm.toString(tokenAddress), '"',
            ',"bridge":"', vm.toString(bridgeAddress), '"',
            ',"timelock":"', vm.toString(timelockAddress), '"',
            ',"governor":"', vm.toString(governor), '"',
            ',"timelockDelay":', vm.toString(MIN_DELAY), '}'
        );

        // Write to file (for CI/CD integration)
        vm.writeJson(deploymentInfo, "deployment-output.json");
        console.log("Deployment artifacts saved to deployment-output.json");
    }
}
