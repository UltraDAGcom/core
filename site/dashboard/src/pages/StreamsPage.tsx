import { useState, useEffect, useCallback, useRef } from 'react';
import { getNodeUrl, formatUdag, shortAddr } from '../lib/api';
import type { NetworkType } from '../lib/api';

const SATS = 100_000_000;
const ROUNDS_PER_HOUR = 720; // 3600s / 5s per round
const MIN_FEE = 10_000; // 0.0001 UDAG

interface Stream {
  id: string;
  sender: string;
  recipient: string;
  rate_per_round: number;
  deposited: number;
  accrued: number;
  withdrawable: number;
  start_round: number;
  end_round: number;
  status: 'Active' | 'Cancelled' | 'Depleted';
}

interface StreamsPageProps {
  wallets: Array<{ name: string; address: string; secret_key: string }>;
  network: NetworkType;
}

const S = {
  card: { background: 'var(--dag-card)', border: '1px solid var(--dag-border)', borderRadius: 14, padding: '20px 22px' } as React.CSSProperties,
  stat: { background: 'var(--dag-card)', borderRadius: 10, padding: '12px 14px' } as React.CSSProperties,
  label: { fontSize: 10.5, fontWeight: 600, color: 'var(--dag-text-muted)', letterSpacing: 1.2, textTransform: 'uppercase' as const, marginBottom: 6, display: 'block' },
  input: { width: '100%', padding: '12px 14px', borderRadius: 10, background: 'var(--dag-input-bg)', border: '1px solid var(--dag-border)', color: 'var(--dag-text)', fontSize: 15, outline: 'none', fontFamily: "'DM Mono',monospace", boxSizing: 'border-box' as const } as React.CSSProperties,
  btn: (c = '#00E0C4') => ({ padding: '6px 14px', borderRadius: 8, background: `${c}12`, border: `1px solid ${c}25`, color: c, fontSize: 11, fontWeight: 600, cursor: 'pointer', transition: 'opacity 0.2s' }),
  mono: { fontFamily: "'DM Mono',monospace" },
};

function statusColor(status: string): string {
  if (status === 'Active') return '#00E0C4';
  if (status === 'Depleted') return '#FFB800';
  return '#EF4444';
}

function statusBg(status: string): string {
  if (status === 'Active') return 'rgba(0,224,196,0.08)';
  if (status === 'Depleted') return 'rgba(255,184,0,0.08)';
  return 'rgba(239,68,68,0.08)';
}

function rateToUdagPerHour(ratePerRound: number): string {
  const perHour = (ratePerRound * ROUNDS_PER_HOUR) / SATS;
  return perHour.toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 4 });
}

function ProgressBar({ value, max }: { value: number; max: number }) {
  const pct = max > 0 ? Math.min(100, (value / max) * 100) : 0;
  return (
    <div style={{ position: 'relative', height: 6, borderRadius: 3, background: 'var(--dag-card-hover)', overflow: 'hidden', minWidth: 80 }}>
      <div style={{
        height: '100%', borderRadius: 3, width: `${pct}%`,
        background: 'linear-gradient(90deg, #00E0C4, #0066FF)',
        transition: 'width 0.5s ease',
      }} />
      <span style={{
        position: 'absolute', right: 0, top: -16, fontSize: 9,
        color: 'var(--dag-text-muted)', fontFamily: "'DM Mono',monospace",
      }}>{pct.toFixed(1)}%</span>
    </div>
  );
}

function PulsingValue({ value, suffix = '' }: { value: string; suffix?: string }) {
  return (
    <span style={{ ...S.mono, color: '#00E0C4', fontSize: 13, fontWeight: 600, animation: 'streamPulse 2s ease-in-out infinite' }}>
      {value}{suffix && <span style={{ fontSize: 10, color: 'rgba(0,224,196,0.5)', marginLeft: 3 }}>{suffix}</span>}
    </span>
  );
}

