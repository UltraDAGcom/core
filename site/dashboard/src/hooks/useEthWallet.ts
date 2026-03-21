import { useState, useCallback, useEffect } from 'react';
import { BrowserProvider, Contract, formatUnits, parseUnits } from 'ethers';
import {
  ARBITRUM_CHAIN_ID,
  ARBITRUM_SEPOLIA_CHAIN_ID,
  UDAG_TOKEN_ADDRESS,
  UDAG_BRIDGE_ADDRESS,
  UDAG_TOKEN_ABI,
  UDAG_BRIDGE_ABI,
  CONTRACTS_DEPLOYED,
} from '../lib/contracts';

interface EthWalletState {
  connected: boolean;
  address: string;
  chainId: number;
  balance: string; // ETH balance
  udagBalance: string; // UDAG ERC-20 balance (formatted)
  udagBalanceRaw: bigint; // UDAG ERC-20 balance (raw sats)
  udagAllowance: bigint; // UDAG allowance for bridge contract
  isCorrectChain: boolean;
  bridgeActive: boolean;
  bridgePaused: boolean;
  dailyVolume: bigint;
  dailyCap: bigint;
  maxPerTx: bigint;
  nonce: bigint;
}

const defaultState: EthWalletState = {
  connected: false,
  address: '',
  chainId: 0,
  balance: '0',
  udagBalance: '0',
  udagBalanceRaw: 0n,
  udagAllowance: 0n,
  isCorrectChain: false,
  bridgeActive: false,
  bridgePaused: false,
  dailyVolume: 0n,
  dailyCap: 0n,
  maxPerTx: 0n,
  nonce: 0n,
};

// Acceptable chain IDs
const ACCEPTED_CHAINS = [ARBITRUM_CHAIN_ID, ARBITRUM_SEPOLIA_CHAIN_ID];

function getEthereum(): any {
  // Prefer MetaMask if available
  const ethereum = (window as any).ethereum;
  if (ethereum?.isMetaMask) {
    return ethereum;
  }
  // Fallback to any injected provider
  return ethereum;
}

