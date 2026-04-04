import { useState, useEffect, useCallback } from 'react';
import { getValidators, getDelegation, postDelegate, postUndelegate, getNodeUrl } from '../lib/api';
import { DisplayIdentity } from '../components/shared/DisplayIdentity';
import { useKeystore } from '../hooks/useKeystore';
import { hasPasskeyWallet, getPasskeyWallet } from '../lib/passkey-wallet';
import { signAndSubmitSmartOp } from '../lib/webauthn-sign';
import { Pagination } from '../components/shared/Pagination';
import { useIsMobile } from '../hooks/useIsMobile';

const SATS = 100_000_000;
const fmt = (s: number) => (s / SATS).toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 });

interface ValidatorInfo { address: string; staked: number; effective_stake: number; delegator_count: number; commission_percent: number; is_active?: boolean }
interface DelegInfo { address: string; name: string; delegated: number; validator: string; is_undelegating: boolean; unlock_at_round: number | null }

function pickBest(vs: ValidatorInfo[]): ValidatorInfo | null {
  const a = vs.filter(v => v.is_active);
  if (!a.length) return null;
  return [...a].sort((x, y) => x.commission_percent - y.commission_percent || x.effective_stake - y.effective_stake)[0];
}

const S = {
  card: { background: 'var(--dag-card)', border: '1px solid var(--dag-border)', borderRadius: 14, padding: '20px 22px' } as React.CSSProperties,
  stat: { background: 'var(--dag-card)', borderRadius: 10, padding: '12px 14px' } as React.CSSProperties,
  label: { fontSize: 10.5, fontWeight: 600, color: 'var(--dag-text-muted)', letterSpacing: 1.2, textTransform: 'uppercase' as const, marginBottom: 6, display: 'block' },
  input: { width: '100%', padding: '12px 14px', borderRadius: 10, background: 'var(--dag-input-bg)', border: '1px solid var(--dag-border)', color: 'var(--dag-text)', fontSize: 15, outline: 'none', fontFamily: "'DM Mono',monospace" } as React.CSSProperties,
  btnSolid: (c = '#00E0C4') => ({ width: '100%', padding: '12px 0', borderRadius: 10, background: c, color: '#080C14', fontSize: 13, fontWeight: 700, cursor: 'pointer', border: 'none', transition: 'opacity 0.2s' }),
  btn: (c = '#00E0C4') => ({ padding: '6px 14px', borderRadius: 8, background: `${c}12`, border: `1px solid ${c}25`, color: c, fontSize: 11, fontWeight: 600, cursor: 'pointer' }),
  mono: { fontFamily: "'DM Mono',monospace" },
};

