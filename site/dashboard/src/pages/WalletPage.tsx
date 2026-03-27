import { useState } from 'react';
import { Link } from 'react-router-dom';
import { formatUdag, shortAddr, fullAddr } from '../lib/api';
import { getPasskeyWallet } from '../lib/passkey-wallet';
import { CreateKeystoreModal } from '../components/wallet/CreateKeystoreModal';
import { AddWalletModal } from '../components/wallet/AddWalletModal';
import { changePassword } from '../lib/keystore';
import { CopyButton } from '../components/shared/CopyButton';
import type { Wallet } from '../lib/keystore';
import type { WalletBalance } from '../hooks/useWalletBalances';

const SATS = 100_000_000;
const fmt = (v: number) => (v / SATS).toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 4 });

const S = {
  card: { background: 'rgba(255,255,255,0.018)', border: '1px solid rgba(255,255,255,0.055)', borderRadius: 14, padding: '18px 20px' } as React.CSSProperties,
  stat: { background: 'rgba(255,255,255,0.025)', borderRadius: 10, padding: '12px 14px' } as React.CSSProperties,
  input: { width: '100%', padding: '10px 14px', borderRadius: 10, background: 'rgba(255,255,255,0.03)', border: '1px solid rgba(255,255,255,0.055)', color: '#fff', fontSize: 13, outline: 'none', fontFamily: "'DM Sans',sans-serif" } as React.CSSProperties,
  btn: (c = '#00E0C4') => ({ padding: '7px 14px', borderRadius: 8, background: `${c}10`, border: `1px solid ${c}20`, color: c, fontSize: 11, fontWeight: 600 as const, cursor: 'pointer', transition: 'all 0.2s', display: 'inline-flex' as const, alignItems: 'center' as const, gap: 5 }),
  btnSolid: { padding: '9px 18px', borderRadius: 10, background: '#00E0C4', color: '#080C14', fontSize: 12, fontWeight: 700 as const, cursor: 'pointer', border: 'none' },
  mono: { fontFamily: "'DM Mono',monospace" },
};

function Modal({ open, title, onClose, children }: { open: boolean; title: string; onClose: () => void; children: React.ReactNode }) {
  if (!open) return null;
  return (
    <div style={{ position: 'fixed', inset: 0, zIndex: 50, display: 'flex', alignItems: 'center', justifyContent: 'center', background: 'rgba(0,0,0,0.65)', backdropFilter: 'blur(6px)' }}>
      <div style={{ ...S.card, maxWidth: 440, width: '100%', boxShadow: '0 24px 60px rgba(0,0,0,0.6)' }}>
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 18 }}>
          <h2 style={{ fontSize: 15, fontWeight: 700, color: '#fff' }}>{title}</h2>
          <button onClick={onClose} style={{ background: 'none', border: 'none', color: 'rgba(255,255,255,0.25)', cursor: 'pointer', fontSize: 16 }}>✕</button>
        </div>
        {children}
      </div>
    </div>
  );
}

