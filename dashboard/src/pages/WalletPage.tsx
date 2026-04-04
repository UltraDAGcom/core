import { useState, useEffect } from 'react';
import { Link } from 'react-router-dom';
import { fullAddr } from '../lib/api';
import { DisplayIdentity } from '../components/shared/DisplayIdentity';
import { getPasskeyWallet } from '../lib/passkey-wallet';
import { CreateKeystoreModal } from '../components/wallet/CreateKeystoreModal';
import { AddWalletModal } from '../components/wallet/AddWalletModal';
import { changePassword } from '../lib/keystore';
import { CopyButton } from '../components/shared/CopyButton';
import { Pagination } from '../components/shared/Pagination';
import { useIsMobile } from '../hooks/useIsMobile';
import { PageHeader } from '../components/shared/PageHeader';
import type { Wallet } from '../lib/keystore';
import type { WalletBalance } from '../hooks/useWalletBalances';

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
}: WalletPageProps) {
  const [showKsModal, setShowKsModal] = useState(false);
  const [showAddModal, setShowAddModal] = useState(false);
  const [showPwModal, setShowPwModal] = useState(false);
  const [sel, setSel] = useState<number | null>(null);
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [walletPage, setWalletPage] = useState(1);
  const [removeConfirm, setRemoveConfirm] = useState<number | null>(null);
  const pw = getPasskeyWallet();
  const m = useIsMobile();
  const WALLET_PAGE_SIZE = 10;

  useEffect(() => {
    setWalletPage(1);
  }, [wallets.length]);

  // Reset selection if wallet is removed
  useEffect(() => {
    if (sel !== null && sel >= wallets.length) {
      setSel(null);
    }
  }, [wallets.length, sel]);

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
              borderRadius: 20,
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
                background: 'linear-gradient(135deg, #00E0C4, #0066FF)',
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

  const selected = sel !== null ? wallets[sel] : null;
  const selBal = selected ? balances.get(selected.address) : null;

  // Totals
  let totalBal = 0;
  let totalStaked = 0;
  let totalDelegated = 0;
  for (const w of wallets) {
    const b = balances.get(w.address);
    if (b) {
      totalBal += b.balance;
      totalStaked += b.staked;
      totalDelegated += b.delegated;
    }
  }
  const grandTotal = totalBal + totalStaked + totalDelegated;

  return (
    <div style={{ padding: m ? '12px 14px' : '18px 26px', fontFamily: "'DM Sans',sans-serif" }}>
      <style>{CSS}</style>

      {/* ── Header ── */}
      <PageHeader
        title="Wallets"
        subtitle={`${wallets.length} wallet${wallets.length !== 1 ? 's' : ''} managed${grandTotal > 0 ? ` · ${fmt(grandTotal)} UDAG total` : ''}`}
        right={
          <div style={{ display: 'flex', gap: 7, flexWrap: 'wrap' }}>
            {webauthnAvailable && onEnrollWebAuthn && onRemoveWebAuthn && (
              <button
                className="header-btn"
                onClick={async () => {
                  webauthnEnrolled ? onRemoveWebAuthn() : await onEnrollWebAuthn?.();
                }}
                style={S.btn(webauthnEnrolled ? '#00E0C4' : 'var(--dag-text-muted)')}
              >
                ◎ {webauthnEnrolled ? 'Biometrics On' : 'Biometrics'}
              </button>
            )}
            <button className="header-btn" onClick={() => setShowPwModal(true)} style={S.btn('var(--dag-text-muted)')}>
              ⚿ Password
            </button>
            <button className="header-btn" onClick={handleExport} style={S.btn('var(--dag-text-muted)')}>
              ↓ Export
            </button>
            <button
              className="header-btn"
              onClick={() => setShowAddModal(true)}
              style={{ ...S.btnSolid, boxShadow: '0 2px 12px rgba(0,224,196,0.12)' }}
            >
              + Add Wallet
            </button>
          </div>
        }
      />

      {/* ── Portfolio Summary ── */}
      <div
        style={{
          display: 'grid',
          gridTemplateColumns: m ? 'repeat(2,1fr)' : 'repeat(4,1fr)',
          gap: m ? 10 : 12,
          marginBottom: 18,
          animation: 'slideUp 0.4s ease',
        }}
      >
        {[
          { l: 'TOTAL', v: grandTotal, c: '#fff', i: '◈' },
          { l: 'AVAILABLE', v: totalBal, c: '#00E0C4', i: '◎' },
          { l: 'STAKED', v: totalStaked, c: '#0066FF', i: '⬡' },
          { l: 'DELEGATED', v: totalDelegated, c: '#A855F7', i: '◇' },
        ].map((p, i) => (
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
          <div style={{ fontSize: 40, opacity: 0.1 }}>◇</div>
          <p style={{ fontSize: 13, color: 'var(--dag-text-muted)' }}>No wallets yet</p>
          <p style={{ fontSize: 11, color: 'var(--dag-text-faint)' }}>
            Add a wallet to manage your UDAG holdings
          </p>
          <button
            onClick={() => setShowAddModal(true)}
            style={{ ...S.btnSolid, boxShadow: '0 4px 20px rgba(0,224,196,0.15)' }}
          >
            + Add Wallet
          </button>
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
          {/* ── Wallet List ── */}
          <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
            {wallets
              .slice(
                (walletPage - 1) * WALLET_PAGE_SIZE,
                walletPage * WALLET_PAGE_SIZE,
              )
              .map((w, _pi) => {
                const i = (walletPage - 1) * WALLET_PAGE_SIZE + _pi;
                const bal = balances.get(w.address);
                const isPk = pw?.address === w.address;
                const active = sel === i;
                const totalVal =
                  (bal?.balance ?? 0) + (bal?.staked ?? 0) + (bal?.delegated ?? 0);

                return (
                  <div
                    key={w.address}
                    className="wallet-card"
                    onClick={() => setSel(active ? null : i)}
                    style={{
                      ...S.card,
                      cursor: 'pointer',
                      borderColor: active
                        ? 'rgba(0,224,196,0.25)'
                        : 'var(--dag-border)',
                      background: active
                        ? 'rgba(0,224,196,0.025)'
                        : 'var(--dag-card)',
                      transform: active ? 'scale(1.005)' : 'none',
                      boxShadow: active ? '0 0 20px rgba(0,224,196,0.04)' : 'none',
                    }}
                  >
                    <div
                      style={{
                        display: 'flex',
                        justifyContent: 'space-between',
                        alignItems: 'center',
                      }}
                    >
                      <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
                        <div
                          style={{
                            width: 40,
                            height: 40,
                            borderRadius: 11,
                            display: 'flex',
                            alignItems: 'center',
                            justifyContent: 'center',
                            background: isPk
                              ? 'linear-gradient(135deg, #00E0C4, #0066FF)'
                              : `hsl(${i * 60 + 180}, 35%, 18%)`,
                            fontSize: 15,
                            fontWeight: 800,
                            color: isPk ? '#080C14' : 'var(--dag-text)',
                            boxShadow: active
                              ? '0 0 12px rgba(0,224,196,0.15)'
                              : 'none',
                            transition: 'box-shadow 0.25s',
                          }}
                        >
                          {w.name[0]?.toUpperCase() || '?'}
                        </div>
                        <div>
                          <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
                            <span
                              style={{
                                fontSize: 13,
                                fontWeight: 600,
                                color: 'var(--dag-text)',
                              }}
                            >
                              {w.name}
                            </span>
                            {isPk && (
                              <span
                                style={{
                                  fontSize: 8,
                                  background: 'rgba(0,224,196,0.1)',
                                  color: '#00E0C4',
                                  padding: '1px 6px',
                                  borderRadius: 3,
                                  fontWeight: 700,
                                  letterSpacing: 0.6,
                                }}
                              >
                                PASSKEY
                              </span>
                            )}
                          </div>
                          <div
                            style={{
                              marginTop: 2,
                            }}
                          >
                            <DisplayIdentity address={w.address} size="xs" />
                          </div>
                        </div>
                      </div>

                      <div style={{ textAlign: 'right' }}>
                        <div
                          style={{
                            fontSize: 16,
                            fontWeight: 700,
                            color: active ? '#00E0C4' : 'var(--dag-text)',
                            ...S.mono,
                            transition: 'color 0.25s',
                          }}
                        >
                          {fmt(totalVal)}
                        </div>
                        <div style={{ fontSize: 9, color: 'var(--dag-text-faint)' }}>
                          UDAG
                        </div>
                      </div>
                    </div>

                    <BalanceBar
                      balance={bal?.balance ?? 0}
                      staked={bal?.staked ?? 0}
                      delegated={bal?.delegated ?? 0}
                      total={totalVal}
                    />
                  </div>
                );
              })}

            <Pagination
              page={walletPage}
              totalPages={Math.ceil(wallets.length / WALLET_PAGE_SIZE)}
              onPageChange={setWalletPage}
              totalItems={wallets.length}
              pageSize={WALLET_PAGE_SIZE}
            />
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
            {selected ? (
              <div>
                {/* Detail Header */}
                <div
                  style={{
                    display: 'flex',
                    alignItems: 'center',
                    gap: 12,
                    marginBottom: 18,
                    paddingBottom: 14,
                    borderBottom: '1px solid var(--dag-table-border)',
                  }}
                >
                  <div
                    style={{
                      width: 48,
                      height: 48,
                      borderRadius: 14,
                      display: 'flex',
                      alignItems: 'center',
                      justifyContent: 'center',
                      background:
                        pw?.address === selected.address
                          ? 'linear-gradient(135deg, #00E0C4, #0066FF)'
                          : 'var(--dag-card-hover)',
                      fontSize: 20,
                      fontWeight: 800,
                      color:
                        pw?.address === selected.address
                          ? '#080C14'
                          : 'var(--dag-text)',
                    }}
                  >
                    {selected.name[0]?.toUpperCase()}
                  </div>
                  <div style={{ flex: 1 }}>
                    <div
                      style={{
                        fontSize: 16,
                        fontWeight: 700,
                        color: 'var(--dag-text)',
                      }}
                    >
                      {selected.name}
                    </div>
                    <div
                      style={{
                        marginTop: 2,
                      }}
                    >
                      <DisplayIdentity address={selected.address} size="xs" />
                    </div>
                  </div>
                  {pw?.address === selected.address && (
                    <span
                      style={{
                        fontSize: 8.5,
                        background: 'rgba(0,224,196,0.08)',
                        color: '#00E0C4',
                        padding: '3px 8px',
                        borderRadius: 4,
                        fontWeight: 700,
                        letterSpacing: 0.8,
                      }}
                    >
                      PASSKEY
                    </span>
                  )}
                </div>

                {/* Stats grid */}
                <div
                  style={{
                    display: 'grid',
                    gridTemplateColumns: '1fr 1fr',
                    gap: 8,
                    marginBottom: 14,
                  }}
                >
                  {[
                    {
                      l: 'AVAILABLE',
                      v: fmt(selBal?.balance ?? 0),
                      c: '#00E0C4',
                      i: '◎',
                    },
                    {
                      l: 'STAKED',
                      v: fmt(selBal?.staked ?? 0),
                      c: '#0066FF',
                      i: '⬡',
                    },
                    {
                      l: 'DELEGATED',
                      v: fmt(selBal?.delegated ?? 0),
                      c: '#A855F7',
                      i: '◈',
                    },
                    {
                      l: 'NONCE',
                      v: String(selBal?.nonce ?? 0),
                      c: 'var(--dag-text)',
                      i: '#',
                    },
                  ].map((x, i) => (
                    <div
                      key={i}
                      style={{
                        ...S.stat,
                        border: '1px solid var(--dag-border)',
                        transition: 'border-color 0.2s',
                      }}
                    >
                      <div
                        style={{
                          display: 'flex',
                          alignItems: 'center',
                          gap: 4,
                          marginBottom: 4,
                        }}
                      >
                        <span style={{ fontSize: 11, color: x.c }}>{x.i}</span>
                        <span
                          style={{
                            fontSize: 8.5,
                            color: 'var(--dag-text-faint)',
                            letterSpacing: 1,
                            fontWeight: 600,
                          }}
                        >
                          {x.l}
                        </span>
                      </div>
                      <div
                        style={{
                          fontSize: 17,
                          fontWeight: 700,
                          color: x.c,
                          ...S.mono,
                        }}
                      >
                        {x.v}
                      </div>
                      {x.l !== 'NONCE' && (
                        <div
                          style={{
                            fontSize: 8.5,
                            color: 'var(--dag-text-faint)',
                            marginTop: 2,
                          }}
                        >
                          UDAG
                        </div>
                      )}
                    </div>
                  ))}
                </div>

                {/* Quick actions */}
                <div style={{ display: 'flex', gap: 8, marginBottom: 14, flexWrap: 'wrap' }}>
                  <Link
                    to="/wallet/send"
                    className="action-btn"
                    style={{ ...S.btn(), textDecoration: 'none' }}
                  >
                    ⇄ Send
                  </Link>
                  <Link
                    to="/staking"
                    className="action-btn"
                    style={{ ...S.btn('#0066FF'), textDecoration: 'none' }}
                  >
                    ⬡ Stake
                  </Link>
                  <Link
                    to="/smart-account"
                    className="action-btn"
                    style={{ ...S.btn('#A855F7'), textDecoration: 'none' }}
                  >
                    ◎ SmartAccount
                  </Link>
                </div>

                {/* Copy address + remove */}
                <div
                  style={{
                    display: 'flex',
                    alignItems: 'center',
                    gap: 8,
                    paddingTop: 12,
                    borderTop: '1px solid var(--dag-table-border)',
                  }}
                >
                  <CopyButton text={fullAddr(selected.address)} label="Copy Bech32m Address" />
                  {sel !== null && (
                    <button
                      onClick={e => {
                        e.stopPropagation();
                        handleRemove(sel);
                      }}
                      style={{
                        ...S.btn(removeConfirm === sel ? '#ff3333' : '#EF4444'),
                        background: removeConfirm === sel ? 'rgba(239,68,68,0.12)' : `#EF444410`,
                      }}
                    >
                      {removeConfirm === sel ? 'Confirm Remove?' : 'Remove'}
                    </button>
                  )}
                </div>

                {/* Advanced section */}
                <div style={{ marginTop: 14 }}>
                  <button
                    onClick={() => setShowAdvanced(!showAdvanced)}
                    style={{
                      background: 'none',
                      border: 'none',
                      color: 'var(--dag-text-faint)',
                      fontSize: 10,
                      cursor: 'pointer',
                      display: 'flex',
                      alignItems: 'center',
                      gap: 4,
                      padding: 0,
                    }}
                  >
                    <span
                      style={{
                        transform: showAdvanced ? 'rotate(90deg)' : 'rotate(0deg)',
                        transition: 'transform 0.2s',
                        display: 'inline-block',
                      }}
                    >
                      ▶
                    </span>
                    Advanced Details
                  </button>

                  {showAdvanced && (
                    <div
                      style={{
                        marginTop: 10,
                        padding: '12px 14px',
                        background: 'var(--dag-input-bg)',
                        border: '1px solid var(--dag-border)',
                        borderRadius: 10,
                        display: 'flex',
                        flexDirection: 'column',
                        gap: 10,
                      }}
                    >
                      {[
                        { label: 'Full Bech32m Address', value: fullAddr(selected.address) },
                        { label: 'Hex Address', value: selected.address },
                      ].map((field, i) => (
                        <div key={i}>
                          <div
                            style={{
                              fontSize: 9,
                              color: 'var(--dag-text-faint)',
                              marginBottom: 3,
                              fontWeight: 600,
                              letterSpacing: 0.5,
                            }}
                          >
                            {field.label}
                          </div>
                          <div
                            style={{
                              fontSize: 10,
                              color: 'var(--dag-subheading)',
                              ...S.mono,
                              wordBreak: 'break-all',
                              padding: '6px 8px',
                              background: 'var(--dag-card)',
                              borderRadius: 6,
                              border: '1px solid var(--dag-border)',
                              userSelect: 'all',
                            }}
                          >
                            {field.value}
                          </div>
                        </div>
                      ))}
                    </div>
                  )}
                </div>
              </div>
            ) : (
              <div
                style={{
                  display: 'flex',
                  flexDirection: 'column',
                  alignItems: 'center',
                  justifyContent: 'center',
                  minHeight: 280,
                  gap: 10,
                }}
              >
                <div
                  style={{
                    width: 56,
                    height: 56,
                    borderRadius: 16,
                    background: 'linear-gradient(135deg, rgba(0,224,196,0.04), rgba(0,102,255,0.04))',
                    border: '1px solid rgba(0,224,196,0.06)',
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'center',
                    fontSize: 22,
                    opacity: 0.4,
                  }}
                >
                  ◇
                </div>
                <p style={{ fontSize: 12, color: 'var(--dag-text-faint)' }}>
                  Select a wallet to view details
                </p>
                <p style={{ fontSize: 10, color: 'var(--dag-text-faint)', opacity: 0.6 }}>
                  Click any wallet on the left
                </p>
              </div>
            )}
          </div>
        </div>
      )}

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
