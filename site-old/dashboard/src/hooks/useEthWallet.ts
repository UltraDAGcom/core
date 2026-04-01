import { useState, useCallback, useEffect, useRef } from 'react';
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

export interface DiscoveredWallet {
  uuid: string;
  name: string;
  icon: string; // data URI or URL
  rdns: string; // reverse DNS identifier
  provider: any; // EIP-1193 provider
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

const safeBigInt = (v: any): bigint => {
  try {
    return BigInt(v);
  } catch {
    return 0n;
  }
};

// EIP-6963 event types
interface EIP6963ProviderInfo {
  uuid: string;
  name: string;
  icon: string;
  rdns: string;
}

interface EIP6963ProviderDetail {
  info: EIP6963ProviderInfo;
  provider: any;
}

interface EIP6963AnnounceEvent extends Event {
  detail: EIP6963ProviderDetail;
}

// Detect wallet name/icon from injected provider flags
function identifyInjectedProvider(provider: any): { name: string; icon: string } {
  if (provider.isMetaMask) {
    return {
      name: 'MetaMask',
      icon: 'data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHdpZHRoPSIyMTIiIGhlaWdodD0iMTg5Ij48ZyBmaWxsPSJub25lIiBmaWxsLXJ1bGU9ImV2ZW5vZGQiPjxwb2x5Z29uIGZpbGw9IiNDREJEQjIiIHBvaW50cz0iNjAuNzUgMTczLjI1IDkwLjc1IDE4Ny44NzUgOTAuNzUgMTcxIDkwLjc1IDE1Ny41IDY4LjYyNSAxNjQuNjI1Ii8+PHBvbHlnb24gZmlsbD0iI0NEQkRCMiIgcG9pbnRzPSIxMDUuNzUgMTczLjI1IDc1Ljc1IDE4Ny44NzUgNzUuNzUgMTcxIDc1Ljc1IDE1Ny41IDk3Ljg3NSAxNjQuNjI1Ii8+PHBvbHlnb24gZmlsbD0iIzM5MzkzOSIgcG9pbnRzPSI5MC43NSAxNTAuMzc1IDY4LjYyNSAxNjQuNjI1IDc2LjUgMTQ1LjEyNSA2My4zNzUgMTQ4Ljg3NSIvPjxwb2x5Z29uIGZpbGw9IiMzOTM5MzkiIHBvaW50cz0iNzUuNzUgMTUwLjM3NSA5Ny44NzUgMTY0LjYyNSA5MCAxNDUuMTI1IDEwMy4xMjUgMTQ4Ljg3NSIvPjwvZz48L3N2Zz4=',
    };
  }
  if (provider.isPhantom) {
    return {
      name: 'Phantom',
      icon: 'data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iMTI4IiBoZWlnaHQ9IjEyOCIgdmlld0JveD0iMCAwIDEyOCAxMjgiIGZpbGw9Im5vbmUiIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyI+PHJlY3Qgd2lkdGg9IjEyOCIgaGVpZ2h0PSIxMjgiIHJ4PSIyNCIgZmlsbD0iIzU0MUQ5RSIvPjwvc3ZnPg==',
    };
  }
  if (provider.isCoinbaseWallet) {
    return {
      name: 'Coinbase Wallet',
      icon: 'data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iMTI4IiBoZWlnaHQ9IjEyOCIgdmlld0JveD0iMCAwIDEyOCAxMjgiIGZpbGw9Im5vbmUiIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyI+PHJlY3Qgd2lkdGg9IjEyOCIgaGVpZ2h0PSIxMjgiIHJ4PSIyNCIgZmlsbD0iIzAwNTJGRiIvPjwvc3ZnPg==',
    };
  }
  if (provider.isBraveWallet) {
    return {
      name: 'Brave Wallet',
      icon: 'data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iMTI4IiBoZWlnaHQ9IjEyOCIgdmlld0JveD0iMCAwIDEyOCAxMjgiIGZpbGw9Im5vbmUiIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyI+PHJlY3Qgd2lkdGg9IjEyOCIgaGVpZ2h0PSIxMjgiIHJ4PSIyNCIgZmlsbD0iI0ZCNTQyQiIvPjwvc3ZnPg==',
    };
  }
  if (provider.isRabby) {
    return {
      name: 'Rabby',
      icon: 'data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iMTI4IiBoZWlnaHQ9IjEyOCIgdmlld0JveD0iMCAwIDEyOCAxMjgiIGZpbGw9Im5vbmUiIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyI+PHJlY3Qgd2lkdGg9IjEyOCIgaGVpZ2h0PSIxMjgiIHJ4PSIyNCIgZmlsbD0iIzgxNjdGNSIvPjwvc3ZnPg==',
    };
  }
  return {
    name: 'Browser Wallet',
    icon: 'data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iMTI4IiBoZWlnaHQ9IjEyOCIgdmlld0JveD0iMCAwIDEyOCAxMjgiIGZpbGw9Im5vbmUiIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyI+PHJlY3Qgd2lkdGg9IjEyOCIgaGVpZ2h0PSIxMjgiIHJ4PSIyNCIgZmlsbD0iIzMzNDE1NSIvPjwvc3ZnPg==',
  };
}

export function useEthWallet() {
  const [state, setState] = useState<EthWalletState>(defaultState);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const [discoveredWallets, setDiscoveredWallets] = useState<DiscoveredWallet[]>([]);
  const [selectedWallet, setSelectedWallet] = useState<{ name: string; icon: string } | null>(null);

  // Ref to the currently selected provider so all operations use the same wallet
  const selectedProviderRef = useRef<any>(null);
  // Ref to track whether EIP-6963 discovered any wallets
  const eip6963DiscoveredRef = useRef(false);

  // --- EIP-6963 Wallet Discovery ---
  useEffect(() => {
    const walletMap = new Map<string, DiscoveredWallet>();

    const handleAnnounce = (event: Event) => {
      const e = event as EIP6963AnnounceEvent;
      if (!e.detail?.info?.uuid || !e.detail?.provider) return;
      const { info, provider } = e.detail;
      walletMap.set(info.uuid, {
        uuid: info.uuid,
        name: info.name,
        icon: info.icon,
        rdns: info.rdns,
        provider,
      });
      eip6963DiscoveredRef.current = true;
      setDiscoveredWallets(Array.from(walletMap.values()));
    };

    window.addEventListener('eip6963:announceProvider', handleAnnounce);
    // Request all providers to announce themselves
    window.dispatchEvent(new Event('eip6963:requestProvider'));

    // Fallback: if no EIP-6963 wallets discovered after 500ms, check window.ethereum
    const fallbackTimeout = setTimeout(() => {
      if (eip6963DiscoveredRef.current) return; // EIP-6963 wallets already found

      const ethereum = (window as any).ethereum;
      if (!ethereum) return;

      // EIP-5749: check for providers array (multiple injected providers)
      const providers: any[] = ethereum.providers || [ethereum];
      const fallbackWallets: DiscoveredWallet[] = [];
      const seen = new Set<string>();

      for (const prov of providers) {
        const identified = identifyInjectedProvider(prov);
        // Deduplicate by name
        if (seen.has(identified.name)) continue;
        seen.add(identified.name);

        fallbackWallets.push({
          uuid: `fallback-${identified.name.toLowerCase().replace(/\s+/g, '-')}`,
          name: identified.name,
          icon: identified.icon,
          rdns: '',
          provider: prov,
        });
      }

      if (fallbackWallets.length > 0) {
        setDiscoveredWallets(fallbackWallets);
      }
    }, 500);

    return () => {
      window.removeEventListener('eip6963:announceProvider', handleAnnounce);
      clearTimeout(fallbackTimeout);
    };
  }, []);

  // --- Helper: get the active provider ---
  const getProvider = useCallback((): any => {
    if (selectedProviderRef.current) return selectedProviderRef.current;
    // Fallback to window.ethereum if nothing selected
    return (window as any).ethereum || null;
  }, []);

  // --- Fetch balances from the selected provider ---
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

        udagBalanceRaw = safeBigInt(bal);
        udagBalance = formatUnits(udagBalanceRaw, 8);
        udagAllowance = safeBigInt(allowance);
        bridgeActive = active;
        bridgePaused = paused;
        dailyVolume = safeBigInt(dv);
        dailyCap = safeBigInt(dc);
        maxPerTx = safeBigInt(mpt);
        nonce = safeBigInt(n);
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
    } catch (err: any) {
      setError(err?.message || 'Failed to fetch balances');
    }
  }, []);

  // --- Connect to a specific wallet (by uuid) or the first available ---
  const connect = useCallback(async (walletUuid?: string) => {
    setLoading(true);
    setError('');

    let targetProvider: any = null;
    let walletInfo: { name: string; icon: string } | null = null;

    if (walletUuid) {
      // Find the specific wallet by uuid
      const wallet = discoveredWallets.find(w => w.uuid === walletUuid);
      if (wallet) {
        targetProvider = wallet.provider;
        walletInfo = { name: wallet.name, icon: wallet.icon };
      }
    }

    if (!targetProvider && discoveredWallets.length > 0) {
      // Use the first discovered wallet
      const first = discoveredWallets[0];
      targetProvider = first.provider;
      walletInfo = { name: first.name, icon: first.icon };
    }

    if (!targetProvider) {
      // Final fallback to window.ethereum
      const ethereum = (window as any).ethereum;
      if (ethereum) {
        targetProvider = ethereum;
        const identified = identifyInjectedProvider(ethereum);
        walletInfo = { name: identified.name, icon: identified.icon };
      }
    }

    if (!targetProvider) {
      setError('No Ethereum wallet detected. Please install MetaMask or another Web3 wallet.');
      setLoading(false);
      return;
    }

    try {
      selectedProviderRef.current = targetProvider;
      setSelectedWallet(walletInfo);

      const provider = new BrowserProvider(targetProvider);
      const accounts = await provider.send('eth_requestAccounts', []);
      if (accounts.length === 0) {
        setError('No accounts found');
        selectedProviderRef.current = null;
        setSelectedWallet(null);
        return;
      }
      await fetchBalances(provider, accounts[0]);
    } catch (e: any) {
      selectedProviderRef.current = null;
      setSelectedWallet(null);
      if (e.code === 4001) {
        setError('Connection rejected by user');
      } else {
        setError(e.message || 'Failed to connect wallet');
      }
    } finally {
      setLoading(false);
    }
  }, [fetchBalances, discoveredWallets]);

  // --- Disconnect ---
  const disconnect = useCallback(async () => {
    // Try to revoke wallet permissions if supported
    try {
      const provider = getProvider();
      if (provider?.request) {
        await provider.request({ method: 'wallet_revokePermissions', params: [{ eth_accounts: {} }] });
      }
    } catch {} // Not all wallets support this
    selectedProviderRef.current = null;
    setSelectedWallet(null);
    setState(defaultState);
    setError('');
  }, [getProvider]);

  // --- Switch to Arbitrum ---
  const switchToArbitrum = useCallback(async () => {
    const ethereum = getProvider();
    if (!ethereum) {
      setError('No Ethereum wallet detected. Please install a Web3 wallet.');
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
  }, [getProvider]);

  // --- Approve token spend ---
  const approve = useCallback(async (_amount: bigint): Promise<boolean> => {
    const ethereum = getProvider();
    if (!ethereum || !CONTRACTS_DEPLOYED) return false;
    try {
      const provider = new BrowserProvider(ethereum);
      const signer = await provider.getSigner();
      const token = new Contract(UDAG_TOKEN_ADDRESS, UDAG_TOKEN_ABI, signer);
      const MAX_UINT256 = 2n ** 256n - 1n;
      const tx = await token.approve(UDAG_BRIDGE_ADDRESS, MAX_UINT256);
      await tx.wait();
      // Refresh balances
      const accounts = await provider.send('eth_accounts', []);
      if (accounts[0]) await fetchBalances(provider, accounts[0]);
      return true;
    } catch (e: any) {
      setError(e.reason || e.message || 'Approval failed');
      return false;
    }
  }, [fetchBalances, getProvider]);

  // --- Bridge to native ---
  const bridgeToNative = useCallback(async (nativeRecipient: string, amount: bigint): Promise<{ hash: string | null; error: string | null }> => {
    if (!state.isCorrectChain) return { hash: null, error: 'Please switch to Arbitrum first' };
    const ethereum = getProvider();
    if (!ethereum || !CONTRACTS_DEPLOYED) return { hash: null, error: 'Wallet not connected or contracts not deployed' };
    // Validate recipient is exactly 40 hex chars
    const cleanRecipient = nativeRecipient.replace(/^0x/, '').toLowerCase();
    if (!/^[0-9a-f]{40}$/.test(cleanRecipient)) {
      return { hash: null, error: 'Invalid UltraDAG recipient address (must be 40 hex characters)' };
    }
    try {
      const provider = new BrowserProvider(ethereum);
      const signer = await provider.getSigner();
      const bridge = new Contract(UDAG_BRIDGE_ADDRESS, UDAG_BRIDGE_ABI, signer);
      const recipientBytes = '0x' + cleanRecipient;
      const tx = await bridge.bridgeToNative(recipientBytes, amount);
      const receipt = await tx.wait();
      // Refresh balances
      const accounts = await provider.send('eth_accounts', []);
      if (accounts[0]) await fetchBalances(provider, accounts[0]);
      return { hash: receipt.hash, error: null };
    } catch (e: any) {
      const errMsg = e.reason || e.message || 'Bridge transaction failed';
      setError(errMsg);
      return { hash: null, error: errMsg };
    }
  }, [fetchBalances, getProvider, state.isCorrectChain]);

  // --- Claim withdrawal on Arbitrum ---
  const claimWithdrawal = useCallback(async (
    sender: string,
    recipient: string,
    amount: bigint,
    depositNonce: bigint,
    signatures: string[],
  ): Promise<{ success: boolean; error: string | null }> => {
    if (!state.isCorrectChain) return { success: false, error: 'Please switch to Arbitrum first' };
    const ethereum = getProvider();
    if (!ethereum || !CONTRACTS_DEPLOYED) return { success: false, error: 'Wallet not connected or contracts not deployed' };
    try {
      const provider = new BrowserProvider(ethereum);
      const signer = await provider.getSigner();
      const bridge = new Contract(UDAG_BRIDGE_ADDRESS, UDAG_BRIDGE_ABI, signer);
      const tx = await bridge.claimWithdrawal(sender, recipient, amount, depositNonce, signatures);
      await tx.wait();
      // Refresh balances
      const accounts = await provider.send('eth_accounts', []);
      if (accounts[0]) await fetchBalances(provider, accounts[0]);
      return { success: true, error: null };
    } catch (e: any) {
      const errMsg = e.reason || e.message || 'Claim failed';
      setError(errMsg);
      return { success: false, error: errMsg };
    }
  }, [fetchBalances, getProvider, state.isCorrectChain]);

  // --- Refund bridge ---
  const refundBridge = useCallback(async (bridgeNonce: bigint): Promise<boolean> => {
    const ethereum = getProvider();
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
  }, [fetchBalances, getProvider]);

  // --- Listen for account/chain changes on the SELECTED provider ---
  useEffect(() => {
    const ethereum = getProvider();
    if (!ethereum) return;

    const handleAccountsChanged = async (accounts: string[]) => {
      if (accounts.length === 0) {
        disconnect();
      } else if (state.connected) {
        const provider = new BrowserProvider(ethereum);
        await fetchBalances(provider, accounts[0]);
      }
    };

    const handleChainChanged = (chainIdHex: string) => {
      // Immediately update isCorrectChain before async work
      const newChainId = parseInt(chainIdHex, 16);
      const correctChain = ACCEPTED_CHAINS.includes(newChainId);
      setState(prev => ({ ...prev, chainId: newChainId, isCorrectChain: correctChain }));

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
  }, [state.connected, disconnect, fetchBalances, getProvider]);

  // No auto-reconnect -- user must explicitly click "Connect Wallet"

  return {
    ...state,
    loading,
    error,
    discoveredWallets,
    selectedWallet,
    hasMetaMask: discoveredWallets.length > 0 || !!(window as any).ethereum,
    contractsDeployed: CONTRACTS_DEPLOYED,
    connect,
    disconnect,
    switchToArbitrum,
    approve,
    bridgeToNative,
    claimWithdrawal,
    refundBridge,
    clearError: () => setError(''),
    parseUdag: (udag: string) => {
      try {
        return parseUnits(udag, 8);
      } catch {
        return 0n;
      }
    },
  };
}