function ChangePwModal({ open, onClose }: { open: boolean; onClose: () => void }) {
  const [cur, setCur] = useState(''); const [np, setNp] = useState(''); const [cp, setCp] = useState('');
  const [err, setErr] = useState(''); const [ok, setOk] = useState(false); const [ld, setLd] = useState(false);
  const close = () => { setCur(''); setNp(''); setCp(''); setErr(''); setOk(false); onClose(); };
  return (
    <Modal open={open} title="Change Password" onClose={close}>
      <form onSubmit={async e => { e.preventDefault(); setErr(''); setOk(false); if (np.length < 8) { setErr('Min 8 chars.'); return; } if (np !== cp) { setErr("Don't match."); return; } setLd(true); try { (await changePassword(cur, np)) ? setOk(true) : setErr('Wrong current password.'); } catch { setErr('Failed.'); } finally { setLd(false); } }}
        style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
        {[{ l: 'Current', v: cur, s: setCur }, { l: 'New', v: np, s: setNp }, { l: 'Confirm', v: cp, s: setCp }].map((f, i) => (
          <div key={i}><div style={{ fontSize: 10, color: 'rgba(255,255,255,0.28)', marginBottom: 3 }}>{f.l}</div><input type="password" style={S.input} value={f.v} onChange={e => f.s(e.target.value)} required /></div>
        ))}
        {err && <div style={{ fontSize: 10.5, color: '#EF4444', background: 'rgba(239,68,68,0.06)', border: '1px solid rgba(239,68,68,0.15)', borderRadius: 8, padding: '7px 10px' }}>{err}</div>}
        {ok && <div style={{ fontSize: 10.5, color: '#00E0C4', background: 'rgba(0,224,196,0.06)', border: '1px solid rgba(0,224,196,0.15)', borderRadius: 8, padding: '7px 10px' }}>Changed!</div>}
        <div style={{ display: 'flex', justifyContent: 'flex-end', gap: 8, marginTop: 4 }}>
          <button type="button" onClick={close} style={S.btn('rgba(255,255,255,0.3)')}>Cancel</button>
          <button type="submit" disabled={ld} style={S.btnSolid}>{ld ? '...' : 'Change'}</button>
        </div>
      </form>
    </Modal>
  );
}

interface WalletPageProps {
  unlocked: boolean; hasStore: boolean; wallets: Wallet[]; balances: Map<string, WalletBalance>;
  onCreate: (p: string) => Promise<void>; onUnlock: (p: string) => Promise<boolean>;
  onImportBlob: (j: string) => boolean; onAddWallet: (n: string, s: string, a: string) => Promise<void>;
  onRemoveWallet: (i: number) => Promise<void>; onExportBlob: () => string | null;
  onGenerateKeypair: () => Promise<{ secret_key: string; address: string } | null>;
  webauthnAvailable?: boolean; webauthnEnrolled?: boolean;
  onEnrollWebAuthn?: () => Promise<boolean>; onRemoveWebAuthn?: () => void;
  notificationsSupported?: boolean; notificationsEnabled?: boolean;
  onToggleNotifications?: () => Promise<void>;
}