export function useEthWallet() {
  const [state, setState] = useState<EthWalletState>(defaultState);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');

  const fetchBalances = useCallback(async (provider: BrowserProvider, address: string) => {
    try {
      const ethBal = await provider.getBalance(address);
      const signer = await provider.getSigner();
      const network = await provider.getNetwork();
      const chainId = Number(network.chainId);
      const isCorrectChain = ACCEPTED_CHAINS.includes(chainId);

      let udagBalance = '0';
      let udagBalanceRaw = 0n;
      let udagAllowance = 0n;
      let bridgeActive = false;
      let bridgePaused = false;
      let dailyVolume = 0n;
      let dailyCap = 0n;
      let maxPerTx = 0n;
      let nonce = 0n;

      if (CONTRACTS_DEPLOYED && isCorrectChain) {
        const token = new Contract(UDAG_TOKEN_ADDRESS, UDAG_TOKEN_ABI, signer);
        const bridge = new Contract(UDAG_BRIDGE_ADDRESS, UDAG_BRIDGE_ABI, signer);

        const [bal, allowance, active, paused, dv, dc, mpt, n] = await Promise.all([
          token.balanceOf(address).catch(() => 0n),
          token.allowance(address, UDAG_BRIDGE_ADDRESS).catch(() => 0n),
          bridge.bridgeActive().catch(() => false),
          bridge.paused().catch(() => false),
          bridge.dailyVolume().catch(() => 0n),
          bridge.DAILY_VOLUME_CAP().catch(() => 0n),
          bridge.MAX_BRIDGE_PER_TX().catch(() => 0n),
          bridge.nonce().catch(() => 0n),
        ]);

        udagBalanceRaw = BigInt(bal);
        udagBalance = formatUnits(bal, 8);
        udagAllowance = BigInt(allowance);
        bridgeActive = active;
        bridgePaused = paused;
        dailyVolume = BigInt(dv);
        dailyCap = BigInt(dc);
        maxPerTx = BigInt(mpt);
        nonce = BigInt(n);
      }

      setState({
        connected: true,
        address,
        chainId,
        balance: formatUnits(ethBal, 18),
        udagBalance,
        udagBalanceRaw,
        udagAllowance,
        isCorrectChain,
        bridgeActive,
        bridgePaused,
        dailyVolume,
        dailyCap,
        maxPerTx,
        nonce,
      });
    } catch (e) {
      console.error('Failed to fetch balances:', e);
    }
  }, []);

  const connect = useCallback(async () => {
    const ethereum = getEthereum();
    if (!ethereum) {
      setError('MetaMask or compatible wallet not detected. Please install MetaMask.');
      return;
    }

    setLoading(true);
    setError('');
    try {
      const provider = new BrowserProvider(ethereum);
      const accounts = await provider.send('eth_requestAccounts', []);
      if (accounts.length === 0) {
        setError('No accounts found');
        return;
      }
      await fetchBalances(provider, accounts[0]);
    } catch (e: any) {
      if (e.code === 4001) {
        setError('Connection rejected by user');
      } else {
        setError(e.message || 'Failed to connect wallet');
      }
    } finally {
      setLoading(false);
    }
  }, [fetchBalances]);

  const disconnect = useCallback(() => {
    setState(defaultState);
    setError('');
  }, []);

  const switchToArbitrum = useCallback(async () => {
    const ethereum = getEthereum();
    if (!ethereum) {
      setError('No Ethereum wallet detected. Please install MetaMask.');
      return;
    }
    try {
      await ethereum.request({
        method: 'wallet_switchEthereumChain',
        params: [{ chainId: '0x' + ARBITRUM_CHAIN_ID.toString(16) }],
      });
    } catch (e: any) {
      if (e.code === 4902) {
        // Chain not added, add it
        try {
          await ethereum.request({
            method: 'wallet_addEthereumChain',
            params: [{
              chainId: '0x' + ARBITRUM_CHAIN_ID.toString(16),
              chainName: 'Arbitrum One',
              nativeCurrency: { name: 'ETH', symbol: 'ETH', decimals: 18 },
              rpcUrls: ['https://arb1.arbitrum.io/rpc'],
              blockExplorerUrls: ['https://arbiscan.io'],
            }],
          });
        } catch (addError) {
          setError('Failed to add Arbitrum network. Please add it manually in your wallet.');
        }
      } else if (e.code === 4001) {
        setError('Network switch rejected by user');
      } else {
        setError('Failed to switch to Arbitrum. Please switch manually in your wallet.');
      }
    }
  }, []);

  const approve = useCallback(async (amount: bigint): Promise<boolean> => {
    const ethereum = getEthereum();
    if (!ethereum || !CONTRACTS_DEPLOYED) return false;
    try {
      const provider = new BrowserProvider(ethereum);
      const signer = await provider.getSigner();
      const token = new Contract(UDAG_TOKEN_ADDRESS, UDAG_TOKEN_ABI, signer);
      const tx = await token.approve(UDAG_BRIDGE_ADDRESS, amount);
      await tx.wait();
      // Refresh balances
      const accounts = await provider.send('eth_accounts', []);
      if (accounts[0]) await fetchBalances(provider, accounts[0]);
      return true;
    } catch (e: any) {
      setError(e.reason || e.message || 'Approval failed');
      return false;
    }
  }, [fetchBalances]);

  const bridgeToNative = useCallback(async (nativeRecipient: string, amount: bigint): Promise<string | null> => {
    const ethereum = getEthereum();
    if (!ethereum || !CONTRACTS_DEPLOYED) return null;
    try {
      const provider = new BrowserProvider(ethereum);
      const signer = await provider.getSigner();
      const bridge = new Contract(UDAG_BRIDGE_ADDRESS, UDAG_BRIDGE_ABI, signer);
      // Convert hex address to bytes20
      const recipientBytes = '0x' + nativeRecipient.replace(/^0x/, '').padStart(40, '0');
      const tx = await bridge.bridgeToNative(recipientBytes, amount);
      const receipt = await tx.wait();
      // Refresh balances
      const accounts = await provider.send('eth_accounts', []);
      if (accounts[0]) await fetchBalances(provider, accounts[0]);
      return receipt.hash;
    } catch (e: any) {
      setError(e.reason || e.message || 'Bridge transaction failed');
      return null;
    }
  }, [fetchBalances]);

  const refundBridge = useCallback(async (bridgeNonce: bigint): Promise<boolean> => {
    const ethereum = getEthereum();
    if (!ethereum || !CONTRACTS_DEPLOYED) return false;
    try {
      const provider = new BrowserProvider(ethereum);
      const signer = await provider.getSigner();
      const bridge = new Contract(UDAG_BRIDGE_ADDRESS, UDAG_BRIDGE_ABI, signer);
      const tx = await bridge.refundBridge(bridgeNonce);
      await tx.wait();
      const accounts = await provider.send('eth_accounts', []);
      if (accounts[0]) await fetchBalances(provider, accounts[0]);
      return true;
    } catch (e: any) {
      setError(e.reason || e.message || 'Refund failed');
      return false;
    }
  }, [fetchBalances]);

  // Listen for account/chain changes
  useEffect(() => {
    const ethereum = getEthereum();
    if (!ethereum) return;

    const handleAccountsChanged = async (accounts: string[]) => {
      if (accounts.length === 0) {
        disconnect();
      } else if (state.connected) {
        const provider = new BrowserProvider(ethereum);
        await fetchBalances(provider, accounts[0]);
      }
    };

    const handleChainChanged = () => {
      // Reload to reflect new chain
      if (state.connected) {
        const provider = new BrowserProvider(ethereum);
        provider.send('eth_accounts', []).then((accounts: string[]) => {
          if (accounts[0]) fetchBalances(provider, accounts[0]);
        });
      }
    };

    ethereum.on('accountsChanged', handleAccountsChanged);
    ethereum.on('chainChanged', handleChainChanged);

    return () => {
      ethereum.removeListener('accountsChanged', handleAccountsChanged);
      ethereum.removeListener('chainChanged', handleChainChanged);
    };
  }, [state.connected, disconnect, fetchBalances]);

  // No auto-reconnect — user must explicitly click "Connect Wallet"

  return {
    ...state,
    loading,
    error,
    hasMetaMask: !!getEthereum(),
    contractsDeployed: CONTRACTS_DEPLOYED,
    connect,
    disconnect,
    switchToArbitrum,
    approve,
    bridgeToNative,
    refundBridge,
    clearError: () => setError(''),
    parseUdag: (udag: string) => parseUnits(udag, 8),
  };
}
