import { useState, useEffect, useCallback } from 'react';
import { Link } from 'react-router-dom';
import { fullAddr, getNodeUrl } from '../lib/api';
import { DisplayIdentity } from '../components/shared/DisplayIdentity';
import { getPasskeyWallet } from '../lib/passkey-wallet';
import { signAndSubmitSmartOp } from '../lib/webauthn-sign';
import { CreateKeystoreModal } from '../components/wallet/CreateKeystoreModal';
import { AddWalletModal } from '../components/wallet/AddWalletModal';
import { changePassword } from '../lib/keystore';
import { CopyButton } from '../components/shared/CopyButton';
import { Pagination } from '../components/shared/Pagination';
import { useIsMobile } from '../hooks/useIsMobile';
import { PageHeader } from '../components/shared/PageHeader';
import { primaryButtonStyle } from '../lib/theme';
import type { Wallet } from '../lib/keystore';
import type { WalletBalance } from '../hooks/useWalletBalances';

interface PocketInfo { label: string; address: string; address_bech32: string; balance: number; staked: number; delegated: number }

/** A unified item in the wallet list — either the main passkey wallet or a derived pocket. */
interface WalletItem {
  type: 'main' | 'pocket';
  name: string;       // display name
  label: string;      // pocket label or 'main'
  address: string;    // hex address
  balance: number;
  staked: number;
  delegated: number;
  pending?: boolean;   // true while tx is confirming on-chain
}

const SATS = 100_000_000;
const fmt = (v: number) =>
  (v / SATS).toLocaleString(undefined, {
    minimumFractionDigits: 2,
    maximumFractionDigits: 4,
  });

const S = {
  card: {
    background: 'var(--dag-card)',
    border: '1px solid var(--dag-border)',
    borderRadius: 14,
    padding: '18px 20px',
  } as React.CSSProperties,
  stat: {
    background: 'var(--dag-card)',
    borderRadius: 10,
    padding: '12px 14px',
  } as React.CSSProperties,
  input: {
    width: '100%',
    padding: '10px 14px',
    borderRadius: 10,
    background: 'var(--dag-input-bg)',
    border: '1px solid var(--dag-border)',
    color: 'var(--dag-text)',
    fontSize: 13,
    outline: 'none',
    fontFamily: "'DM Sans',sans-serif",
    boxSizing: 'border-box' as const,
  } as React.CSSProperties,
  btn: (c = '#00E0C4') =>
    ({
      padding: '7px 14px',
      borderRadius: 8,
      background: `${c}10`,
      border: `1px solid ${c}20`,
      color: c,
      fontSize: 11,
      fontWeight: 600 as const,
      cursor: 'pointer',
      transition: 'all 0.2s',
      display: 'inline-flex' as const,
      alignItems: 'center' as const,
      gap: 5,
    }),
  btnSolid: {
    padding: '9px 18px',
    borderRadius: 10,
    background: '#00E0C4',
    color: '#080C14',
    fontSize: 12,
    fontWeight: 700 as const,
    cursor: 'pointer',
    border: 'none',
    transition: 'all 0.2s',
  },
  mono: { fontFamily: "'DM Mono',monospace" },
};

const CSS = `
  @keyframes slideUp {
    from { opacity: 0; transform: translateY(10px) }
    to { opacity: 1; transform: translateY(0) }
  }
  @keyframes spin {
    to { transform: rotate(360deg) }
  }
  @keyframes pulse {
    0%, 100% { opacity: 0.4 }
    50% { opacity: 0.15 }
  }
  @keyframes shimmer {
    0% { background-position: -200% 0 }
    100% { background-position: 200% 0 }
  }
  @keyframes balancePulse {
    0%, 100% { text-shadow: 0 0 4px rgba(0,224,196,0.1) }
    50% { text-shadow: 0 0 10px rgba(0,224,196,0.2) }
  }
  .wallet-card {
    transition: all 0.25s ease;
  }
  .wallet-card:hover {
    border-color: rgba(0,224,196,0.15) !important;
    background: rgba(0,224,196,0.015) !important;
  }
  .action-btn:hover {
    opacity: 0.85;
    transform: translateY(-1px);
  }
  .header-btn:hover {
    opacity: 0.8;
  }
  input:focus {
    border-color: rgba(0,224,196,0.3) !important;
  }
`;

/* ── Modal ── */
function Modal({
  open,
  title,
  onClose,
  children,
}: {
  open: boolean;
  title: string;
  onClose: () => void;
  children: React.ReactNode;
}) {
  if (!open) return null;
  return (
    <div
      style={{
        position: 'fixed',
        inset: 0,
        zIndex: 50,
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        background: 'rgba(0,0,0,0.65)',
        backdropFilter: 'blur(6px)',
      }}
    >
      <div
        style={{
          ...S.card,
          maxWidth: 440,
          width: '100%',
          boxShadow: '0 24px 60px rgba(0,0,0,0.6)',
          animation: 'slideUp 0.2s ease',
        }}
      >
        <div
          style={{
            display: 'flex',
            justifyContent: 'space-between',
            alignItems: 'center',
            marginBottom: 18,
          }}
        >
          <h2 style={{ fontSize: 15, fontWeight: 700, color: 'var(--dag-text)' }}>
            {title}
          </h2>
          <button
            onClick={onClose}
            style={{
              background: 'none',
              border: 'none',
              color: 'var(--dag-subheading)',
              cursor: 'pointer',
              fontSize: 16,
              padding: '4px 8px',
              borderRadius: 6,
              transition: 'background 0.15s',
            }}
            onMouseEnter={e => (e.currentTarget.style.background = 'var(--dag-input-bg)')}
            onMouseLeave={e => (e.currentTarget.style.background = 'none')}
          >
            ✕
          </button>
        </div>
        {children}
      </div>
    </div>
  );
}