export function WalletPage({
  unlocked, hasStore, wallets, balances, onCreate, onUnlock, onImportBlob,
  onAddWallet, onRemoveWallet, onExportBlob, onGenerateKeypair,
  webauthnAvailable, webauthnEnrolled, onEnrollWebAuthn, onRemoveWebAuthn,
}: WalletPageProps) {
  const [showKsModal, setShowKsModal] = useState(false);
  const [showAddModal, setShowAddModal] = useState(false);
  const [showPwModal, setShowPwModal] = useState(false);
  const [sel, setSel] = useState<number | null>(null);
  const pw = getPasskeyWallet();

  if (!unlocked) {
    return (
      <div style={{ padding: '18px 26px', fontFamily: "'DM Sans',sans-serif" }}>
        <style>{`@keyframes slideUp{from{opacity:0;transform:translateY(10px)}to{opacity:1;transform:translateY(0)}}`}</style>
        <h1 style={{ fontSize: 21, fontWeight: 700, color: '#fff', marginBottom: 4, animation: 'slideUp 0.3s ease' }}>Wallet</h1>
        <p style={{ fontSize: 11.5, color: 'rgba(255,255,255,0.25)' }}>Manage your UltraDAG wallets</p>
        <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', padding: '70px 0', gap: 18 }}>
          <div style={{ width: 72, height: 72, borderRadius: 18, background: 'linear-gradient(135deg,rgba(0,224,196,0.06),rgba(0,102,255,0.06))', border: '1px solid rgba(0,224,196,0.1)', display: 'flex', alignItems: 'center', justifyContent: 'center', fontSize: 28 }}>◇</div>
          <div style={{ textAlign: 'center' }}>
            <div style={{ fontSize: 16, fontWeight: 600, color: '#fff' }}>{hasStore ? 'Wallet Locked' : 'No Wallet Yet'}</div>
            <div style={{ fontSize: 11.5, color: 'rgba(255,255,255,0.25)', marginTop: 4, maxWidth: 280 }}>{hasStore ? 'Enter your password to access your wallets.' : 'Create a new wallet or import an existing one.'}</div>
          </div>
          <button onClick={() => setShowKsModal(true)} style={S.btnSolid}>{hasStore ? 'Unlock' : 'Get Started'}</button>
        </div>
        <CreateKeystoreModal open={showKsModal} onClose={() => setShowKsModal(false)}
          onCreateOrUnlock={async pw => { if (hasStore) return onUnlock(pw); await onCreate(pw); return true; }}
          onCreateWithKey={async (pw, n, s, a) => { await onCreate(pw); await onAddWallet(n, s, a); return true; }}
          onImport={onImportBlob} hasExisting={hasStore} />
      </div>
    );
  }

  const handleExport = () => { const j = onExportBlob(); if (j) { const b = new Blob([j], { type: 'application/json' }); const u = URL.createObjectURL(b); const a = document.createElement('a'); a.href = u; a.download = 'ultradag-keystore.json'; a.click(); URL.revokeObjectURL(u); } };
  const selected = sel !== null ? wallets[sel] : null;
  const selBal = selected ? balances.get(selected.address) : null;

  // Totals
  let totalBal = 0, totalStaked = 0, totalDelegated = 0;
  for (const w of wallets) { const b = balances.get(w.address); if (b) { totalBal += b.balance; totalStaked += b.staked; totalDelegated += b.delegated; } }

  return (
    <div style={{ padding: '18px 26px', fontFamily: "'DM Sans',sans-serif" }}>
      <style>{`@keyframes slideUp{from{opacity:0;transform:translateY(10px)}to{opacity:1;transform:translateY(0)}}`}</style>

      {/* Header */}
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 18, animation: 'slideUp 0.3s ease' }}>
        <div>
          <h1 style={{ fontSize: 21, fontWeight: 700, color: '#fff' }}>Wallets</h1>
          <p style={{ fontSize: 11.5, color: 'rgba(255,255,255,0.25)', marginTop: 2 }}>{wallets.length} wallet{wallets.length !== 1 ? 's' : ''} managed</p>
        </div>
        <div style={{ display: 'flex', gap: 7 }}>
          {webauthnAvailable && onEnrollWebAuthn && onRemoveWebAuthn && (
            <button onClick={async () => { webauthnEnrolled ? onRemoveWebAuthn() : await onEnrollWebAuthn?.(); }} style={S.btn(webauthnEnrolled ? '#00E0C4' : 'rgba(255,255,255,0.3)')}>
              ◎ {webauthnEnrolled ? 'Biometrics On' : 'Biometrics'}
            </button>
          )}
          <button onClick={() => setShowPwModal(true)} style={S.btn('rgba(255,255,255,0.3)')}>⚿ Password</button>
          <button onClick={handleExport} style={S.btn('rgba(255,255,255,0.3)')}>↓ Export</button>
          <button onClick={() => setShowAddModal(true)} style={S.btnSolid}>+ Add Wallet</button>
        </div>
      </div>

      {/* Portfolio Summary */}
      <div style={{
        background: 'linear-gradient(135deg, rgba(0,224,196,0.03), rgba(0,102,255,0.03))',
        border: '1px solid rgba(0,224,196,0.08)', borderRadius: 16, padding: '18px 24px', marginBottom: 16,
        animation: 'slideUp 0.4s ease',
      }}>
        <div style={{ fontSize: 10, fontWeight: 600, color: 'rgba(255,255,255,0.25)', letterSpacing: 1.8, marginBottom: 10 }}>TOTAL PORTFOLIO</div>
        <div style={{ display: 'grid', gridTemplateColumns: 'repeat(4,1fr)', gap: 16 }}>
          {[
            { l: 'Total', v: totalBal + totalStaked + totalDelegated, c: '#fff' },
            { l: 'Available', v: totalBal, c: '#00E0C4' },
            { l: 'Staked', v: totalStaked, c: '#0066FF' },
            { l: 'Delegated', v: totalDelegated, c: '#A855F7' },
          ].map((p, i) => (
            <div key={i}>
              <div style={{ fontSize: 10, color: 'rgba(255,255,255,0.25)', marginBottom: 3 }}>{p.l}</div>
              <div style={{ fontSize: 21, fontWeight: 700, color: p.c, ...S.mono }}>{fmt(p.v)}</div>
              <div style={{ fontSize: 9.5, color: 'rgba(255,255,255,0.15)' }}>UDAG</div>
            </div>
          ))}
        </div>
      </div>

      {wallets.length === 0 ? (
        <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', padding: '50px 0', gap: 14 }}>
          <span style={{ fontSize: 36, opacity: 0.1 }}>◇</span>
          <p style={{ fontSize: 12, color: 'rgba(255,255,255,0.25)' }}>No wallets yet. Add one to get started.</p>
          <button onClick={() => setShowAddModal(true)} style={S.btnSolid}>Add Wallet</button>
        </div>
      ) : (
        <div style={{ display: 'grid', gridTemplateColumns: '1.2fr 1fr', gap: 14, animation: 'slideUp 0.5s ease' }}>
          {/* Wallet List */}
          <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
            {wallets.map((w, i) => {
              const bal = balances.get(w.address);
              const isPk = pw?.address === w.address;
              const active = sel === i;
              const totalVal = (bal?.balance ?? 0) + (bal?.staked ?? 0) + (bal?.delegated ?? 0);
              return (
                <div key={w.address} onClick={() => setSel(active ? null : i)} style={{
                  ...S.card, cursor: 'pointer', transition: 'all 0.25s',
                  borderColor: active ? 'rgba(0,224,196,0.25)' : 'rgba(255,255,255,0.04)',
                  background: active ? 'rgba(0,224,196,0.025)' : 'rgba(255,255,255,0.012)',
                  transform: active ? 'scale(1.005)' : 'none',
                }}>
                  <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                    <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
                      <div style={{
                        width: 40, height: 40, borderRadius: 11, display: 'flex', alignItems: 'center', justifyContent: 'center',
                        background: isPk ? 'linear-gradient(135deg,#00E0C4,#0066FF)' : `hsl(${i * 60}, 40%, 20%)`,
                        fontSize: 15, fontWeight: 800, color: '#fff',
                        boxShadow: active ? '0 0 12px rgba(0,224,196,0.15)' : 'none',
                      }}>{w.name[0]?.toUpperCase() || '?'}</div>
                      <div>
                        <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
                          <span style={{ fontSize: 13, fontWeight: 600, color: '#fff' }}>{w.name}</span>
                          {isPk && <span style={{ fontSize: 8, background: 'rgba(0,224,196,0.1)', color: '#00E0C4', padding: '1px 5px', borderRadius: 3, fontWeight: 700, letterSpacing: 0.6 }}>PASSKEY</span>}
                        </div>
                        <div style={{ fontSize: 10, color: 'rgba(255,255,255,0.18)', ...S.mono, marginTop: 2 }}>{shortAddr(w.address)}</div>
                      </div>
                    </div>
                    <div style={{ textAlign: 'right' }}>
                      <div style={{ fontSize: 16, fontWeight: 700, color: '#fff', ...S.mono }}>{fmt(totalVal)}</div>
                      <div style={{ fontSize: 9, color: 'rgba(255,255,255,0.18)' }}>UDAG</div>
                    </div>
                  </div>
                  {/* Mini balance bar */}
                  {totalVal > 0 && (
                    <div style={{ display: 'flex', borderRadius: 3, overflow: 'hidden', height: 3, marginTop: 10, background: 'rgba(255,255,255,0.02)' }}>
                      {(bal?.balance ?? 0) > 0 && <div style={{ width: `${((bal?.balance ?? 0) / totalVal) * 100}%`, background: '#00E0C4' }} />}
                      {(bal?.staked ?? 0) > 0 && <div style={{ width: `${((bal?.staked ?? 0) / totalVal) * 100}%`, background: '#0066FF' }} />}
                      {(bal?.delegated ?? 0) > 0 && <div style={{ width: `${((bal?.delegated ?? 0) / totalVal) * 100}%`, background: '#A855F7' }} />}
                    </div>
                  )}
                </div>
              );
            })}
          </div>

          {/* Detail Panel */}
          <div style={S.card}>
            {selected ? (
              <div>
                {/* Header */}
                <div style={{ display: 'flex', alignItems: 'center', gap: 12, marginBottom: 18 }}>
                  <div style={{
                    width: 48, height: 48, borderRadius: 14, display: 'flex', alignItems: 'center', justifyContent: 'center',
                    background: pw?.address === selected.address ? 'linear-gradient(135deg,#00E0C4,#0066FF)' : 'rgba(255,255,255,0.04)',
                    fontSize: 20, fontWeight: 800, color: '#fff',
                  }}>{selected.name[0]?.toUpperCase()}</div>
                  <div>
                    <div style={{ fontSize: 16, fontWeight: 700, color: '#fff' }}>{selected.name}</div>
                    <div style={{ fontSize: 10, color: 'rgba(255,255,255,0.2)', ...S.mono, marginTop: 1 }}>{fullAddr(selected.address)}</div>
                  </div>
                </div>

                {/* Stats grid */}
                <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 8, marginBottom: 14 }}>
                  {[
                    { l: 'AVAILABLE', v: fmt(selBal?.balance ?? 0), c: '#00E0C4', i: '◎' },
                    { l: 'STAKED', v: fmt(selBal?.staked ?? 0), c: '#0066FF', i: '⬡' },
                    { l: 'DELEGATED', v: fmt(selBal?.delegated ?? 0), c: '#A855F7', i: '◈' },
                    { l: 'NONCE', v: String(selBal?.nonce ?? 0), c: '#fff', i: '#' },
                  ].map((x, i) => (
                    <div key={i} style={S.stat}>
                      <div style={{ display: 'flex', alignItems: 'center', gap: 4, marginBottom: 3 }}>
                        <span style={{ fontSize: 10, color: x.c }}>{x.i}</span>
                        <span style={{ fontSize: 8.5, color: 'rgba(255,255,255,0.22)', letterSpacing: 1 }}>{x.l}</span>
                      </div>
                      <div style={{ fontSize: 17, fontWeight: 700, color: x.c, ...S.mono }}>{x.v}</div>
                    </div>
                  ))}
                </div>

                {/* Quick actions */}
                <div style={{ display: 'flex', gap: 8, marginBottom: 14 }}>
                  <Link to="/wallet/send" style={{ ...S.btn(), textDecoration: 'none' }}>⇄ Send</Link>
                  <Link to="/staking" style={{ ...S.btn('#0066FF'), textDecoration: 'none' }}>⬡ Stake</Link>
                  <Link to="/smart-account" style={{ ...S.btn('#A855F7'), textDecoration: 'none' }}>◎ SmartAccount</Link>
                </div>

                {/* Address copy + remove */}
                <div style={{ display: 'flex', alignItems: 'center', gap: 8, paddingTop: 12, borderTop: '1px solid rgba(255,255,255,0.03)' }}>
                  <CopyButton text={selected.address} />
                  <CopyButton text={fullAddr(selected.address)} />
                  {sel !== null && (
                    <button onClick={() => { onRemoveWallet(sel); setSel(null); }} style={S.btn('#EF4444')}>Remove</button>
                  )}
                </div>
              </div>
            ) : (
              <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center', minHeight: 260, gap: 10 }}>
                <span style={{ fontSize: 32, opacity: 0.08 }}>◇</span>
                <p style={{ fontSize: 12, color: 'rgba(255,255,255,0.18)' }}>Select a wallet to view details</p>
                <p style={{ fontSize: 10, color: 'rgba(255,255,255,0.1)' }}>Click any wallet on the left</p>
              </div>
            )}
          </div>
        </div>
      )}

      <AddWalletModal open={showAddModal} onClose={() => setShowAddModal(false)} onGenerate={onGenerateKeypair} onAdd={onAddWallet} />
      <ChangePwModal open={showPwModal} onClose={() => setShowPwModal(false)} />
    </div>
  );
}
