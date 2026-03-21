// Contract ABIs and addresses for the UDAG bridge on Arbitrum
// These will be updated after deployment

// Arbitrum One chain ID
export const ARBITRUM_CHAIN_ID = 42161;
// Arbitrum Sepolia testnet chain ID (for testing)
export const ARBITRUM_SEPOLIA_CHAIN_ID = 421614;

// Contract addresses — UPDATE AFTER DEPLOYMENT
export const UDAG_TOKEN_ADDRESS = ''; // e.g., '0x1234...'
export const UDAG_BRIDGE_ADDRESS = ''; // e.g., '0x5678...'

// Whether contracts are deployed
export const CONTRACTS_DEPLOYED = UDAG_TOKEN_ADDRESS !== '' && UDAG_BRIDGE_ADDRESS !== '';

// Minimal ABIs — only the functions we call from the dashboard
export const UDAG_TOKEN_ABI = [
  'function name() view returns (string)',
  'function symbol() view returns (string)',
  'function decimals() view returns (uint8)',
  'function totalSupply() view returns (uint256)',
  'function balanceOf(address) view returns (uint256)',
  'function allowance(address owner, address spender) view returns (uint256)',
  'function approve(address spender, uint256 amount) returns (bool)',
  'function transfer(address to, uint256 amount) returns (bool)',
  'event Transfer(address indexed from, address indexed to, uint256 value)',
  'event Approval(address indexed owner, address indexed spender, uint256 value)',
];

export const UDAG_BRIDGE_ABI = [
  'function token() view returns (address)',
  'function bridgeActive() view returns (bool)',
  'function paused() view returns (bool)',
  'function nonce() view returns (uint256)',
  'function requiredSignatures() view returns (uint256)',
  'function relayerCount() view returns (uint256)',
  'function dailyVolume() view returns (uint256)',
  'function dailyVolumeResetTime() view returns (uint256)',
  'function MAX_BRIDGE_PER_TX() view returns (uint256)',
  'function DAILY_VOLUME_CAP() view returns (uint256)',
  'function REFUND_TIMEOUT() view returns (uint256)',
  'function bridgeRequests(uint256) view returns (address sender, bytes20 nativeRecipient, uint256 amount, uint256 timestamp, bool completed, bool refunded)',
  'function bridgeToNative(bytes20 nativeRecipient, uint256 amount)',
  'function refundBridge(uint256 bridgeNonce)',
  'event BridgeToNative(address indexed sender, bytes20 indexed nativeRecipient, uint256 amount, uint256 indexed bridgeNonce)',
  'event BridgeCompleted(uint256 indexed bridgeNonce)',
  'event BridgeRefunded(uint256 indexed bridgeNonce, address indexed sender, uint256 amount)',
];
