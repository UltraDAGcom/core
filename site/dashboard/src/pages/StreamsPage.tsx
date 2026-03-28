import { useState, useEffect, useCallback } from 'react';
import { getNodeUrl } from '../lib/api';
import { useIsMobile } from '../hooks/useIsMobile';
import type { NetworkType } from '../lib/api';

const SATS = 100_000_000;
const ROUNDS_PER_SECOND = 1 / 5; // 1 round per 5 seconds
const MIN_FEE = 10_000;

type Frequency = 'per_second' | 'per_minute' | 'per_hour' | 'per_day' | 'per_week' | 'per_month';
type DurationUnit = 'minutes' | 'hours' | 'days' | 'weeks' | 'months';
type StreamType = 'continuous' | 'salary' | 'subscription' | 'vesting';

const FREQ_LABELS: Record<Frequency, string> = {
  per_second: '/sec', per_minute: '/min', per_hour: '/hr', per_day: '/day', per_week: '/wk', per_month: '/mo',
};
const FREQ_SECONDS: Record<Frequency, number> = {
  per_second: 1, per_minute: 60, per_hour: 3600, per_day: 86400, per_week: 604800, per_month: 2592000,
};
const DUR_SECONDS: Record<DurationUnit, number> = {
  minutes: 60, hours: 3600, days: 86400, weeks: 604800, months: 2592000,
};

interface Stream {
  id: string; sender: string; recipient: string; rate_per_round: number;
  deposited: number; accrued: number; withdrawable: number; withdrawn: number;
  start_round: number; end_round: number; status: 'Active' | 'Cancelled' | 'Depleted';
  rate_udag_per_hour?: number; remaining_rounds?: number; elapsed_rounds?: number;
}

interface StreamsPageProps {
  wallets: Array<{ name: string; address: string; secret_key: string }>;
  network: NetworkType;
}

function fmtUdag(sats: number): string {
  const u = sats / SATS;
  if (u >= 1000) return u.toLocaleString(undefined, { maximumFractionDigits: 2 });
  if (u >= 1) return u.toLocaleString(undefined, { maximumFractionDigits: 4 });
  return u.toLocaleString(undefined, { maximumFractionDigits: 8 });
}

function shortAddr(addr: string): string {
  if (!addr) return '';
  return addr.length > 16 ? addr.slice(0, 8) + '...' + addr.slice(-6) : addr;
}

function statusColor(s: string) { return s === 'Active' ? '#00E0C4' : s === 'Depleted' ? '#FFB800' : '#EF4444'; }
function statusBg(s: string) { return s === 'Active' ? 'rgba(0,224,196,0.08)' : s === 'Depleted' ? 'rgba(255,184,0,0.08)' : 'rgba(239,68,68,0.08)'; }

const STREAM_TYPE_INFO: Record<StreamType, { label: string; desc: string; icon: string; defaultFreq: Frequency; defaultDurUnit: DurationUnit }> = {
  continuous: { label: 'Continuous', desc: 'Real-time payment flow', icon: '〰', defaultFreq: 'per_second', defaultDurUnit: 'hours' },
  salary: { label: 'Salary', desc: 'Recurring payroll', icon: '💼', defaultFreq: 'per_month', defaultDurUnit: 'months' },
  subscription: { label: 'Subscription', desc: 'Service access fee', icon: '🔄', defaultFreq: 'per_month', defaultDurUnit: 'months' },
  vesting: { label: 'Vesting', desc: 'Token release schedule', icon: '🔐', defaultFreq: 'per_day', defaultDurUnit: 'months' },
};

const S = {
  card: { background: 'var(--dag-card)', border: '1px solid var(--dag-border)', borderRadius: 14, padding: '20px 22px' } as React.CSSProperties,
  label: { fontSize: 10, fontWeight: 600, color: 'var(--dag-text-muted)', letterSpacing: 1.2, textTransform: 'uppercase' as const, marginBottom: 5, display: 'block' },
  input: { width: '100%', padding: '11px 14px', borderRadius: 10, background: 'var(--dag-input-bg)', border: '1px solid var(--dag-border)', color: 'var(--dag-text)', fontSize: 14, outline: 'none', fontFamily: "'DM Mono',monospace", boxSizing: 'border-box' as const } as React.CSSProperties,
  select: { padding: '11px 10px', borderRadius: 10, background: 'var(--dag-input-bg)', border: '1px solid var(--dag-border)', color: 'var(--dag-text)', fontSize: 12, outline: 'none', cursor: 'pointer', fontFamily: "'DM Sans',sans-serif" } as React.CSSProperties,
  mono: { fontFamily: "'DM Mono',monospace" },
  btn: (c = '#00E0C4') => ({ padding: '6px 14px', borderRadius: 8, background: `${c}12`, border: `1px solid ${c}25`, color: c, fontSize: 11, fontWeight: 600, cursor: 'pointer', transition: 'opacity 0.2s' }),
};