export function StakingPage() {
  const { wallets, unlocked } = useKeystore();
  const m = useIsMobile();
  const [validators, setValidators] = useState<ValidatorInfo[]>([]);
  const [totalStaked, setTotalStaked] = useState(0);
  const [totalDelegated, setTotalDelegated] = useState(0);
  const [delegations, setDelegations] = useState<DelegInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [amount, setAmount] = useState('');
  const [walletIdx, setWalletIdx] = useState(0);
  const [stakeLoading, setStakeLoading] = useState(false);
  const [stakeMsg, setStakeMsg] = useState('');
  const [showValidators, setShowValidators] = useState(false);
  const [customValidator, setCustomValidator] = useState<string | null>(null);
  const [undelegateLoading, setUndelegateLoading] = useState('');
  const [validatorPage, setValidatorPage] = useState(1);
  const [delegationPage, setDelegationPage] = useState(1);
  const STAKING_PAGE_SIZE = 10;
  const pw = getPasskeyWallet();

  const refresh = useCallback(async () => {
    try {
      const v = await getValidators();
      const raw: ValidatorInfo[] = Array.isArray(v) ? v : (v.validators ?? []);
      setValidators(raw.map(vi => ({ ...vi, is_active: vi.is_active ?? true })));
      if (!Array.isArray(v)) { setTotalStaked(v.total_staked ?? 0); setTotalDelegated(v.total_delegated ?? 0); }
    } catch {}
    if (unlocked && wallets.length > 0) {
      const ds: DelegInfo[] = [];
      for (const w of wallets) {
        try { const d = await getDelegation(w.address); if (d.delegated > 0 || d.is_undelegating) ds.push({ address: w.address, name: w.name, ...d }); } catch {}
      }
      setDelegations(ds);
    }
    setLoading(false);
  }, [wallets, unlocked]);

  useEffect(() => { refresh(); const iv = setInterval(refresh, 30000); return () => clearInterval(iv); }, [refresh]);

  useEffect(() => {
    const handler = () => { setValidators([]); setTotalStaked(0); setTotalDelegated(0); setDelegations([]); setLoading(true); setValidatorPage(1); setDelegationPage(1); refresh(); };
    window.addEventListener('ultradag-network-switch', handler);
    return () => window.removeEventListener('ultradag-network-switch', handler);
  }, [refresh]);

  const best = pickBest(validators);
  const target = customValidator || best?.address || null;

  const handleStake = async () => {
    const wallet = wallets[walletIdx];
    if (!wallet || !target) return;
    const sats = Math.floor(parseFloat(amount) * SATS);
    if (isNaN(sats) || sats < 100 * SATS) { setStakeMsg('⚠ Minimum 100 UDAG'); return; }
    setStakeLoading(true); setStakeMsg('');
    try {
      if (hasPasskeyWallet() && !wallet.secret_key) {
        const br = await fetch(`${getNodeUrl()}/balance/${wallet.address}`, { signal: AbortSignal.timeout(5000) });
        const bd = await br.json();
        await signAndSubmitSmartOp({ Delegate: { validator: target, amount: sats } }, 0, bd.nonce ?? 0);
      } else {
        await postDelegate({ secret_key: wallet.secret_key, validator: target, amount: sats });
      }
      setStakeMsg('✓ ' + amount + ' UDAG staked!'); setAmount(''); setCustomValidator(null); refresh();
    } catch (e: unknown) { setStakeMsg('⚠ ' + (e instanceof Error ? e.message : 'Failed')); }
    finally { setStakeLoading(false); }
  };

  const handleUndelegate = async (addr: string) => {
    const wallet = wallets.find(w => w.address === addr);
    if (!wallet) return;
    setUndelegateLoading(addr);
    try {
      if (hasPasskeyWallet() && !wallet.secret_key) {
        const br = await fetch(`${getNodeUrl()}/balance/${wallet.address}`, { signal: AbortSignal.timeout(5000) });
        const bd = await br.json();
        await signAndSubmitSmartOp({ Undelegate: {} }, 0, bd.nonce ?? 0);
      } else { await postUndelegate({ secret_key: wallet.secret_key }); }
      refresh();
    } catch {} finally { setUndelegateLoading(''); }
  };

  return (
    <div style={{ padding: m ? '12px 14px' : '18px 26px', fontFamily: "'DM Sans',sans-serif" }}>
      <style>{`@keyframes slideUp{from{opacity:0;transform:translateY(10px)}to{opacity:1;transform:translateY(0)}} input:focus,select:focus{border-color:rgba(0,224,196,0.3)!important}`}</style>

      <div style={{ marginBottom: m ? 16 : 22, animation: 'slideUp 0.3s ease' }}>
        <h1 style={{ fontSize: m ? 18 : 21, fontWeight: 700, color: 'var(--dag-text)' }}>Staking</h1>
        <p style={{ fontSize: 11.5, color: 'var(--dag-subheading)', marginTop: 2 }}>Stake UDAG to earn passive rewards — no node required</p>
      </div>

      {/* Stats Row */}
      <div style={{ display: 'grid', gridTemplateColumns: m ? 'repeat(2,1fr)' : 'repeat(3,1fr)', gap: m ? 10 : 12, marginBottom: 18, animation: 'slideUp 0.4s ease' }}>
        {[
          { l: 'TOTAL STAKED', v: fmt(totalStaked) + ' UDAG', c: '#00E0C4', i: '⬡' },
          { l: 'TOTAL DELEGATED', v: fmt(totalDelegated) + ' UDAG', c: '#0066FF', i: '◎' },
          { l: 'ACTIVE VALIDATORS', v: String(validators.filter(v => v.is_active).length), c: '#A855F7', i: '♛' },
        ].map((s, i) => (
          <div key={i} style={S.card}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 6, marginBottom: 8 }}>
              <span style={{ color: s.c, fontSize: 14 }}>{s.i}</span>
              <span style={{ fontSize: 9.5, color: 'var(--dag-text-muted)', letterSpacing: 1.2 }}>{s.l}</span>
            </div>
            <div style={{ fontSize: 22, fontWeight: 700, color: 'var(--dag-text)', ...S.mono }}>{s.v}</div>
          </div>
        ))}
      </div>

      <div style={{ display: 'grid', gridTemplateColumns: m ? '1fr' : '2fr 3fr', gap: m ? 14 : 16, animation: 'slideUp 0.5s ease' }}>
        {/* ── Stake Form ── */}
        <div style={{ display: 'flex', flexDirection: 'column', gap: 14 }}>
          <div style={{
            ...S.card, background: 'linear-gradient(135deg, rgba(0,224,196,0.03), rgba(0,102,255,0.02))',
            borderColor: 'rgba(0,224,196,0.12)',
          }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 7, marginBottom: 16 }}>
              <span style={{ color: '#00E0C4', fontSize: 16 }}>⬡</span>
              <span style={{ fontSize: 14, fontWeight: 600, color: 'var(--dag-text-secondary)' }}>Stake UDAG</span>
              {pw && <span style={{ fontSize: 8.5, background: 'rgba(0,224,196,0.12)', color: '#00E0C4', padding: '1px 6px', borderRadius: 4, fontWeight: 600 }}>PASSKEY</span>}
            </div>

            {!unlocked ? (
              <p style={{ fontSize: 12, color: 'var(--dag-subheading)', textAlign: 'center', padding: '20px 0' }}>Unlock wallet to stake.</p>
            ) : (
              <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
                {wallets.length > 1 && (
                  <div>
                    <span style={S.label}>Wallet</span>
                    <select value={walletIdx} onChange={e => setWalletIdx(Number(e.target.value))} style={{ ...S.input, fontSize: 13, fontFamily: "'DM Sans',sans-serif" }}>
                      {wallets.map((w, i) => <option key={i} value={i} style={{ background: 'var(--dag-bg)' }}>{w.name}</option>)}
                    </select>
                  </div>
                )}
                <div>
                  <span style={S.label}>Amount</span>
                  <div style={{ position: 'relative' }}>
                    <input type="number" min="100" step="1" value={amount}
                      onChange={e => { setAmount(e.target.value); setStakeMsg(''); }} placeholder="100" style={S.input} />
                    <span style={{ position: 'absolute', right: 14, top: '50%', transform: 'translateY(-50%)', color: 'var(--dag-text-faint)', fontSize: 12 }}>UDAG</span>
                  </div>
                  <p style={{ fontSize: 10, color: 'var(--dag-text-faint)', marginTop: 4 }}>Minimum 100 UDAG</p>
                </div>

                {target && (
                  <div style={{ ...S.stat, display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                    <div>
                      <div style={{ fontSize: 9.5, color: 'var(--dag-subheading)', letterSpacing: 1 }}>{customValidator ? 'SELECTED' : 'AUTO-SELECTED'}</div>
                      <div style={{ marginTop: 2 }}><DisplayIdentity address={target} link size="xs" /></div>
                    </div>
                    {best && !customValidator && (
                      <span style={{ fontSize: 10, background: 'rgba(0,224,196,0.1)', color: '#00E0C4', padding: '2px 8px', borderRadius: 4 }}>{best.commission_percent}% fee</span>
                    )}
                  </div>
                )}

                {stakeMsg && (
                  <div style={{ fontSize: 11, color: stakeMsg.startsWith('✓') ? '#00E0C4' : '#FFB800', background: stakeMsg.startsWith('✓') ? 'rgba(0,224,196,0.06)' : 'rgba(255,184,0,0.06)', border: `1px solid ${stakeMsg.startsWith('✓') ? 'rgba(0,224,196,0.15)' : 'rgba(255,184,0,0.15)'}`, borderRadius: 8, padding: '8px 12px' }}>
                    {stakeMsg}
                  </div>
                )}

                <button onClick={handleStake} disabled={stakeLoading || !target || !amount} style={{ ...S.btnSolid(), opacity: stakeLoading || !target || !amount ? 0.3 : 1 }}>
                  {stakeLoading ? 'Staking...' : pw ? '◎ Stake with Biometrics' : '⬡ Stake'}
                </button>
              </div>
            )}
          </div>

          {/* How it works */}
          <div style={S.card}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 7, marginBottom: 10 }}>
              <span style={{ color: '#0066FF', fontSize: 13 }}>◈</span>
              <span style={{ fontSize: 12, fontWeight: 600, color: 'var(--dag-cell-text)' }}>How staking works</span>
            </div>
            <div style={{ fontSize: 11, color: 'var(--dag-subheading)', lineHeight: 1.7 }}>
              <p>Your UDAG is delegated to a validator who secures the network. You earn rewards proportional to your stake, minus commission.</p>
              <p style={{ marginTop: 6 }}>Unstaking has a ~2.8 hour cooldown before funds are returned.</p>
            </div>
          </div>
        </div>

        {/* ── Right: Delegations + Validators ── */}
        <div style={{ display: 'flex', flexDirection: 'column', gap: 14 }}>
          {/* Active Delegations */}
          {unlocked && delegations.length > 0 && (
            <div style={S.card}>
              <div style={{ display: 'flex', alignItems: 'center', gap: 7, marginBottom: 14 }}>
                <span style={{ color: '#00E0C4', fontSize: 14 }}>✓</span>
                <span style={{ fontSize: 13.5, fontWeight: 600, color: 'var(--dag-text-secondary)' }}>Your Staked UDAG</span>
              </div>
              <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
                {delegations.slice((delegationPage - 1) * STAKING_PAGE_SIZE, delegationPage * STAKING_PAGE_SIZE).map(d => (
                  <div key={d.address} style={{ ...S.stat, display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                    <div>
                      <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
                        <span style={{ fontSize: 13, fontWeight: 600, color: 'var(--dag-text)' }}>{d.name}</span>
                        <span style={{ fontSize: 8.5, padding: '1px 6px', borderRadius: 4, fontWeight: 600, background: d.is_undelegating ? 'rgba(255,184,0,0.12)' : 'rgba(0,224,196,0.12)', color: d.is_undelegating ? '#FFB800' : '#00E0C4' }}>
                          {d.is_undelegating ? 'UNSTAKING' : 'EARNING'}
                        </span>
                      </div>
                      <div style={{ marginTop: 2, display: 'flex', alignItems: 'center', gap: 4 }}><span style={{ fontSize: 10.5, color: 'var(--dag-text-faint)' }}>→</span> <DisplayIdentity address={d.validator} link size="xs" /></div>
                    </div>
                    <div style={{ textAlign: 'right' }}>
                      <div style={{ fontSize: 17, fontWeight: 700, color: 'var(--dag-text)', ...S.mono }}>{fmt(d.delegated)}</div>
                      {d.is_undelegating ? (
                        <div style={{ fontSize: 9.5, color: '#FFB800' }}>Round {d.unlock_at_round}</div>
                      ) : (
                        <button onClick={() => handleUndelegate(d.address)} disabled={undelegateLoading === d.address}
                          style={{ ...S.btn('#EF4444'), marginTop: 4, opacity: undelegateLoading === d.address ? 0.5 : 1 }}>
                          {undelegateLoading === d.address ? '...' : 'Unstake'}
                        </button>
                      )}
                    </div>
                  </div>
                ))}
                <Pagination page={delegationPage} totalPages={Math.ceil(delegations.length / STAKING_PAGE_SIZE)} onPageChange={setDelegationPage} totalItems={delegations.length} pageSize={STAKING_PAGE_SIZE} />
              </div>
            </div>
          )}

          {/* Validator List */}
          <div style={S.card}>
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: showValidators ? 14 : 0, cursor: 'pointer' }}
              onClick={() => setShowValidators(!showValidators)}>
              <div style={{ display: 'flex', alignItems: 'center', gap: 7 }}>
                <span style={{ color: '#A855F7', fontSize: 14 }}>♛</span>
                <span style={{ fontSize: 13.5, fontWeight: 600, color: 'var(--dag-text-secondary)' }}>All Validators</span>
                <span style={{ fontSize: 9.5, color: 'var(--dag-text-faint)' }}>{validators.length}</span>
              </div>
              <span style={{ color: 'var(--dag-text-faint)', fontSize: 12 }}>{showValidators ? '▲' : '▼'}</span>
            </div>
            {showValidators && (
              <div>
                {loading ? <p style={{ fontSize: 12, color: 'var(--dag-subheading)', padding: '16px 0', textAlign: 'center' }}>Loading...</p> : validators.length === 0 ? (
                  <p style={{ fontSize: 12, color: 'var(--dag-subheading)', padding: '16px 0', textAlign: 'center' }}>No validators yet.</p>
                ) : (
                  <div style={{ display: 'flex', flexDirection: 'column', gap: 6, overflowX: 'auto', WebkitOverflowScrolling: 'touch' }}>
                    {/* Header */}
                    <div style={{ display: 'grid', gridTemplateColumns: '2fr 1fr 1fr 1fr auto', gap: 8, padding: '0 4px', minWidth: m ? 500 : undefined }}>
                      {['ADDRESS', 'STAKE', 'DELEGATORS', 'FEE', ''].map((h, i) => (
                        <div key={i} style={{ fontSize: 8.5, fontWeight: 600, color: 'var(--dag-text-faint)', letterSpacing: 1.5 }}>{h}</div>
                      ))}
                    </div>
                    {validators.slice((validatorPage - 1) * STAKING_PAGE_SIZE, validatorPage * STAKING_PAGE_SIZE).map(v => (
                      <div key={v.address} style={{ display: 'grid', gridTemplateColumns: '2fr 1fr 1fr 1fr auto', gap: 8, alignItems: 'center', padding: '8px 4px', borderTop: '1px solid var(--dag-row-border)', minWidth: m ? 500 : undefined }}>
                        <DisplayIdentity address={v.address} link size="xs" />
                        <div style={{ fontSize: 11, color: 'var(--dag-cell-text)' }}>{fmt(v.effective_stake)}</div>
                        <div style={{ fontSize: 11, color: 'var(--dag-cell-text)' }}>{v.delegator_count}</div>
                        <div style={{ fontSize: 11, color: v.commission_percent <= 10 ? '#00E0C4' : '#FFB800' }}>{v.commission_percent}%</div>
                        {unlocked && wallets.length > 0 && (
                          <button onClick={() => { setCustomValidator(v.address); setShowValidators(false); window.scrollTo({ top: 0, behavior: 'smooth' }); }}
                            style={S.btn()}>Select</button>
                        )}
                      </div>
                    ))}
                    <Pagination page={validatorPage} totalPages={Math.ceil(validators.length / STAKING_PAGE_SIZE)} onPageChange={setValidatorPage} totalItems={validators.length} pageSize={STAKING_PAGE_SIZE} />
                  </div>
                )}
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
