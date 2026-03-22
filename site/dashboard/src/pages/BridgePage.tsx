import { useState, useEffect, useRef } from 'react';
import {
  ArrowDown,
  ArrowRight,
  ExternalLink,
  Shield,
  Clock,
  Info,
  Unplug,
  Loader2,
  CheckCircle,
  Wallet,
  X,
  Zap,
  Lock,
  ChevronDown,
  AlertTriangle,
} from 'lucide-react';
import { formatUnits } from 'ethers';
import { Card } from '../components/shared/Card.tsx';
import { useKeystore } from '../hooks/useKeystore.ts';
import { useEthWallet } from '../hooks/useEthWallet.ts';
import type { DiscoveredWallet } from '../hooks/useEthWallet.ts';
import { useToast } from '../hooks/useToast.tsx';
import { normalizeAddress, isValidAddress, formatUdag, formatUdagBigint, shortAddr, getBridgeNonce, getBridgeAttestation, getBridgeReserve, postBridgeDeposit, isConnected } from '../lib/api.ts';
import { CopyButton } from '../components/shared/CopyButton.tsx';
import { WalletSelector } from '../components/shared/WalletSelector.tsx';
import { useWalletBalances } from '../hooks/useWalletBalances.ts';
import { CONTRACTS_DEPLOYED } from '../lib/contracts.ts';

interface BridgeAttestation {
  nonce: number;
  sender: string;
  sender_bech32: string;
  recipient: string;
  amount: number;
  amount_udag: number;
  destination_chain_id: number;
  signature_count: number;
  threshold: number;
  ready: boolean;
  proof?: {
    signatures?: string[];
    sender_eth?: string;
    recipient_eth?: string;
    amount_raw?: string;
  };
}