/* ── Change Password Modal ── */
function ChangePwModal({ open, onClose }: { open: boolean; onClose: () => void }) {
  const [cur, setCur] = useState('');
  const [np, setNp] = useState('');
  const [cp, setCp] = useState('');
  const [err, setErr] = useState('');
  const [ok, setOk] = useState(false);
  const [ld, setLd] = useState(false);

  const close = () => {
    setCur('');
    setNp('');
    setCp('');
    setErr('');
    setOk(false);
    onClose();
  };

  return (
    <Modal open={open} title="Change Password" onClose={close}>
      <form
        onSubmit={async e => {
          e.preventDefault();
          setErr('');
          setOk(false);
          if (np.length < 8) { setErr('Min 8 chars.'); return; }
          if (np !== cp) { setErr("Passwords don't match."); return; }
          setLd(true);
          try {
            (await changePassword(cur, np)) ? setOk(true) : setErr('Wrong current password.');
          } catch {
            setErr('Failed.');
          } finally {
            setLd(false);
          }
        }}
        style={{ display: 'flex', flexDirection: 'column', gap: 12 }}
      >
        {[
          { l: 'Current Password', v: cur, s: setCur },
          { l: 'New Password', v: np, s: setNp },
          { l: 'Confirm New Password', v: cp, s: setCp },
        ].map((f, i) => (
          <div key={i}>
            <div style={{ fontSize: 10, color: 'var(--dag-text-muted)', marginBottom: 4, fontWeight: 500 }}>
              {f.l}
            </div>
            <input
              type="password"
              style={S.input}
              value={f.v}
              onChange={e => f.s(e.target.value)}
              required
            />
          </div>
        ))}
        {err && (
          <div
            style={{
              fontSize: 10.5,
              color: '#EF4444',
              background: 'rgba(239,68,68,0.06)',
              border: '1px solid rgba(239,68,68,0.15)',
              borderRadius: 8,
              padding: '7px 10px',
            }}
          >
            {err}
          </div>
        )}
        {ok && (
          <div
            style={{
              fontSize: 10.5,
              color: '#00E0C4',
              background: 'rgba(0,224,196,0.06)',
              border: '1px solid rgba(0,224,196,0.15)',
              borderRadius: 8,
              padding: '7px 10px',
            }}
          >
            Password changed successfully.
          </div>
        )}
        <div style={{ display: 'flex', justifyContent: 'flex-end', gap: 8, marginTop: 4 }}>
          <button type="button" onClick={close} style={S.btn('var(--dag-text-muted)')}>
            Cancel
          </button>
          <button type="submit" disabled={ld} style={{ ...S.btnSolid, opacity: ld ? 0.6 : 1 }}>
            {ld ? 'Changing...' : 'Change Password'}
          </button>
        </div>
      </form>
    </Modal>
  );
}

/* ── Balance Bar ── */
function BalanceBar({
  balance,
  staked,
  delegated,
  total,
}: {
  balance: number;
  staked: number;
  delegated: number;
  total: number;
}) {
  if (total <= 0) return null;
  const segments = [
    { value: balance, color: '#00E0C4', label: 'Available' },
    { value: staked, color: '#0066FF', label: 'Staked' },
    { value: delegated, color: '#A855F7', label: 'Delegated' },
  ].filter(s => s.value > 0);

  return (
    <div style={{ marginTop: 10 }}>
      <div
        style={{
          display: 'flex',
          borderRadius: 3,
          overflow: 'hidden',
          height: 3,
          background: 'var(--dag-input-bg)',
        }}
      >
        {segments.map((seg, i) => (
          <div
            key={i}
            style={{
              width: `${(seg.value / total) * 100}%`,
              background: seg.color,
              transition: 'width 0.5s ease',
            }}
          />
        ))}
      </div>
      {/* Inline legend */}
      <div style={{ display: 'flex', gap: 10, marginTop: 5 }}>
        {segments.map((seg, i) => (
          <span key={i} style={{ fontSize: 8, color: 'var(--dag-text-faint)', display: 'flex', alignItems: 'center', gap: 3 }}>
            <span style={{ width: 5, height: 5, borderRadius: 1, background: seg.color, display: 'inline-block' }} />
            {((seg.value / total) * 100).toFixed(0)}% {seg.label}
          </span>
        ))}
      </div>
    </div>
  );
}

/* ── Main Component ── */

interface WalletPageProps {
  unlocked: boolean;
  hasStore: boolean;
  wallets: Wallet[];
  balances: Map<string, WalletBalance>;
  onCreate: (p: string) => Promise<void>;
  onUnlock: (p: string) => Promise<boolean>;
  onImportBlob: (j: string) => boolean;
  onAddWallet: (n: string, s: string, a: string) => Promise<void>;
  onRemoveWallet: (i: number) => Promise<void>;
  onExportBlob: () => string | null;
  onGenerateKeypair: () => Promise<{ secret_key: string; address: string } | null>;
  webauthnAvailable?: boolean;
  webauthnEnrolled?: boolean;
  onEnrollWebAuthn?: () => Promise<boolean>;
  onRemoveWebAuthn?: () => void;
  notificationsSupported?: boolean;
  notificationsEnabled?: boolean;
  onToggleNotifications?: () => Promise<void>;
  primaryAddress?: string | null;
  onSetPrimary?: (address: string | null) => void;
  /** True when a passkey wallet is active — passkey always wins, so the UI hides "Set as primary". */
  isPasskeyPrimary?: boolean;
}

