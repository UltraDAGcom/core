import { useState, useEffect, useCallback } from 'react';
import { getNodeUrl } from '../lib/api';
import { getPasskeyWallet } from '../lib/passkey-wallet';
import { VerifiedAddressInput } from '../components/shared/VerifiedAddressInput';

interface AuthorizedKey { key_id: string; key_type: string; label: string; daily_limit: number | null }
interface SmartAccountInfo {
  address: string; created_at_round: number; authorized_keys: AuthorizedKey[];
  has_recovery: boolean; guardian_count: number | null; recovery_threshold: number | null;
  has_pending_recovery: boolean; has_policy: boolean; instant_limit: number | null;
  vault_threshold: number | null; daily_limit: number | null; pending_vault_count: number;
  pending_key_removal: { key_id: string; executes_at_round: number } | null;
}
interface NameInfo { name: string; address: string; expiry_round: number | null }

const SATS = 100_000_000;
const fmt = (s: number) => (s / SATS).toFixed(4);

const S = {
  card: { background: 'var(--dag-card)', border: '1px solid var(--dag-border)', borderRadius: 14, padding: '20px 22px' } as React.CSSProperties,
  label: { fontSize: 10.5, fontWeight: 600, color: 'var(--dag-text-muted)', letterSpacing: 1.2, textTransform: 'uppercase' as const, marginBottom: 6 },
  input: { width: '100%', padding: '10px 14px', borderRadius: 10, background: 'var(--dag-input-bg)', border: '1px solid var(--dag-border)', color: 'var(--dag-text)', fontSize: 13, outline: 'none', fontFamily: "'DM Sans',sans-serif" } as React.CSSProperties,
  btn: (c = '#00E0C4') => ({ padding: '7px 14px', borderRadius: 8, background: `${c}12`, border: `1px solid ${c}25`, color: c, fontSize: 11, fontWeight: 600, cursor: 'pointer', transition: 'all 0.2s' }),
  btnSolid: (c = '#00E0C4') => ({ padding: '8px 16px', borderRadius: 8, background: c, color: '#080C14', fontSize: 11.5, fontWeight: 700, cursor: 'pointer', border: 'none' }),
  mono: { fontFamily: "'DM Mono',monospace" },
  stat: { background: 'var(--dag-card)', borderRadius: 10, padding: '10px 13px' } as React.CSSProperties,
};

function Section({ icon, color, title, action, children }: { icon: string; color: string; title: string; action?: React.ReactNode; children: React.ReactNode }) {
  return (
    <div style={S.card}>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 16 }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          <span style={{ color, fontSize: 16 }}>{icon}</span>
          <span style={{ fontSize: 13.5, fontWeight: 600, color: 'var(--dag-text-secondary)' }}>{title}</span>
        </div>
        {action}
      </div>
      {children}
    </div>
  );
}