// --- Wallet Picker Modal ---
function WalletPickerModal({
  wallets,
  onSelect,
  onClose,
}: {
  wallets: DiscoveredWallet[];
  onSelect: (uuid: string) => void;
  onClose: () => void;
}) {
  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center p-4 modal-backdrop bg-black/70"
      onClick={(e) => { if (e.target === e.currentTarget) onClose(); }}
    >
      <div className="modal-content bg-dag-card border border-dag-border rounded-2xl shadow-2xl w-full max-w-sm overflow-hidden">
        <div className="flex items-center justify-between p-5 pb-3">
          <h2 className="text-lg font-semibold text-white">Connect Wallet</h2>
          <button
            onClick={onClose}
            className="p-1.5 rounded-lg text-dag-muted hover:text-white hover:bg-dag-surface transition-colors"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        {wallets.length === 0 ? (
          <div className="px-5 pb-6 space-y-4">
            <div className="text-center py-6">
              <Wallet className="w-10 h-10 text-dag-muted mx-auto mb-3 opacity-40" />
              <p className="text-sm text-dag-muted">No wallets detected</p>
              <p className="text-xs text-dag-muted mt-1">Install a Web3 wallet to get started</p>
            </div>
            <a
              href="https://metamask.io/download/"
              target="_blank"
              rel="noopener noreferrer"
              className="flex items-center gap-3 w-full p-3 rounded-xl bg-dag-surface border border-dag-border hover:border-dag-accent/40 transition-all group"
            >
              <div className="w-9 h-9 rounded-lg bg-[#F6851B]/10 flex items-center justify-center">
                <span className="text-lg">🦊</span>
              </div>
              <div className="flex-1 text-left">
                <span className="text-sm font-medium text-white group-hover:text-dag-accent transition-colors">Install MetaMask</span>
                <p className="text-[10px] text-dag-muted">The most popular Ethereum wallet</p>
              </div>
              <ExternalLink className="w-3.5 h-3.5 text-dag-muted group-hover:text-dag-accent transition-colors" />
            </a>
            <a
              href="https://rabby.io/"
              target="_blank"
              rel="noopener noreferrer"
              className="flex items-center gap-3 w-full p-3 rounded-xl bg-dag-surface border border-dag-border hover:border-dag-accent/40 transition-all group"
            >
              <div className="w-9 h-9 rounded-lg bg-[#8167F5]/10 flex items-center justify-center">
                <span className="text-lg">🐰</span>
              </div>
              <div className="flex-1 text-left">
                <span className="text-sm font-medium text-white group-hover:text-dag-accent transition-colors">Install Rabby</span>
                <p className="text-[10px] text-dag-muted">Multi-chain wallet with security focus</p>
              </div>
              <ExternalLink className="w-3.5 h-3.5 text-dag-muted group-hover:text-dag-accent transition-colors" />
            </a>
          </div>
        ) : (
          <div className="px-5 pb-5 space-y-2">
            <p className="text-xs text-dag-muted mb-3">Choose your wallet to connect</p>
            {wallets.map((w) => (
              <button
                key={w.uuid}
                onClick={() => onSelect(w.uuid)}
                className="flex items-center gap-3 w-full p-3 rounded-xl bg-dag-surface border border-dag-border hover:border-dag-accent/40 hover:bg-dag-surface/80 transition-all group"
              >
                <img
                  src={w.icon}
                  alt={w.name}
                  className="w-9 h-9 rounded-lg object-contain"
                  onError={(e) => {
                    (e.target as HTMLImageElement).style.display = 'none';
                  }}
                />
                <span className="text-sm font-medium text-white group-hover:text-dag-accent transition-colors flex-1 text-left">
                  {w.name}
                </span>
                <ArrowRight className="w-4 h-4 text-dag-muted group-hover:text-dag-accent transition-colors opacity-0 group-hover:opacity-100" />
              </button>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

export function BridgePage() {
  const { wallets } = useKeystore();
  const eth = useEthWallet();
  const { toast } = useToast();

  const [direction, setDirection] = useState<'to-native' | 'to-arbitrum'>('to-native');
  const [amount, setAmount] = useState('');
  const [nativeAddress, setNativeAddress] = useState('');
  const [selectedWalletIdx, setSelectedWalletIdx] = useState(0);
  const [bridging, setBridging] = useState(false);
  const [approving, setApproving] = useState(false);
  const [txHash, setTxHash] = useState('');
  const [claiming, setClaiming] = useState<number | null>(null);
  const [bridgeReserve, setBridgeReserve] = useState<{ reserve_sats: number; reserve_udag: number } | null>(null);
  const [attestations, setAttestations] = useState<BridgeAttestation[]>([]);
  const [loadingAttestations, setLoadingAttestations] = useState(false);
  const [showWalletPicker, setShowWalletPicker] = useState(false);
  const isMounted = useRef(true);

  // Native -> Arbitrum deposit state
  const [depositWalletIdx, setDepositWalletIdx] = useState(0);
  const [depositAmount, setDepositAmount] = useState('');
  const [depositRecipient, setDepositRecipient] = useState('');
  const [depositing, setDepositing] = useState(false);
  const [depositTxHash, setDepositTxHash] = useState('');
  const { balances: walletBalances } = useWalletBalances(wallets, isConnected());

  const wallet = wallets[selectedWalletIdx];
  // The validator federation bridge is always active on the native side.
  // Arbitrum contract status only matters when contracts are deployed.
  const bridgeActive = eth.contractsDeployed ? eth.bridgeActive : true;
  const bridgePaused = eth.contractsDeployed ? eth.bridgePaused : false;
  const canBridge = bridgeActive && !bridgePaused;

  // Parse amount to sats
  const amountSats = (() => {
    try {
      return eth.parseUdag(amount || '0');
    } catch {
      return 0n;
    }
  })();
  // Only show approval when Arbitrum contracts are actually deployed
  const needsApproval = CONTRACTS_DEPLOYED && eth.connected && amountSats > 0n && eth.udagAllowance < amountSats;

  // Track component mount state
  useEffect(() => {
    isMounted.current = true;
    return () => { isMounted.current = false; };
  }, []);

  // Display eth.error in toast when it changes
  useEffect(() => {
    if (eth.error) {
      toast(eth.error, 'error');
    }
  }, [eth.error]);

  // Fetch bridge reserve on mount
  useEffect(() => {
    const fetchReserve = async () => {
      try {
        const reserve = await getBridgeReserve();
        if (isMounted.current) setBridgeReserve(reserve);
      } catch (e) {
        // Node might not have bridge endpoints yet
      }
    };
    fetchReserve();
    const interval = setInterval(fetchReserve, 30000);
    return () => clearInterval(interval);
  }, []);

  // Fetch recent attestations (parallel fetch, isMounted guard, tab-gated + backoff)
  const consecutiveErrorsRef = useRef(0);
  useEffect(() => {
    if (direction !== 'to-native') return;

    const fetchAttestations = async () => {
      if (isMounted.current) setLoadingAttestations(true);
      try {
        const nonceRes = await getBridgeNonce();
        const indices: number[] = [];
        for (let i = Math.max(0, nonceRes.next_nonce - 5); i < nonceRes.next_nonce; i++) {
          indices.push(i);
        }
        const results = await Promise.all(
          indices.map(i => getBridgeAttestation(i).catch(() => null))
        );
        const recent = results.filter((att): att is BridgeAttestation => att !== null);
        if (isMounted.current) setAttestations(recent.reverse());
        consecutiveErrorsRef.current = 0;
      } catch (e) {
        consecutiveErrorsRef.current += 1;
      } finally {
        if (isMounted.current) setLoadingAttestations(false);
      }
    };
    fetchAttestations();

    const getInterval = () => Math.min(10000 * Math.pow(2, consecutiveErrorsRef.current), 60000);
    let timeoutId: ReturnType<typeof setTimeout>;
    const scheduleNext = () => {
      timeoutId = setTimeout(async () => {
        await fetchAttestations();
        if (isMounted.current) scheduleNext();
      }, getInterval());
    };
    scheduleNext();

    return () => clearTimeout(timeoutId);
  }, [direction]);

  const handleConnectClick = () => {
    if (eth.discoveredWallets.length > 1) {
      setShowWalletPicker(true);
    } else if (eth.discoveredWallets.length === 1) {
      eth.connect(eth.discoveredWallets[0].uuid);
    } else if (eth.hasMetaMask) {
      eth.connect();
    } else {
      setShowWalletPicker(true);
    }
  };

  const handleWalletSelect = (uuid: string) => {
    setShowWalletPicker(false);
    eth.connect(uuid);
  };

  const handleApprove = async () => {
    setApproving(true);
    const ok = await eth.approve(amountSats);
    setApproving(false);
    if (ok) toast('Approval confirmed', 'success');
  };

  const handleBridgeToNative = async () => {
    if (!eth.connected) return;
    eth.clearError();

    const rawAddress = wallet ? wallet.address : nativeAddress;
    if (!isValidAddress(rawAddress)) {
      toast('Invalid UltraDAG recipient address', 'error');
      return;
    }
    const recipient = normalizeAddress(rawAddress);
    if (!/^[0-9a-f]{40}$/.test(recipient)) {
      toast('Invalid UltraDAG recipient address (could not normalize to 40 hex chars)', 'error');
      return;
    }
    if (amountSats <= 0n) {
      toast('Enter a valid amount', 'error');
      return;
    }

    // Balance sufficiency check
    if (amountSats > eth.udagBalanceRaw) {
      toast('Insufficient UDAG balance', 'error');
      return;
    }

    // Per-tx and daily cap validation
    const effectiveMaxPerTx = eth.maxPerTx > 0n ? eth.maxPerTx : 0n;
    if (effectiveMaxPerTx > 0n && amountSats > effectiveMaxPerTx) {
      toast(`Amount exceeds per-transaction limit of ${formatUdagBigint(effectiveMaxPerTx)} UDAG`, 'error');
      return;
    }
    const effectiveDailyCap = dailyCap;
    if (effectiveDailyCap > 0n && dailyVolume + amountSats > effectiveDailyCap) {
      toast('Amount would exceed daily bridge limit', 'error');
      return;
    }

    setBridging(true);
    setTxHash('');
    const result = await eth.bridgeToNative(recipient, amountSats);
    setBridging(false);
    if (result.hash) {
      setTxHash(result.hash);
      setAmount('');
      toast('Bridge transfer submitted! Tokens escrowed.', 'success');
    } else if (result.error) {
      toast(result.error, 'error');
    }
  };

  // Claim withdrawal on Arbitrum
  const handleClaim = async (att: BridgeAttestation) => {
    if (!eth.connected) {
      toast('Connect your Arbitrum wallet first', 'error');
      return;
    }
    eth.clearError();

    const signatures = att.proof?.signatures;
    if (!signatures || signatures.length === 0) {
      toast('No signatures available for this attestation', 'error');
      return;
    }

    const senderEth = att.proof?.sender_eth;
    const recipientEth = att.proof?.recipient_eth;

    const isValidEthAddr = (addr: string | undefined): addr is string =>
      typeof addr === 'string' && /^0x[0-9a-fA-F]{40}$/.test(addr);
    if (!isValidEthAddr(senderEth) || !isValidEthAddr(recipientEth)) {
      toast('Invalid Ethereum addresses in attestation proof', 'error');
      return;
    }

    let amountRaw: bigint;
    let nonceValue: bigint;
    try {
      amountRaw = att.proof?.amount_raw ? BigInt(att.proof.amount_raw) : BigInt(att.amount);
      nonceValue = BigInt(att.nonce);
    } catch {
      toast('Invalid numeric data in attestation', 'error');
      return;
    }

    setClaiming(att.nonce);
    const result = await eth.claimWithdrawal(
      senderEth,
      recipientEth,
      amountRaw,
      nonceValue,
      signatures,
    );
    setClaiming(null);

    if (result.success) {
      toast(`Withdrawal #${att.nonce} claimed successfully!`, 'success');
    } else if (result.error) {
      toast(result.error, 'error');
    }
  };

  // Handle Native -> Arbitrum bridge deposit
  const handleBridgeToArbitrum = async () => {
    const depositWallet = wallets[depositWalletIdx];
    if (!depositWallet) {
      toast('Select a wallet first', 'error');
      return;
    }

    // Validate recipient (0x-prefixed Ethereum address)
    const recipient = depositRecipient.trim();
    if (!/^0x[0-9a-fA-F]{40}$/.test(recipient)) {
      toast('Invalid Ethereum address: must be 0x-prefixed 40-char hex', 'error');
      return;
    }

    // Parse amount to sats
    let sats: number;
    try {
      const parsed = parseFloat(depositAmount || '0');
      if (isNaN(parsed) || parsed <= 0) {
        toast('Enter a valid amount', 'error');
        return;
      }
      sats = Math.round(parsed * 100_000_000);
    } catch {
      toast('Invalid amount', 'error');
      return;
    }

    // Min 1 UDAG (100_000_000 sats)
    if (sats < 100_000_000) {
      toast('Minimum bridge amount is 1 UDAG', 'error');
      return;
    }

    // Max 100,000 UDAG
    if (sats > 100_000 * 100_000_000) {
      toast('Maximum bridge amount is 100,000 UDAG', 'error');
      return;
    }

    // Balance check
    const wb = walletBalances.get(depositWallet.address);
    const balance = wb?.balance ?? 0;
    const fee = 10_000; // MIN_FEE_SATS
    if (balance < sats + fee) {
      toast(`Insufficient balance: need ${formatUdag(sats + fee)} UDAG, have ${formatUdag(balance)} UDAG`, 'error');
      return;
    }

    setDepositing(true);
    setDepositTxHash('');
    try {
      const result = await postBridgeDeposit({
        secret_key: depositWallet.secret_key,
        recipient,
        amount: sats,
        fee,
        destination_chain_id: 42161,
      });
      setDepositTxHash(result.tx_hash);
      setDepositAmount('');
      toast('Bridge deposit submitted! Validators will sign attestations.', 'success');
    } catch (e: any) {
      const msg = e?.message || String(e);
      try {
        const parsed = JSON.parse(msg);
        toast(parsed.error || msg, 'error');
      } catch {
        toast(msg, 'error');
      }
    } finally {
      setDepositing(false);
    }
  };

  const depositAmountInputHandler = (e: React.ChangeEvent<HTMLInputElement>) => {
    const val = e.target.value.replace(/[^0-9.]/g, '');
    const parts = val.split('.');
    const sanitized = parts[0] + (parts.length > 1 ? '.' + parts.slice(1).join('') : '');
    if (parts.length > 1 && parts[1].length > 8) return;
    setDepositAmount(sanitized);
  };

  // Format bridge stats from contract
  const dailyCap = eth.contractsDeployed && eth.dailyCap > 0n ? eth.dailyCap : 0n;
  const dailyVolume = eth.dailyVolume;

  const amountInputHandler = (e: React.ChangeEvent<HTMLInputElement>) => {
    const val = e.target.value.replace(/[^0-9.]/g, '');
    const parts = val.split('.');
    const sanitized = parts[0] + (parts.length > 1 ? '.' + parts.slice(1).join('') : '');
    if (parts.length > 1 && parts[1].length > 8) return;
    setAmount(sanitized);
  };

  return (
    <div className="animate-page-enter">
      {/* Wallet Picker Modal */}
      {showWalletPicker && (
        <WalletPickerModal
          wallets={eth.discoveredWallets}
          onSelect={handleWalletSelect}
          onClose={() => setShowWalletPicker(false)}
        />
      )}

      {/* Hero Header */}
      <div className="mb-8">
        <div className="flex items-center justify-between flex-wrap gap-4">
          <div>
            <h1 className="text-3xl font-bold bg-gradient-to-r from-dag-accent via-dag-blue to-dag-purple bg-clip-text text-transparent">
              UltraDAG Bridge
            </h1>
            <p className="text-sm text-dag-muted mt-1.5">
              Secured by the same validator federation that powers DAG consensus — no external relayers
            </p>
          </div>
          <div className="flex items-center gap-3">
            {/* Bridge status pill */}
            <div className={`flex items-center gap-2 px-3 py-1.5 rounded-full text-xs font-medium border ${
              canBridge
                ? 'bg-dag-green/10 text-dag-green border-dag-green/20'
                : bridgePaused
                  ? 'bg-dag-red/10 text-dag-red border-dag-red/20'
                  : 'bg-dag-yellow/10 text-dag-yellow border-dag-yellow/20'
            }`}>
              <div className={`w-1.5 h-1.5 rounded-full ${canBridge ? 'bg-dag-green animate-pulse' : bridgePaused ? 'bg-dag-red' : 'bg-dag-yellow'}`} />
              {canBridge ? 'Bridge Active' : bridgePaused ? 'Bridge Paused' : 'Inactive'}
            </div>

            {/* Connected wallet pill */}
            {eth.connected && eth.selectedWallet ? (
              <div className="flex items-center gap-2 px-3 py-1.5 rounded-full bg-dag-surface border border-dag-border">
                <img src={eth.selectedWallet.icon} alt="" className="w-4 h-4 rounded" onError={(e) => { (e.target as HTMLImageElement).style.display = 'none'; }} />
                <span className="text-xs text-white font-mono">{eth.address.slice(0, 6)}...{eth.address.slice(-4)}</span>
                <button
                  onClick={eth.disconnect}
                  className="ml-0.5 p-0.5 rounded text-dag-muted hover:text-dag-red transition-colors"
                  title="Disconnect"
                >
                  <X className="w-3 h-3" />
                </button>
              </div>
            ) : eth.connected ? (
              <div className="flex items-center gap-2 px-3 py-1.5 rounded-full bg-dag-surface border border-dag-border">
                <div className="w-2 h-2 rounded-full bg-dag-green" />
                <span className="text-xs text-white font-mono">{eth.address.slice(0, 6)}...{eth.address.slice(-4)}</span>
                <button
                  onClick={eth.disconnect}
                  className="ml-0.5 p-0.5 rounded text-dag-muted hover:text-dag-red transition-colors"
                  title="Disconnect"
                >
                  <X className="w-3 h-3" />
                </button>
              </div>
            ) : null}
          </div>
        </div>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-5 gap-6">
        {/* Left: Bridge Form (3 cols) */}
        <div className="lg:col-span-3 space-y-6">

          {/* Main Bridge Card */}
          <div className="rounded-2xl bg-dag-card border border-dag-border overflow-hidden card-glow">
            <div className="p-6 space-y-5">

              {/* Direction tabs */}
              <div className="flex gap-1 p-1 bg-dag-bg rounded-xl">
                <button
                  onClick={() => setDirection('to-native')}
                  className={`flex-1 py-2.5 px-3 rounded-lg text-sm font-medium transition-all ${
                    direction === 'to-native'
                      ? 'bg-dag-surface text-white shadow-sm'
                      : 'text-dag-muted hover:text-white'
                  }`}
                >
                  <div className="flex items-center justify-center gap-2">
                    <span className="text-[10px] px-1.5 py-0.5 rounded bg-blue-500/20 text-blue-400 font-semibold">ARB</span>
                    <ArrowRight className="w-3.5 h-3.5" />
                    <span className="text-[10px] px-1.5 py-0.5 rounded bg-dag-accent/20 text-dag-accent font-semibold">UDAG</span>
                  </div>
                </button>
                <button
                  onClick={() => setDirection('to-arbitrum')}
                  className={`flex-1 py-2.5 px-3 rounded-lg text-sm font-medium transition-all ${
                    direction === 'to-arbitrum'
                      ? 'bg-dag-surface text-white shadow-sm'
                      : 'text-dag-muted hover:text-white'
                  }`}
                >
                  <div className="flex items-center justify-center gap-2">
                    <span className="text-[10px] px-1.5 py-0.5 rounded bg-dag-accent/20 text-dag-accent font-semibold">UDAG</span>
                    <ArrowRight className="w-3.5 h-3.5" />
                    <span className="text-[10px] px-1.5 py-0.5 rounded bg-blue-500/20 text-blue-400 font-semibold">ARB</span>
                  </div>
                </button>
              </div>

              {/* ---- Arbitrum -> Native ---- */}
              {direction === 'to-native' ? (
                <div className="space-y-3">
                  {/* Source: Arbitrum */}
                  <div className="rounded-xl bg-dag-bg border border-dag-border/50 p-4 space-y-3">
                    <div className="flex items-center justify-between">
                      <span className="text-xs text-dag-muted font-medium uppercase tracking-wider">From</span>
                      <div className="flex items-center gap-1.5">
                        <div className="w-4 h-4 rounded-full bg-blue-500/20 flex items-center justify-center">
                          <div className="w-2 h-2 rounded-full bg-blue-400" />
                        </div>
                        <span className="text-xs font-medium text-blue-400">Arbitrum</span>
                        <span className="text-[10px] px-1.5 py-0.5 rounded bg-blue-500/10 text-blue-400/70 border border-blue-500/20">ERC-20</span>
                      </div>
                    </div>

                    {eth.connected ? (
                      <div className="space-y-3">
                        {/* Wallet info row */}
                        <div className="flex items-center justify-between">
                          <div className="flex items-center gap-2">
                            {eth.selectedWallet && (
                              <img src={eth.selectedWallet.icon} alt="" className="w-5 h-5 rounded" onError={(e) => { (e.target as HTMLImageElement).style.display = 'none'; }} />
                            )}
                            <span className="text-sm text-white font-mono">{eth.address.slice(0, 6)}...{eth.address.slice(-4)}</span>
                            <CopyButton text={eth.address} />
                          </div>
                          <button
                            onClick={eth.disconnect}
                            className="text-[10px] text-dag-muted hover:text-dag-red flex items-center gap-1 transition-colors"
                          >
                            <Unplug className="w-3 h-3" /> Disconnect
                          </button>
                        </div>

                        {/* Amount input */}
                        <div className="relative">
                          <input
                            type="text"
                            inputMode="decimal"
                            value={amount}
                            onChange={amountInputHandler}
                            placeholder="0.00"
                            disabled={!canBridge}
                            className="w-full px-4 py-4 bg-dag-surface border border-dag-border/50 rounded-xl text-2xl font-mono text-white placeholder-dag-muted/30 focus:outline-none focus:border-dag-accent/50 disabled:opacity-50 pr-28"
                          />
                          <div className="absolute right-3 top-1/2 -translate-y-1/2 flex items-center gap-2">
                            {eth.connected && (
                              <button
                                onClick={() => setAmount(formatUnits(eth.udagBalanceRaw, 8))}
                                className="text-[10px] font-bold text-dag-accent hover:text-dag-accent/80 px-2 py-1 rounded bg-dag-accent/10 hover:bg-dag-accent/15 transition-colors"
                              >
                                MAX
                              </button>
                            )}
                            <span className="text-sm font-medium text-dag-muted">UDAG</span>
                          </div>
                        </div>

                        {/* Balance row */}
                        <div className="flex items-center justify-between text-xs">
                          <span className="text-dag-muted">
                            Balance: <span className="text-white font-mono">{Number(eth.udagBalance).toLocaleString(undefined, { maximumFractionDigits: 4 })}</span> UDAG
                          </span>
                          <span className="text-dag-muted">
                            <span className="text-white font-mono">{Number(eth.balance).toFixed(4)}</span> ETH
                          </span>
                        </div>

                        {!eth.isCorrectChain && (
                          <button
                            onClick={eth.switchToArbitrum}
                            className="w-full py-2.5 rounded-xl bg-dag-yellow/10 text-dag-yellow border border-dag-yellow/20 text-xs font-medium hover:bg-dag-yellow/15 transition-colors flex items-center justify-center gap-2"
                          >
                            <AlertTriangle className="w-3.5 h-3.5" />
                            Switch to Arbitrum Network
                          </button>
                        )}
                      </div>
                    ) : (
                      <div className="space-y-3">
                        {!CONTRACTS_DEPLOYED && (
                          <div className="flex items-start gap-2 p-2.5 rounded-lg bg-dag-accent/5 border border-dag-accent/10">
                            <Shield className="w-4 h-4 text-dag-accent mt-0.5 shrink-0" />
                            <p className="text-xs text-dag-muted">
                              Secured by UltraDAG's validator federation — same BFT consensus that protects the network.
                            </p>
                          </div>
                        )}
                        <button
                          onClick={handleConnectClick}
                          disabled={eth.loading}
                          className="w-full py-3.5 rounded-xl bg-gradient-to-r from-dag-accent to-dag-blue text-white font-medium text-sm disabled:opacity-50 flex items-center justify-center gap-2 hover:shadow-lg hover:shadow-dag-accent/20 transition-all"
                        >
                          {eth.loading ? <Loader2 className="w-4 h-4 animate-spin" /> : <Wallet className="w-4 h-4" />}
                          {eth.loading ? 'Connecting...' : 'Connect Wallet'}
                        </button>
                      </div>
                    )}
                  </div>

                  {/* Arrow Divider */}
                  <div className="flex justify-center -my-1 relative z-10">
                    <div className="w-10 h-10 rounded-xl bg-dag-card border border-dag-border flex items-center justify-center shadow-lg">
                      <ArrowDown className="w-4 h-4 text-dag-accent" />
                    </div>
                  </div>

                  {/* Destination: UltraDAG */}
                  <div className="rounded-xl bg-dag-bg border border-dag-border/50 p-4 space-y-3">
                    <div className="flex items-center justify-between">
                      <span className="text-xs text-dag-muted font-medium uppercase tracking-wider">To</span>
                      <div className="flex items-center gap-1.5">
                        <div className="w-4 h-4 rounded-full bg-dag-accent/20 flex items-center justify-center">
                          <div className="w-2 h-2 rounded-full bg-dag-accent" />
                        </div>
                        <span className="text-xs font-medium text-dag-accent">UltraDAG</span>
                        <span className="text-[10px] px-1.5 py-0.5 rounded bg-dag-accent/10 text-dag-accent/70 border border-dag-accent/20">Native</span>
                      </div>
                    </div>

                    {wallets.length > 0 ? (
                      <>
                        <select
                          value={selectedWalletIdx}
                          onChange={(e) => setSelectedWalletIdx(Number(e.target.value))}
                          className="w-full px-4 py-3 bg-dag-surface border border-dag-border/50 rounded-xl text-sm font-mono text-white focus:outline-none focus:border-dag-accent/50 appearance-none cursor-pointer"
                          style={{ backgroundImage: `url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='12' height='12' viewBox='0 0 24 24' fill='none' stroke='%2394a3b8' stroke-width='2'%3E%3Cpolyline points='6 9 12 15 18 9'/%3E%3C/svg%3E")`, backgroundRepeat: 'no-repeat', backgroundPosition: 'right 12px center' }}
                        >
                          {wallets.map((w, i) => (
                            <option key={w.address} value={i}>
                              {w.name || `Wallet ${i + 1}`} — {w.address.slice(0, 10)}...{w.address.slice(-6)}
                            </option>
                          ))}
                        </select>
                        <div className="flex items-center gap-1.5 text-xs text-dag-muted">
                          <CheckCircle className="w-3 h-3 text-dag-green" />
                          <span>Receiving on {wallet?.name || 'your UltraDAG wallet'}</span>
                        </div>
                      </>
                    ) : (
                      <>
                        <input
                          type="text"
                          value={nativeAddress}
                          onChange={(e) => setNativeAddress(e.target.value)}
                          placeholder="tudg1... or 40-char hex address"
                          disabled={!canBridge}
                          className="w-full px-4 py-3 bg-dag-surface border border-dag-border/50 rounded-xl text-sm font-mono text-white placeholder-dag-muted/30 focus:outline-none focus:border-dag-accent/50 disabled:opacity-50"
                        />
                        <p className="text-[10px] text-dag-muted">Enter your UltraDAG address or create a wallet in the Wallet tab</p>
                      </>
                    )}
                  </div>

                  {/* Action Buttons */}
                  <div className="space-y-2.5 pt-1">
                    {/* Approval button */}
                    {needsApproval && (
                      <button
                        onClick={handleApprove}
                        disabled={approving || !canBridge}
                        className="w-full py-3.5 rounded-xl bg-dag-yellow/10 text-dag-yellow border border-dag-yellow/20 font-medium text-sm disabled:opacity-50 flex items-center justify-center gap-2 hover:bg-dag-yellow/15 transition-colors"
                      >
                        {approving ? <Loader2 className="w-4 h-4 animate-spin" /> : <Shield className="w-4 h-4" />}
                        {approving ? 'Approving...' : 'Approve UDAG Transfer'}
                      </button>
                    )}

                    {/* Bridge button */}
                    <button
                      onClick={handleBridgeToNative}
                      disabled={!eth.connected || !canBridge || bridging || needsApproval || amountSats <= 0n}
                      className="w-full py-4 rounded-xl bg-gradient-to-r from-dag-accent to-dag-blue text-white font-semibold text-sm disabled:opacity-40 disabled:cursor-not-allowed flex items-center justify-center gap-2 hover:shadow-lg hover:shadow-dag-accent/20 transition-all"
                    >
                      {bridging ? (
                        <Loader2 className="w-4 h-4 animate-spin" />
                      ) : !eth.connected ? (
                        <Wallet className="w-4 h-4" />
                      ) : (
                        <Zap className="w-4 h-4" />
                      )}
                      {bridging
                        ? 'Bridging...'
                        : !eth.connected
                          ? 'Connect Wallet to Bridge'
                          : !canBridge
                            ? 'Bridge Paused'
                            : needsApproval
                              ? 'Approve First'
                              : amountSats <= 0n
                                ? 'Enter Amount'
                                : 'Bridge to UltraDAG'}
                    </button>
                  </div>

                  {/* Success message */}
                  {txHash && (
                    <div className="rounded-xl bg-dag-green/5 border border-dag-green/20 p-3.5 flex items-center gap-2.5">
                      <div className="w-8 h-8 rounded-full bg-dag-green/10 flex items-center justify-center shrink-0">
                        <CheckCircle className="w-4 h-4 text-dag-green" />
                      </div>
                      <div className="flex-1 min-w-0">
                        <p className="text-xs font-medium text-dag-green">Transaction submitted!</p>
                        <p className="text-[10px] text-dag-muted mt-0.5 truncate font-mono">{txHash}</p>
                      </div>
                      <a
                        href={`https://arbiscan.io/tx/${txHash}`}
                        target="_blank"
                        rel="noopener noreferrer"
                        className="text-xs text-dag-green hover:underline flex items-center gap-1 shrink-0"
                      >
                        View <ExternalLink className="w-3 h-3" />
                      </a>
                    </div>
                  )}
                </div>
              ) : (
                /* ---- Native -> Arbitrum ---- */
                <div className="space-y-3">
                  {/* Source: UltraDAG Wallet */}
                  <div className="rounded-xl bg-dag-bg border border-dag-border/50 p-4 space-y-3">
                    <div className="flex items-center justify-between">
                      <span className="text-xs text-dag-muted font-medium uppercase tracking-wider">From</span>
                      <div className="flex items-center gap-1.5">
                        <div className="w-4 h-4 rounded-full bg-dag-accent/20 flex items-center justify-center">
                          <div className="w-2 h-2 rounded-full bg-dag-accent" />
                        </div>
                        <span className="text-xs font-medium text-dag-accent">UltraDAG</span>
                        <span className="text-[10px] px-1.5 py-0.5 rounded bg-dag-accent/10 text-dag-accent/70 border border-dag-accent/20">Native</span>
                      </div>
                    </div>

                    {wallets.length > 0 ? (
                      <div className="space-y-3">
                        <WalletSelector wallets={wallets} selectedIdx={depositWalletIdx} onChange={setDepositWalletIdx} label="Source Wallet" />

                        {/* Amount input */}
                        <div className="relative">
                          <input
                            type="text"
                            inputMode="decimal"
                            value={depositAmount}
                            onChange={depositAmountInputHandler}
                            placeholder="0.00"
                            className="w-full px-4 py-4 bg-dag-surface border border-dag-border/50 rounded-xl text-2xl font-mono text-white placeholder-dag-muted/30 focus:outline-none focus:border-dag-accent/50 pr-28"
                          />
                          <div className="absolute right-3 top-1/2 -translate-y-1/2 flex items-center gap-2">
                            {(() => {
                              const wb = wallets[depositWalletIdx] ? walletBalances.get(wallets[depositWalletIdx].address) : undefined;
                              const bal = wb?.balance ?? 0;
                              return bal > 10000 ? (
                                <button
                                  onClick={() => setDepositAmount(((bal - 10000) / 100_000_000).toFixed(8).replace(/0+$/, '').replace(/\.$/, ''))}
                                  className="text-[10px] font-bold text-dag-accent hover:text-dag-accent/80 px-2 py-1 rounded bg-dag-accent/10 hover:bg-dag-accent/15 transition-colors"
                                >
                                  MAX
                                </button>
                              ) : null;
                            })()}
                            <span className="text-sm font-medium text-dag-muted">UDAG</span>
                          </div>
                        </div>

                        {/* Balance row */}
                        <div className="flex items-center justify-between text-xs">
                          <span className="text-dag-muted">
                            Balance: <span className="text-white font-mono">
                              {formatUdag(walletBalances.get(wallets[depositWalletIdx]?.address ?? '')?.balance ?? 0)}
                            </span> UDAG
                          </span>
                          <span className="text-dag-muted">
                            Fee: <span className="text-white font-mono">0.0001</span> UDAG
                          </span>
                        </div>
                      </div>
                    ) : (
                      <p className="text-sm text-dag-muted">Create a wallet in the Wallet tab to bridge UDAG to Arbitrum.</p>
                    )}
                  </div>

                  {/* Arrow Divider */}
                  <div className="flex justify-center -my-1 relative z-10">
                    <div className="w-10 h-10 rounded-xl bg-dag-card border border-dag-border flex items-center justify-center shadow-lg">
                      <ArrowDown className="w-4 h-4 text-dag-accent" />
                    </div>
                  </div>

                  {/* Destination: Arbitrum */}
                  <div className="rounded-xl bg-dag-bg border border-dag-border/50 p-4 space-y-3">
                    <div className="flex items-center justify-between">
                      <span className="text-xs text-dag-muted font-medium uppercase tracking-wider">To</span>
                      <div className="flex items-center gap-1.5">
                        <div className="w-4 h-4 rounded-full bg-blue-500/20 flex items-center justify-center">
                          <div className="w-2 h-2 rounded-full bg-blue-400" />
                        </div>
                        <span className="text-xs font-medium text-blue-400">Arbitrum</span>
                        <span className="text-[10px] px-1.5 py-0.5 rounded bg-blue-500/10 text-blue-400/70 border border-blue-500/20">ERC-20</span>
                      </div>
                    </div>

                    <input
                      type="text"
                      value={depositRecipient}
                      onChange={(e) => setDepositRecipient(e.target.value)}
                      placeholder="0x... (Ethereum/Arbitrum address)"
                      className="w-full px-4 py-3 bg-dag-surface border border-dag-border/50 rounded-xl text-sm font-mono text-white placeholder-dag-muted/30 focus:outline-none focus:border-dag-accent/50"
                    />
                    {eth.connected && eth.address && (
                      <button
                        onClick={() => setDepositRecipient(eth.address)}
                        className="text-[10px] text-dag-accent hover:text-dag-accent/80 transition-colors"
                      >
                        Use connected wallet: {eth.address.slice(0, 6)}...{eth.address.slice(-4)}
                      </button>
                    )}
                    <p className="text-[10px] text-dag-muted">Enter your Arbitrum/Ethereum address to receive bridged UDAG ERC-20 tokens</p>
                  </div>

                  {/* Bridge button */}
                  <div className="space-y-2.5 pt-1">
                    <button
                      onClick={handleBridgeToArbitrum}
                      disabled={wallets.length === 0 || depositing || !depositAmount || !depositRecipient}
                      className="w-full py-4 rounded-xl bg-gradient-to-r from-dag-accent to-dag-blue text-white font-semibold text-sm disabled:opacity-40 disabled:cursor-not-allowed flex items-center justify-center gap-2 hover:shadow-lg hover:shadow-dag-accent/20 transition-all"
                    >
                      {depositing ? (
                        <Loader2 className="w-4 h-4 animate-spin" />
                      ) : (
                        <Zap className="w-4 h-4" />
                      )}
                      {depositing
                        ? 'Bridging...'
                        : wallets.length === 0
                          ? 'Create Wallet First'
                          : !depositAmount
                            ? 'Enter Amount'
                            : !depositRecipient
                              ? 'Enter Recipient'
                              : 'Bridge to Arbitrum'}
                    </button>
                  </div>

                  {/* Info note */}
                  <div className="flex items-start gap-2 p-2.5 rounded-lg bg-dag-accent/5 border border-dag-accent/10">
                    <Info className="w-3.5 h-3.5 text-dag-accent mt-0.5 shrink-0" />
                    <p className="text-xs text-dag-muted">
                      Validators sign attestations as part of consensus. Once 2/3+ signatures are collected, you can claim on Arbitrum.
                    </p>
                  </div>

                  {/* Validator badge */}
                  <div className="flex justify-center">
                    <div className="inline-flex items-center gap-2 px-3 py-1.5 rounded-full bg-dag-green/10 text-dag-green text-xs font-medium border border-dag-green/20">
                      <div className="w-1.5 h-1.5 rounded-full bg-dag-green animate-pulse" />
                      5 Validators Active
                    </div>
                  </div>

                  {/* Deposit success message */}
                  {depositTxHash && (
                    <div className="rounded-xl bg-dag-green/5 border border-dag-green/20 p-3.5 flex items-center gap-2.5">
                      <div className="w-8 h-8 rounded-full bg-dag-green/10 flex items-center justify-center shrink-0">
                        <CheckCircle className="w-4 h-4 text-dag-green" />
                      </div>
                      <div className="flex-1 min-w-0">
                        <p className="text-xs font-medium text-dag-green">Bridge deposit submitted!</p>
                        <p className="text-[10px] text-dag-muted mt-0.5">Validators will sign attestations during consensus. Track status in the Arbitrum &rarr; Native tab.</p>
                        <p className="text-[10px] text-dag-muted mt-0.5 truncate font-mono">{depositTxHash}</p>
                      </div>
                      <CopyButton text={depositTxHash} />
                    </div>
                  )}
                </div>
              )}
            </div>
          </div>

          {/* Bridge Stats Row */}
          <div className="grid grid-cols-1 sm:grid-cols-3 gap-3 stagger-enter">
            <div className="rounded-xl bg-dag-card border border-dag-border p-4 card-glow">
              <div className="flex items-center gap-2 mb-2">
                <Lock className="w-3.5 h-3.5 text-dag-accent" />
                <span className="text-[10px] text-dag-muted uppercase tracking-wider font-medium">Bridge Reserve</span>
              </div>
              <p className="text-lg font-bold text-white font-mono">
                {bridgeReserve ? formatUdag(bridgeReserve.reserve_udag) : '0.00'}
              </p>
              <p className="text-[10px] text-dag-muted mt-0.5">UDAG locked on native chain</p>
            </div>
            <div className="rounded-xl bg-dag-card border border-dag-border p-4 card-glow">
              <div className="flex items-center gap-2 mb-2">
                <Clock className="w-3.5 h-3.5 text-dag-muted" />
                <span className="text-[10px] text-dag-muted uppercase tracking-wider font-medium">24h Volume</span>
              </div>
              <p className="text-lg font-bold text-white font-mono">
                {dailyCap > 0n ? formatUdagBigint(dailyVolume) : '0.00'}
              </p>
              {dailyCap > 0n && (
                <div className="mt-1.5">
                  <div className="flex items-center justify-between text-[10px] text-dag-muted mb-1">
                    <span>{formatUdagBigint(dailyVolume)}</span>
                    <span>{formatUdagBigint(dailyCap)} cap</span>
                  </div>
                  <div className="w-full bg-dag-bg rounded-full h-1">
                    <div
                      className="bg-gradient-to-r from-dag-accent to-dag-blue h-1 rounded-full transition-all"
                      style={{ width: `${Math.min(100, Number((dailyVolume * 100n) / dailyCap))}%` }}
                    />
                  </div>
                </div>
              )}
            </div>
            <div className="rounded-xl bg-dag-card border border-dag-border p-4 card-glow">
              <div className="flex items-center gap-2 mb-2">
                <Shield className="w-3.5 h-3.5 text-dag-green" />
                <span className="text-[10px] text-dag-muted uppercase tracking-wider font-medium">Security</span>
              </div>
              <p className="text-lg font-bold text-white">2/3 BFT</p>
              <p className="text-[10px] text-dag-muted mt-0.5">Validator threshold consensus</p>
            </div>
          </div>
        </div>

        {/* Right: Attestations + How it Works (2 cols) */}
        <div className="lg:col-span-2 space-y-6">

          {/* Attestations Card */}
          <div className="rounded-2xl bg-dag-card border border-dag-border overflow-hidden card-glow">
            <div className="flex items-center justify-between p-5 pb-0">
              <h3 className="text-sm font-semibold text-white">Recent Attestations</h3>
              {loadingAttestations && <Loader2 className="w-3.5 h-3.5 text-dag-muted animate-spin" />}
            </div>
            <div className="p-5 pt-3">
              {attestations.length === 0 ? (
                <div className="text-center py-8">
                  <div className="w-10 h-10 rounded-xl bg-dag-surface border border-dag-border flex items-center justify-center mx-auto mb-3">
                    <Shield className="w-5 h-5 text-dag-muted/30" />
                  </div>
                  <p className="text-sm text-dag-muted">No recent attestations</p>
                  <p className="text-[10px] text-dag-muted mt-1">Bridge transfers will appear here</p>
                </div>
              ) : (
                <div className="space-y-3">
                  {attestations.map((att) => (
                    <div key={att.nonce} className="rounded-xl bg-dag-bg border border-dag-border/50 p-3.5 space-y-2.5">
                      {/* Header row */}
                      <div className="flex items-center justify-between">
                        <span className="text-xs font-mono text-dag-muted">#{att.nonce}</span>
                        {att.ready ? (
                          <span className="flex items-center gap-1 text-[10px] px-2 py-0.5 rounded-full bg-dag-green/10 text-dag-green border border-dag-green/20">
                            <div className="w-1.5 h-1.5 rounded-full bg-dag-green animate-pulse" />
                            Ready to Claim
                          </span>
                        ) : (
                          <span className="text-[10px] px-2 py-0.5 rounded-full bg-dag-yellow/10 text-dag-yellow border border-dag-yellow/20">
                            {att.signature_count}/{att.threshold} signatures
                          </span>
                        )}
                      </div>

                      {/* Progress bar */}
                      <div className="w-full bg-dag-surface rounded-full h-1.5">
                        <div
                          className={`h-1.5 rounded-full transition-all ${att.ready ? 'bg-dag-green' : 'bg-dag-yellow'}`}
                          style={{ width: `${Math.min(100, (att.signature_count / att.threshold) * 100)}%` }}
                        />
                      </div>

                      {/* Details */}
                      <div className="space-y-1">
                        <div className="flex items-center justify-between text-xs">
                          <span className="text-dag-muted">Amount</span>
                          <span className="text-white font-mono font-medium">{formatUdag(att.amount_udag)} UDAG</span>
                        </div>
                        <div className="flex items-center justify-between text-xs">
                          <span className="text-dag-muted">From</span>
                          <span className="text-white font-mono text-[11px] truncate max-w-[140px]">{att.sender_bech32}</span>
                        </div>
                        <div className="flex items-center justify-between text-xs">
                          <span className="text-dag-muted">To</span>
                          <span className="text-white font-mono text-[11px] truncate max-w-[140px]">{att.recipient.slice(0, 10)}...{att.recipient.slice(-6)}</span>
                        </div>
                      </div>

                      {/* Claim button */}
                      {att.ready && (
                        <button
                          onClick={() => handleClaim(att)}
                          disabled={claiming === att.nonce}
                          className="w-full mt-1 py-2.5 rounded-xl bg-dag-green/10 text-dag-green border border-dag-green/20 text-xs font-semibold hover:bg-dag-green/15 transition-all flex items-center justify-center gap-1.5 disabled:opacity-50"
                        >
                          {claiming === att.nonce ? (
                            <Loader2 className="w-3.5 h-3.5 animate-spin" />
                          ) : (
                            <CheckCircle className="w-3.5 h-3.5" />
                          )}
                          {claiming === att.nonce ? 'Claiming...' : 'Claim on Arbitrum'}
                        </button>
                      )}
                    </div>
                  ))}
                </div>
              )}
            </div>
          </div>

          {/* How it Works Card */}
          <div className="rounded-2xl bg-dag-card border border-dag-border overflow-hidden card-glow">
            <div className="p-5">
              <div className="flex items-center gap-2 mb-4">
                <Shield className="w-4 h-4 text-dag-accent" />
                <h3 className="text-sm font-semibold text-white">How the Bridge Works</h3>
              </div>

              <div className="space-y-4">
                {/* Step 1 */}
                <div className="flex items-start gap-3">
                  <div className="w-8 h-8 rounded-lg bg-dag-accent/10 border border-dag-accent/20 flex items-center justify-center shrink-0">
                    <span className="text-xs font-bold text-dag-accent">1</span>
                  </div>
                  <div>
                    <p className="text-sm font-medium text-white">Deposit</p>
                    <p className="text-xs text-dag-muted mt-0.5">Lock tokens on the source chain. Funds are escrowed in the bridge contract.</p>
                  </div>
                </div>

                {/* Step 2 */}
                <div className="flex items-start gap-3">
                  <div className="w-8 h-8 rounded-lg bg-dag-blue/10 border border-dag-blue/20 flex items-center justify-center shrink-0">
                    <span className="text-xs font-bold text-dag-blue">2</span>
                  </div>
                  <div>
                    <p className="text-sm font-medium text-white">Attestation</p>
                    <p className="text-xs text-dag-muted mt-0.5">Validators sign the transfer as part of normal consensus. 2/3+ signatures required.</p>
                  </div>
                </div>

                {/* Step 3 */}
                <div className="flex items-start gap-3">
                  <div className="w-8 h-8 rounded-lg bg-dag-green/10 border border-dag-green/20 flex items-center justify-center shrink-0">
                    <span className="text-xs font-bold text-dag-green">3</span>
                  </div>
                  <div>
                    <p className="text-sm font-medium text-white">Claim</p>
                    <p className="text-xs text-dag-muted mt-0.5">Submit the attestation proof to unlock tokens on the destination chain.</p>
                  </div>
                </div>
              </div>

              <div className="mt-4 p-3 rounded-xl bg-dag-green/5 border border-dag-green/10">
                <div className="flex items-center gap-2">
                  <CheckCircle className="w-3.5 h-3.5 text-dag-green shrink-0" />
                  <div>
                    <span className="text-xs font-medium text-dag-green">Same security as DAG consensus</span>
                    <p className="text-[10px] text-dag-muted mt-0.5">No external relayers. Uses the existing validator federation with 2/3 BFT threshold.</p>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