function ProgressBar({ value, max }: { value: number; max: number }) {
  const pct = max > 0 ? Math.min(100, (value / max) * 100) : 0;
  return (
    <div style={{ position: 'relative', height: 6, borderRadius: 3, background: 'var(--dag-input-bg)', overflow: 'hidden', minWidth: 80 }}>
      <div style={{ height: '100%', borderRadius: 3, width: `${pct}%`, background: 'linear-gradient(90deg, #00E0C4, #0066FF)', transition: 'width 0.5s ease' }} />
      <span style={{ position: 'absolute', right: 0, top: -15, fontSize: 9, color: 'var(--dag-text-muted)', ...S.mono }}>{pct.toFixed(1)}%</span>
    </div>
  );
}

export function StreamsPage({ wallets, network: _network }: StreamsPageProps) {
  const m = useIsMobile();
  const [loading, setLoading] = useState(true);
  const [allStreams, setAllStreams] = useState<Stream[]>([]);
  const [, setTick] = useState(0);

  // Form state
  const [streamType, setStreamType] = useState<StreamType>('continuous');
  const [recipient, setRecipient] = useState('');
  const [rateAmount, setRateAmount] = useState('');
  const [frequency, setFrequency] = useState<Frequency>('per_second');
  const [durAmount, setDurAmount] = useState('');
  const [durUnit, setDurUnit] = useState<DurationUnit>('hours');
  const [useTotalDeposit, setUseTotalDeposit] = useState(false);
  const [totalDepositInput, setTotalDepositInput] = useState('');
  const [cliffRounds, setCliffRounds] = useState('');
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [formMsg, setFormMsg] = useState('');
  const [formError, setFormError] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [actionLoading, setActionLoading] = useState<string | null>(null);
  const [tab, setTab] = useState<'create' | 'outgoing' | 'incoming'>('create');

  const myAddresses = wallets.map(w => w.address.toLowerCase());
  const outgoing = allStreams.filter(s => myAddresses.includes(s.sender.toLowerCase()));
  const incoming = allStreams.filter(s => myAddresses.includes(s.recipient.toLowerCase()));
  const activeCount = allStreams.filter(s => s.status === 'Active').length;
  const totalStreaming = allStreams.filter(s => s.status === 'Active').reduce((sum, s) => sum + (s.deposited - (s.withdrawn || 0)), 0);
  // myCount available if needed: new Set([...outgoing.map(s => s.id), ...incoming.map(s => s.id)]).size

  // When stream type changes, update defaults
  useEffect(() => {
    const info = STREAM_TYPE_INFO[streamType];
    setFrequency(info.defaultFreq);
    setDurUnit(info.defaultDurUnit);
  }, [streamType]);

  // Computed values
  const rateNum = parseFloat(rateAmount) || 0;
  const durNum = parseFloat(durAmount) || 0;
  const totalDepNum = parseFloat(totalDepositInput) || 0;

  // Convert rate to sats per round
  const ratePerSecond = rateNum * SATS / FREQ_SECONDS[frequency];
  const satsPerRound = Math.floor(ratePerSecond / ROUNDS_PER_SECOND); // sats per round = sats_per_second * seconds_per_round

  // Compute total deposit
  let totalDeposit: number;
  let durationSeconds: number;
  if (useTotalDeposit && totalDepNum > 0) {
    totalDeposit = totalDepNum;
    durationSeconds = ratePerSecond > 0 ? (totalDepNum * SATS) / ratePerSecond : 0;
  } else {
    durationSeconds = durNum * DUR_SECONDS[durUnit];
    totalDeposit = (ratePerSecond * durationSeconds) / SATS;
  }
  const totalRounds = Math.ceil(durationSeconds / 5);
  const cliffRoundsNum = parseInt(cliffRounds) || 0;

  // Human readable duration
  function fmtDuration(secs: number): string {
    if (secs < 60) return `${Math.round(secs)}s`;
    if (secs < 3600) return `${Math.round(secs / 60)}m`;
    if (secs < 86400) return `${(secs / 3600).toFixed(1)}h`;
    if (secs < 604800) return `${(secs / 86400).toFixed(1)}d`;
    if (secs < 2592000) return `${(secs / 604800).toFixed(1)}w`;
    return `${(secs / 2592000).toFixed(1)}mo`;
  }

  // Rate display in multiple units
  function rateBreakdown(): { perSec: string; perMin: string; perHour: string; perDay: string } {
    const ps = ratePerSecond / SATS;
    return {
      perSec: ps.toFixed(8).replace(/0+$/, '').replace(/\.$/, '.0'),
      perMin: (ps * 60).toFixed(6).replace(/0+$/, '').replace(/\.$/, '.0'),
      perHour: (ps * 3600).toFixed(4).replace(/0+$/, '').replace(/\.$/, '.0'),
      perDay: (ps * 86400).toFixed(2),
    };
  }

  const fetchStreams = useCallback(async () => {
    try {
      const streams: Stream[] = [];
      const seen = new Set<string>();
      for (const addr of myAddresses) {
        for (const role of ['sender', 'recipient']) {
          try {
            const res = await fetch(`${getNodeUrl()}/streams/${role}/${addr}`, { signal: AbortSignal.timeout(5000) });
            if (res.ok) {
              const data = await res.json();
              for (const s of (data.streams ?? [])) { if (!seen.has(s.id)) { seen.add(s.id); streams.push(s); } }
            }
          } catch { /* endpoint may not exist yet */ }
        }
      }
      setAllStreams(streams);
    } catch { /* ignore */ }
    setLoading(false);
  }, [myAddresses.join(',')]);

  useEffect(() => { fetchStreams(); const iv = setInterval(fetchStreams, 5000); return () => clearInterval(iv); }, [fetchStreams]);
  useEffect(() => { const h = () => { setAllStreams([]); setLoading(true); fetchStreams(); }; window.addEventListener('ultradag-network-switch', h); return () => window.removeEventListener('ultradag-network-switch', h); }, [fetchStreams]);
  useEffect(() => { const iv = setInterval(() => setTick(t => t + 1), 1000); return () => clearInterval(iv); }, []);

  const handleCreate = async () => {
    if (!recipient) { setFormMsg('Enter a recipient address'); setFormError(true); return; }
    if (satsPerRound <= 0) { setFormMsg('Rate must be greater than 0'); setFormError(true); return; }
    if (totalDeposit < 0.0001) { setFormMsg('Total deposit too small'); setFormError(true); return; }
    const wallet = wallets[0];
    if (!wallet?.secret_key) { setFormMsg('No wallet with secret key available'); setFormError(true); return; }

    setFormMsg('');
    setFormError(false);
    setSubmitting(true);
    try {
      const depositSats = Math.floor(totalDeposit * SATS);
      const res = await fetch(`${getNodeUrl()}/stream/create`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          secret_key: wallet.secret_key,
          recipient,
          rate_sats_per_round: satsPerRound,
          deposit: depositSats,
          cliff_rounds: cliffRoundsNum,
        }),
        signal: AbortSignal.timeout(10000),
      });
      const data = await res.json();
      if (!res.ok) {
        setFormMsg(data.error || 'Failed to create stream');
        setFormError(true);
        return;
      }
      setFormMsg(`Stream created! TX: ${(data.tx_hash || '').slice(0, 12)}... Stream ID: ${(data.stream_id || '').slice(0, 12)}...`);
      setFormError(false);
      // Clear form
      setRecipient('');
      setRateAmount('');
      setDurAmount('');
      setTotalDepositInput('');
      setCliffRounds('');
      // Refresh streams and switch to outgoing tab
      setTimeout(() => { fetchStreams(); setTab('outgoing'); }, 2000);
    } catch (err) {
      setFormMsg(err instanceof Error ? err.message : 'Network error');
      setFormError(true);
    } finally {
      setSubmitting(false);
    }
  };

  const isValid = recipient && satsPerRound > 0 && totalDeposit >= 0.0001;
  const rates = ratePerSecond > 0 ? rateBreakdown() : null;

  return (
    <div style={{ padding: m ? '12px 14px' : '18px 26px', fontFamily: "'DM Sans',sans-serif" }}>
      <style>{`
        @keyframes slideUp{from{opacity:0;transform:translateY(10px)}to{opacity:1;transform:translateY(0)}}
        @keyframes streamPulse{0%,100%{opacity:1;text-shadow:0 0 6px rgba(0,224,196,0.2)}50%{opacity:0.75;text-shadow:0 0 12px rgba(0,224,196,0.4)}}
        @keyframes pulse{0%,100%{opacity:0.4}50%{opacity:0.15}}
        @keyframes flowDot{0%{transform:translateX(0)}100%{transform:translateX(60px)}}
        input:focus,select:focus{border-color:rgba(0,224,196,0.3)!important}
        .stream-row:hover{background:var(--dag-card)!important}
      `}</style>

      {/* Header */}
      <div style={{ marginBottom: m ? 16 : 22, animation: 'slideUp 0.3s ease' }}>
        <h1 style={{ fontSize: m ? 18 : 21, fontWeight: 700, color: 'var(--dag-text)', display: 'flex', alignItems: 'center', gap: 10 }}>
          <span style={{ background: 'linear-gradient(135deg, #00E0C4, #0066FF)', WebkitBackgroundClip: 'text', WebkitTextFillColor: 'transparent', fontSize: 24 }}>≋</span>
          Streaming Payments
        </h1>
        <p style={{ fontSize: 11.5, color: 'var(--dag-subheading)', marginTop: 2 }}>
          Continuous money flow — pay by the second, minute, or month
        </p>
      </div>

      {/* Stats Row */}
      <div style={{ display: 'grid', gridTemplateColumns: m ? 'repeat(2,1fr)' : 'repeat(4,1fr)', gap: m ? 10 : 12, marginBottom: 18, animation: 'slideUp 0.4s ease' }}>
        {[
          { l: 'ACTIVE', v: loading ? '—' : String(activeCount), c: '#00E0C4', i: '≋' },
          { l: 'LOCKED IN STREAMS', v: loading ? '—' : fmtUdag(totalStreaming) + ' UDAG', c: '#0066FF', i: '◈' },
          { l: 'OUTGOING', v: loading ? '—' : String(outgoing.length), c: '#A855F7', i: '↑' },
          { l: 'INCOMING', v: loading ? '—' : String(incoming.length), c: '#FFB800', i: '↓' },
        ].map((s, i) => (
          <div key={i} style={{ ...S.card, padding: '14px 16px' }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 6, marginBottom: 6 }}>
              <span style={{ color: s.c, fontSize: 14 }}>{s.i}</span>
              <span style={{ fontSize: 9, color: 'var(--dag-text-muted)', letterSpacing: 1 }}>{s.l}</span>
            </div>
            <div style={{ fontSize: 20, fontWeight: 700, color: 'var(--dag-text)', ...S.mono }}>{s.v}</div>
          </div>
        ))}
      </div>

      {/* Tab Bar */}
      <div style={{ display: 'flex', gap: 2, marginBottom: 16, animation: 'slideUp 0.45s ease' }}>
        {([
          { key: 'create', label: '+ Create Stream', count: 0 },
          { key: 'outgoing', label: '↑ Outgoing', count: outgoing.length },
          { key: 'incoming', label: '↓ Incoming', count: incoming.length },
        ] as const).map(t => (
          <button key={t.key} onClick={() => setTab(t.key)} style={{
            padding: '8px 18px', borderRadius: 8, border: 'none', cursor: 'pointer',
            background: tab === t.key ? 'rgba(0,224,196,0.08)' : 'transparent',
            color: tab === t.key ? '#00E0C4' : 'var(--dag-text-muted)',
            fontSize: 12, fontWeight: tab === t.key ? 600 : 400, transition: 'all 0.2s',
            borderBottom: tab === t.key ? '2px solid #00E0C4' : '2px solid transparent',
          }}>
            {t.label}
            {t.count > 0 && <span style={{ marginLeft: 6, fontSize: 9, background: `${tab === t.key ? '#00E0C4' : 'var(--dag-text-faint)'}20`, padding: '1px 5px', borderRadius: 3 }}>{t.count}</span>}
          </button>
        ))}
      </div>

      {/* CREATE TAB */}
      {tab === 'create' && (
        <div style={{ display: 'grid', gridTemplateColumns: m ? '1fr' : '1fr 1fr', gap: m ? 14 : 16, animation: 'slideUp 0.5s ease' }}>
          {/* Left: Form */}
          <div style={{ display: 'flex', flexDirection: 'column', gap: 14 }}>

            {/* Stream Type Selector */}
            <div style={S.card}>
              <span style={S.label}>Stream Type</span>
              <div style={{ display: 'grid', gridTemplateColumns: m ? 'repeat(2,1fr)' : 'repeat(4,1fr)', gap: 8, marginTop: 4 }}>
                {(Object.entries(STREAM_TYPE_INFO) as [StreamType, typeof STREAM_TYPE_INFO[StreamType]][]).map(([key, info]) => (
                  <button key={key} onClick={() => { setStreamType(key); setFormMsg(''); }} style={{
                    padding: '10px 8px', borderRadius: 10, border: streamType === key ? '1px solid rgba(0,224,196,0.3)' : '1px solid var(--dag-border)',
                    background: streamType === key ? 'rgba(0,224,196,0.06)' : 'var(--dag-input-bg)',
                    cursor: 'pointer', textAlign: 'center', transition: 'all 0.2s',
                  }}>
                    <div style={{ fontSize: 18, marginBottom: 4 }}>{info.icon}</div>
                    <div style={{ fontSize: 11, fontWeight: 600, color: streamType === key ? '#00E0C4' : 'var(--dag-text-secondary)' }}>{info.label}</div>
                    <div style={{ fontSize: 9, color: 'var(--dag-text-faint)', marginTop: 2 }}>{info.desc}</div>
                  </button>
                ))}
              </div>
            </div>

            {/* Recipient */}
            <div style={S.card}>
              <span style={S.label}>Recipient</span>
              <input type="text" value={recipient} onChange={e => { setRecipient(e.target.value); setFormMsg(''); }}
                placeholder="udag1... or 0x address or name" style={S.input} />
            </div>

            {/* Rate + Frequency */}
            <div style={S.card}>
              <span style={S.label}>Payment Rate</span>
              <div style={{ display: 'flex', gap: 8 }}>
                <div style={{ flex: 1, position: 'relative' }}>
                  <input type="number" min="0" step="any" value={rateAmount}
                    onChange={e => { setRateAmount(e.target.value); setFormMsg(''); }} placeholder="0.001" style={S.input} />
                  <span style={{ position: 'absolute', right: 12, top: '50%', transform: 'translateY(-50%)', color: 'var(--dag-text-faint)', fontSize: 11 }}>UDAG</span>
                </div>
                <select value={frequency} onChange={e => setFrequency(e.target.value as Frequency)} style={{ ...S.select, minWidth: 100 }}>
                  <option value="per_second">per second</option>
                  <option value="per_minute">per minute</option>
                  <option value="per_hour">per hour</option>
                  <option value="per_day">per day</option>
                  <option value="per_week">per week</option>
                  <option value="per_month">per month</option>
                </select>
              </div>

              {/* Rate breakdown */}
              {rates && (
                <div style={{ marginTop: 8, display: 'flex', gap: 12, flexWrap: 'wrap' }}>
                  {[
                    { l: '/sec', v: rates.perSec },
                    { l: '/min', v: rates.perMin },
                    { l: '/hr', v: rates.perHour },
                    { l: '/day', v: rates.perDay },
                  ].map((r, i) => (
                    <span key={i} style={{ fontSize: 9.5, color: 'var(--dag-text-faint)', ...S.mono }}>
                      <span style={{ color: 'var(--dag-text-muted)' }}>{r.v}</span> {r.l}
                    </span>
                  ))}
                  <span style={{ fontSize: 9.5, color: 'var(--dag-text-faint)', ...S.mono }}>
                    = <span style={{ color: '#00E0C4' }}>{satsPerRound.toLocaleString()}</span> sats/round
                  </span>
                </div>
              )}
            </div>

            {/* Duration / Total Deposit toggle */}
            <div style={S.card}>
              <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 8 }}>
                <span style={S.label}>
                  {useTotalDeposit ? 'Total Deposit (auto-calculates duration)' : 'Duration'}
                </span>
                <button onClick={() => setUseTotalDeposit(!useTotalDeposit)} style={{
                  fontSize: 9, color: '#0066FF', background: 'rgba(0,102,255,0.08)', border: '1px solid rgba(0,102,255,0.15)',
                  borderRadius: 6, padding: '3px 8px', cursor: 'pointer', transition: 'all 0.2s',
                }}>
                  {useTotalDeposit ? 'Set duration instead' : 'Set total deposit instead'}
                </button>
              </div>

              {useTotalDeposit ? (
                <div style={{ position: 'relative' }}>
                  <input type="number" min="0" step="any" value={totalDepositInput}
                    onChange={e => { setTotalDepositInput(e.target.value); setFormMsg(''); }}
                    placeholder="100" style={S.input} />
                  <span style={{ position: 'absolute', right: 12, top: '50%', transform: 'translateY(-50%)', color: 'var(--dag-text-faint)', fontSize: 11 }}>UDAG</span>
                  {durationSeconds > 0 && (
                    <p style={{ fontSize: 10, color: 'var(--dag-text-faint)', marginTop: 4, ...S.mono }}>
                      Duration: {fmtDuration(durationSeconds)} ({totalRounds.toLocaleString()} rounds)
                    </p>
                  )}
                </div>
              ) : (
                <div style={{ display: 'flex', gap: 8 }}>
                  <div style={{ flex: 1 }}>
                    <input type="number" min="0" step="any" value={durAmount}
                      onChange={e => { setDurAmount(e.target.value); setFormMsg(''); }} placeholder="24" style={S.input} />
                  </div>
                  <select value={durUnit} onChange={e => setDurUnit(e.target.value as DurationUnit)} style={{ ...S.select, minWidth: 100 }}>
                    <option value="minutes">minutes</option>
                    <option value="hours">hours</option>
                    <option value="days">days</option>
                    <option value="weeks">weeks</option>
                    <option value="months">months</option>
                  </select>
                </div>
              )}
            </div>

            {/* Advanced Options */}
            <button onClick={() => setShowAdvanced(!showAdvanced)} style={{
              background: 'none', border: 'none', color: 'var(--dag-text-faint)', fontSize: 11,
              cursor: 'pointer', textAlign: 'left', padding: '4px 0',
            }}>
              {showAdvanced ? '▾' : '▸'} Advanced Options
            </button>
            {showAdvanced && (
              <div style={{ ...S.card, background: 'var(--dag-input-bg)' }}>
                <div style={{ marginBottom: 12 }}>
                  <span style={S.label}>Cliff Period (rounds)</span>
                  <input type="number" min="0" step="1" value={cliffRounds}
                    onChange={e => setCliffRounds(e.target.value)} placeholder="0" style={S.input} />
                  <p style={{ fontSize: 9, color: 'var(--dag-text-faint)', marginTop: 3 }}>
                    No funds accrue during the cliff. After cliff ends, all accrued funds become available at once.
                    {cliffRoundsNum > 0 && <span style={S.mono}> = {fmtDuration(cliffRoundsNum * 5)}</span>}
                  </p>
                </div>
              </div>
            )}
          </div>

          {/* Right: Summary + How it works */}
          <div style={{ display: 'flex', flexDirection: 'column', gap: 14 }}>

            {/* Live Summary */}
            <div style={{
              ...S.card, background: 'linear-gradient(135deg, rgba(0,224,196,0.03), rgba(0,102,255,0.02))',
              borderColor: 'rgba(0,224,196,0.12)',
            }}>
              <div style={{ fontSize: 13, fontWeight: 600, color: 'var(--dag-text-secondary)', marginBottom: 14 }}>Stream Summary</div>

              {/* Visual flow indicator */}
              <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', gap: 12, marginBottom: 16, padding: '12px 0' }}>
                <div style={{ padding: '6px 12px', borderRadius: 8, background: 'var(--dag-input-bg)', border: '1px solid var(--dag-border)', fontSize: 11, color: 'var(--dag-text-muted)' }}>
                  You
                </div>
                <div style={{ position: 'relative', width: 60, height: 2, background: 'var(--dag-border)' }}>
                  {isValid && <div style={{ position: 'absolute', width: 6, height: 6, borderRadius: '50%', background: '#00E0C4', top: -2, animation: 'flowDot 1.5s linear infinite', boxShadow: '0 0 6px #00E0C4' }} />}
                </div>
                <div style={{ padding: '6px 12px', borderRadius: 8, background: isValid ? 'rgba(0,224,196,0.06)' : 'var(--dag-input-bg)', border: `1px solid ${isValid ? 'rgba(0,224,196,0.2)' : 'var(--dag-border)'}`, fontSize: 11, color: isValid ? '#00E0C4' : 'var(--dag-text-muted)' }}>
                  {recipient ? shortAddr(recipient) : 'Recipient'}
                </div>
              </div>

              {/* Details */}
              <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
                {[
                  { l: 'Type', v: STREAM_TYPE_INFO[streamType].icon + ' ' + STREAM_TYPE_INFO[streamType].label },
                  { l: 'Rate', v: rateNum > 0 ? `${rateAmount} UDAG ${FREQ_LABELS[frequency]}` : '—' },
                  { l: 'On-chain rate', v: satsPerRound > 0 ? `${satsPerRound.toLocaleString()} sats/round` : '—' },
                  { l: 'Duration', v: durationSeconds > 0 ? fmtDuration(durationSeconds) + ` (${totalRounds.toLocaleString()} rounds)` : '—' },
                  ...(cliffRoundsNum > 0 ? [{ l: 'Cliff', v: `${cliffRoundsNum} rounds (${fmtDuration(cliffRoundsNum * 5)})` }] : []),
                ].map((row, i) => (
                  <div key={i} style={{ display: 'flex', justifyContent: 'space-between', padding: '4px 0' }}>
                    <span style={{ fontSize: 11, color: 'var(--dag-text-muted)' }}>{row.l}</span>
                    <span style={{ fontSize: 11, fontWeight: 500, color: 'var(--dag-text)', ...S.mono }}>{row.v}</span>
                  </div>
                ))}

                <div style={{ borderTop: '1px solid var(--dag-row-border)', paddingTop: 8, marginTop: 4 }}>
                  <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: 4 }}>
                    <span style={{ fontSize: 11, color: 'var(--dag-text-muted)' }}>Total Deposit</span>
                    <span style={{ fontSize: 15, fontWeight: 700, color: 'var(--dag-text)', ...S.mono }}>
                      {totalDeposit > 0 ? totalDeposit.toFixed(4) : '—'} UDAG
                    </span>
                  </div>
                  <div style={{ display: 'flex', justifyContent: 'space-between' }}>
                    <span style={{ fontSize: 10, color: 'var(--dag-text-faint)' }}>Network Fee</span>
                    <span style={{ fontSize: 10, color: 'var(--dag-text-faint)', ...S.mono }}>0.0001 UDAG</span>
                  </div>
                  <div style={{ display: 'flex', justifyContent: 'space-between', marginTop: 6, paddingTop: 6, borderTop: '1px solid var(--dag-row-border)' }}>
                    <span style={{ fontSize: 12, fontWeight: 600, color: 'var(--dag-text-muted)' }}>Total Cost</span>
                    <span style={{ fontSize: 16, fontWeight: 700, color: '#00E0C4', ...S.mono }}>
                      {totalDeposit > 0 ? (totalDeposit + MIN_FEE / SATS).toFixed(4) : '—'} UDAG
                    </span>
                  </div>
                </div>
              </div>

              {formMsg && (
                <div style={{ fontSize: 11, color: formError ? '#EF4444' : '#00E0C4', background: formError ? 'rgba(239,68,68,0.06)' : 'rgba(0,224,196,0.06)', border: `1px solid ${formError ? 'rgba(239,68,68,0.15)' : 'rgba(0,224,196,0.15)'}`, borderRadius: 8, padding: '8px 12px', marginTop: 12 }}>
                  {formMsg}
                </div>
              )}

              <button onClick={handleCreate} disabled={!isValid || submitting} style={{
                width: '100%', padding: '13px 0', borderRadius: 10, border: 'none', marginTop: 14,
                background: (!isValid || submitting) ? 'var(--dag-border)' : 'linear-gradient(135deg, #00E0C4, #0066FF)',
                color: (!isValid || submitting) ? 'var(--dag-text-faint)' : '#fff',
                fontSize: 14, fontWeight: 700, cursor: (!isValid || submitting) ? 'not-allowed' : 'pointer',
                transition: 'all 0.2s',
                boxShadow: isValid && !submitting ? '0 4px 20px rgba(0,224,196,0.15)' : 'none',
                opacity: submitting ? 0.7 : 1,
              }}>
                {submitting ? 'Creating...' : '\u224B Start Stream'}
              </button>
            </div>

            {/* How it works */}
            <div style={S.card}>
              <div style={{ fontSize: 12, fontWeight: 600, color: 'var(--dag-cell-text)', marginBottom: 10 }}>How streaming works</div>
              <div style={{ fontSize: 11, color: 'var(--dag-subheading)', lineHeight: 1.7 }}>
                <p>Streams are <strong style={{ color: 'var(--dag-text-secondary)' }}>native protocol transactions</strong> — not smart contracts. Validators compute accrued amounts at the consensus layer with zero gas overhead.</p>
                <div style={{ marginTop: 10, display: 'flex', flexDirection: 'column', gap: 6 }}>
                  {[
                    { n: '1', t: 'Deposit locks funds in the stream' },
                    { n: '2', t: 'Funds accrue to recipient every round (~5s)' },
                    { n: '3', t: 'Recipient withdraws accrued funds anytime' },
                    { n: '4', t: 'Sender can cancel and reclaim the remainder' },
                  ].map((step, i) => (
                    <div key={i} style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
                      <span style={{ width: 18, height: 18, borderRadius: '50%', background: 'rgba(0,224,196,0.08)', border: '1px solid rgba(0,224,196,0.15)', display: 'flex', alignItems: 'center', justifyContent: 'center', fontSize: 9, fontWeight: 700, color: '#00E0C4', flexShrink: 0 }}>{step.n}</span>
                      <span style={{ fontSize: 11, color: 'var(--dag-text-muted)' }}>{step.t}</span>
                    </div>
                  ))}
                </div>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* OUTGOING TAB */}
      {tab === 'outgoing' && (
        <div style={{ animation: 'slideUp 0.3s ease' }}>
          <div style={S.card}>
            {loading ? (
              <div>{[0, 1, 2].map(i => <div key={i} style={{ display: 'flex', gap: 16, padding: '12px 0' }}>{[120, 80, 100, 60, 60, 50].map((w, j) => <div key={j} style={{ width: w, height: 14, borderRadius: 4, background: 'var(--dag-input-bg)', animation: 'pulse 1.5s infinite' }} />)}</div>)}</div>
            ) : outgoing.length === 0 ? (
              <div style={{ padding: '40px 0', textAlign: 'center' }}>
                <div style={{ fontSize: 36, marginBottom: 10, opacity: 0.12 }}>↑</div>
                <p style={{ fontSize: 13, color: 'var(--dag-text-muted)' }}>No outgoing streams</p>
                <p style={{ fontSize: 11, color: 'var(--dag-text-faint)', marginTop: 4 }}>Create a stream to start sending continuous payments</p>
                <button onClick={() => setTab('create')} style={{ ...S.btn(), marginTop: 12 }}>+ Create Stream</button>
              </div>
            ) : (
              <div style={{ overflowX: 'auto', WebkitOverflowScrolling: 'touch' }}>
                <div style={{ display: 'grid', gridTemplateColumns: '2fr 1fr 2fr 1fr 1fr auto', gap: 8, padding: '0 4px', marginBottom: 8, minWidth: m ? 650 : undefined }}>
                  {['RECIPIENT', 'RATE', 'PROGRESS', 'ACCRUED', 'STATUS', ''].map((h, i) => (
                    <div key={i} style={{ fontSize: 8.5, fontWeight: 600, color: 'var(--dag-text-faint)', letterSpacing: 1.5 }}>{h}</div>
                  ))}
                </div>
                {outgoing.map(s => (
                  <div key={s.id} className="stream-row" style={{ display: 'grid', gridTemplateColumns: '2fr 1fr 2fr 1fr 1fr auto', gap: 8, alignItems: 'center', padding: '10px 4px', borderTop: '1px solid var(--dag-row-border)', borderRadius: 6, transition: 'background 0.15s', minWidth: m ? 650 : undefined }}>
                    <div style={{ fontSize: 11, color: 'var(--dag-text)', ...S.mono }}>{shortAddr(s.recipient)}</div>
                    <div style={{ fontSize: 11, color: 'var(--dag-cell-text)' }}>{s.rate_udag_per_hour?.toFixed(4) ?? '—'}/hr</div>
                    <div style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
                      <ProgressBar value={s.accrued} max={s.deposited} />
                      <span style={{ fontSize: 9, color: 'var(--dag-text-faint)', ...S.mono }}>{fmtUdag(s.accrued)} / {fmtUdag(s.deposited)}</span>
                    </div>
                    <div style={{ fontSize: 11, color: 'var(--dag-text-secondary)', ...S.mono }}>{fmtUdag(s.accrued)}</div>
                    <span style={{ fontSize: 9, fontWeight: 600, padding: '2px 8px', borderRadius: 4, background: statusBg(s.status), color: statusColor(s.status), display: 'inline-block', textAlign: 'center' }}>{s.status.toUpperCase()}</span>
                    {s.status === 'Active' && <button disabled={actionLoading === s.id} onClick={async () => {
                      const wallet = wallets[0];
                      if (!wallet?.secret_key) { alert('No wallet with secret key'); return; }
                      setActionLoading(s.id);
                      try {
                        const res = await fetch(`${getNodeUrl()}/stream/cancel`, {
                          method: 'POST',
                          headers: { 'Content-Type': 'application/json' },
                          body: JSON.stringify({ secret_key: wallet.secret_key, stream_id: s.id }),
                          signal: AbortSignal.timeout(10000),
                        });
                        const data = await res.json();
                        if (!res.ok) { alert(data.error || 'Failed to cancel stream'); return; }
                        fetchStreams();
                      } catch (err) { alert(err instanceof Error ? err.message : 'Network error'); }
                      finally { setActionLoading(null); }
                    }} style={S.btn('#EF4444')}>{actionLoading === s.id ? '...' : 'Cancel'}</button>}
                  </div>
                ))}
              </div>
            )}
          </div>
        </div>
      )}

      {/* INCOMING TAB */}
      {tab === 'incoming' && (
        <div style={{ animation: 'slideUp 0.3s ease' }}>
          <div style={S.card}>
            {loading ? (
              <div>{[0, 1, 2].map(i => <div key={i} style={{ display: 'flex', gap: 16, padding: '12px 0' }}>{[120, 80, 100, 80, 60, 50].map((w, j) => <div key={j} style={{ width: w, height: 14, borderRadius: 4, background: 'var(--dag-input-bg)', animation: 'pulse 1.5s infinite' }} />)}</div>)}</div>
            ) : incoming.length === 0 ? (
              <div style={{ padding: '40px 0', textAlign: 'center' }}>
                <div style={{ fontSize: 36, marginBottom: 10, opacity: 0.12 }}>↓</div>
                <p style={{ fontSize: 13, color: 'var(--dag-text-muted)' }}>No incoming streams</p>
                <p style={{ fontSize: 11, color: 'var(--dag-text-faint)', marginTop: 4 }}>Share your address to receive continuous payments</p>
              </div>
            ) : (
              <div style={{ overflowX: 'auto', WebkitOverflowScrolling: 'touch' }}>
                <div style={{ display: 'grid', gridTemplateColumns: '2fr 1fr 1.5fr 1.5fr 1fr auto', gap: 8, padding: '0 4px', marginBottom: 8, minWidth: m ? 650 : undefined }}>
                  {['SENDER', 'RATE', 'ACCRUED', 'WITHDRAWABLE', 'STATUS', ''].map((h, i) => (
                    <div key={i} style={{ fontSize: 8.5, fontWeight: 600, color: 'var(--dag-text-faint)', letterSpacing: 1.5 }}>{h}</div>
                  ))}
                </div>
                {incoming.map(s => (
                  <div key={s.id} className="stream-row" style={{ display: 'grid', gridTemplateColumns: '2fr 1fr 1.5fr 1.5fr 1fr auto', gap: 8, alignItems: 'center', padding: '10px 4px', borderTop: '1px solid var(--dag-row-border)', borderRadius: 6, transition: 'background 0.15s', minWidth: m ? 650 : undefined }}>
                    <div style={{ fontSize: 11, color: 'var(--dag-text)', ...S.mono }}>{shortAddr(s.sender)}</div>
                    <div style={{ fontSize: 11, color: 'var(--dag-cell-text)' }}>{s.rate_udag_per_hour?.toFixed(4) ?? '—'}/hr</div>
                    <div style={{ fontSize: 13, fontWeight: 600, color: 'var(--dag-text)', ...S.mono }}>{fmtUdag(s.accrued)}</div>
                    <div>
                      {s.withdrawable > 0 ? (
                        <span style={{ ...S.mono, color: '#00E0C4', fontSize: 13, fontWeight: 600, animation: 'streamPulse 2s ease-in-out infinite' }}>
                          {fmtUdag(s.withdrawable)} <span style={{ fontSize: 10, color: 'rgba(0,224,196,0.5)' }}>UDAG</span>
                        </span>
                      ) : (
                        <span style={{ fontSize: 11, color: 'var(--dag-text-faint)', ...S.mono }}>0.00</span>
                      )}
                    </div>
                    <span style={{ fontSize: 9, fontWeight: 600, padding: '2px 8px', borderRadius: 4, background: statusBg(s.status), color: statusColor(s.status), display: 'inline-block', textAlign: 'center' }}>{s.status.toUpperCase()}</span>
                    {s.withdrawable > 0 && <button disabled={actionLoading === s.id} onClick={async () => {
                      const wallet = wallets[0];
                      if (!wallet?.secret_key) { alert('No wallet with secret key'); return; }
                      setActionLoading(s.id);
                      try {
                        const res = await fetch(`${getNodeUrl()}/stream/withdraw`, {
                          method: 'POST',
                          headers: { 'Content-Type': 'application/json' },
                          body: JSON.stringify({ secret_key: wallet.secret_key, stream_id: s.id }),
                          signal: AbortSignal.timeout(10000),
                        });
                        const data = await res.json();
                        if (!res.ok) { alert(data.error || 'Failed to withdraw'); return; }
                        fetchStreams();
                      } catch (err) { alert(err instanceof Error ? err.message : 'Network error'); }
                      finally { setActionLoading(null); }
                    }} style={S.btn('#00E0C4')}>{actionLoading === s.id ? '...' : 'Withdraw'}</button>}
                  </div>
                ))}
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