export function SmartAccountPage({ walletAddress, nodeUrl }: { walletAddress?: string; nodeUrl: string }) {
  const [info, setInfo] = useState<SmartAccountInfo | null>(null);
  const [nameInfo, setNameInfo] = useState<NameInfo | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');
  const [showNameForm, setShowNameForm] = useState(false);
  const [nameInput, setNameInput] = useState('');
  const [nameAvail, setNameAvail] = useState<boolean | null>(null);
  const [nameChecking, setNameChecking] = useState(false);
  const [nameFee, setNameFee] = useState(0);
  const [nameWarn, setNameWarn] = useState('');
  const [showRecoveryForm, setShowRecoveryForm] = useState(false);
  const [guardians, setGuardians] = useState(['', '', '']);
  const [threshold, setThreshold] = useState(2);
  const [showPolicyForm, setShowPolicyForm] = useState(false);

  const pw = getPasskeyWallet();
  const localName = pw?.name || null;

  const fetchInfo = useCallback(async () => {
    if (!walletAddress) return;
    setLoading(true);
    try {
      const [saRes, nameRes] = await Promise.all([
        fetch(`${nodeUrl}/smart-account/${walletAddress}`, { signal: AbortSignal.timeout(5000) }).catch(() => null),
        fetch(`${nodeUrl}/name/reverse/${walletAddress}`, { signal: AbortSignal.timeout(5000) }).catch(() => null),
      ]);
      if (saRes?.ok) setInfo(await saRes.json()); else setInfo(null);
      if (nameRes?.ok) setNameInfo(await nameRes.json()); else setNameInfo(null);
    } catch { setError('Failed to fetch'); }
    finally { setLoading(false); }
  }, [walletAddress, nodeUrl]);

  useEffect(() => { fetchInfo(); }, [fetchInfo]);

  const checkName = useCallback(async (name: string) => {
    setNameInput(name); setNameAvail(null); setNameWarn('');
    if (name.length < 3) return;
    setNameChecking(true);
    try {
      const res = await fetch(`${nodeUrl}/name/available/${encodeURIComponent(name)}`, { signal: AbortSignal.timeout(5000) });
      if (res.ok) {
        const d = await res.json();
        setNameAvail(d.available);
        setNameFee(d.annual_fee || 0);
        if (d.similar_warning) setNameWarn(d.similar_warning);
        if (!d.valid) setNameWarn('Invalid name format');
        if (!d.available) setNameWarn('Name taken');
      }
    } catch {} finally { setNameChecking(false); }
  }, [nodeUrl]);

  if (!walletAddress) return <div style={{ padding: '18px 26px', color: 'var(--dag-text-muted)', fontSize: 13, fontFamily: "'DM Sans',sans-serif" }}>Create a wallet first.</div>;
  if (loading) return <div style={{ padding: '18px 26px', color: 'var(--dag-text-muted)', fontSize: 13, fontFamily: "'DM Sans',sans-serif", animation: 'pulse 1.5s infinite' }}>Loading SmartAccount...</div>;

  return (
    <div style={{ padding: '18px 26px', fontFamily: "'DM Sans',sans-serif" }}>
      <style>{`@keyframes slideUp{from{opacity:0;transform:translateY(10px)}to{opacity:1;transform:translateY(0)}} @keyframes pulse{0%,100%{opacity:1}50%{opacity:.4}} input:focus,select:focus{border-color:rgba(0,224,196,0.3)!important}`}</style>

      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 22, animation: 'slideUp 0.3s ease' }}>
        <div>
          <h1 style={{ fontSize: 21, fontWeight: 700, color: 'var(--dag-text)' }}>SmartAccount</h1>
          <p style={{ fontSize: 11.5, color: 'var(--dag-subheading)', marginTop: 2 }}>Keys, recovery, spending limits, and name</p>
        </div>
        <button onClick={fetchInfo} style={S.btn()}>↻ Refresh</button>
      </div>

      {error && <div style={{ fontSize: 11, color: '#EF4444', background: 'rgba(239,68,68,0.06)', border: '1px solid rgba(239,68,68,0.15)', borderRadius: 8, padding: '8px 12px', marginBottom: 14 }}>{error}</div>}

      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 14, animation: 'slideUp 0.4s ease' }}>

        {/* ── Name ── */}
        <Section icon="◎" color="#00E0C4" title="Your Name"
          action={!nameInfo && !localName && !showNameForm ? <button onClick={() => setShowNameForm(true)} style={S.btn()}>+ Claim</button> : undefined}>
          {nameInfo ? (
            <div>
              <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 6 }}>
                <span style={{ fontSize: 24, fontWeight: 700, color: '#00E0C4' }}>{nameInfo.name}</span>
                <span style={{ fontSize: 8.5, background: 'rgba(0,224,196,0.12)', color: '#00E0C4', padding: '2px 7px', borderRadius: 4, fontWeight: 600 }}>ON-CHAIN</span>
              </div>
              {nameInfo.expiry_round && <p style={{ fontSize: 10.5, color: 'var(--dag-text-faint)' }}>Expires round {nameInfo.expiry_round.toLocaleString()}</p>}
            </div>
          ) : localName ? (
            <div>
              <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 6 }}>
                <span style={{ fontSize: 24, fontWeight: 700, color: '#00E0C4' }}>{localName}</span>
                <span style={{ fontSize: 8.5, background: 'rgba(255,184,0,0.12)', color: '#FFB800', padding: '2px 7px', borderRadius: 4, fontWeight: 600 }}>LOCAL</span>
              </div>
              <p style={{ fontSize: 10.5, color: 'var(--dag-text-faint)' }}>Saved locally. On-chain registration available via SmartOp.</p>
            </div>
          ) : showNameForm ? (
            <div style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
              <div style={{ position: 'relative' }}>
                <input type="text" value={nameInput} onChange={e => checkName(e.target.value.toLowerCase().replace(/[^a-z0-9-]/g, ''))}
                  placeholder="john29" maxLength={20} autoFocus style={S.input} />
                {nameChecking && <span style={{ position: 'absolute', right: 12, top: 11, color: 'var(--dag-text-faint)', fontSize: 11 }}>...</span>}
                {nameAvail === true && <span style={{ position: 'absolute', right: 12, top: 10, color: '#00E0C4', fontSize: 14 }}>✓</span>}
              </div>
              {nameWarn && <p style={{ fontSize: 10, color: '#FFB800' }}>{nameWarn}</p>}
              {nameAvail && <p style={{ fontSize: 10, color: '#00E0C4' }}>{nameInput} available! {nameFee > 0 ? `${fmt(nameFee)} UDAG/yr` : 'Free'}</p>}
              <div style={{ display: 'flex', gap: 8 }}>
                <button style={S.btnSolid()}>Register</button>
                <button onClick={() => setShowNameForm(false)} style={S.btn('var(--dag-text-muted)')}>Cancel</button>
              </div>
            </div>
          ) : (
            <p style={{ fontSize: 11.5, color: 'var(--dag-text-faint)' }}>No name registered. Claim one like <span style={{ color: '#00E0C4' }}>alice</span> or <span style={{ color: '#00E0C4' }}>john29</span>.</p>
          )}
        </Section>

        {/* ── Keys ── */}
        <Section icon="◇" color="#0066FF" title="Authorized Keys">
          {info && info.authorized_keys.length > 0 ? (
            <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
              {info.authorized_keys.map(key => (
                <div key={key.key_id} style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', background: 'var(--dag-card)', borderRadius: 10, padding: '10px 13px' }}>
                  <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
                    <div style={{ width: 32, height: 32, borderRadius: 8, display: 'flex', alignItems: 'center', justifyContent: 'center', background: key.key_type === 'p256' ? 'rgba(168,85,247,0.12)' : 'rgba(0,102,255,0.12)', fontSize: 14 }}>
                      {key.key_type === 'p256' ? '◎' : '◇'}
                    </div>
                    <div>
                      <div style={{ fontSize: 12, fontWeight: 600, color: 'var(--dag-text)' }}>{key.label}</div>
                      <div style={{ fontSize: 10, color: 'var(--dag-text-faint)', ...S.mono }}>{key.key_id.slice(0, 12)}… ({key.key_type === 'p256' ? 'Passkey' : 'Ed25519'})</div>
                    </div>
                  </div>
                  <div style={{ fontSize: 10, color: 'var(--dag-subheading)' }}>{key.daily_limit ? `${fmt(key.daily_limit)}/day` : 'No limit'}</div>
                </div>
              ))}
              {info.pending_key_removal && (
                <div style={{ fontSize: 10.5, color: '#FFB800', background: 'rgba(255,184,0,0.06)', border: '1px solid rgba(255,184,0,0.15)', borderRadius: 8, padding: '8px 12px', display: 'flex', alignItems: 'center', gap: 6 }}>
                  ◷ Key removal pending (round {info.pending_key_removal.executes_at_round})
                </div>
              )}
              <p style={{ fontSize: 10, color: 'var(--dag-text-faint)' }}>Add a backup device by scanning a QR from another wallet.</p>
            </div>
          ) : (
            <p style={{ fontSize: 11.5, color: 'var(--dag-text-faint)' }}>{info ? 'Key auto-registers on first transaction.' : 'SmartAccount activates when you send or receive.'}</p>
          )}
        </Section>

        {/* ── Recovery ── */}
        <Section icon="♛" color="#A855F7" title="Social Recovery"
          action={!info?.has_recovery && !showRecoveryForm ? <button onClick={() => setShowRecoveryForm(true)} style={S.btn('#A855F7')}>+ Set Up</button> : undefined}>
          {info?.has_recovery ? (
            <div>
              <div style={{ display: 'flex', alignItems: 'center', gap: 6, marginBottom: 6 }}>
                <span style={{ color: '#00E0C4' }}>✓</span>
                <span style={{ fontSize: 12, fontWeight: 600, color: '#00E0C4' }}>Configured</span>
              </div>
              <p style={{ fontSize: 11, color: 'var(--dag-text-muted)' }}>{info.recovery_threshold}-of-{info.guardian_count} guardians required to recover</p>
              <p style={{ fontSize: 10, color: 'var(--dag-text-faint)', marginTop: 4 }}>If you lose all devices, contact your guardians. Each one opens their wallet and approves your recovery. After {info.recovery_threshold} approve, a time-lock starts — then your new device gets access.</p>
              {info.has_pending_recovery && (
                <div style={{ fontSize: 10.5, color: '#FFB800', background: 'rgba(255,184,0,0.06)', border: '1px solid rgba(255,184,0,0.15)', borderRadius: 8, padding: '8px 12px', marginTop: 8 }}>
                  ⚠ A recovery is in progress — if you didn't initiate this, cancel it immediately from any authorized device!
                </div>
              )}
            </div>
          ) : showRecoveryForm ? (
            <div style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
              {/* Explainer */}
              <div style={{ background: 'rgba(168,85,247,0.04)', border: '1px solid rgba(168,85,247,0.1)', borderRadius: 10, padding: '12px 14px' }}>
                <div style={{ fontSize: 11.5, fontWeight: 600, color: '#A855F7', marginBottom: 6 }}>How recovery works</div>
                <ol style={{ fontSize: 10.5, color: 'var(--dag-text-muted)', lineHeight: 1.7, margin: 0, paddingLeft: 16 }}>
                  <li>Pick friends or family who have UltraDAG wallets</li>
                  <li>If you lose all devices, call them</li>
                  <li>Each guardian opens <strong style={{ color: 'var(--dag-text-secondary)' }}>their own wallet</strong> and taps "Approve Recovery"</li>
                  <li>When enough guardians approve, a time-lock starts (~2.8 hours)</li>
                  <li>After the time-lock, your new device gets access to your account</li>
                </ol>
                <p style={{ fontSize: 10, color: 'var(--dag-text-faint)', marginTop: 8 }}>Guardians can never access your funds — they can only authorize a key change.</p>
              </div>

              {/* Guardian inputs */}
              <div style={{ fontSize: 10.5, fontWeight: 600, color: 'var(--dag-text-muted)', letterSpacing: 1, marginTop: 4 }}>GUARDIAN WALLET ADDRESSES</div>
              {guardians.map((g, i) => (
                <VerifiedAddressInput key={i} value={g}
                  onChange={v => { const n = [...guardians]; n[i] = v; setGuardians(n); }}
                  placeholder={`Guardian ${i + 1} — paste their wallet address`} />
              ))}
              <button onClick={() => setGuardians([...guardians, ''])} style={{ ...S.btn('#A855F7'), alignSelf: 'flex-start', fontSize: 10 }}>+ Add another guardian</button>

              <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                <span style={{ fontSize: 10.5, color: 'var(--dag-subheading)' }}>How many must approve:</span>
                <select value={threshold} onChange={e => setThreshold(Number(e.target.value))}
                  style={{ ...S.input, width: 'auto', padding: '6px 10px' }}>
                  {Array.from({ length: guardians.filter(g => g.length > 0).length || 1 }, (_, i) => i + 1).map(n => (
                    <option key={n} value={n}>{n} of {guardians.filter(g => g.length > 0).length || '?'}</option>
                  ))}
                </select>
              </div>

              <div style={{ display: 'flex', gap: 8 }}>
                <button style={S.btnSolid('#A855F7')}>Save Guardians</button>
                <button onClick={() => setShowRecoveryForm(false)} style={S.btn('var(--dag-text-muted)')}>Cancel</button>
              </div>
            </div>
          ) : (
            <div>
              <p style={{ fontSize: 11.5, color: 'var(--dag-text-faint)' }}>No recovery guardians set.</p>
              <p style={{ fontSize: 10, color: 'var(--dag-text-faint)', marginTop: 4, lineHeight: 1.6 }}>
                If you lose all your devices, recovery guardians can authorize a new device.
                Pick 3–5 people you trust (friends, family) who have UltraDAG wallets.
                They can never access your funds — only help you regain access.
              </p>
            </div>
          )}
        </Section>

        {/* ── Spending Policy ── */}
        <Section icon="⬡" color="#FFB800" title="Spending Policy"
          action={!info?.has_policy && !showPolicyForm ? <button onClick={() => setShowPolicyForm(true)} style={S.btn('#FFB800')}>+ Set Up</button> : undefined}>
          {info?.has_policy ? (
            <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 8 }}>
              {[
                { l: 'INSTANT LIMIT', v: info.instant_limit != null ? `${fmt(info.instant_limit)} UDAG` : '—' },
                { l: 'VAULT THRESHOLD', v: info.vault_threshold && info.vault_threshold > 0 ? `${fmt(info.vault_threshold)} UDAG` : '—' },
                { l: 'DAILY CAP', v: info.daily_limit != null ? `${fmt(info.daily_limit)} UDAG` : '—' },
                { l: 'PENDING VAULTS', v: String(info.pending_vault_count) },
              ].map((x, i) => (
                <div key={i} style={S.stat}>
                  <div style={{ fontSize: 9, color: 'var(--dag-subheading)', letterSpacing: 1, marginBottom: 3 }}>{x.l}</div>
                  <div style={{ fontSize: 15, fontWeight: 700, color: 'var(--dag-text)' }}>{x.v}</div>
                </div>
              ))}
            </div>
          ) : showPolicyForm ? (
            <div style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
              {[
                { l: 'Instant limit (UDAG)', p: '1' },
                { l: 'Vault threshold (UDAG)', p: '100' },
                { l: 'Daily cap (UDAG)', p: '10' },
              ].map((f, i) => (
                <div key={i}>
                  <span style={{ ...S.label, display: 'block' }}>{f.l}</span>
                  <input type="number" placeholder={f.p} style={S.input} />
                </div>
              ))}
              <div style={{ display: 'flex', gap: 8 }}>
                <button style={S.btnSolid('#FFB800')}>Save (2.8hr time-lock)</button>
                <button onClick={() => setShowPolicyForm(false)} style={S.btn('var(--dag-text-muted)')}>Cancel</button>
              </div>
            </div>
          ) : (
            <div>
              <p style={{ fontSize: 11.5, color: 'var(--dag-text-faint)' }}>No spending limits set.</p>
              <p style={{ fontSize: 10, color: 'var(--dag-text-faint)', marginTop: 4 }}>Protect against key theft with daily limits and vault delays.</p>
            </div>
          )}
        </Section>
      </div>
    </div>
  );
}
