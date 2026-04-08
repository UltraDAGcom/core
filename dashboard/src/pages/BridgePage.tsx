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
  AlertTriangle,
} from 'lucide-react';
import { formatUnits } from 'ethers';
import { useKeystore } from '../hooks/useKeystore.ts';
import { useEthWallet } from '../hooks/useEthWallet.ts';
import { useIsMobile } from '../hooks/useIsMobile.ts';
import type { DiscoveredWallet } from '../hooks/useEthWallet.ts';
import { useToast } from '../hooks/useToast.tsx';
import { normalizeAddress, isValidAddress, formatUdag, formatUdagBigint, getBridgeNonce, getBridgeAttestation, getBridgeReserve, postBridgeDeposit, isConnected } from '../lib/api.ts';
import { CopyButton } from '../components/shared/CopyButton.tsx';
import { WalletSelector } from '../components/shared/WalletSelector.tsx';
import { useWalletBalances } from '../hooks/useWalletBalances.ts';
import { CONTRACTS_DEPLOYED } from '../lib/contracts.ts';
import { PageHeader } from '../components/shared/PageHeader';

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
      style={{
        position: 'fixed', inset: 0, zIndex: 50,
        display: 'flex', alignItems: 'center', justifyContent: 'center',
        padding: 16, background: 'rgba(0,0,0,0.7)',
      }}
      onClick={(e) => { if (e.target === e.currentTarget) onClose(); }}
    >
      <div style={{
        background: 'var(--dag-card)', border: '1px solid var(--dag-border)',
        borderRadius: 14, boxShadow: '0 25px 50px -12px rgba(0,0,0,0.5)',
        width: '100%', maxWidth: 384, overflow: 'hidden',
      }}>
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', padding: '20px 20px 12px' }}>
          <h2 style={{ fontSize: 16, fontWeight: 600, color: 'var(--dag-text)' }}>Connect Wallet</h2>
          <button
            onClick={onClose}
            style={{
              padding: 6, borderRadius: 8, border: 'none', cursor: 'pointer',
              color: 'var(--dag-text-muted)', background: 'transparent',
              transition: 'all 0.15s',
            }}
          >
            <X style={{ width: 16, height: 16 }} />
          </button>
        </div>

        {wallets.length === 0 ? (
          <div style={{ padding: '0 20px 24px', display: 'flex', flexDirection: 'column', gap: 16 }}>
            <div style={{ textAlign: 'center', padding: '24px 0' }}>
              <Wallet style={{ width: 40, height: 40, color: 'var(--dag-text-muted)', margin: '0 auto 12px', opacity: 0.4 }} />
              <p style={{ fontSize: 12, color: 'var(--dag-text-muted)' }}>No wallets detected</p>
              <p style={{ fontSize: 10, color: 'var(--dag-text-muted)', marginTop: 4 }}>Install a Web3 wallet to get started</p>
            </div>
            <a
              href="https://metamask.io/download/"
              target="_blank"
              rel="noopener noreferrer"
              style={{
                display: 'flex', alignItems: 'center', gap: 12, width: '100%', padding: 12,
                borderRadius: 12, background: 'var(--dag-input-bg)', border: '1px solid var(--dag-border)',
                transition: 'all 0.15s', textDecoration: 'none',
              }}
            >
              <div style={{
                width: 36, height: 36, borderRadius: 8,
                background: 'rgba(246,133,27,0.1)',
                display: 'flex', alignItems: 'center', justifyContent: 'center',
              }}>
                <span style={{ fontSize: 16 }}>🦊</span>
              </div>
              <div style={{ flex: 1, textAlign: 'left' }}>
                <span style={{ fontSize: 12, fontWeight: 500, color: 'var(--dag-text)', transition: 'all 0.15s' }}>Install MetaMask</span>
                <p style={{ fontSize: 10, color: 'var(--dag-text-muted)' }}>The most popular Ethereum wallet</p>
              </div>
              <ExternalLink style={{ width: 14, height: 14, color: 'var(--dag-text-muted)', transition: 'all 0.15s' }} />
            </a>
            <a
              href="https://rabby.io/"
              target="_blank"
              rel="noopener noreferrer"
              style={{
                display: 'flex', alignItems: 'center', gap: 12, width: '100%', padding: 12,
                borderRadius: 12, background: 'var(--dag-input-bg)', border: '1px solid var(--dag-border)',
                transition: 'all 0.15s', textDecoration: 'none',
              }}
            >
              <div style={{
                width: 36, height: 36, borderRadius: 8,
                background: 'rgba(129,103,245,0.1)',
                display: 'flex', alignItems: 'center', justifyContent: 'center',
              }}>
                <span style={{ fontSize: 16 }}>🐰</span>
              </div>
              <div style={{ flex: 1, textAlign: 'left' }}>
                <span style={{ fontSize: 12, fontWeight: 500, color: 'var(--dag-text)', transition: 'all 0.15s' }}>Install Rabby</span>
                <p style={{ fontSize: 10, color: 'var(--dag-text-muted)' }}>Multi-chain wallet with security focus</p>
              </div>
              <ExternalLink style={{ width: 14, height: 14, color: 'var(--dag-text-muted)', transition: 'all 0.15s' }} />
            </a>
          </div>
        ) : (
          <div style={{ padding: '0 20px 20px', display: 'flex', flexDirection: 'column', gap: 8 }}>
            <p style={{ fontSize: 10, color: 'var(--dag-text-muted)', marginBottom: 12 }}>Choose your wallet to connect</p>
            {wallets.map((w) => (
              <button
                key={w.uuid}
                onClick={() => onSelect(w.uuid)}
                style={{
                  display: 'flex', alignItems: 'center', gap: 12, width: '100%', padding: 12,
                  borderRadius: 12, background: 'var(--dag-input-bg)', border: '1px solid var(--dag-border)',
                  transition: 'all 0.15s', cursor: 'pointer',
                }}
              >
                <img
                  src={w.icon}
                  alt={w.name}
                  style={{ width: 36, height: 36, borderRadius: 8, objectFit: 'contain' }}
                  onError={(e) => {
                    (e.target as HTMLImageElement).style.display = 'none';
                  }}
                />
                <span style={{
                  fontSize: 12, fontWeight: 500, color: 'var(--dag-text)',
                  transition: 'all 0.15s', flex: 1, textAlign: 'left',
                }}>
                  {w.name}
                </span>
                <ArrowRight style={{ width: 16, height: 16, color: 'var(--dag-text-muted)', transition: 'all 0.15s' }} />
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
  const m = useIsMobile();

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
  const [attestationError, setAttestationError] = useState('');
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
        if (isMounted.current) { setAttestations(recent.reverse()); setAttestationError(''); }
        consecutiveErrorsRef.current = 0;
      } catch (e) {
        consecutiveErrorsRef.current += 1;
        if (isMounted.current) setAttestationError('Unable to fetch recent attestations — retrying...');
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

  // Shared inline style fragments
  const chipBadge = (bg: string, color: string, borderColor: string): React.CSSProperties => ({
    fontSize: 10, padding: '2px 6px', borderRadius: 4,
    background: bg, color, border: `1px solid ${borderColor}`, fontWeight: 600,
  });
  const arbChip = chipBadge('rgba(59,130,246,0.2)', '#60A5FA', 'rgba(59,130,246,0.3)');
  const udagChip = chipBadge('rgba(0,224,196,0.2)', '#00E0C4', 'rgba(0,224,196,0.3)');

  const sectionBox: React.CSSProperties = {
    borderRadius: 12, background: 'var(--dag-bg)',
    border: '1px solid rgba(255,255,255,0.045)', padding: 16,
    display: 'flex', flexDirection: 'column', gap: 12,
  };

  const labelRow: React.CSSProperties = {
    display: 'flex', alignItems: 'center', justifyContent: 'space-between',
  };

  const labelText: React.CSSProperties = {
    fontSize: 10, color: 'var(--dag-text-muted)', fontWeight: 500,
    textTransform: 'uppercase', letterSpacing: 1.5,
  };

  const chainIndicator = (bg: string, _dotColor?: string): React.CSSProperties => ({
    width: 16, height: 16, borderRadius: '50%', background: bg,
    display: 'flex', alignItems: 'center', justifyContent: 'center',
  });

  const chainDot = (color: string): React.CSSProperties => ({
    width: 8, height: 8, borderRadius: '50%', background: color,
  });

  const inputBox: React.CSSProperties = {
    width: '100%', padding: '16px 112px 16px 16px',
    background: 'var(--dag-input-bg)', border: '1px solid rgba(255,255,255,0.045)',
    borderRadius: 12, fontSize: 22, fontFamily: "'DM Mono',monospace",
    color: 'var(--dag-text)', outline: 'none', transition: 'all 0.15s',
  };

  const inputBoxSmall: React.CSSProperties = {
    width: '100%', padding: '12px 16px',
    background: 'var(--dag-input-bg)', border: '1px solid rgba(255,255,255,0.045)',
    borderRadius: 12, fontSize: 12, fontFamily: "'DM Mono',monospace",
    color: 'var(--dag-text)', outline: 'none', transition: 'all 0.15s',
  };

  const gradientButton: React.CSSProperties = {
    width: '100%', padding: '16px 0', borderRadius: 12, border: 'none',
    background: 'linear-gradient(to right, #00E0C4, #0066FF)',
    color: '#fff', fontWeight: 600, fontSize: 12,
    cursor: 'pointer', display: 'flex', alignItems: 'center', justifyContent: 'center',
    gap: 8, transition: 'all 0.15s',
  };

  const warningButton = (color: string): React.CSSProperties => ({
    width: '100%', padding: '10px 0', borderRadius: 12,
    background: `${color}15`, color, border: `1px solid ${color}30`,
    fontSize: 10, fontWeight: 500, cursor: 'pointer',
    display: 'flex', alignItems: 'center', justifyContent: 'center', gap: 8,
    transition: 'all 0.15s',
  });

  const successBox: React.CSSProperties = {
    borderRadius: 12, background: 'rgba(0,224,196,0.05)',
    border: '1px solid rgba(0,224,196,0.2)', padding: 14,
    display: 'flex', alignItems: 'center', gap: 10,
  };

  const statCard: React.CSSProperties = {
    borderRadius: 12, background: 'var(--dag-card)',
    border: '1px solid var(--dag-border)', padding: 16,
  };

  return (
    <div style={{ padding: m ? '12px 14px' : '18px 26px', fontFamily: "'DM Sans',sans-serif", animation: 'slideUp 0.3s ease' }}>
      <style>{`@keyframes slideUp{from{opacity:0;transform:translateY(10px)}to{opacity:1;transform:translateY(0)}}`}</style>
      {/* Wallet Picker Modal */}
      {showWalletPicker && (
        <WalletPickerModal
          wallets={eth.discoveredWallets}
          onSelect={handleWalletSelect}
          onClose={() => setShowWalletPicker(false)}
        />
      )}

      {/* Hero Header */}
      <PageHeader
        title="UltraDAG Bridge"
        subtitle="Secured by the validator federation — no external relayers"
        right={<>
          {/* Bridge status pill */}
          <div style={{
            display: 'flex', alignItems: 'center', gap: 8,
            padding: '6px 12px', borderRadius: 999, fontSize: 10, fontWeight: 500,
            background: canBridge ? 'rgba(0,224,196,0.1)' : bridgePaused ? 'rgba(239,68,68,0.1)' : 'rgba(255,184,0,0.1)',
            color: canBridge ? '#00E0C4' : bridgePaused ? '#EF4444' : '#FFB800',
            border: `1px solid ${canBridge ? 'rgba(0,224,196,0.2)' : bridgePaused ? 'rgba(239,68,68,0.2)' : 'rgba(255,184,0,0.2)'}`,
          }}>
            <div style={{
              width: 6, height: 6, borderRadius: '50%',
              background: canBridge ? '#00E0C4' : bridgePaused ? '#EF4444' : '#FFB800',
              animation: canBridge ? 'pulse 2s infinite' : 'none',
            }} />
            {canBridge ? 'Bridge Active' : bridgePaused ? 'Bridge Paused' : 'Inactive'}
          </div>

          {/* Connected wallet pill */}
          {eth.connected && eth.selectedWallet ? (
            <div style={{
              display: 'flex', alignItems: 'center', gap: 8,
              padding: '6px 12px', borderRadius: 999,
              background: 'var(--dag-input-bg)', border: '1px solid var(--dag-border)',
            }}>
              <img src={eth.selectedWallet.icon} alt="" style={{ width: 16, height: 16, borderRadius: 4 }} onError={(e) => { (e.target as HTMLImageElement).style.display = 'none'; }} />
              <span style={{ fontSize: 10, color: 'var(--dag-text)', fontFamily: "'DM Mono',monospace" }}>{eth.address.slice(0, 6)}...{eth.address.slice(-4)}</span>
              <button
                onClick={eth.disconnect}
                style={{ marginLeft: 2, padding: 2, borderRadius: 4, border: 'none', background: 'transparent', color: 'var(--dag-text-muted)', cursor: 'pointer', transition: 'all 0.15s' }}
                title="Disconnect"
              >
                <X style={{ width: 12, height: 12 }} />
              </button>
            </div>
          ) : eth.connected ? (
            <div style={{
              display: 'flex', alignItems: 'center', gap: 8,
              padding: '6px 12px', borderRadius: 999,
              background: 'var(--dag-input-bg)', border: '1px solid var(--dag-border)',
            }}>
              <div style={{ width: 8, height: 8, borderRadius: '50%', background: '#00E0C4' }} />
              <span style={{ fontSize: 10, color: 'var(--dag-text)', fontFamily: "'DM Mono',monospace" }}>{eth.address.slice(0, 6)}...{eth.address.slice(-4)}</span>
              <button
                onClick={eth.disconnect}
                style={{ marginLeft: 2, padding: 2, borderRadius: 4, border: 'none', background: 'transparent', color: 'var(--dag-text-muted)', cursor: 'pointer', transition: 'all 0.15s' }}
                title="Disconnect"
              >
                <X style={{ width: 12, height: 12 }} />
              </button>
            </div>
          ) : null}
        </>}
      />

      {!CONTRACTS_DEPLOYED && (
        <div role="alert" style={{
          display: 'flex', alignItems: 'flex-start', gap: 10,
          padding: '14px 18px', marginBottom: 18, borderRadius: 12,
          background: 'rgba(255,184,0,0.06)', border: '1px solid rgba(255,184,0,0.25)',
        }}>
          <AlertTriangle style={{ width: 18, height: 18, color: '#FFB800', flexShrink: 0, marginTop: 1 }} />
          <div style={{ flex: 1 }}>
            <div style={{ fontSize: 12.5, fontWeight: 600, color: '#FFB800', marginBottom: 3 }}>
              Bridge contracts not yet deployed
            </div>
            <div style={{ fontSize: 11, color: 'var(--dag-text-muted)', lineHeight: 1.5 }}>
              The UDAG token and bridge contracts on Arbitrum have not been deployed yet. Deposits and withdrawals are disabled until <code style={{ fontSize: 10 }}>UDAG_TOKEN_ADDRESS</code> and <code style={{ fontSize: 10 }}>UDAG_BRIDGE_ADDRESS</code> in <code style={{ fontSize: 10 }}>lib/contracts.ts</code> are populated.
            </div>
          </div>
        </div>
      )}

      <div style={{ display: 'grid', gridTemplateColumns: m ? '1fr' : '3fr 2fr', gap: 24 }}>
        {/* Left: Bridge Form */}
        <div style={{ display: 'flex', flexDirection: 'column', gap: 24 }}>

          {/* Main Bridge Card */}
          <div style={{ borderRadius: 14, background: 'var(--dag-card)', border: '1px solid var(--dag-border)', overflow: 'hidden' }}>
            <div style={{ padding: 24, display: 'flex', flexDirection: 'column', gap: 20 }}>

              {/* Direction tabs */}
              <div style={{ display: 'flex', gap: 4, padding: 4, background: 'var(--dag-bg)', borderRadius: 12 }}>
                <button
                  onClick={() => setDirection('to-native')}
                  style={{
                    flex: 1, padding: '10px 12px', borderRadius: 8, fontSize: 12, fontWeight: 500,
                    transition: 'all 0.15s', border: 'none', cursor: 'pointer',
                    background: direction === 'to-native' ? 'var(--dag-input-bg)' : 'transparent',
                    color: direction === 'to-native' ? 'var(--dag-text)' : 'var(--dag-text-muted)',
                    boxShadow: direction === 'to-native' ? '0 1px 3px rgba(0,0,0,0.2)' : 'none',
                  }}
                >
                  <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', gap: 8 }}>
                    <span style={arbChip}>ARB</span>
                    <ArrowRight style={{ width: 14, height: 14 }} />
                    <span style={udagChip}>UDAG</span>
                  </div>
                </button>
                <button
                  onClick={() => setDirection('to-arbitrum')}
                  style={{
                    flex: 1, padding: '10px 12px', borderRadius: 8, fontSize: 12, fontWeight: 500,
                    transition: 'all 0.15s', border: 'none', cursor: 'pointer',
                    background: direction === 'to-arbitrum' ? 'var(--dag-input-bg)' : 'transparent',
                    color: direction === 'to-arbitrum' ? 'var(--dag-text)' : 'var(--dag-text-muted)',
                    boxShadow: direction === 'to-arbitrum' ? '0 1px 3px rgba(0,0,0,0.2)' : 'none',
                  }}
                >
                  <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', gap: 8 }}>
                    <span style={udagChip}>UDAG</span>
                    <ArrowRight style={{ width: 14, height: 14 }} />
                    <span style={arbChip}>ARB</span>
                  </div>
                </button>
              </div>

              {/* ---- Arbitrum -> Native ---- */}
              {direction === 'to-native' ? (
                <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
                  {/* Source: Arbitrum */}
                  <div style={sectionBox}>
                    <div style={labelRow}>
                      <span style={labelText}>From</span>
                      <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
                        <div style={chainIndicator('rgba(59,130,246,0.2)', '#60A5FA')}>
                          <div style={chainDot('#60A5FA')} />
                        </div>
                        <span style={{ fontSize: 10, fontWeight: 500, color: '#60A5FA' }}>Arbitrum</span>
                        <span style={chipBadge('rgba(59,130,246,0.1)', 'rgba(96,165,250,0.7)', 'rgba(59,130,246,0.2)')}>ERC-20</span>
                      </div>
                    </div>

                    {eth.connected ? (
                      <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
                        {/* Wallet info row */}
                        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
                          <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                            {eth.selectedWallet && (
                              <img src={eth.selectedWallet.icon} alt="" style={{ width: 20, height: 20, borderRadius: 4 }} onError={(e) => { (e.target as HTMLImageElement).style.display = 'none'; }} />
                            )}
                            <span style={{ fontSize: 12, color: 'var(--dag-text)', fontFamily: "'DM Mono',monospace" }}>{eth.address.slice(0, 6)}...{eth.address.slice(-4)}</span>
                            <CopyButton text={eth.address} />
                          </div>
                          <button
                            onClick={eth.disconnect}
                            style={{
                              fontSize: 10, color: 'var(--dag-text-muted)', background: 'transparent',
                              border: 'none', cursor: 'pointer', display: 'flex', alignItems: 'center', gap: 4,
                              transition: 'all 0.15s',
                            }}
                          >
                            <Unplug style={{ width: 12, height: 12 }} /> Disconnect
                          </button>
                        </div>

                        {/* Amount input */}
                        <div style={{ position: 'relative' }}>
                          <input
                            type="text"
                            inputMode="decimal"
                            value={amount}
                            onChange={amountInputHandler}
                            placeholder="0.00"
                            disabled={!canBridge}
                            style={{ ...inputBox, opacity: !canBridge ? 0.5 : 1 }}
                          />
                          <div style={{ position: 'absolute', right: 12, top: '50%', transform: 'translateY(-50%)', display: 'flex', alignItems: 'center', gap: 8 }}>
                            {eth.connected && (
                              <button
                                onClick={() => setAmount(formatUnits(eth.udagBalanceRaw, 8))}
                                style={{
                                  fontSize: 10, fontWeight: 700, color: '#00E0C4',
                                  padding: '4px 8px', borderRadius: 4,
                                  background: 'rgba(0,224,196,0.1)', border: 'none',
                                  cursor: 'pointer', transition: 'all 0.15s',
                                }}
                              >
                                MAX
                              </button>
                            )}
                            <span style={{ fontSize: 12, fontWeight: 500, color: 'var(--dag-text-muted)' }}>UDAG</span>
                          </div>
                        </div>

                        {/* Balance row */}
                        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', fontSize: 10 }}>
                          <span style={{ color: 'var(--dag-text-muted)' }}>
                            Balance: <span style={{ color: 'var(--dag-text)', fontFamily: "'DM Mono',monospace" }}>{Number(eth.udagBalance).toLocaleString(undefined, { maximumFractionDigits: 4 })}</span> UDAG
                          </span>
                          <span style={{ color: 'var(--dag-text-muted)' }}>
                            <span style={{ color: 'var(--dag-text)', fontFamily: "'DM Mono',monospace" }}>{Number(eth.balance).toFixed(4)}</span> ETH
                          </span>
                        </div>

                        {!eth.isCorrectChain && (
                          <button
                            onClick={eth.switchToArbitrum}
                            style={warningButton('#FFB800')}
                          >
                            <AlertTriangle style={{ width: 14, height: 14 }} />
                            Switch to Arbitrum Network
                          </button>
                        )}
                      </div>
                    ) : (
                      <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
                        {CONTRACTS_DEPLOYED && (
                          <div style={{
                            display: 'flex', alignItems: 'flex-start', gap: 8, padding: 10,
                            borderRadius: 8, background: 'rgba(0,224,196,0.05)', border: '1px solid rgba(0,224,196,0.1)',
                          }}>
                            <Shield style={{ width: 16, height: 16, color: '#00E0C4', marginTop: 2, flexShrink: 0 }} />
                            <p style={{ fontSize: 10, color: 'var(--dag-text-muted)' }}>
                              Secured by UltraDAG's validator federation — same BFT consensus that protects the network.
                            </p>
                          </div>
                        )}
                        <button
                          onClick={handleConnectClick}
                          disabled={eth.loading || !CONTRACTS_DEPLOYED}
                          title={!CONTRACTS_DEPLOYED ? 'Bridge contracts not deployed yet' : undefined}
                          style={{ ...gradientButton, padding: '14px 0', opacity: eth.loading || !CONTRACTS_DEPLOYED ? 0.4 : 1, cursor: !CONTRACTS_DEPLOYED ? 'not-allowed' : 'pointer' }}
                        >
                          {eth.loading ? <Loader2 style={{ width: 16, height: 16, animation: 'spin 1s linear infinite' }} /> : <Wallet style={{ width: 16, height: 16 }} />}
                          {!CONTRACTS_DEPLOYED ? 'Bridge Unavailable' : eth.loading ? 'Connecting...' : 'Connect Wallet'}
                        </button>
                        <style>{`@keyframes spin{to{transform:rotate(360deg)}}`}</style>
                      </div>
                    )}
                  </div>

                  {/* Arrow Divider */}
                  <div style={{ display: 'flex', justifyContent: 'center', margin: '-4px 0', position: 'relative', zIndex: 10 }}>
                    <div style={{
                      width: 40, height: 40, borderRadius: 12,
                      background: 'var(--dag-card)', border: '1px solid var(--dag-border)',
                      display: 'flex', alignItems: 'center', justifyContent: 'center',
                      boxShadow: '0 4px 12px rgba(0,0,0,0.3)',
                    }}>
                      <ArrowDown style={{ width: 16, height: 16, color: '#00E0C4' }} />
                    </div>
                  </div>

                  {/* Destination: UltraDAG */}
                  <div style={sectionBox}>
                    <div style={labelRow}>
                      <span style={labelText}>To</span>
                      <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
                        <div style={chainIndicator('rgba(0,224,196,0.2)', '#00E0C4')}>
                          <div style={chainDot('#00E0C4')} />
                        </div>
                        <span style={{ fontSize: 10, fontWeight: 500, color: '#00E0C4' }}>UltraDAG</span>
                        <span style={chipBadge('rgba(0,224,196,0.1)', 'rgba(0,224,196,0.7)', 'rgba(0,224,196,0.2)')}>Native</span>
                      </div>
                    </div>

                    {wallets.length > 0 ? (
                      <>
                        <select
                          value={selectedWalletIdx}
                          onChange={(e) => setSelectedWalletIdx(Number(e.target.value))}
                          style={{
                            ...inputBoxSmall,
                            appearance: 'none' as const, cursor: 'pointer',
                            backgroundImage: `url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='12' height='12' viewBox='0 0 24 24' fill='none' stroke='%2394a3b8' stroke-width='2'%3E%3Cpolyline points='6 9 12 15 18 9'/%3E%3C/svg%3E")`,
                            backgroundRepeat: 'no-repeat', backgroundPosition: 'right 12px center',
                          }}
                        >
                          {wallets.map((w, i) => (
                            <option key={w.address} value={i}>
                              {w.name || `Wallet ${i + 1}`}
                            </option>
                          ))}
                        </select>
                        <div style={{ display: 'flex', alignItems: 'center', gap: 6, fontSize: 10, color: 'var(--dag-text-muted)' }}>
                          <CheckCircle style={{ width: 12, height: 12, color: '#00E0C4' }} />
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
                          style={{ ...inputBoxSmall, opacity: !canBridge ? 0.5 : 1 }}
                        />
                        <p style={{ fontSize: 10, color: 'var(--dag-text-muted)' }}>Enter your UltraDAG address or create a wallet in the Wallet tab</p>
                      </>
                    )}
                  </div>

                  {/* Action Buttons */}
                  <div style={{ display: 'flex', flexDirection: 'column', gap: 10, paddingTop: 4 }}>
                    {/* Approval button */}
                    {needsApproval && (
                      <button
                        onClick={handleApprove}
                        disabled={approving || !canBridge}
                        style={{ ...warningButton('#FFB800'), opacity: (approving || !canBridge) ? 0.5 : 1, padding: '14px 0' }}
                      >
                        {approving ? <Loader2 style={{ width: 16, height: 16, animation: 'spin 1s linear infinite' }} /> : <Shield style={{ width: 16, height: 16 }} />}
                        {approving ? 'Approving...' : 'Approve UDAG Transfer'}
                      </button>
                    )}

                    {/* Bridge button */}
                    <button
                      onClick={handleBridgeToNative}
                      disabled={!eth.connected || !canBridge || bridging || needsApproval || amountSats <= 0n}
                      style={{
                        ...gradientButton,
                        opacity: (!eth.connected || !canBridge || bridging || needsApproval || amountSats <= 0n) ? 0.4 : 1,
                        cursor: (!eth.connected || !canBridge || bridging || needsApproval || amountSats <= 0n) ? 'not-allowed' : 'pointer',
                      }}
                    >
                      {bridging ? (
                        <Loader2 style={{ width: 16, height: 16, animation: 'spin 1s linear infinite' }} />
                      ) : !eth.connected ? (
                        <Wallet style={{ width: 16, height: 16 }} />
                      ) : (
                        <Zap style={{ width: 16, height: 16 }} />
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
                    <div style={successBox}>
                      <div style={{
                        width: 32, height: 32, borderRadius: '50%',
                        background: 'rgba(0,224,196,0.1)',
                        display: 'flex', alignItems: 'center', justifyContent: 'center', flexShrink: 0,
                      }}>
                        <CheckCircle style={{ width: 16, height: 16, color: '#00E0C4' }} />
                      </div>
                      <div style={{ flex: 1, minWidth: 0 }}>
                        <p style={{ fontSize: 10, fontWeight: 500, color: '#00E0C4' }}>Transaction submitted!</p>
                        <p style={{ fontSize: 10, color: 'var(--dag-text-muted)', marginTop: 2, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', fontFamily: "'DM Mono',monospace" }}>{txHash}</p>
                      </div>
                      <a
                        href={`https://arbiscan.io/tx/${txHash}`}
                        target="_blank"
                        rel="noopener noreferrer"
                        style={{ fontSize: 10, color: '#00E0C4', display: 'flex', alignItems: 'center', gap: 4, flexShrink: 0, textDecoration: 'none' }}
                      >
                        View <ExternalLink style={{ width: 12, height: 12 }} />
                      </a>
                    </div>
                  )}
                </div>
              ) : (
                /* ---- Native -> Arbitrum ---- */
                <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
                  {/* Source: UltraDAG Wallet */}
                  <div style={sectionBox}>
                    <div style={labelRow}>
                      <span style={labelText}>From</span>
                      <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
                        <div style={chainIndicator('rgba(0,224,196,0.2)', '#00E0C4')}>
                          <div style={chainDot('#00E0C4')} />
                        </div>
                        <span style={{ fontSize: 10, fontWeight: 500, color: '#00E0C4' }}>UltraDAG</span>
                        <span style={chipBadge('rgba(0,224,196,0.1)', 'rgba(0,224,196,0.7)', 'rgba(0,224,196,0.2)')}>Native</span>
                      </div>
                    </div>

                    {wallets.length > 0 ? (
                      <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
                        <WalletSelector wallets={wallets} selectedIdx={depositWalletIdx} onChange={setDepositWalletIdx} label="Source Wallet" />

                        {/* Amount input */}
                        <div style={{ position: 'relative' }}>
                          <input
                            type="text"
                            inputMode="decimal"
                            value={depositAmount}
                            onChange={depositAmountInputHandler}
                            placeholder="0.00"
                            style={inputBox}
                          />
                          <div style={{ position: 'absolute', right: 12, top: '50%', transform: 'translateY(-50%)', display: 'flex', alignItems: 'center', gap: 8 }}>
                            {(() => {
                              const wb = wallets[depositWalletIdx] ? walletBalances.get(wallets[depositWalletIdx].address) : undefined;
                              const bal = wb?.balance ?? 0;
                              return bal > 10000 ? (
                                <button
                                  onClick={() => setDepositAmount(((bal - 10000) / 100_000_000).toFixed(8).replace(/0+$/, '').replace(/\.$/, ''))}
                                  style={{
                                    fontSize: 10, fontWeight: 700, color: '#00E0C4',
                                    padding: '4px 8px', borderRadius: 4,
                                    background: 'rgba(0,224,196,0.1)', border: 'none',
                                    cursor: 'pointer', transition: 'all 0.15s',
                                  }}
                                >
                                  MAX
                                </button>
                              ) : null;
                            })()}
                            <span style={{ fontSize: 12, fontWeight: 500, color: 'var(--dag-text-muted)' }}>UDAG</span>
                          </div>
                        </div>

                        {/* Balance row */}
                        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', fontSize: 10 }}>
                          <span style={{ color: 'var(--dag-text-muted)' }}>
                            Balance: <span style={{ color: 'var(--dag-text)', fontFamily: "'DM Mono',monospace" }}>
                              {formatUdag(walletBalances.get(wallets[depositWalletIdx]?.address ?? '')?.balance ?? 0)}
                            </span> UDAG
                          </span>
                          <span style={{ color: 'var(--dag-text-muted)' }}>
                            Fee: <span style={{ color: 'var(--dag-text)', fontFamily: "'DM Mono',monospace" }}>0.0001</span> UDAG
                          </span>
                        </div>
                      </div>
                    ) : (
                      <p style={{ fontSize: 12, color: 'var(--dag-text-muted)' }}>Create a wallet in the Wallet tab to bridge UDAG to Arbitrum.</p>
                    )}
                  </div>

                  {/* Arrow Divider */}
                  <div style={{ display: 'flex', justifyContent: 'center', margin: '-4px 0', position: 'relative', zIndex: 10 }}>
                    <div style={{
                      width: 40, height: 40, borderRadius: 12,
                      background: 'var(--dag-card)', border: '1px solid var(--dag-border)',
                      display: 'flex', alignItems: 'center', justifyContent: 'center',
                      boxShadow: '0 4px 12px rgba(0,0,0,0.3)',
                    }}>
                      <ArrowDown style={{ width: 16, height: 16, color: '#00E0C4' }} />
                    </div>
                  </div>

                  {/* Destination: Arbitrum */}
                  <div style={sectionBox}>
                    <div style={labelRow}>
                      <span style={labelText}>To</span>
                      <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
                        <div style={chainIndicator('rgba(59,130,246,0.2)', '#60A5FA')}>
                          <div style={chainDot('#60A5FA')} />
                        </div>
                        <span style={{ fontSize: 10, fontWeight: 500, color: '#60A5FA' }}>Arbitrum</span>
                        <span style={chipBadge('rgba(59,130,246,0.1)', 'rgba(96,165,250,0.7)', 'rgba(59,130,246,0.2)')}>ERC-20</span>
                      </div>
                    </div>

                    <input
                      type="text"
                      value={depositRecipient}
                      onChange={(e) => setDepositRecipient(e.target.value)}
                      placeholder="0x... (Ethereum/Arbitrum address)"
                      style={inputBoxSmall}
                    />
                    {eth.connected && eth.address && (
                      <button
                        onClick={() => setDepositRecipient(eth.address)}
                        style={{ fontSize: 10, color: '#00E0C4', background: 'transparent', border: 'none', cursor: 'pointer', textAlign: 'left', transition: 'all 0.15s' }}
                      >
                        Use connected wallet: {eth.address.slice(0, 6)}...{eth.address.slice(-4)}
                      </button>
                    )}
                    <p style={{ fontSize: 10, color: 'var(--dag-text-muted)' }}>Enter your Arbitrum/Ethereum address to receive bridged UDAG ERC-20 tokens</p>
                  </div>

                  {/* Bridge button */}
                  <div style={{ display: 'flex', flexDirection: 'column', gap: 10, paddingTop: 4 }}>
                    <button
                      onClick={handleBridgeToArbitrum}
                      disabled={wallets.length === 0 || depositing || !depositAmount || !depositRecipient}
                      style={{
                        ...gradientButton,
                        opacity: (wallets.length === 0 || depositing || !depositAmount || !depositRecipient) ? 0.4 : 1,
                        cursor: (wallets.length === 0 || depositing || !depositAmount || !depositRecipient) ? 'not-allowed' : 'pointer',
                      }}
                    >
                      {depositing ? (
                        <Loader2 style={{ width: 16, height: 16, animation: 'spin 1s linear infinite' }} />
                      ) : (
                        <Zap style={{ width: 16, height: 16 }} />
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
                  <div style={{
                    display: 'flex', alignItems: 'flex-start', gap: 8, padding: 10,
                    borderRadius: 8, background: 'rgba(0,224,196,0.05)', border: '1px solid rgba(0,224,196,0.1)',
                  }}>
                    <Info style={{ width: 14, height: 14, color: '#00E0C4', marginTop: 2, flexShrink: 0 }} />
                    <p style={{ fontSize: 10, color: 'var(--dag-text-muted)' }}>
                      Validators sign attestations as part of consensus. Once 2/3+ signatures are collected, you can claim on Arbitrum.
                    </p>
                  </div>

                  {/* Validator badge */}
                  <div style={{ display: 'flex', justifyContent: 'center' }}>
                    <div style={{
                      display: 'inline-flex', alignItems: 'center', gap: 8,
                      padding: '6px 12px', borderRadius: 999,
                      background: 'rgba(0,224,196,0.1)', color: '#00E0C4',
                      fontSize: 10, fontWeight: 500, border: '1px solid rgba(0,224,196,0.2)',
                    }}>
                      <div style={{ width: 6, height: 6, borderRadius: '50%', background: '#00E0C4', animation: 'pulse 2s infinite' }} />
                      5 Validators Active
                    </div>
                  </div>

                  {/* Deposit success message */}
                  {depositTxHash && (
                    <div style={successBox}>
                      <div style={{
                        width: 32, height: 32, borderRadius: '50%',
                        background: 'rgba(0,224,196,0.1)',
                        display: 'flex', alignItems: 'center', justifyContent: 'center', flexShrink: 0,
                      }}>
                        <CheckCircle style={{ width: 16, height: 16, color: '#00E0C4' }} />
                      </div>
                      <div style={{ flex: 1, minWidth: 0 }}>
                        <p style={{ fontSize: 10, fontWeight: 500, color: '#00E0C4' }}>Bridge deposit submitted!</p>
                        <p style={{ fontSize: 10, color: 'var(--dag-text-muted)', marginTop: 2 }}>Validators will sign attestations during consensus. Track status in the Arbitrum &rarr; Native tab.</p>
                        <p style={{ fontSize: 10, color: 'var(--dag-text-muted)', marginTop: 2, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', fontFamily: "'DM Mono',monospace" }}>{depositTxHash}</p>
                      </div>
                      <CopyButton text={depositTxHash} />
                    </div>
                  )}
                </div>
              )}
            </div>
          </div>

          {/* Bridge Stats Row */}
          <div style={{ display: 'grid', gridTemplateColumns: m ? '1fr' : 'repeat(3, 1fr)', gap: 12 }}>
            <div style={statCard}>
              <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 8 }}>
                <Lock style={{ width: 14, height: 14, color: '#00E0C4' }} />
                <span style={{ fontSize: 10, color: 'var(--dag-text-muted)', textTransform: 'uppercase', letterSpacing: 1.5, fontWeight: 500 }}>Bridge Reserve</span>
              </div>
              <p style={{ fontSize: 16, fontWeight: 700, color: 'var(--dag-text)', fontFamily: "'DM Mono',monospace" }}>
                {bridgeReserve ? formatUdag(bridgeReserve.reserve_udag) : '0.00'}
              </p>
              <p style={{ fontSize: 10, color: 'var(--dag-text-muted)', marginTop: 2 }}>UDAG locked on native chain</p>
            </div>
            <div style={statCard}>
              <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 8 }}>
                <Clock style={{ width: 14, height: 14, color: 'var(--dag-text-muted)' }} />
                <span style={{ fontSize: 10, color: 'var(--dag-text-muted)', textTransform: 'uppercase', letterSpacing: 1.5, fontWeight: 500 }}>24h Volume</span>
              </div>
              <p style={{ fontSize: 16, fontWeight: 700, color: 'var(--dag-text)', fontFamily: "'DM Mono',monospace" }}>
                {dailyCap > 0n ? formatUdagBigint(dailyVolume) : '0.00'}
              </p>
              {dailyCap > 0n && (
                <div style={{ marginTop: 6 }}>
                  <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', fontSize: 10, color: 'var(--dag-text-muted)', marginBottom: 4 }}>
                    <span>{formatUdagBigint(dailyVolume)}</span>
                    <span>{formatUdagBigint(dailyCap)} cap</span>
                  </div>
                  <div style={{ width: '100%', background: 'var(--dag-bg)', borderRadius: 999, height: 4 }}>
                    <div
                      style={{
                        background: 'linear-gradient(to right, #00E0C4, #0066FF)',
                        height: 4, borderRadius: 999, transition: 'all 0.3s',
                        width: `${Math.min(100, Number((dailyVolume * 100n) / dailyCap))}%`,
                      }}
                    />
                  </div>
                </div>
              )}
            </div>
            <div style={statCard}>
              <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 8 }}>
                <Shield style={{ width: 14, height: 14, color: '#00E0C4' }} />
                <span style={{ fontSize: 10, color: 'var(--dag-text-muted)', textTransform: 'uppercase', letterSpacing: 1.5, fontWeight: 500 }}>Security</span>
              </div>
              <p style={{ fontSize: 16, fontWeight: 700, color: 'var(--dag-text)' }}>2/3 BFT</p>
              <p style={{ fontSize: 10, color: 'var(--dag-text-muted)', marginTop: 2 }}>Validator threshold consensus</p>
            </div>
          </div>
        </div>

        {/* Right: Attestations + How it Works */}
        <div style={{ display: 'flex', flexDirection: 'column', gap: 24 }}>

          {/* Attestations Card */}
          <div style={{ borderRadius: 14, background: 'var(--dag-card)', border: '1px solid var(--dag-border)', overflow: 'hidden' }}>
            <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', padding: '20px 20px 0' }}>
              <h3 style={{ fontSize: 12, fontWeight: 600, color: 'var(--dag-text)' }}>Recent Attestations</h3>
              {loadingAttestations && <Loader2 style={{ width: 14, height: 14, color: 'var(--dag-text-muted)', animation: 'spin 1s linear infinite' }} />}
            </div>
            <div style={{ padding: '12px 20px 20px' }}>
              {attestationError && attestations.length === 0 ? (
                <div style={{ textAlign: 'center', padding: '32px 0' }}>
                  <AlertTriangle style={{ width: 20, height: 20, color: '#FFB800', margin: '0 auto 10px' }} />
                  <p style={{ fontSize: 12, color: '#FFB800' }}>{attestationError}</p>
                </div>
              ) : attestations.length === 0 ? (
                <div style={{ textAlign: 'center', padding: '32px 0' }}>
                  <div style={{
                    width: 40, height: 40, borderRadius: 12,
                    background: 'var(--dag-input-bg)', border: '1px solid var(--dag-border)',
                    display: 'flex', alignItems: 'center', justifyContent: 'center',
                    margin: '0 auto 12px',
                  }}>
                    <Shield style={{ width: 20, height: 20, color: 'var(--dag-text-faint)' }} />
                  </div>
                  <p style={{ fontSize: 12, color: 'var(--dag-text-muted)' }}>No recent attestations</p>
                  <p style={{ fontSize: 10, color: 'var(--dag-text-muted)', marginTop: 4 }}>Bridge transfers will appear here</p>
                </div>
              ) : (
                <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
                  {attestations.map((att) => (
                    <div key={att.nonce} style={{
                      borderRadius: 12, background: 'var(--dag-bg)',
                      border: '1px solid rgba(255,255,255,0.045)', padding: 14,
                      display: 'flex', flexDirection: 'column', gap: 10,
                    }}>
                      {/* Header row */}
                      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
                        <span style={{ fontSize: 10, fontFamily: "'DM Mono',monospace", color: 'var(--dag-text-muted)' }}>#{att.nonce}</span>
                        {att.ready ? (
                          <span style={{
                            display: 'flex', alignItems: 'center', gap: 4,
                            fontSize: 10, padding: '2px 8px', borderRadius: 999,
                            background: 'rgba(0,224,196,0.1)', color: '#00E0C4', border: '1px solid rgba(0,224,196,0.2)',
                          }}>
                            <div style={{ width: 6, height: 6, borderRadius: '50%', background: '#00E0C4', animation: 'pulse 2s infinite' }} />
                            Ready to Claim
                          </span>
                        ) : (
                          <span style={{
                            fontSize: 10, padding: '2px 8px', borderRadius: 999,
                            background: 'rgba(255,184,0,0.1)', color: '#FFB800', border: '1px solid rgba(255,184,0,0.2)',
                          }}>
                            {att.signature_count}/{att.threshold} signatures
                          </span>
                        )}
                      </div>

                      {/* Progress bar */}
                      <div style={{ width: '100%', background: 'var(--dag-input-bg)', borderRadius: 999, height: 6 }}>
                        <div
                          style={{
                            height: 6, borderRadius: 999, transition: 'all 0.3s',
                            background: att.ready ? '#00E0C4' : '#FFB800',
                            width: `${Math.min(100, (att.signature_count / att.threshold) * 100)}%`,
                          }}
                        />
                      </div>

                      {/* Details */}
                      <div style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
                        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', fontSize: 10 }}>
                          <span style={{ color: 'var(--dag-text-muted)' }}>Amount</span>
                          <span style={{ color: 'var(--dag-text)', fontFamily: "'DM Mono',monospace", fontWeight: 500 }}>{formatUdag(att.amount_udag)} UDAG</span>
                        </div>
                        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', fontSize: 10 }}>
                          <span style={{ color: 'var(--dag-text-muted)' }}>From</span>
                          <span style={{ color: 'var(--dag-text)', fontFamily: "'DM Mono',monospace", fontSize: 11, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', maxWidth: 140 }}>{att.sender_bech32}</span>
                        </div>
                        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', fontSize: 10 }}>
                          <span style={{ color: 'var(--dag-text-muted)' }}>To</span>
                          <span style={{ color: 'var(--dag-text)', fontFamily: "'DM Mono',monospace", fontSize: 11, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', maxWidth: 140 }}>{att.recipient.slice(0, 10)}...{att.recipient.slice(-6)}</span>
                        </div>
                      </div>

                      {/* Claim button */}
                      {att.ready && (
                        <button
                          onClick={() => handleClaim(att)}
                          disabled={claiming === att.nonce}
                          style={{
                            width: '100%', marginTop: 4, padding: '10px 0', borderRadius: 12,
                            background: 'rgba(0,224,196,0.1)', color: '#00E0C4',
                            border: '1px solid rgba(0,224,196,0.2)',
                            fontSize: 10, fontWeight: 600, cursor: claiming === att.nonce ? 'default' : 'pointer',
                            display: 'flex', alignItems: 'center', justifyContent: 'center', gap: 6,
                            transition: 'all 0.15s', opacity: claiming === att.nonce ? 0.5 : 1,
                          }}
                        >
                          {claiming === att.nonce ? (
                            <Loader2 style={{ width: 14, height: 14, animation: 'spin 1s linear infinite' }} />
                          ) : (
                            <CheckCircle style={{ width: 14, height: 14 }} />
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
          <div style={{ borderRadius: 14, background: 'var(--dag-card)', border: '1px solid var(--dag-border)', overflow: 'hidden' }}>
            <div style={{ padding: 20 }}>
              <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 16 }}>
                <Shield style={{ width: 16, height: 16, color: '#00E0C4' }} />
                <h3 style={{ fontSize: 12, fontWeight: 600, color: 'var(--dag-text)' }}>How the Bridge Works</h3>
              </div>

              <div style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>
                {/* Step 1 */}
                <div style={{ display: 'flex', alignItems: 'flex-start', gap: 12 }}>
                  <div style={{
                    width: 32, height: 32, borderRadius: 8,
                    background: 'rgba(0,224,196,0.1)', border: '1px solid rgba(0,224,196,0.2)',
                    display: 'flex', alignItems: 'center', justifyContent: 'center', flexShrink: 0,
                  }}>
                    <span style={{ fontSize: 10, fontWeight: 700, color: '#00E0C4' }}>1</span>
                  </div>
                  <div>
                    <p style={{ fontSize: 12, fontWeight: 500, color: 'var(--dag-text)' }}>Deposit</p>
                    <p style={{ fontSize: 10, color: 'var(--dag-text-muted)', marginTop: 2 }}>Lock tokens on the source chain. Funds are escrowed in the bridge contract.</p>
                  </div>
                </div>

                {/* Step 2 */}
                <div style={{ display: 'flex', alignItems: 'flex-start', gap: 12 }}>
                  <div style={{
                    width: 32, height: 32, borderRadius: 8,
                    background: 'rgba(0,102,255,0.1)', border: '1px solid rgba(0,102,255,0.2)',
                    display: 'flex', alignItems: 'center', justifyContent: 'center', flexShrink: 0,
                  }}>
                    <span style={{ fontSize: 10, fontWeight: 700, color: '#0066FF' }}>2</span>
                  </div>
                  <div>
                    <p style={{ fontSize: 12, fontWeight: 500, color: 'var(--dag-text)' }}>Attestation</p>
                    <p style={{ fontSize: 10, color: 'var(--dag-text-muted)', marginTop: 2 }}>Validators sign the transfer as part of normal consensus. 2/3+ signatures required.</p>
                  </div>
                </div>

                {/* Step 3 */}
                <div style={{ display: 'flex', alignItems: 'flex-start', gap: 12 }}>
                  <div style={{
                    width: 32, height: 32, borderRadius: 8,
                    background: 'rgba(0,224,196,0.1)', border: '1px solid rgba(0,224,196,0.2)',
                    display: 'flex', alignItems: 'center', justifyContent: 'center', flexShrink: 0,
                  }}>
                    <span style={{ fontSize: 10, fontWeight: 700, color: '#00E0C4' }}>3</span>
                  </div>
                  <div>
                    <p style={{ fontSize: 12, fontWeight: 500, color: 'var(--dag-text)' }}>Claim</p>
                    <p style={{ fontSize: 10, color: 'var(--dag-text-muted)', marginTop: 2 }}>Submit the attestation proof to unlock tokens on the destination chain.</p>
                  </div>
                </div>
              </div>

              <div style={{
                marginTop: 16, padding: 12, borderRadius: 12,
                background: 'rgba(0,224,196,0.05)', border: '1px solid rgba(0,224,196,0.1)',
              }}>
                <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                  <CheckCircle style={{ width: 14, height: 14, color: '#00E0C4', flexShrink: 0 }} />
                  <div>
                    <span style={{ fontSize: 10, fontWeight: 500, color: '#00E0C4' }}>Same security as DAG consensus</span>
                    <p style={{ fontSize: 10, color: 'var(--dag-text-muted)', marginTop: 2 }}>No external relayers. Uses the existing validator federation with 2/3 BFT threshold.</p>
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
