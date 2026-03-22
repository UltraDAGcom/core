import { useState, useEffect, useRef } from 'react';
import { ArrowRightLeft, ArrowRight, ExternalLink, Shield, Clock, Info, Unplug, Loader2, CheckCircle, Wallet } from 'lucide-react';
import { formatUnits } from 'ethers';
import { Card } from '../components/shared/Card.tsx';
import { useKeystore } from '../hooks/useKeystore.ts';
import { useEthWallet } from '../hooks/useEthWallet.ts';
import { useToast } from '../hooks/useToast.tsx';
import { normalizeAddress, isValidAddress, formatUdag, formatUdagBigint, getBridgeNonce, getBridgeAttestation, getBridgeReserve } from '../lib/api.ts';
import { CopyButton } from '../components/shared/CopyButton.tsx';
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

export function BridgePage() {
  const { wallets } = useKeystore();
  const eth = useEthWallet();
  const { toast } = useToast();

  const [direction, setDirection] = useState<'to-native' | 'to-arbitrum'>('to-native');
  const [amount, setAmount] = useState('');
  const [nativeAddress, setNativeAddress] = useState('');
  const [selectedWalletIdx] = useState(0);
  const [bridging, setBridging] = useState(false);
  const [approving, setApproving] = useState(false);
  const [txHash, setTxHash] = useState('');
  const [claiming, setClaiming] = useState<number | null>(null); // nonce being claimed
  const [bridgeReserve, setBridgeReserve] = useState<{ reserve_sats: number; reserve_udag: number } | null>(null);
  const [attestations, setAttestations] = useState<BridgeAttestation[]>([]);
  const [loadingAttestations, setLoadingAttestations] = useState(false);
  const isMounted = useRef(true);

  const wallet = wallets[selectedWalletIdx];
  const bridgeActive = eth.contractsDeployed ? eth.bridgeActive : false;
  const bridgePaused = eth.contractsDeployed ? eth.bridgePaused : false;
  const canBridge = bridgeActive && !bridgePaused;

  // Parse amount to sats
  const amountSats = (() => {
    try { return eth.parseUdag(amount || '0'); } catch { return 0n; }
  })();
  const needsApproval = eth.connected && amountSats > 0n && eth.udagAllowance < amountSats;

  // Track component mount state (M12)
  useEffect(() => {
    isMounted.current = true;
    return () => { isMounted.current = false; };
  }, []);

  // H5: Display eth.error in toast when it changes
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
    const interval = setInterval(fetchReserve, 30000); // Refresh every 30s
    return () => clearInterval(interval);
  }, []);

  // Fetch recent attestations (L4: parallel fetch, M12: isMounted guard, M6: tab-gated + backoff)
  const consecutiveErrorsRef = useRef(0);
  useEffect(() => {
    if (direction !== 'to-native') return; // M6: only poll on the tab that displays attestations

    const fetchAttestations = async () => {
      if (isMounted.current) setLoadingAttestations(true);
      try {
        const nonceRes = await getBridgeNonce();
        // Fetch last 5 attestations in parallel (L4)
        const indices: number[] = [];
        for (let i = Math.max(0, nonceRes.next_nonce - 5); i < nonceRes.next_nonce; i++) {
          indices.push(i);
        }
        const results = await Promise.all(
          indices.map(i => getBridgeAttestation(i).catch(() => null))
        );
        const recent = results.filter((att): att is BridgeAttestation => att !== null);
        if (isMounted.current) setAttestations(recent.reverse());
        consecutiveErrorsRef.current = 0; // Reset on success
      } catch (e) {
        consecutiveErrorsRef.current += 1;
        // Node might not have bridge endpoints yet
      } finally {
        if (isMounted.current) setLoadingAttestations(false);
      }
    };
    fetchAttestations();

    // M6: Backoff on consecutive errors: 10s base, doubles per error, max 60s
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

  const handleApprove = async () => {
    setApproving(true);
    const ok = await eth.approve(amountSats);
    setApproving(false);
    if (ok) toast('Approval confirmed', 'success');
  };

  const handleBridgeToNative = async () => {
    if (!eth.connected) return;
    eth.clearError(); // M11: clear stale errors

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
    if (amountSats <= 0n) { toast('Enter a valid amount', 'error'); return; }

    // M7: Balance sufficiency check
    if (amountSats > eth.udagBalanceRaw) {
      toast('Insufficient UDAG balance', 'error');
      return;
    }

    // M8: Per-tx and daily cap validation
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
    // M9: Use return value directly instead of stale eth.error
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

  // H12: Claim withdrawal on Arbitrum
  const handleClaim = async (att: BridgeAttestation) => {
    if (!eth.connected) {
      toast('Connect your Arbitrum wallet first', 'error');
      return;
    }
    eth.clearError(); // M11: clear stale errors

    const signatures = att.proof?.signatures;
    if (!signatures || signatures.length === 0) {
      toast('No signatures available for this attestation', 'error');
      return;
    }

    const senderEth = att.proof?.sender_eth;
    const recipientEth = att.proof?.recipient_eth;

    // H4: Validate Ethereum addresses before calling contract
    const isValidEthAddr = (addr: string | undefined): addr is string =>
      typeof addr === 'string' && /^0x[0-9a-fA-F]{40}$/.test(addr);
    if (!isValidEthAddr(senderEth) || !isValidEthAddr(recipientEth)) {
      toast('Invalid Ethereum addresses in attestation proof', 'error');
      return;
    }

    // C1: Safely parse BigInt from untrusted API data
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

  // Format bridge stats from contract (no hardcoded fallback)
  const dailyCap = eth.contractsDeployed && eth.dailyCap > 0n ? eth.dailyCap : 0n;
  const dailyVolume = eth.dailyVolume;

  return (
    <div className="space-y-6 animate-page-enter">
      <div>
        <h1 className="text-2xl font-bold text-white">Bridge</h1>
        <p className="text-sm text-dag-muted mt-1">Transfer UDAG between Arbitrum and UltraDAG native chain</p>
      </div>

      {/* Status Banner */}
      <div className="bg-dag-surface border border-dag-border rounded-xl p-4">
        <div className="flex flex-wrap items-center gap-3">
          <div className={`w-3 h-3 rounded-full ${canBridge ? 'bg-dag-green animate-pulse' : bridgePaused ? 'bg-dag-red' : bridgeActive ? 'bg-dag-yellow' : 'bg-dag-yellow'}`} />
          <div className="flex-1 min-w-[200px]">
            <span className={`text-sm font-medium ${canBridge ? 'text-dag-green' : 'text-dag-yellow'}`}>
              {canBridge ? 'Bridge Active' : bridgePaused ? 'Bridge Paused' : CONTRACTS_DEPLOYED ? 'Bridge Inactive' : 'Validator Federation Bridge Active'}
            </span>
            <p className="text-xs text-dag-muted mt-0.5">
              {canBridge
                ? 'Transfer UDAG between Arbitrum and the native chain.'
                : CONTRACTS_DEPLOYED
                  ? 'Bridge contracts deployed. Waiting for activation.'
                  : 'Validator Federation Bridge: Validators sign attestations on UltraDAG native chain. Arbitrum contracts coming soon.'}
            </p>
          </div>
          {eth.connected && (
            <div className="flex items-center gap-2">
              <div className="w-2 h-2 rounded-full bg-dag-green" />
              <span className="text-xs text-dag-muted font-mono">{eth.address.slice(0, 6)}...{eth.address.slice(-4)}</span>
            </div>
          )}
        </div>
      </div>

      {/* Live Bridge Stats - Always visible */}
      <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
        <Card>
          <div className="flex items-center gap-2 mb-2">
            <Shield className="w-4 h-4 text-dag-accent" />
            <span className="text-xs text-dag-muted uppercase">Bridge Reserve</span>
          </div>
          <p className="text-xl font-bold text-white">{bridgeReserve ? formatUdag(bridgeReserve.reserve_udag) : '—'} UDAG</p>
          <p className="text-xs text-dag-muted mt-1">Locked on UltraDAG</p>
        </Card>
        <Card>
          <div className="flex items-center gap-2 mb-2">
            <Clock className="w-4 h-4 text-dag-muted" />
            <span className="text-xs text-dag-muted uppercase">Daily Volume</span>
          </div>
          <p className="text-xl font-bold text-white">
            {dailyCap > 0n ? `${formatUdagBigint(dailyVolume)} / ${formatUdagBigint(dailyCap)} UDAG` : '-- / -- UDAG'}
          </p>
          <div className="w-full bg-dag-bg rounded-full h-1.5 mt-2">
            <div className="bg-dag-accent h-1.5 rounded-full" style={{ width: `${dailyCap > 0n ? Math.min(100, Number((dailyVolume * 100n) / dailyCap)) : 0}%` }} />
          </div>
        </Card>
        <Card>
          <div className="flex items-center gap-2 mb-2">
            <CheckCircle className="w-4 h-4 text-dag-green" />
            <span className="text-xs text-dag-muted uppercase">Security</span>
          </div>
          <p className="text-sm font-bold text-white">2/3 Validator Threshold</p>
          <p className="text-xs text-dag-muted mt-1">Same security as DAG consensus</p>
        </Card>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        {/* Left: Bridge Form */}
        <div className="lg:col-span-2 space-y-6">
          <Card>
            <div className="space-y-5">
              <div className="flex items-center gap-3">
                <ArrowRightLeft className="w-5 h-5 text-dag-accent" />
                <h2 className="text-lg font-semibold text-white">Bridge Transfer</h2>
              </div>

              {/* Direction toggle */}
              <div className="flex gap-2">
                <button
                  onClick={() => setDirection('to-native')}
                  className={`flex-1 py-3 px-4 rounded-lg text-sm font-medium transition-all ${
                    direction === 'to-native'
                      ? 'bg-dag-accent/15 text-dag-accent border border-dag-accent/40'
                      : 'bg-dag-surface border border-dag-border text-dag-muted hover:text-white'
                  }`}
                >
                  <div className="flex items-center justify-center gap-2">
                    <span className="text-xs px-1.5 py-0.5 rounded bg-blue-500/20 text-blue-400">Arbitrum</span>
                    <ArrowRight className="w-4 h-4" />
                    <span className="text-xs px-1.5 py-0.5 rounded bg-dag-accent/20 text-dag-accent">UltraDAG</span>
                  </div>
                </button>
                <button
                  onClick={() => setDirection('to-arbitrum')}
                  className={`flex-1 py-3 px-4 rounded-lg text-sm font-medium transition-all ${
                    direction === 'to-arbitrum'
                      ? 'bg-dag-accent/15 text-dag-accent border border-dag-accent/40'
                      : 'bg-dag-surface border border-dag-border text-dag-muted hover:text-white'
                  }`}
                >
                  <div className="flex items-center justify-center gap-2">
                    <span className="text-xs px-1.5 py-0.5 rounded bg-dag-accent/20 text-dag-accent">UltraDAG</span>
                    <ArrowRight className="w-4 h-4" />
                    <span className="text-xs px-1.5 py-0.5 rounded bg-blue-500/20 text-blue-400">Arbitrum</span>
                  </div>
                </button>
              </div>

              {/* ─── Arbitrum → Native ─── */}
              {direction === 'to-native' ? (
                <div className="space-y-4">
                  {/* Arbitrum wallet connection */}
                  <div className="bg-dag-surface border border-dag-border rounded-lg p-4">
                    <div className="flex items-center justify-between mb-3">
                      <span className="text-xs text-dag-muted uppercase tracking-wider">From (Arbitrum)</span>
                      <span className="text-xs px-1.5 py-0.5 rounded bg-blue-500/20 text-blue-400">ERC-20</span>
                    </div>
                    {eth.connected ? (
                      <div className="space-y-2">
                        <div className="flex items-center justify-between">
                          <div className="flex items-center gap-2">
                            <div className="w-2 h-2 rounded-full bg-dag-green" />
                            <span className="text-sm text-white font-mono">{eth.address.slice(0, 6)}...{eth.address.slice(-4)}</span>
                            <CopyButton text={eth.address} />
                          </div>
                          <button onClick={eth.disconnect} className="text-xs text-dag-muted hover:text-dag-red flex items-center gap-1">
                            <Unplug className="w-3 h-3" /> Disconnect
                          </button>
                        </div>
                        <div className="grid grid-cols-2 gap-3 mt-2">
                          <div className="bg-dag-bg rounded px-3 py-2">
                            <span className="text-[10px] text-dag-muted uppercase">UDAG Balance</span>
                            <p className="text-sm text-white font-mono">{Number(eth.udagBalance).toLocaleString(undefined, { maximumFractionDigits: 4 })} UDAG</p>
                          </div>
                          <div className="bg-dag-bg rounded px-3 py-2">
                            <span className="text-[10px] text-dag-muted uppercase">ETH Balance</span>
                            <p className="text-sm text-white font-mono">{Number(eth.balance).toFixed(6)} ETH</p>
                          </div>
                        </div>
                        {!eth.isCorrectChain && (
                          <button onClick={eth.switchToArbitrum} className="w-full py-2 mt-1 rounded bg-dag-yellow/20 text-dag-yellow border border-dag-yellow/30 text-xs font-medium">
                            Switch to Arbitrum
                          </button>
                        )}
                      </div>
                    ) : (
                      <div className="space-y-3">
                        {!CONTRACTS_DEPLOYED && (
                          <div className="flex items-start gap-2">
                            <Info className="w-4 h-4 text-dag-yellow mt-0.5" />
                            <p className="text-sm text-dag-muted">
                              Bridge contracts are not yet deployed. Connect your wallet now to be ready, or buy UDAG on Uniswap in the meantime.
                            </p>
                          </div>
                        )}
                        <button
                          onClick={eth.connect}
                          disabled={eth.loading}
                          className="w-full py-3 rounded-lg bg-dag-accent text-white font-medium text-sm disabled:opacity-50 flex items-center justify-center gap-2 hover:bg-dag-accent/90 transition-colors"
                        >
                          {eth.loading ? <Loader2 className="w-4 h-4 animate-spin" /> : <Wallet className="w-4 h-4" />}
                          {eth.loading ? 'Connecting...' : 'Connect Arbitrum Wallet'}
                        </button>
                        {!CONTRACTS_DEPLOYED && (
                          <a
                            href="https://app.uniswap.org"
                            target="_blank"
                            rel="noopener noreferrer"
                            className="block text-center text-xs text-dag-accent hover:text-dag-accent/80"
                          >
                            Open Uniswap <ExternalLink className="w-3 h-3 inline" />
                          </a>
                        )}
                      </div>
                    )}
                  </div>

                  {/* Amount */}
                  <label className="block">
                    <div className="flex items-center justify-between">
                      <span className="text-sm text-dag-muted">Amount (UDAG)</span>
                      {eth.connected && (
                        <button onClick={() => setAmount(formatUnits(eth.udagBalanceRaw, 8))} className="text-xs text-dag-accent hover:text-dag-accent/80">
                          Max
                        </button>
                      )}
                    </div>
                    <input
                      type="text"
                      inputMode="decimal"
                      value={amount}
                      onChange={(e) => {
                        const val = e.target.value.replace(/[^0-9.]/g, '');
                        // Prevent multiple dots
                        const parts = val.split('.');
                        const sanitized = parts[0] + (parts.length > 1 ? '.' + parts.slice(1).join('') : '');
                        // Limit to 8 decimal places
                        if (parts.length > 1 && parts[1].length > 8) return;
                        setAmount(sanitized);
                      }}
                      placeholder="0.00"
                      disabled={!eth.connected || !canBridge}
                      className="w-full mt-1 px-4 py-3 bg-dag-bg border border-dag-border rounded-lg text-white placeholder-dag-muted focus:outline-none focus:border-dag-accent disabled:opacity-50"
                    />
                  </label>

                  {/* UltraDAG recipient */}
                  <label className="block">
                    <span className="text-sm text-dag-muted">To (UltraDAG Address)</span>
                    <input
                      type="text"
                      value={wallet ? wallet.address : nativeAddress}
                      onChange={(e) => setNativeAddress(e.target.value)}
                      placeholder="tudg1... or 40-char hex"
                      disabled={!!wallet || !canBridge}
                      className="w-full mt-1 px-4 py-3 bg-dag-bg border border-dag-border rounded-lg text-white placeholder-dag-muted focus:outline-none focus:border-dag-accent disabled:opacity-50"
                    />
                    {!wallet && (
                      <p className="text-xs text-dag-muted mt-1">Enter your UltraDAG address or select a wallet above.</p>
                    )}
                  </label>

                  {/* Approval */}
                  {needsApproval && (
                    <button
                      onClick={handleApprove}
                      disabled={approving || !canBridge}
                      className="w-full py-3 rounded-lg bg-dag-yellow/20 text-dag-yellow border border-dag-yellow/30 font-medium text-sm disabled:opacity-50 flex items-center justify-center gap-2"
                    >
                      {approving ? <Loader2 className="w-4 h-4 animate-spin" /> : <Shield className="w-4 h-4" />}
                      Approve UDAG Transfer
                    </button>
                  )}

                  {/* Submit */}
                  <button
                    onClick={handleBridgeToNative}
                    disabled={!eth.connected || !canBridge || bridging || needsApproval || amountSats <= 0n}
                    className="w-full py-3 rounded-lg bg-dag-accent text-white font-medium text-sm disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-2 hover:bg-dag-accent/90 transition-colors"
                  >
                    {bridging ? <Loader2 className="w-4 h-4 animate-spin" /> : <ArrowRight className="w-4 h-4" />}
                    {bridging ? 'Bridging...' : 'Bridge to UltraDAG'}
                  </button>

                  {/* Tx hash */}
                  {txHash && (
                    <div className="bg-dag-green/10 border border-dag-green/30 rounded-lg p-3 flex items-center gap-2">
                      <CheckCircle className="w-4 h-4 text-dag-green" />
                      <span className="text-xs text-dag-green">Transaction submitted!</span>
                      <a href={`https://arbiscan.io/tx/${txHash}`} target="_blank" rel="noopener noreferrer" className="ml-auto text-xs text-dag-green hover:underline flex items-center gap-1">
                        View on Arbiscan <ExternalLink className="w-3 h-3" />
                      </a>
                    </div>
                  )}
                </div>
              ) : (
                /* ─── Native → Arbitrum ─── */
                <div className="space-y-4">
                  <div className="bg-dag-surface border border-dag-border rounded-lg p-4">
                    <div className="flex items-center justify-between mb-3">
                      <span className="text-xs text-dag-muted uppercase tracking-wider">From (UltraDAG)</span>
                      <span className="text-xs px-1.5 py-0.5 rounded bg-dag-accent/20 text-dag-accent">Native</span>
                    </div>
                    <p className="text-sm text-dag-muted">
                      Bridge from UltraDAG native chain to Arbitrum.
                    </p>
                    <p className="text-xs text-dag-muted mt-2">
                      <Info className="w-3 h-3 inline mr-1" />
                      This direction uses the Validator Federation Bridge. Validators sign attestations as part of consensus. Once 2/3+ signatures are collected, you can claim on Arbitrum.
                    </p>
                  </div>

                  {/* Bridge from native form - coming soon */}
                  <div className="text-center py-8">
                    <Clock className="w-12 h-12 text-dag-muted mx-auto mb-3" />
                    <h3 className="text-lg font-semibold text-white">Coming Soon</h3>
                    <p className="text-sm text-dag-muted mt-1">Bridge from UltraDAG native to Arbitrum will be available soon.</p>
                  </div>
                </div>
              )}
            </div>
          </Card>
        </div>

        {/* Right: Recent Attestations */}
        <div className="space-y-6">
          <Card>
            <div className="flex items-center justify-between mb-4">
              <h3 className="text-sm font-semibold text-white">Recent Bridge Attestations</h3>
              {loadingAttestations && <Loader2 className="w-4 h-4 text-dag-muted animate-spin" />}
            </div>
            {attestations.length === 0 ? (
              <p className="text-sm text-dag-muted text-center py-4">No recent attestations</p>
            ) : (
              <div className="space-y-3">
                {attestations.map((att) => (
                  <div key={att.nonce} className="bg-dag-bg border border-dag-border rounded-lg p-3">
                    <div className="flex items-center justify-between mb-2">
                      <span className="text-xs font-mono text-dag-muted">#{att.nonce}</span>
                      {att.ready ? (
                        <span className="text-[10px] px-1.5 py-0.5 rounded bg-dag-green/20 text-dag-green">Ready</span>
                      ) : (
                        <span className="text-[10px] px-1.5 py-0.5 rounded bg-dag-yellow/20 text-dag-yellow">{att.signature_count}/{att.threshold} sigs</span>
                      )}
                    </div>
                    <div className="space-y-1">
                      <div className="flex items-center justify-between text-xs">
                        <span className="text-dag-muted">Amount</span>
                        <span className="text-white font-mono">{formatUdag(att.amount_udag)} UDAG</span>
                      </div>
                      <div className="flex items-center justify-between text-xs">
                        <span className="text-dag-muted">Sender</span>
                        <span className="text-white font-mono truncate max-w-[120px]">{att.sender_bech32.slice(0, 12)}...</span>
                      </div>
                      <div className="flex items-center justify-between text-xs">
                        <span className="text-dag-muted">Recipient</span>
                        <span className="text-white font-mono truncate max-w-[120px]">{att.recipient.slice(0, 10)}...</span>
                      </div>
                    </div>
                    {att.ready && (
                      <button
                        onClick={() => handleClaim(att)}
                        disabled={claiming === att.nonce}
                        className="w-full mt-2 py-1.5 rounded bg-dag-accent/20 text-dag-accent border border-dag-accent/40 text-xs font-medium hover:bg-dag-accent/30 transition-colors flex items-center justify-center gap-1 disabled:opacity-50"
                      >
                        {claiming === att.nonce ? <Loader2 className="w-3 h-3 animate-spin" /> : <ExternalLink className="w-3 h-3" />}
                        {claiming === att.nonce ? 'Claiming...' : 'Claim on Arbitrum'}
                      </button>
                    )}
                  </div>
                ))}
              </div>
            )}
          </Card>

          {/* Info Card - Validator Federation Bridge */}
          <Card>
            <div className="flex items-center gap-2 mb-3">
              <Shield className="w-4 h-4 text-dag-accent" />
              <h3 className="text-sm font-semibold text-white">Validator Federation Bridge</h3>
            </div>
            <p className="text-xs text-dag-muted mb-3">
              The UltraDAG bridge uses the existing validator set (2/3 threshold) instead of external relayers. 
              Validators sign attestations as part of normal block production.
            </p>
            <ol className="space-y-2 text-xs text-dag-muted">
              <li className="flex items-start gap-2">
                <span className="text-dag-accent font-bold">1.</span>
                <span>Deposit on source chain (tokens locked)</span>
              </li>
              <li className="flex items-start gap-2">
                <span className="text-dag-accent font-bold">2.</span>
                <span>Validators sign attestation (2/3+ required)</span>
              </li>
              <li className="flex items-start gap-2">
                <span className="text-dag-accent font-bold">3.</span>
                <span>Claim on destination chain with proof</span>
              </li>
            </ol>
            <div className="mt-3 p-2 bg-dag-accent/10 border border-dag-accent/30 rounded">
              <div className="flex items-center gap-2 text-xs">
                <CheckCircle className="w-3 h-3 text-dag-green" />
                <span className="text-dag-green font-medium">No external relayers needed</span>
              </div>
              <p className="text-[10px] text-dag-muted mt-1">Same security as DAG consensus (2/3 BFT)</p>
            </div>
          </Card>
        </div>
      </div>
    </div>
  );
}