export function WalletPage({
  unlocked,
  hasStore,
  wallets,
  balances,
  onCreate,
  onUnlock,
  onImportBlob,
  onAddWallet,
  onRemoveWallet,
  onExportBlob,
  onGenerateKeypair,
  webauthnAvailable,
  webauthnEnrolled,
  onEnrollWebAuthn,
  onRemoveWebAuthn,
  primaryAddress: _primaryAddress,
  onSetPrimary: _onSetPrimary,
  isPasskeyPrimary: _isPasskeyPrimary,
}: WalletPageProps) {
  const [showKsModal, setShowKsModal] = useState(false);
  const [showAddModal, setShowAddModal] = useState(false);
  const [showPwModal, setShowPwModal] = useState(false);
  const [sel, setSel] = useState<number | null>(null);
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [showSettings, setShowSettings] = useState(false);
  const [walletPage, setWalletPage] = useState(1);
  const [removeConfirm, setRemoveConfirm] = useState<number | null>(null);
  const pw = getPasskeyWallet();
  const m = useIsMobile();
  const WALLET_PAGE_SIZE = 10;
  // Pocket state
  const [pockets, setPockets] = useState<PocketInfo[]>([]);
  const [pendingPockets, setPendingPockets] = useState<string[]>([]); // labels being confirmed
  const [showCreatePocket, setShowCreatePocket] = useState(false);
  const [newPocketLabel, setNewPocketLabel] = useState('');
  const [pocketLoading, setPocketLoading] = useState(false);
  const [pocketMsg, setPocketMsg] = useState('');

  const fetchPockets = useCallback(async () => {
    if (!pw?.address) return;
    try {
      const res = await fetch(`${getNodeUrl()}/smart-account/${pw.address}`, { signal: AbortSignal.timeout(5000) });
      if (!res.ok) return;
      const data = await res.json();
      const rawPockets: Array<{ label: string; address: string; address_bech32: string }> = data.pockets ?? [];
      // Fetch balances for all pockets in parallel.
      const withBalances = await Promise.all(rawPockets.map(async (p) => {
        try {
          const bRes = await fetch(`${getNodeUrl()}/balance/${p.address}`, { signal: AbortSignal.timeout(5000) });
          if (bRes.ok) {
            const bd = await bRes.json();
            return { ...p, balance: bd.balance ?? 0, staked: bd.staked ?? 0, delegated: bd.delegated ?? 0 };
          }
        } catch { /* offline */ }
        return { ...p, balance: 0, staked: 0, delegated: 0 };
      }));
      setPockets(withBalances);
      // Clear pending labels that are now confirmed on-chain.
      const confirmedLabels = new Set(withBalances.map(p => p.label));
      setPendingPockets(prev => prev.filter(l => !confirmedLabels.has(l)));
    } catch { /* offline — keep existing */ }
  }, [pw?.address]);

  useEffect(() => {
    fetchPockets();
    // Refresh pockets every 5s so newly-created pockets confirm quickly.
    const iv = setInterval(fetchPockets, 5000);
    return () => clearInterval(iv);
  }, [fetchPockets]);

  useEffect(() => {
    setWalletPage(1);
  }, [wallets.length]);

  // Reset selection if wallet is removed
  useEffect(() => {
    if (sel !== null && sel >= wallets.length + pockets.length) {
      setSel(null);
    }
  }, [wallets.length, pockets.length, sel]);

  /* ── Locked / No Store ── */
  if (!unlocked) {
    return (
      <div style={{ padding: m ? '12px 14px' : '18px 26px', fontFamily: "'DM Sans',sans-serif" }}>
        <style>{CSS}</style>
        <PageHeader title="Wallet" subtitle="Manage your UltraDAG wallets" />

        <div
          style={{
            display: 'flex',
            flexDirection: 'column',
            alignItems: 'center',
            padding: '60px 0',
            gap: 18,
            animation: 'slideUp 0.4s ease',
          }}
        >
          <div
            style={{
              width: 80,
              height: 80,
              borderRadius: 12,
              background: 'linear-gradient(135deg, rgba(0,224,196,0.06), rgba(0,102,255,0.06))',
              border: '1px solid rgba(0,224,196,0.1)',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              fontSize: 32,
            }}
          >
            <span
              style={{
                background: '#00E0C4',
                WebkitBackgroundClip: 'text',
                WebkitTextFillColor: 'transparent',
              }}
            >
              ◇
            </span>
          </div>

          <div style={{ textAlign: 'center' }}>
            <div style={{ fontSize: 16, fontWeight: 600, color: 'var(--dag-text)' }}>
              {hasStore ? 'Wallet Locked' : 'No Wallet Yet'}
            </div>
            <div
              style={{
                fontSize: 11.5,
                color: 'var(--dag-subheading)',
                marginTop: 6,
                maxWidth: 300,
                lineHeight: 1.6,
              }}
            >
              {hasStore
                ? 'Enter your password to access your wallets.'
                : 'Create a new wallet or import an existing one to get started.'}
            </div>
          </div>

          <button
            onClick={() => setShowKsModal(true)}
            style={{
              ...S.btnSolid,
              padding: '11px 28px',
              fontSize: 13,
              boxShadow: '0 4px 20px rgba(0,224,196,0.15)',
            }}
          >
            {hasStore ? '⚿ Unlock Wallet' : '+ Get Started'}
          </button>
        </div>

        <CreateKeystoreModal
          open={showKsModal}
          onClose={() => setShowKsModal(false)}
          onCreateOrUnlock={async pw => {
            if (hasStore) return onUnlock(pw);
            await onCreate(pw);
            return true;
          }}
          onCreateWithKey={async (pw, n, s, a) => {
            await onCreate(pw);
            await onAddWallet(n, s, a);
            return true;
          }}
          onImport={onImportBlob}
          hasExisting={hasStore}
        />
      </div>
    );
  }

  /* ── Unlocked ── */
  const handleExport = () => {
    const j = onExportBlob();
    if (j) {
      const b = new Blob([j], { type: 'application/json' });
      const u = URL.createObjectURL(b);
      const a = document.createElement('a');
      a.href = u;
      a.download = 'ultradag-keystore.json';
      a.click();
      URL.revokeObjectURL(u);
    }
  };

  const handleRemove = (index: number) => {
    if (removeConfirm === index) {
      onRemoveWallet(index);
      setSel(null);
      setRemoveConfirm(null);
    } else {
      setRemoveConfirm(index);
      // Auto-clear confirm state after 3s
      setTimeout(() => setRemoveConfirm(null), 3000);
    }
  };

  // Build unified wallet items: main wallet + pockets, all rendered identically.
  const items: WalletItem[] = [];
  // Add main wallet(s) from keystore/passkey.
  for (const w of wallets) {
    const b = balances.get(w.address);
    const isPk = pw?.address === w.address;
    items.push({
      type: 'main',
      name: isPk ? (pw?.name ? `@${pw.name}` : w.name) : w.name,
      label: 'main',
      address: w.address,
      balance: b?.balance ?? 0,
      staked: b?.staked ?? 0,
      delegated: b?.delegated ?? 0,
    });
  }
  // Add confirmed pockets.
  for (const p of pockets) {
    items.push({
      type: 'pocket',
      name: pw?.name ? `@${pw.name}.${p.label}` : p.label,
      label: p.label,
      address: p.address,
      balance: p.balance,
      staked: p.staked,
      delegated: p.delegated,
    });
  }
  // Add pending pockets (not yet confirmed on-chain) — shown with animation.
  for (const label of pendingPockets) {
    if (pockets.some(p => p.label === label)) continue; // already confirmed
    items.push({
      type: 'pocket',
      name: pw?.name ? `@${pw.name}.${label}` : label,
      label,
      address: '', // not yet known
      balance: 0,
      staked: 0,
      delegated: 0,
      pending: true,
    });
  }

  const selectedItem = sel !== null && sel < items.length ? items[sel] : null;

  // Totals across ALL items (main + pockets).
  let totalBal = 0;
  let totalStaked = 0;
  let totalDelegated = 0;
  for (const item of items) {
    totalBal += item.balance;
    totalStaked += item.staked;
    totalDelegated += item.delegated;
  }
  const grandTotal = totalBal + totalStaked + totalDelegated;

  return (
    <div style={{ padding: m ? '12px 14px' : '18px 26px', fontFamily: "'DM Sans',sans-serif" }}>
      <style>{CSS}</style>

      {/* ── Header ── */}
      <PageHeader
        title="Wallet"
        subtitle={`${grandTotal > 0 ? fmt(grandTotal) + ' UDAG' : 'Your passkey-secured account'}${pockets.length > 0 ? ` · ${pockets.length} pocket${pockets.length !== 1 ? 's' : ''}` : ''}`}
      />

      {/* ── Portfolio Summary ── */}
      {(() => {
        const summaryCards = [
          { l: 'TOTAL', v: grandTotal, c: '#fff', i: '◈', always: true },
          { l: 'AVAILABLE', v: totalBal, c: '#00E0C4', i: '◎', always: true },
          { l: 'STAKED', v: totalStaked, c: '#0066FF', i: '⬡', always: false },
          { l: 'DELEGATED', v: totalDelegated, c: '#A855F7', i: '◇', always: false },
        ].filter(c => c.always || c.v > 0);
        const cols = Math.min(summaryCards.length, m ? 2 : 4);
        return (
          <div
            style={{
              display: 'grid',
              gridTemplateColumns: `repeat(${cols},1fr)`,
              gap: m ? 10 : 12,
              marginBottom: 18,
              animation: 'slideUp 0.4s ease',
            }}
          >
            {summaryCards.map((p, i) => (
              <div key={i} style={{ ...S.card, padding: '14px 16px' }}>
                <div style={{ display: 'flex', alignItems: 'center', gap: 6, marginBottom: 6 }}>
                  <span style={{ color: p.c, fontSize: 14 }}>{p.i}</span>
                  <span style={{ fontSize: 9, color: 'var(--dag-text-muted)', letterSpacing: 1 }}>{p.l}</span>
                </div>
                <div style={{ fontSize: m ? 16 : 20, fontWeight: 700, color: p.c, ...S.mono }}>
                  {fmt(p.v)}
                </div>
                <div style={{ fontSize: 9, color: 'var(--dag-text-faint)', marginTop: 2 }}>UDAG</div>
              </div>
            ))}
          </div>
        );
      })()}

      {/* ── Wallet List + Detail ── */}
      {wallets.length === 0 ? (
        <div
          style={{
            display: 'flex',
            flexDirection: 'column',
            alignItems: 'center',
            padding: '50px 0',
            gap: 14,
            animation: 'slideUp 0.5s ease',
          }}
        >
          <div style={{ fontSize: 40, opacity: 0.1 }}>◎</div>
          <p style={{ fontSize: 13, color: 'var(--dag-text-muted)' }}>Create a wallet to get started</p>
          <p style={{ fontSize: 11, color: 'var(--dag-text-faint)' }}>
            Your passkey secures everything — no seed phrases needed
          </p>
        </div>
      ) : (
        <div
          style={{
            display: 'grid',
            gridTemplateColumns: m ? '1fr' : '1.2fr 1fr',
            gap: 14,
            animation: 'slideUp 0.5s ease',
          }}
        >
          {/* ── Unified item list (main wallet + pockets) ── */}
          <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
            {items
              .slice(
                (walletPage - 1) * WALLET_PAGE_SIZE,
                walletPage * WALLET_PAGE_SIZE,
              )
              .map((item, _pi) => {
                const i = (walletPage - 1) * WALLET_PAGE_SIZE + _pi;
                const active = sel === i;
                const totalVal = item.balance + item.staked + item.delegated;
                const isMain = item.type === 'main';
                const isPk = pw?.address === item.address;

                return (
                  <div
                    key={item.address || `pending-${item.label}`}
                    className="wallet-card"
                    onClick={() => setSel(active ? null : i)}
                    style={{
                      ...S.card,
                      cursor: 'pointer',
                      borderColor: item.pending
                        ? 'rgba(0,224,196,0.3)'
                        : active ? 'rgba(0,224,196,0.25)'
                        : 'var(--dag-border)',
                      background: item.pending
                        ? 'rgba(0,224,196,0.02)'
                        : active ? 'rgba(0,224,196,0.025)'
                        : 'var(--dag-card)',
                      transform: active ? 'scale(1.005)' : 'none',
                      boxShadow: active ? '0 0 20px rgba(0,224,196,0.04)' : 'none',
                    }}
                  >
                    <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                      <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
                        <div style={{
                          width: 40, height: 40, borderRadius: 11,
                          display: 'flex', alignItems: 'center', justifyContent: 'center',
                          background: isMain && isPk ? '#00E0C4'
                            : item.type === 'pocket' ? 'rgba(255,184,0,0.15)'
                            : `hsl(${i * 60 + 180}, 35%, 18%)`,
                          fontSize: 15, fontWeight: 800,
                          color: isMain && isPk ? '#080C14'
                            : item.type === 'pocket' ? '#FFB800'
                            : 'var(--dag-text)',
                          boxShadow: active ? '0 0 12px rgba(0,224,196,0.15)' : 'none',
                          transition: 'box-shadow 0.25s',
                        }}>
                          {item.type === 'pocket' ? '◈' : item.name[0]?.toUpperCase() || '?'}
                        </div>
                        <div>
                          <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
                            <span style={{ fontSize: 13, fontWeight: 600, color: 'var(--dag-text)', fontFamily: item.type === 'pocket' ? "'DM Mono',monospace" : 'inherit' }}>
                              {item.name}
                            </span>
                            {isMain && isPk && (
                              <span style={{ fontSize: 8, background: 'rgba(0,224,196,0.1)', color: '#00E0C4', padding: '1px 6px', borderRadius: 3, fontWeight: 700, letterSpacing: 0.6 }}>
                                MAIN
                              </span>
                            )}
                            {item.type === 'pocket' && !item.pending && (
                              <span style={{ fontSize: 8, background: 'rgba(255,184,0,0.12)', color: '#FFB800', padding: '1px 6px', borderRadius: 3, fontWeight: 700, letterSpacing: 0.6 }}>
                                POCKET
                              </span>
                            )}
                            {item.pending && (
                              <span style={{
                                fontSize: 8, padding: '1px 6px', borderRadius: 3, fontWeight: 700, letterSpacing: 0.6,
                                background: 'rgba(0,224,196,0.1)', color: '#00E0C4',
                                animation: 'pulse 1.5s ease-in-out infinite',
                              }}>
                                CONFIRMING
                              </span>
                            )}
                          </div>
                          {item.pending ? (
                            <div style={{ marginTop: 3, fontSize: 10, color: '#00E0C4', animation: 'pulse 1.5s ease-in-out infinite' }}>
                              broadcasting to network...
                            </div>
                          ) : (
                            <div style={{ marginTop: 2 }}>
                              <DisplayIdentity address={item.address} size="xs" />
                            </div>
                          )}
                        </div>
                      </div>

                      <div style={{ textAlign: 'right' }}>
                        {item.pending ? (
                          <div style={{ width: 18, height: 18, border: '2px solid rgba(0,224,196,0.2)', borderTop: '2px solid #00E0C4', borderRadius: '50%', animation: 'spin 0.8s linear infinite' }} />

                        ) : (
                          <>
                            <div style={{ fontSize: 16, fontWeight: 700, color: active ? '#00E0C4' : 'var(--dag-text)', ...S.mono, transition: 'color 0.25s' }}>
                              {fmt(totalVal)}
                            </div>
                            <div style={{ fontSize: 9, color: 'var(--dag-text-faint)' }}>UDAG</div>
                          </>
                        )}
                      </div>
                    </div>

                    {totalVal > 0 && (
                      <BalanceBar
                        balance={item.balance}
                        staked={item.staked}
                        delegated={item.delegated}
                        total={totalVal}
                      />
                    )}
                  </div>
                );
              })}

            <Pagination
              page={walletPage}
              totalPages={Math.ceil(items.length / WALLET_PAGE_SIZE)}
              onPageChange={setWalletPage}
              totalItems={items.length}
              pageSize={WALLET_PAGE_SIZE}
            />

            {/* ── Add Pocket ── */}
            {pw && (
              <div style={{ marginTop: 8 }}>
                {!showCreatePocket ? (
                  <div>
                    {pockets.length === 0 && pendingPockets.length === 0 && (
                      <div style={{
                        ...S.card, padding: '14px 16px', marginBottom: 8,
                        background: 'linear-gradient(135deg, rgba(255,184,0,0.03), rgba(0,224,196,0.02))',
                        borderColor: 'rgba(255,184,0,0.1)',
                      }}>
                        <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 6 }}>
                          <span style={{ fontSize: 14, color: '#FFB800' }}>◈</span>
                          <span style={{ fontSize: 12, fontWeight: 600, color: 'var(--dag-text)' }}>Add pockets for labeled sub-addresses</span>
                        </div>
                        <p style={{ fontSize: 10.5, color: 'var(--dag-text-muted)', lineHeight: 1.5, margin: 0 }}>
                          Pockets let people send to <span style={{ color: '#FFB800', fontFamily: "'DM Mono',monospace" }}>@{pw.name || 'you'}.savings</span> or <span style={{ color: '#FFB800', fontFamily: "'DM Mono',monospace" }}>@{pw.name || 'you'}.business</span>. Each pocket is a separate address controlled by your passkey — available on every device.
                        </p>
                      </div>
                    )}
                    <button onClick={() => { setShowCreatePocket(true); setPocketMsg(''); setNewPocketLabel(''); }}
                      style={{ ...S.btn(), width: '100%', padding: '10px 0', justifyContent: 'center', display: 'flex', alignItems: 'center', gap: 6 }}>
                      ◈ Add Pocket
                    </button>
                  </div>
                ) : (
                  <div style={{ ...S.card }}>
                    <div style={{ fontSize: 10, color: 'var(--dag-text-faint)', letterSpacing: 1, marginBottom: 8 }}>NEW POCKET</div>
                    <input
                      type="text" maxLength={32} autoFocus
                      value={newPocketLabel}
                      onChange={e => setNewPocketLabel(e.target.value.toLowerCase().replace(/[^a-z0-9-]/g, ''))}
                      placeholder="savings"
                      style={{ width: '100%', padding: '8px 12px', borderRadius: 8, background: 'var(--dag-input-bg)', border: '1px solid var(--dag-border)', color: 'var(--dag-text)', fontSize: 12, outline: 'none', fontFamily: "'DM Mono',monospace", marginBottom: 6 }}
                    />
                    {newPocketLabel && pw.name && (
                      <div style={{ fontSize: 11, color: '#00E0C4', fontFamily: "'DM Mono',monospace", marginBottom: 6 }}>
                        @{pw.name}.{newPocketLabel}
                      </div>
                    )}
                    {pocketMsg && (
                      <p role="alert" style={{ fontSize: 10.5, color: pocketMsg.startsWith('✓') ? '#00E0C4' : '#EF4444', marginBottom: 6 }}>{pocketMsg}</p>
                    )}
                    <div style={{ display: 'flex', gap: 8 }}>
                      <button
                        disabled={pocketLoading || !newPocketLabel}
                        onClick={async () => {
                          setPocketLoading(true); setPocketMsg('');
                          try {
                            const balRes = await fetch(`${getNodeUrl()}/balance/${pw.address}`, { signal: AbortSignal.timeout(5000) });
                            const balData = await balRes.json();
                            const createdLabel = newPocketLabel;
                            await signAndSubmitSmartOp({ CreatePocket: { label: createdLabel } }, 0, balData.nonce ?? 0);
                            // Optimistic: show the pocket immediately in the list as "confirming".
                            setPendingPockets(prev => [...prev, createdLabel]);
                            setPocketMsg('');
                            setNewPocketLabel('');
                            setShowCreatePocket(false);
                            // The 5s auto-refresh will pick up the confirmed pocket and clear the pending state.
                          } catch (e: unknown) {
                            setPocketMsg(e instanceof Error ? e.message : 'Failed');
                          } finally { setPocketLoading(false); }
                        }}
                        style={{ ...primaryButtonStyle, padding: '7px 14px', fontSize: 11, opacity: pocketLoading || !newPocketLabel ? 0.4 : 1 }}
                      >
                        {pocketLoading ? 'Creating...' : 'Create'}
                      </button>
                      <button onClick={() => setShowCreatePocket(false)}
                        style={{ padding: '7px 14px', borderRadius: 8, background: 'var(--dag-input-bg)', border: '1px solid var(--dag-border)', color: 'var(--dag-text-muted)', fontSize: 11, fontWeight: 600, cursor: 'pointer' }}>
                        Cancel
                      </button>
                    </div>
                  </div>
                )}
              </div>
            )}
          </div>

          {/* ── Detail Panel ── */}
          <div
            style={{
              ...S.card,
              position: m ? 'static' : 'sticky',
              top: 20,
              alignSelf: 'start',
            }}
          >
            {selectedItem ? (() => {
              const isPocket = selectedItem.type === 'pocket';
              const isPk = pw?.address === selectedItem.address;
              const selTotal = selectedItem.balance + selectedItem.staked + selectedItem.delegated;
              const pocketFullName = isPocket && pw?.name ? `@${pw.name}.${selectedItem.label}` : null;
              const keystoreWallet = !isPocket ? wallets.find(w => w.address === selectedItem.address) : null;
              const mainBal = keystoreWallet ? balances.get(keystoreWallet.address) : null;

              // Special view for pending (confirming) pockets.
              if (selectedItem.pending) {
                return (
                  <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center', minHeight: 280, gap: 14, textAlign: 'center' }}>
                    <div style={{ width: 48, height: 48, border: '3px solid rgba(0,224,196,0.15)', borderTop: '3px solid #00E0C4', borderRadius: '50%', animation: 'spin 0.8s linear infinite' }} />
                    <div>
                      <div style={{ fontSize: 15, fontWeight: 700, color: 'var(--dag-text)', marginBottom: 4 }}>
                        Creating {pocketFullName || selectedItem.label}
                      </div>
                      <p style={{ fontSize: 11, color: 'var(--dag-text-muted)', lineHeight: 1.5, maxWidth: 260, margin: '0 auto' }}>
                        Your passkey signed the transaction. Waiting for the network to confirm it — usually takes one round (~5 seconds).
                      </p>
                    </div>
                    <div style={{ display: 'flex', alignItems: 'center', gap: 6, padding: '6px 12px', borderRadius: 8, background: 'rgba(0,224,196,0.04)', border: '1px solid rgba(0,224,196,0.1)' }}>
                      <span style={{ width: 6, height: 6, borderRadius: '50%', background: '#00E0C4', animation: 'pulse 1.5s ease-in-out infinite' }} />
                      <span style={{ fontSize: 10, color: '#00E0C4', fontWeight: 600 }}>Broadcasting to validators...</span>
                    </div>
                  </div>
                );
              }

              return (
              <div>
                {/* Detail Header */}
                <div style={{ display: 'flex', alignItems: 'center', gap: 12, marginBottom: 16, paddingBottom: 14, borderBottom: '1px solid var(--dag-table-border)' }}>
                  <div style={{
                    width: 48, height: 48, borderRadius: 14,
                    display: 'flex', alignItems: 'center', justifyContent: 'center',
                    background: isPocket ? 'rgba(255,184,0,0.15)' : isPk ? '#00E0C4' : 'var(--dag-card-hover)',
                    fontSize: 20, fontWeight: 800,
                    color: isPocket ? '#FFB800' : isPk ? '#080C14' : 'var(--dag-text)',
                  }}>
                    {isPocket ? '◈' : selectedItem.name[0]?.toUpperCase()}
                  </div>
                  <div style={{ flex: 1 }}>
                    <div style={{ fontSize: 16, fontWeight: 700, color: 'var(--dag-text)' }}>
                      {selectedItem.name}
                    </div>
                    {pocketFullName && (
                      <div style={{ fontSize: 11, color: '#FFB800', fontFamily: "'DM Mono',monospace", marginTop: 1 }}>
                        {pocketFullName}
                      </div>
                    )}
                    <div style={{ marginTop: 2 }}>
                      <DisplayIdentity address={selectedItem.address} size="xs" />
                    </div>
                  </div>
                  {isPk && !isPocket && (
                    <span style={{ fontSize: 8.5, background: 'rgba(0,224,196,0.08)', color: '#00E0C4', padding: '3px 8px', borderRadius: 4, fontWeight: 700, letterSpacing: 0.8 }}>
                      MAIN
                    </span>
                  )}
                  {isPocket && (
                    <span style={{ fontSize: 8.5, background: 'rgba(255,184,0,0.12)', color: '#FFB800', padding: '3px 8px', borderRadius: 4, fontWeight: 700, letterSpacing: 0.8 }}>
                      POCKET
                    </span>
                  )}
                </div>

                {/* Balance stats */}
                <div style={{ display: 'grid', gridTemplateColumns: selTotal > 0 ? '1fr 1fr' : '1fr', gap: 8, marginBottom: 14 }}>
                  <div style={{ ...S.stat, border: '1px solid var(--dag-border)' }}>
                    <div style={{ display: 'flex', alignItems: 'center', gap: 4, marginBottom: 4 }}>
                      <span style={{ fontSize: 11, color: '#00E0C4' }}>◎</span>
                      <span style={{ fontSize: 8.5, color: 'var(--dag-text-faint)', letterSpacing: 1, fontWeight: 600 }}>AVAILABLE</span>
                    </div>
                    <div style={{ fontSize: 17, fontWeight: 700, color: '#00E0C4', ...S.mono }}>{fmt(selectedItem.balance)}</div>
                    <div style={{ fontSize: 8.5, color: 'var(--dag-text-faint)', marginTop: 2 }}>UDAG</div>
                  </div>
                  {selectedItem.staked > 0 && (
                    <div style={{ ...S.stat, border: '1px solid var(--dag-border)' }}>
                      <div style={{ display: 'flex', alignItems: 'center', gap: 4, marginBottom: 4 }}>
                        <span style={{ fontSize: 11, color: '#0066FF' }}>⬡</span>
                        <span style={{ fontSize: 8.5, color: 'var(--dag-text-faint)', letterSpacing: 1, fontWeight: 600 }}>STAKED</span>
                      </div>
                      <div style={{ fontSize: 17, fontWeight: 700, color: '#0066FF', ...S.mono }}>{fmt(selectedItem.staked)}</div>
                      <div style={{ fontSize: 8.5, color: 'var(--dag-text-faint)', marginTop: 2 }}>UDAG</div>
                    </div>
                  )}
                  {selectedItem.delegated > 0 && (
                    <div style={{ ...S.stat, border: '1px solid var(--dag-border)' }}>
                      <div style={{ display: 'flex', alignItems: 'center', gap: 4, marginBottom: 4 }}>
                        <span style={{ fontSize: 11, color: '#A855F7' }}>◇</span>
                        <span style={{ fontSize: 8.5, color: 'var(--dag-text-faint)', letterSpacing: 1, fontWeight: 600 }}>DELEGATED</span>
                      </div>
                      <div style={{ fontSize: 17, fontWeight: 700, color: '#A855F7', ...S.mono }}>{fmt(selectedItem.delegated)}</div>
                      <div style={{ fontSize: 8.5, color: 'var(--dag-text-faint)', marginTop: 2 }}>UDAG</div>
                    </div>
                  )}
                  {!isPocket && mainBal && (
                    <div style={{ ...S.stat, border: '1px solid var(--dag-border)' }}>
                      <div style={{ display: 'flex', alignItems: 'center', gap: 4, marginBottom: 4 }}>
                        <span style={{ fontSize: 11, color: 'var(--dag-text-muted)' }}>#</span>
                        <span style={{ fontSize: 8.5, color: 'var(--dag-text-faint)', letterSpacing: 1, fontWeight: 600 }}>NONCE</span>
                      </div>
                      <div style={{ fontSize: 17, fontWeight: 700, color: 'var(--dag-text)', ...S.mono }}>{mainBal.nonce ?? 0}</div>
                    </div>
                  )}
                </div>

                {/* Quick actions — adapted per type */}
                <div style={{ display: 'flex', gap: 8, marginBottom: 14, flexWrap: 'wrap' }}>
                  {isPocket ? (
                    <>
                      {/* Copy @name.pocket for sharing */}
                      {pocketFullName && (
                        <CopyButton text={pocketFullName} label={`Copy ${pocketFullName}`} />
                      )}
                      <Link to={`/wallet/send?to=${encodeURIComponent(pocketFullName || fullAddr(selectedItem.address))}`} className="action-btn" style={{ ...S.btn(), textDecoration: 'none' }}>
                        ⇄ Send to this pocket
                      </Link>
                    </>
                  ) : (
                    <>
                      <Link to="/wallet/send" className="action-btn" style={{ ...S.btn(), textDecoration: 'none' }}>
                        ⇄ Send
                      </Link>
                      <Link to="/staking" className="action-btn" style={{ ...S.btn(), textDecoration: 'none' }}>
                        ⬡ Stake
                      </Link>
                      <Link to="/smart-account" className="action-btn" style={{ ...S.btn(), textDecoration: 'none' }}>
                        ◎ Security
                      </Link>
                    </>
                  )}
                </div>

                {/* Address actions — copy + type-specific management */}
                <div style={{ display: 'flex', alignItems: 'center', gap: 8, paddingTop: 12, borderTop: '1px solid var(--dag-table-border)', flexWrap: 'wrap' }}>
                  <CopyButton text={fullAddr(selectedItem.address)} label="Copy Bech32m" />

                  {/* Delete Pocket (RemovePocket SmartOp) — only for pockets */}
                  {isPocket && pw && (
                    <button
                      onClick={async (e) => {
                        e.stopPropagation();
                        if (removeConfirm !== sel) {
                          setRemoveConfirm(sel);
                          setTimeout(() => setRemoveConfirm(null), 3000);
                          return;
                        }
                        setRemoveConfirm(null);
                        try {
                          const balRes = await fetch(`${getNodeUrl()}/balance/${pw.address}`, { signal: AbortSignal.timeout(5000) });
                          const balData = await balRes.json();
                          await signAndSubmitSmartOp({ RemovePocket: { label: selectedItem.label } }, 0, balData.nonce ?? 0);
                          setSel(null);
                          await new Promise(r => setTimeout(r, 4000));
                          await fetchPockets();
                        } catch { /* ignore */ }
                      }}
                      style={{
                        ...S.btn(removeConfirm === sel ? '#ff3333' : '#EF4444'),
                        background: removeConfirm === sel ? 'rgba(239,68,68,0.12)' : '#EF444410',
                      }}
                    >
                      {removeConfirm === sel ? 'Confirm Delete?' : 'Delete Pocket'}
                    </button>
                  )}

                  {/* Remove from keystore — only for legacy (non-passkey, non-pocket) wallets */}
                  {!isPocket && !isPk && sel !== null && sel < wallets.length && (
                    <button
                      onClick={e => { e.stopPropagation(); handleRemove(sel); }}
                      style={{
                        ...S.btn(removeConfirm === sel ? '#ff3333' : '#EF4444'),
                        background: removeConfirm === sel ? 'rgba(239,68,68,0.12)' : '#EF444410',
                      }}
                    >
                      {removeConfirm === sel ? 'Confirm Remove?' : 'Remove'}
                    </button>
                  )}
                </div>

                {/* Advanced — always available */}
                <div style={{ marginTop: 14 }}>
                  <button onClick={() => setShowAdvanced(!showAdvanced)} style={{ background: 'none', border: 'none', color: 'var(--dag-text-faint)', fontSize: 10, cursor: 'pointer', display: 'flex', alignItems: 'center', gap: 4, padding: 0 }}>
                    <span style={{ transform: showAdvanced ? 'rotate(90deg)' : 'rotate(0deg)', transition: 'transform 0.2s', display: 'inline-block' }}>▶</span>
                    Advanced Details
                  </button>
                  {showAdvanced && (
                    <div style={{ marginTop: 10, padding: '12px 14px', background: 'var(--dag-input-bg)', border: '1px solid var(--dag-border)', borderRadius: 10, display: 'flex', flexDirection: 'column', gap: 10 }}>
                      {[
                        { label: 'Full Bech32m Address', value: fullAddr(selectedItem.address) },
                        { label: 'Hex Address', value: selectedItem.address },
                        ...(pocketFullName ? [{ label: 'Pocket Name', value: pocketFullName }] : []),
                      ].map((field, i) => (
                        <div key={i}>
                          <div style={{ fontSize: 9, color: 'var(--dag-text-faint)', marginBottom: 3, fontWeight: 600, letterSpacing: 0.5 }}>
                            {field.label}
                          </div>
                          <div style={{ fontSize: 10, color: 'var(--dag-subheading)', ...S.mono, wordBreak: 'break-all', padding: '6px 8px', background: 'var(--dag-card)', borderRadius: 6, border: '1px solid var(--dag-border)', userSelect: 'all' }}>
                            {field.value}
                          </div>
                        </div>
                      ))}
                    </div>
                  )}
                </div>
              </div>
              );
            })() : (
              <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center', minHeight: 280, gap: 10 }}>
                <div style={{ width: 56, height: 56, borderRadius: 14, background: 'linear-gradient(135deg, rgba(0,224,196,0.04), rgba(0,102,255,0.04))', border: '1px solid rgba(0,224,196,0.06)', display: 'flex', alignItems: 'center', justifyContent: 'center', fontSize: 22, opacity: 0.4 }}>
                  ◇
                </div>
                <p style={{ fontSize: 12, color: 'var(--dag-text-faint)' }}>Select an item to view details</p>
                <p style={{ fontSize: 10, color: 'var(--dag-text-faint)', opacity: 0.6 }}>Click any wallet or pocket on the left</p>
              </div>
            )}
          </div>
        </div>
      )}

      {/* Account settings — collapsible */}
      <div style={{ marginTop: 24, textAlign: 'center' }}>
        <button
          onClick={() => setShowSettings(!showSettings)}
          style={{ background: 'none', border: 'none', color: 'var(--dag-text-faint)', fontSize: 10.5, cursor: 'pointer', display: 'inline-flex', alignItems: 'center', gap: 4, padding: '4px 8px' }}
        >
          <span style={{ transform: showSettings ? 'rotate(90deg)' : 'rotate(0deg)', transition: 'transform 0.2s', display: 'inline-block', fontSize: 8 }}>▶</span>
          Account Settings
        </button>
        {showSettings && (
          <div style={{ marginTop: 12, paddingTop: 12, borderTop: '1px solid var(--dag-border)', animation: 'slideUp 0.2s ease' }}>
            <div style={{ display: 'flex', gap: 8, justifyContent: 'center', flexWrap: 'wrap', marginBottom: 12 }}>
              {webauthnAvailable && onEnrollWebAuthn && onRemoveWebAuthn && (
                <button
                  onClick={async () => { webauthnEnrolled ? onRemoveWebAuthn() : await onEnrollWebAuthn?.(); }}
                  style={{ ...S.btn(), fontSize: 10.5, padding: '6px 12px' }}
                >
                  ◎ {webauthnEnrolled ? 'Biometrics On' : 'Enable Biometrics'}
                </button>
              )}
              <button onClick={() => setShowPwModal(true)} style={{ ...S.btn(), fontSize: 10.5, padding: '6px 12px' }}>
                ⚿ Change Password
              </button>
              <button onClick={handleExport} style={{ ...S.btn(), fontSize: 10.5, padding: '6px 12px' }}>
                ↓ Export Keystore
              </button>
              <button
                onClick={() => setShowAddModal(true)}
                style={{ ...S.btn(), fontSize: 10.5, padding: '6px 12px' }}
              >
                ↑ Import Legacy Wallet
              </button>
            </div>
            <p style={{ fontSize: 9, color: 'var(--dag-text-faint)', opacity: 0.5 }}>
              Imported wallets are device-local only. Use pockets for cross-device sub-accounts.
            </p>
          </div>
        )}
      </div>

      <AddWalletModal
        open={showAddModal}
        onClose={() => setShowAddModal(false)}
        onGenerate={onGenerateKeypair}
        onAdd={onAddWallet}
      />
      <ChangePwModal open={showPwModal} onClose={() => setShowPwModal(false)} />
    </div>
  );
}