function SkeletonRow() {
  return (
    <div style={{ display: 'flex', gap: 16, padding: '12px 0' }}>
      {[120, 80, 100, 60, 60, 50].map((w, i) => (
        <div key={i} style={{ width: w, height: 14, borderRadius: 4, background: 'var(--dag-input-bg)', animation: 'pulse 1.5s ease-in-out infinite' }} />
      ))}
    </div>
  );
}

export function StreamsPage({ wallets, network }: StreamsPageProps) {
  const [loading, setLoading] = useState(true);
  const [allStreams, setAllStreams] = useState<Stream[]>([]);
  const [recipient, setRecipient] = useState('');
  const [ratePerHour, setRatePerHour] = useState('');
  const [duration, setDuration] = useState('');
  const [formMsg, setFormMsg] = useState('');
  const tickRef = useRef(0);
  const [, setTick] = useState(0);

  const myAddresses = wallets.map(w => w.address.toLowerCase());

  const outgoing = allStreams.filter(s => myAddresses.includes(s.sender.toLowerCase()));
  const incoming = allStreams.filter(s => myAddresses.includes(s.recipient.toLowerCase()));
  const activeCount = allStreams.filter(s => s.status === 'Active').length;
  const totalStreaming = allStreams.filter(s => s.status === 'Active').reduce((sum, s) => sum + s.deposited, 0);
  const myCount = new Set([...outgoing.map(s => s.id), ...incoming.map(s => s.id)]).size;

  const fetchStreams = useCallback(async () => {
    try {
      // Try fetching streams for each wallet address
      const streams: Stream[] = [];
      const seen = new Set<string>();
      for (const addr of myAddresses) {
        try {
          const res = await fetch(`${getNodeUrl()}/streams/${addr}`, { signal: AbortSignal.timeout(5000) });
          if (res.ok) {
            const data = await res.json();
            const list: Stream[] = Array.isArray(data) ? data : (data.streams ?? []);
            for (const s of list) {
              if (!seen.has(s.id)) { seen.add(s.id); streams.push(s); }
            }
          }
        } catch { /* endpoint may not exist yet */ }
      }
      // Also try global streams endpoint
      try {
        const res = await fetch(`${getNodeUrl()}/streams`, { signal: AbortSignal.timeout(5000) });
        if (res.ok) {
          const data = await res.json();
          const list: Stream[] = Array.isArray(data) ? data : (data.streams ?? []);
          for (const s of list) {
            if (!seen.has(s.id)) { seen.add(s.id); streams.push(s); }
          }
        }
      } catch { /* endpoint may not exist yet */ }
      setAllStreams(streams);
    } catch { /* ignore */ }
    setLoading(false);
  }, [myAddresses.join(',')]);

  useEffect(() => { fetchStreams(); const iv = setInterval(fetchStreams, 5000); return () => clearInterval(iv); }, [fetchStreams]);

  useEffect(() => {
    const handler = () => { setAllStreams([]); setLoading(true); fetchStreams(); };
    window.addEventListener('ultradag-network-switch', handler);
    return () => window.removeEventListener('ultradag-network-switch', handler);
  }, [fetchStreams]);

  // Real-time tick for animated values
  useEffect(() => {
    const iv = setInterval(() => { tickRef.current++; setTick(t => t + 1); }, 1000);
    return () => clearInterval(iv);
  }, []);

  const computedRate = parseFloat(ratePerHour) || 0;
  const computedDuration = parseFloat(duration) || 0;
  const totalDeposit = computedRate * computedDuration;
  const satsPerRound = Math.floor(computedRate * SATS / ROUNDS_PER_HOUR);

  const handleCreate = () => {
    if (!recipient) { setFormMsg('Enter a recipient address'); return; }
    if (computedRate <= 0) { setFormMsg('Rate must be greater than 0'); return; }
    if (computedDuration <= 0) { setFormMsg('Duration must be greater than 0'); return; }
    if (totalDeposit < 0.0001) { setFormMsg('Total deposit too small'); return; }
    alert('Coming soon \u2014 submit via /tx/submit with client-side signing.\n\nStream parameters:\n' +
      `Recipient: ${recipient}\n` +
      `Rate: ${satsPerRound} sats/round (${computedRate} UDAG/hr)\n` +
      `Duration: ${computedDuration}h (${Math.ceil(computedDuration * ROUNDS_PER_HOUR)} rounds)\n` +
      `Total deposit: ${totalDeposit.toFixed(4)} UDAG\n` +
      `Fee: 0.0001 UDAG`);
  };

  return (
    <div style={{ padding: '18px 26px', fontFamily: "'DM Sans',sans-serif" }}>
      <style>{`
        @keyframes slideUp{from{opacity:0;transform:translateY(10px)}to{opacity:1;transform:translateY(0)}}
        @keyframes streamPulse{0%,100%{opacity:1;text-shadow:0 0 6px rgba(0,224,196,0.2)}50%{opacity:0.75;text-shadow:0 0 12px rgba(0,224,196,0.4)}}
        @keyframes pulse{0%,100%{opacity:0.4}50%{opacity:0.15}}
        input:focus,select:focus{border-color:rgba(0,224,196,0.3)!important}
        .stream-row:hover{background:var(--dag-card)!important}
      `}</style>

      {/* Header */}
      <div style={{ marginBottom: 22, animation: 'slideUp 0.3s ease' }}>
        <h1 style={{ fontSize: 21, fontWeight: 700, color: 'var(--dag-text)', display: 'flex', alignItems: 'center', gap: 10 }}>
          <span style={{ background: 'linear-gradient(135deg, #00E0C4, #0066FF)', WebkitBackgroundClip: 'text', WebkitTextFillColor: 'transparent' }}>\u224B</span>
          Streams
        </h1>
        <p style={{ fontSize: 11.5, color: 'var(--dag-subheading)', marginTop: 2 }}>Continuous payment streams \u2014 money flows in real-time</p>
      </div>

      {/* Stats Row */}
      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(3,1fr)', gap: 12, marginBottom: 18, animation: 'slideUp 0.4s ease' }}>
        {[
          { l: 'ACTIVE STREAMS', v: String(activeCount), c: '#00E0C4', i: '\u224B' },
          { l: 'TOTAL STREAMING', v: formatUdag(totalStreaming) + ' UDAG', c: '#0066FF', i: '\u25C8' },
          { l: 'YOUR STREAMS', v: String(myCount), c: '#A855F7', i: '\u25C7' },
        ].map((s, i) => (
          <div key={i} style={S.card}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 6, marginBottom: 8 }}>
              <span style={{ color: s.c, fontSize: 14 }}>{s.i}</span>
              <span style={{ fontSize: 9.5, color: 'var(--dag-text-muted)', letterSpacing: 1.2 }}>{s.l}</span>
            </div>
            <div style={{ fontSize: 22, fontWeight: 700, color: 'var(--dag-text)', ...S.mono }}>{loading ? '\u2014' : s.v}</div>
          </div>
        ))}
      </div>

      <div style={{ display: 'grid', gridTemplateColumns: '2fr 3fr', gap: 16, animation: 'slideUp 0.5s ease' }}>
        {/* Left Column: Create Stream + How it Works */}
        <div style={{ display: 'flex', flexDirection: 'column', gap: 14 }}>
          {/* Create Stream Card */}
          <div style={{
            ...S.card, background: 'linear-gradient(135deg, rgba(0,224,196,0.03), rgba(0,102,255,0.02))',
            borderColor: 'rgba(0,224,196,0.12)',
          }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 7, marginBottom: 16 }}>
              <span style={{ color: '#00E0C4', fontSize: 16 }}>\u224B</span>
              <span style={{ fontSize: 14, fontWeight: 600, color: 'var(--dag-text-secondary)' }}>Create Stream</span>
            </div>

            <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
              <div>
                <span style={S.label}>Recipient</span>
                <input type="text" value={recipient} onChange={e => { setRecipient(e.target.value); setFormMsg(''); }}
                  placeholder="udag1... or hex address" style={S.input} />
              </div>

              <div>
                <span style={S.label}>Rate</span>
                <div style={{ position: 'relative' }}>
                  <input type="number" min="0" step="0.01" value={ratePerHour}
                    onChange={e => { setRatePerHour(e.target.value); setFormMsg(''); }} placeholder="0.50" style={S.input} />
                  <span style={{ position: 'absolute', right: 14, top: '50%', transform: 'translateY(-50%)', color: 'var(--dag-text-faint)', fontSize: 12 }}>UDAG/hr</span>
                </div>
                {satsPerRound > 0 && (
                  <p style={{ fontSize: 9.5, color: 'var(--dag-text-faint)', marginTop: 3, ...S.mono }}>{satsPerRound.toLocaleString()} sats/round</p>
                )}
              </div>

              <div>
                <span style={S.label}>Duration</span>
                <div style={{ position: 'relative' }}>
                  <input type="number" min="0" step="1" value={duration}
                    onChange={e => { setDuration(e.target.value); setFormMsg(''); }} placeholder="24" style={S.input} />
                  <span style={{ position: 'absolute', right: 14, top: '50%', transform: 'translateY(-50%)', color: 'var(--dag-text-faint)', fontSize: 12 }}>hours</span>
                </div>
              </div>

              {/* Computed Summary */}
              {totalDeposit > 0 && (
                <div style={{ ...S.stat, display: 'flex', flexDirection: 'column', gap: 6 }}>
                  <div style={{ display: 'flex', justifyContent: 'space-between' }}>
                    <span style={{ fontSize: 10, color: 'var(--dag-subheading)' }}>Total Deposit</span>
                    <span style={{ fontSize: 13, fontWeight: 700, color: 'var(--dag-text)', ...S.mono }}>{totalDeposit.toFixed(4)} UDAG</span>
                  </div>
                  <div style={{ display: 'flex', justifyContent: 'space-between' }}>
                    <span style={{ fontSize: 10, color: 'var(--dag-subheading)' }}>Fee</span>
                    <span style={{ fontSize: 11, color: 'var(--dag-text-muted)', ...S.mono }}>0.0001 UDAG</span>
                  </div>
                  <div style={{ borderTop: '1px solid var(--dag-table-border)', paddingTop: 6, display: 'flex', justifyContent: 'space-between' }}>
                    <span style={{ fontSize: 10, color: 'var(--dag-subheading)' }}>Total Cost</span>
                    <span style={{ fontSize: 13, fontWeight: 700, color: '#00E0C4', ...S.mono }}>{(totalDeposit + MIN_FEE / SATS).toFixed(4)} UDAG</span>
                  </div>
                </div>
              )}

              {formMsg && (
                <div style={{ fontSize: 11, color: '#FFB800', background: 'rgba(255,184,0,0.06)', border: '1px solid rgba(255,184,0,0.15)', borderRadius: 8, padding: '8px 12px' }}>
                  {formMsg}
                </div>
              )}

              <button onClick={handleCreate} disabled={!recipient || computedRate <= 0 || computedDuration <= 0}
                style={{
                  width: '100%', padding: '12px 0', borderRadius: 10, border: 'none',
                  background: (!recipient || computedRate <= 0 || computedDuration <= 0) ? 'var(--dag-border)' : 'linear-gradient(135deg, #00E0C4, #0066FF)',
                  color: (!recipient || computedRate <= 0 || computedDuration <= 0) ? 'var(--dag-text-faint)' : '#fff',
                  fontSize: 13, fontWeight: 700, cursor: (!recipient || computedRate <= 0 || computedDuration <= 0) ? 'not-allowed' : 'pointer',
                  transition: 'all 0.2s',
                  boxShadow: (recipient && computedRate > 0 && computedDuration > 0) ? '0 4px 20px rgba(0,224,196,0.15)' : 'none',
                }}>
                \u224B Start Stream
              </button>
            </div>
          </div>

          {/* How it Works */}
          <div style={S.card}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 7, marginBottom: 10 }}>
              <span style={{ color: '#0066FF', fontSize: 13 }}>\u25C8</span>
              <span style={{ fontSize: 12, fontWeight: 600, color: 'var(--dag-cell-text)' }}>How streams work</span>
            </div>
            <div style={{ fontSize: 11, color: 'var(--dag-subheading)', lineHeight: 1.7 }}>
              <p>Payment streams continuously transfer UDAG from sender to recipient every round (~5 seconds). The sender deposits upfront, and the recipient can withdraw accrued funds at any time.</p>
              <p style={{ marginTop: 6 }}>Senders can cancel an active stream to reclaim unstreamed funds. Streams automatically stop when the deposit is fully streamed.</p>
              <div style={{ marginTop: 10, display: 'flex', flexDirection: 'column', gap: 4 }}>
                {[
                  { icon: '\u25B6', text: 'Deposit locks funds, streaming begins' },
                  { icon: '\u25CE', text: 'Funds accrue to recipient each round' },
                  { icon: '\u2193', text: 'Recipient withdraws anytime' },
                  { icon: '\u25A0', text: 'Sender can cancel, reclaiming remainder' },
                ].map((step, i) => (
                  <div key={i} style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                    <span style={{ color: '#00E0C4', fontSize: 8 }}>{step.icon}</span>
                    <span style={{ fontSize: 10.5, color: 'var(--dag-text-muted)' }}>{step.text}</span>
                  </div>
                ))}
              </div>
            </div>
          </div>
        </div>

        {/* Right Column: Outgoing + Incoming */}
        <div style={{ display: 'flex', flexDirection: 'column', gap: 14 }}>

          {/* Outgoing Streams */}
          <div style={S.card}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 7, marginBottom: 14 }}>
              <span style={{ color: '#0066FF', fontSize: 14 }}>\u2191</span>
              <span style={{ fontSize: 13.5, fontWeight: 600, color: 'var(--dag-text-secondary)' }}>Outgoing Streams</span>
              {outgoing.length > 0 && (
                <span style={{ fontSize: 9, background: 'rgba(0,102,255,0.12)', color: '#0066FF', padding: '1px 6px', borderRadius: 4, fontWeight: 600 }}>{outgoing.length}</span>
              )}
            </div>

            {loading ? (
              <div>{[0, 1].map(i => <SkeletonRow key={i} />)}</div>
            ) : outgoing.length === 0 ? (
              <div style={{ padding: '24px 0', textAlign: 'center' }}>
                <div style={{ fontSize: 28, marginBottom: 8, opacity: 0.15 }}>\u224B</div>
                <p style={{ fontSize: 12, color: 'var(--dag-text-faint)' }}>No outgoing streams</p>
                <p style={{ fontSize: 10, color: 'var(--dag-text-faint)', marginTop: 4 }}>Create a stream to start sending continuous payments</p>
              </div>
            ) : (
              <div>
                {/* Header */}
                <div style={{ display: 'grid', gridTemplateColumns: '2fr 1fr 2fr 1fr 1fr auto', gap: 8, padding: '0 4px', marginBottom: 6 }}>
                  {['RECIPIENT', 'RATE', 'PROGRESS', 'ACCRUED', 'STATUS', ''].map((h, i) => (
                    <div key={i} style={{ fontSize: 8.5, fontWeight: 600, color: 'var(--dag-text-faint)', letterSpacing: 1.5 }}>{h}</div>
                  ))}
                </div>
                {outgoing.map(s => (
                  <div key={s.id} className="stream-row" style={{ display: 'grid', gridTemplateColumns: '2fr 1fr 2fr 1fr 1fr auto', gap: 8, alignItems: 'center', padding: '10px 4px', borderTop: '1px solid var(--dag-row-border)', borderRadius: 6, transition: 'background 0.15s' }}>
                    <div style={{ fontSize: 11, color: 'var(--dag-text)', ...S.mono }}>{shortAddr(s.recipient)}</div>
                    <div style={{ fontSize: 11, color: 'var(--dag-cell-text)' }}>{rateToUdagPerHour(s.rate_per_round)}/hr</div>
                    <div style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
                      <ProgressBar value={s.accrued} max={s.deposited} />
                      <span style={{ fontSize: 9, color: 'var(--dag-text-faint)', ...S.mono }}>{formatUdag(s.accrued)} / {formatUdag(s.deposited)}</span>
                    </div>
                    <div style={{ fontSize: 11, color: 'var(--dag-text-secondary)', ...S.mono }}>{formatUdag(s.accrued)}</div>
                    <span style={{ fontSize: 9, fontWeight: 600, padding: '2px 8px', borderRadius: 4, background: statusBg(s.status), color: statusColor(s.status), display: 'inline-block', textAlign: 'center' }}>
                      {s.status.toUpperCase()}
                    </span>
                    {s.status === 'Active' && (
                      <button onClick={() => alert('Coming soon \u2014 cancel via /tx/submit')} style={S.btn('#EF4444')}>Cancel</button>
                    )}
                  </div>
                ))}
              </div>
            )}
          </div>

          {/* Incoming Streams */}
          <div style={S.card}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 7, marginBottom: 14 }}>
              <span style={{ color: '#00E0C4', fontSize: 14 }}>\u2193</span>
              <span style={{ fontSize: 13.5, fontWeight: 600, color: 'var(--dag-text-secondary)' }}>Incoming Streams</span>
              {incoming.length > 0 && (
                <span style={{ fontSize: 9, background: 'rgba(0,224,196,0.12)', color: '#00E0C4', padding: '1px 6px', borderRadius: 4, fontWeight: 600 }}>{incoming.length}</span>
              )}
            </div>

            {loading ? (
              <div>{[0, 1].map(i => <SkeletonRow key={i} />)}</div>
            ) : incoming.length === 0 ? (
              <div style={{ padding: '24px 0', textAlign: 'center' }}>
                <div style={{ fontSize: 28, marginBottom: 8, opacity: 0.15 }}>\u2193</div>
                <p style={{ fontSize: 12, color: 'var(--dag-text-faint)' }}>No incoming streams</p>
                <p style={{ fontSize: 10, color: 'var(--dag-text-faint)', marginTop: 4 }}>Share your address to receive continuous payments</p>
              </div>
            ) : (
              <div>
                {/* Header */}
                <div style={{ display: 'grid', gridTemplateColumns: '2fr 1fr 1.5fr 1.5fr 1fr auto', gap: 8, padding: '0 4px', marginBottom: 6 }}>
                  {['SENDER', 'RATE', 'ACCRUED', 'WITHDRAWABLE', 'STATUS', ''].map((h, i) => (
                    <div key={i} style={{ fontSize: 8.5, fontWeight: 600, color: 'var(--dag-text-faint)', letterSpacing: 1.5 }}>{h}</div>
                  ))}
                </div>
                {incoming.map(s => (
                  <div key={s.id} className="stream-row" style={{ display: 'grid', gridTemplateColumns: '2fr 1fr 1.5fr 1.5fr 1fr auto', gap: 8, alignItems: 'center', padding: '10px 4px', borderTop: '1px solid var(--dag-row-border)', borderRadius: 6, transition: 'background 0.15s' }}>
                    <div style={{ fontSize: 11, color: 'var(--dag-text)', ...S.mono }}>{shortAddr(s.sender)}</div>
                    <div style={{ fontSize: 11, color: 'var(--dag-cell-text)' }}>{rateToUdagPerHour(s.rate_per_round)}/hr</div>
                    <div style={{ fontSize: 13, fontWeight: 600, color: 'var(--dag-text)', ...S.mono }}>{formatUdag(s.accrued)}</div>
                    <div>
                      {s.withdrawable > 0 ? (
                        <PulsingValue value={formatUdag(s.withdrawable)} suffix="UDAG" />
                      ) : (
                        <span style={{ fontSize: 11, color: 'var(--dag-text-faint)', ...S.mono }}>0.00</span>
                      )}
                    </div>
                    <span style={{ fontSize: 9, fontWeight: 600, padding: '2px 8px', borderRadius: 4, background: statusBg(s.status), color: statusColor(s.status), display: 'inline-block', textAlign: 'center' }}>
                      {s.status.toUpperCase()}
                    </span>
                    {s.withdrawable > 0 && (
                      <button onClick={() => alert('Coming soon \u2014 withdraw via /tx/submit')} style={S.btn('#00E0C4')}>Withdraw</button>
                    )}
                  </div>
                ))}
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
