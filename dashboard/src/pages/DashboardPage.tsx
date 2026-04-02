import { useState, useEffect, useRef, useCallback } from 'react';
import { Link } from 'react-router-dom';
import { getHealthDetailed, getRound } from '../lib/api';
import { getPasskeyWallet } from '../lib/passkey-wallet';
import { useIsMobile } from '../hooks/useIsMobile';
import type { NodeStatus } from '../hooks/useNode';

interface DashboardPageProps {
  status: NodeStatus | null;
  loading: boolean;
  network: string;
  wallets?: { name: string; address: string }[];
  totalBalance?: number;
  totalStaked?: number;
  totalDelegated?: number;
}

interface HealthData {
  status: string;
  components: {
    dag: { available: boolean; current_round: number; pruning_floor: number; tips_count: number; vertex_count: number };
    finality: { available: boolean; finality_lag: number; last_finalized_round: number; validator_count: number };
    mempool: { available: boolean; transaction_count: number };
    network: { peer_count: number; sync_complete: boolean };
    state: { account_count: number; active_validators: number; available: boolean; total_supply: number };
    checkpoints: { checkpoint_age_seconds: number; disk_count: number; last_checkpoint_round: number; pending_checkpoints: number };
  };
  warnings: string[];
}

interface RoundData {
  round: number;
  vertices: { hash: string; validator: string; tx_count: number; parent_count: number }[];
}

const MAX_SUPPLY = 21_000_000;
const SATS = 100_000_000;

function formatUptime(seconds: number): string {
  const d = Math.floor(seconds / 86400);
  const h = Math.floor((seconds % 86400) / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  if (d > 0) return `${d}d ${h}h ${m}m`;
  if (h > 0) return `${h}h ${m}m`;
  return `${m}m`;
}

// ── Animated Counter ─────────────────────────────────────────────────────
function AnimCounter({ target, decimals = 0, suffix = '' }: { target: number; decimals?: number; suffix?: string }) {
  const [val, setVal] = useState(0);
  const ref = useRef<number>(0);
  useEffect(() => {
    const t0 = performance.now();
    const run = (now: number) => {
      const p = Math.min((now - t0) / 1200, 1);
      setVal(target * (1 - Math.pow(1 - p, 3)));
      if (p < 1) ref.current = requestAnimationFrame(run);
    };
    ref.current = requestAnimationFrame(run);
    return () => cancelAnimationFrame(ref.current);
  }, [target]);
  return <span>{val.toLocaleString(undefined, { minimumFractionDigits: decimals, maximumFractionDigits: decimals })}{suffix}</span>;
}

// ── DAG Visualization (real data) ────────────────────────────────────────

// Stable color per validator address
const VALIDATOR_COLORS = ['#00E0C4', '#0066FF', '#A855F7', '#FFB800', '#34d399', '#f472b6', '#60a5fa', '#fbbf24', '#c084fc', '#fb923c'];
function validatorColor(addr: string): string {
  let h = 0;
  for (let i = 0; i < addr.length; i++) h = ((h << 5) - h + addr.charCodeAt(i)) | 0;
  return VALIDATOR_COLORS[Math.abs(h) % VALIDATOR_COLORS.length];
}

function DagVis({ roundData }: { roundData: RoundData[] }) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const frameRef = useRef<number>(0);
  const timeRef = useRef(0);
  const sceneRef = useRef<{ nodes: { x: number; y: number; col: number; ph: number; sz: number; hasTx: boolean; color: string }[]; edges: { from: number; to: number; sp: number }[] }>({ nodes: [], edges: [] });

  // Build scene from round data whenever it changes
  useEffect(() => {
    const data = roundData.length > 0 ? roundData : null;
    if (!data) return;

    const W = 540, H = 260;
    const rounds = data.length;
    const nodes: typeof sceneRef.current.nodes = [];
    const edges: typeof sceneRef.current.edges = [];

    for (let r = 0; r < rounds; r++) {
      const verts = data[r].vertices;
      const count = verts.length || 1;
      for (let i = 0; i < count; i++) {
        const v = verts[i];
        const x = 36 + (r / Math.max(rounds - 1, 1)) * (W - 72);
        const y = (H / (count + 1)) * (i + 1);
        nodes.push({
          x, y, col: r, ph: Math.random() * 6.28,
          sz: v && v.tx_count > 0 ? 5 : 3 + Math.random() * 1.5,
          hasTx: v ? v.tx_count > 0 : false,
          color: v ? validatorColor(v.validator) : '#00E0C4',
        });
      }
    }

    // Build edges: each vertex connects to parent_count vertices from the previous round
    let offset = 0;
    for (let r = 0; r < rounds; r++) {
      const count = data[r].vertices.length || 1;
      if (r > 0) {
        const prevOffset = offset - (data[r - 1].vertices.length || 1);
        const prevCount = data[r - 1].vertices.length || 1;
        for (let i = 0; i < count; i++) {
          const v = data[r].vertices[i];
          const parentCount = Math.min(v?.parent_count ?? 2, prevCount);
          // Deterministic parent selection based on vertex hash
          const indices = Array.from({ length: prevCount }, (_, k) => k);
          // Shuffle deterministically using hash chars
          const hash = v?.hash || '';
          for (let j = indices.length - 1; j > 0; j--) {
            const seed = hash.charCodeAt(j % hash.length) || (i * 7 + j * 13);
            const k = seed % (j + 1);
            [indices[j], indices[k]] = [indices[k], indices[j]];
          }
          for (let p = 0; p < parentCount; p++) {
            edges.push({ from: prevOffset + indices[p], to: offset + i, sp: 0.3 + (((hash.charCodeAt(p) || 50) % 50) / 100) });
          }
        }
      }
      offset += count;
    }

    sceneRef.current = { nodes, edges };
  }, [roundData]);

  // Animation loop — same visual style as before
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;
    const W = 540, H = 260;
    canvas.width = W * 2; canvas.height = H * 2;
    ctx.scale(2, 2);

    const draw = () => {
      timeRef.current += 0.016;
      const t = timeRef.current;
      const { nodes, edges } = sceneRef.current;
      ctx.clearRect(0, 0, W, H);

      // Draw edges with flowing particles
      for (const e of edges) {
        const from = nodes[e.from], to = nodes[e.to];
        if (!from || !to) continue;
        ctx.beginPath(); ctx.moveTo(from.x, from.y); ctx.lineTo(to.x, to.y);
        ctx.strokeStyle = 'rgba(0,224,196,0.07)'; ctx.lineWidth = 1; ctx.stroke();
        const pr = (t * e.sp) % 1;
        ctx.beginPath(); ctx.arc(from.x + (to.x - from.x) * pr, from.y + (to.y - from.y) * pr, 1.5, 0, 6.28);
        ctx.fillStyle = `rgba(0,224,196,${0.6 - pr * 0.4})`; ctx.fill();
      }

      // Draw nodes with glow
      const totalCols = nodes.length > 0 ? Math.max(...nodes.map(n => n.col)) + 1 : 1;
      for (const n of nodes) {
        const ps = Math.sin(t * 2 + n.ph) * 1.5;
        const g = ctx.createRadialGradient(n.x, n.y, 0, n.x, n.y, n.sz + 8 + ps);
        const c = n.color;
        // Parse hex color for glow
        const r = parseInt(c.slice(1, 3), 16), gr = parseInt(c.slice(3, 5), 16), b = parseInt(c.slice(5, 7), 16);
        g.addColorStop(0, `rgba(${r},${gr},${b},0.25)`); g.addColorStop(1, `rgba(${r},${gr},${b},0)`);
        ctx.beginPath(); ctx.arc(n.x, n.y, n.sz + 8 + ps, 0, 6.28); ctx.fillStyle = g; ctx.fill();
        ctx.beginPath(); ctx.arc(n.x, n.y, n.sz, 0, 6.28);
        ctx.fillStyle = n.col >= totalCols - 2 ? c : `rgba(${r},${gr},${b},0.55)`; ctx.fill();

        // Highlight vertices with transactions
        if (n.hasTx) {
          ctx.beginPath(); ctx.arc(n.x, n.y, n.sz + 3, 0, 6.28);
          ctx.strokeStyle = `rgba(255,184,0,${0.4 + Math.sin(t * 3 + n.ph) * 0.2})`;
          ctx.lineWidth = 1; ctx.stroke();
        }
      }

      frameRef.current = requestAnimationFrame(draw);
    };
    frameRef.current = requestAnimationFrame(draw);
    return () => cancelAnimationFrame(frameRef.current);
  }, []);

  return <canvas ref={canvasRef} style={{ width: '100%', height: 260, borderRadius: 12 }} />;
}

// ── Ring Chart ────────────────────────────────────────────────────────────
function RingChart({ value, max, size = 120, sw = 8 }: { value: number; max: number; size?: number; sw?: number }) {
  const r = (size - sw) / 2, circ = 2 * Math.PI * r, pct = Math.min(value / max, 1);
  const [a, setA] = useState(0);
  useEffect(() => {
    const t0 = performance.now();
    const run = (now: number) => { const p = Math.min((now - t0) / 1000, 1); setA(pct * (1 - Math.pow(1 - p, 3))); if (p < 1) requestAnimationFrame(run); };
    requestAnimationFrame(run);
  }, [pct]);
  return (
    <svg width={size} height={size} style={{ transform: 'rotate(-90deg)' }}>
      <circle cx={size / 2} cy={size / 2} r={r} fill="none" stroke="var(--dag-border)" strokeWidth={sw} />
      <circle cx={size / 2} cy={size / 2} r={r} fill="none" stroke="#00E0C4" strokeWidth={sw}
        strokeDasharray={circ} strokeDashoffset={circ * (1 - a)} strokeLinecap="round"
        style={{ filter: 'drop-shadow(0 0 6px rgba(0,224,196,0.3))' }} />
    </svg>
  );
}

// ── Spark ────────────────────────────────────────────────────────────────
function Spark({ data, color = '#00E0C4', w = 90, h = 24 }: { data: number[]; color?: string; w?: number; h?: number }) {
  const mx = Math.max(...data, 1);
  const pts = data.map((v, i) => `${(i / (data.length - 1)) * w},${h - (v / mx) * (h - 4)}`).join(' ');
  const gid = `s${color.replace('#', '')}`;
  return (
    <svg width={w} height={h} style={{ overflow: 'visible' }}>
      <defs><linearGradient id={gid} x1="0" y1="0" x2="0" y2="1"><stop offset="0%" stopColor={color} stopOpacity="0.25" /><stop offset="100%" stopColor={color} stopOpacity="0" /></linearGradient></defs>
      <polygon points={`0,${h} ${pts} ${w},${h}`} fill={`url(#${gid})`} />
      <polyline points={pts} fill="none" stroke={color} strokeWidth="1.5" strokeLinecap="round" />
    </svg>
  );
}

// ── Card ────────────────────────────────────────────────────────────────
function Card({ label, value, sub, icon, accent = '#00E0C4', spark, children }: {
  label: string; value: React.ReactNode; sub?: string; icon?: string; accent?: string; spark?: number[]; children?: React.ReactNode;
}) {
  const [hov, setHov] = useState(false);
  return (
    <div style={{
      background: 'var(--dag-card)', border: `1px solid ${hov ? accent + '28' : 'var(--dag-border)'}`,
      borderRadius: 14, padding: '18px 20px', position: 'relative', overflow: 'hidden',
      transition: 'all 0.25s', transform: hov ? 'translateY(-1px)' : 'none', cursor: 'default',
    }} onMouseEnter={() => setHov(true)} onMouseLeave={() => setHov(false)}>
      <div style={{ position: 'absolute', top: -20, left: -20, width: 60, height: 60, borderRadius: '50%', background: accent + '06', filter: 'blur(20px)' }} />
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start' }}>
        <div style={{ minWidth: 0, flex: 1 }}>
          <div style={{ fontSize: 10.5, fontWeight: 600, color: 'var(--dag-text-muted)', letterSpacing: 1.5, textTransform: 'uppercase', marginBottom: 8 }}>{label}</div>
          <div style={{ fontSize: 25, fontWeight: 700, color: 'var(--dag-text)', letterSpacing: -0.5, lineHeight: 1.2 }}>{value}</div>
          {sub && <div style={{ fontSize: 11.5, color: 'var(--dag-text-muted)', marginTop: 5 }}>{sub}</div>}
        </div>
        <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'flex-end', gap: 6, flexShrink: 0 }}>
          {icon && <span style={{ fontSize: 17, opacity: 0.4 }}>{icon}</span>}
          {spark && <Spark data={spark} color={accent} />}
        </div>
      </div>
      {children}
    </div>
  );
}

// ── Emission Bar ────────────────────────────────────────────────────────
function EmBar({ splits, colors }: { splits: number[]; colors: string[] }) {
  const [a, setA] = useState(0);
  useEffect(() => { const t0 = performance.now(); const run = (now: number) => { const p = Math.min((now - t0) / 1200, 1); setA(1 - Math.pow(1 - p, 3)); if (p < 1) requestAnimationFrame(run); }; requestAnimationFrame(run); }, []);
  const lbl = ['Validators', 'Council', 'Treasury', 'Founder'];
  return (
    <div>
      <div style={{ display: 'flex', borderRadius: 5, overflow: 'hidden', height: 7, background: 'var(--dag-input-bg)' }}>
        {splits.map((s, i) => <div key={i} style={{ width: `${s * a}%`, background: colors[i] }} />)}
      </div>
      <div style={{ display: 'flex', gap: 14, marginTop: 10, flexWrap: 'wrap' }}>
        {splits.map((s, i) => (
          <div key={i} style={{ display: 'flex', alignItems: 'center', gap: 5 }}>
            <div style={{ width: 7, height: 7, borderRadius: 2, background: colors[i] }} />
            <span style={{ fontSize: 10.5, color: 'var(--dag-text-muted)' }}>{lbl[i]} {s}%</span>
          </div>
        ))}
      </div>
    </div>
  );
}

// ── Main Dashboard ──────────────────────────────────────────────────────
export function DashboardPage({ status, loading: _loading, network, wallets, totalBalance, totalStaked, totalDelegated }: DashboardPageProps) {
  const [health, setHealth] = useState<HealthData | null>(null);
  const [recentRounds, setRecentRounds] = useState<RoundData[]>([]);
  const [vertexHistory, setVertexHistory] = useState<number[]>([]);
  const m = useIsMobile();

  const pw = getPasskeyWallet();
  const userName = pw?.name || wallets?.[0]?.name || 'Wallet';

  const fetchHealth = useCallback(async () => {
    try {
      const data = await getHealthDetailed();
      setHealth(data);
    } catch { /* ignore */ }
  }, []);

  useEffect(() => {
    fetchHealth();
    const iv = setInterval(fetchHealth, 10_000);
    return () => { clearInterval(iv); };
  }, [fetchHealth]);

  useEffect(() => {
    const handler = () => { setHealth(null); setRecentRounds([]); setVertexHistory([]); fetchHealth(); };
    window.addEventListener('ultradag-network-switch', handler);
    return () => window.removeEventListener('ultradag-network-switch', handler);
  }, [fetchHealth]);

  useEffect(() => {
    if (!health) return;
    let mounted = true;
    const fin = health.components.finality.last_finalized_round;
    const fetchRounds = async () => {
      const rounds: RoundData[] = [];
      for (let r = fin; r > Math.max(0, fin - 8); r--) {
        try {
          const data = await getRound(r);
          const verts = Array.isArray(data) ? data : data?.vertices ?? [];
          if (mounted) rounds.push({ round: r, vertices: verts });
        } catch { break; }
      }
      if (mounted) {
        setRecentRounds(rounds.reverse());
        setVertexHistory(prev => {
          const next = [...prev, ...rounds.map(r => r.vertices?.length ?? 0)].slice(-12);
          return next;
        });
      }
    };
    fetchRounds();
    return () => { mounted = false; };
  }, [health?.components.finality.last_finalized_round]);

  const dag = health?.components.dag;
  const fin = health?.components.finality;
  const st = health?.components.state;
  const net = health?.components.network;
  const mp = health?.components.mempool;
  const ck = health?.components.checkpoints;

  const round = dag?.current_round ?? status?.dag_round ?? 0;
  const finalized = fin?.last_finalized_round ?? status?.last_finalized_round ?? 0;
  const supplyUdag = (st?.total_supply ?? status?.total_supply ?? 0) / SATS;
  const supplyPct = Math.min((supplyUdag / MAX_SUPPLY) * 100, 100);
  const validators = (st?.active_validators || 0) > 0 ? st!.active_validators : (fin?.validator_count ?? 0);
  const peers = net?.peer_count ?? 0;
  const treasuryUdag = (status?.treasury_balance ?? 0) / SATS;
  const accounts = st?.account_count ?? 0;
  const mempoolCount = mp?.transaction_count ?? 0;
  const vertices = dag?.vertex_count ?? 0;
  const memoryMB = (status?.memory_usage_bytes ?? 0) / 1048576;
  const uptime = status?.uptime_seconds ? formatUptime(status.uptime_seconds) : '-';
  const portfolioTotal = (totalBalance ?? 0) / SATS;
  const portfolioAvailable = portfolioTotal - (totalStaked ?? 0) / SATS - (totalDelegated ?? 0) / SATS;
  const portfolioStaked = (totalStaked ?? 0) / SATS;
  const portfolioDelegated = (totalDelegated ?? 0) / SATS;
  const healthScore = health?.status === 'healthy' ? 98 : health?.status === 'degraded' ? 75 : 50;

  return (
    <div style={{ padding: m ? '12px 14px' : '18px 26px', fontFamily: "'DM Sans',sans-serif" }}>
      <style>{`
        @import url('https://fonts.googleapis.com/css2?family=DM+Sans:wght@400;500;600;700&family=DM+Mono:wght@400;500&display=swap');
        @keyframes pulse{0%,100%{opacity:1}50%{opacity:.5}}
        @keyframes slideUp{from{opacity:0;transform:translateY(10px)}to{opacity:1;transform:translateY(0)}}
      `}</style>

      {/* Top bar */}
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: m ? 'flex-start' : 'center', marginBottom: m ? 16 : 22, animation: 'slideUp 0.3s ease', flexDirection: m ? 'column' : 'row', gap: m ? 10 : 0 }}>
        <div>
          <h1 style={{ fontSize: m ? 18 : 21, fontWeight: 700, letterSpacing: -0.3, color: 'var(--dag-text)' }}>Dashboard</h1>
          <p style={{ fontSize: 11.5, color: 'var(--dag-subheading)', marginTop: 2 }}>Real-time network overview</p>
        </div>
        <div style={{ display: 'flex', alignItems: 'center', gap: m ? 8 : 12, flexWrap: 'wrap' }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 7 }}>
            <div style={{ width: 8, height: 8, borderRadius: '50%', background: '#00E0C4', boxShadow: '0 0 8px #00E0C4', animation: 'pulse 2s ease-in-out infinite' }} />
            <span style={{ fontSize: 12, fontWeight: 600, color: healthScore >= 90 ? '#00E0C4' : healthScore >= 60 ? '#FFB800' : '#EF4444' }}>{health?.status?.toUpperCase() ?? 'CONNECTING'}</span>
            <span style={{ fontSize: 11, color: 'var(--dag-subheading)' }}>{health ? `${healthScore}%` : ''}</span>
          </div>
          {!m && <div style={{ padding: '5px 13px', borderRadius: 18, background: 'rgba(0,224,196,0.06)', border: '1px solid rgba(0,224,196,0.12)', fontSize: 10.5, fontWeight: 600, color: '#00E0C4', letterSpacing: 1, textTransform: 'uppercase' }}>{network}</div>}
          {!m && <div style={{ display: 'flex', alignItems: 'center', gap: 7, padding: '5px 13px', borderRadius: 18, background: 'var(--dag-card)', border: '1px solid var(--dag-border)' }}>
            <div style={{ width: 18, height: 18, borderRadius: 5, background: 'linear-gradient(135deg,#00E0C4,#0066FF)', display: 'flex', alignItems: 'center', justifyContent: 'center', fontSize: 8, fontWeight: 800, color: '#fff' }}>{userName[0]?.toUpperCase()}</div>
            <span style={{ fontSize: 12, fontWeight: 600, color: 'var(--dag-text)' }}>{userName}</span>
            <span style={{ color: 'var(--dag-text-faint)' }}>|</span>
            <span style={{ fontSize: 12, color: '#00E0C4', fontWeight: 600 }}>{portfolioTotal.toFixed(2)} UDAG</span>
          </div>}
        </div>
      </div>

      {/* Portfolio */}
      <div style={{
        background: 'linear-gradient(135deg, rgba(0,224,196,0.03), rgba(0,102,255,0.03))',
        border: '1px solid rgba(0,224,196,0.08)', borderRadius: 16, padding: '20px 26px', marginBottom: 16,
        animation: 'slideUp 0.4s ease',
      }}>
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 14 }}>
          <span style={{ fontSize: 10.5, fontWeight: 600, color: 'var(--dag-text-muted)', letterSpacing: 2 }}>YOUR PORTFOLIO</span>
          <Link to="/wallet" style={{ fontSize: 11, color: 'var(--dag-subheading)', textDecoration: 'none' }}>Manage Wallets →</Link>
        </div>
        <div style={{ display: 'grid', gridTemplateColumns: m ? 'repeat(2,1fr)' : 'repeat(4,1fr)', gap: m ? 12 : 20 }}>
          {[
            { l: 'Total Value', v: portfolioTotal, c: '#fff' },
            { l: 'Available', v: portfolioAvailable, c: '#00E0C4' },
            { l: 'Staked', v: portfolioStaked, c: '#0066FF' },
            { l: 'Delegated', v: portfolioDelegated, c: '#A855F7' },
          ].map((p, i) => (
            <div key={i}>
              <div style={{ fontSize: 10.5, color: 'var(--dag-text-muted)', marginBottom: 5, letterSpacing: 0.5 }}>{p.l}</div>
              <div style={{ fontSize: m ? 18 : 23, fontWeight: 700, color: p.c }}><AnimCounter target={p.v} decimals={2} /></div>
              <div style={{ fontSize: 10, color: 'var(--dag-text-faint)', marginTop: 2 }}>UDAG</div>
            </div>
          ))}
        </div>
      </div>

      {/* Primary Stats */}
      <div style={{ display: 'grid', gridTemplateColumns: m ? 'repeat(2,1fr)' : 'repeat(4,1fr)', gap: m ? 10 : 12, marginBottom: 16, animation: 'slideUp 0.5s ease' }}>
        <Card label="Round" icon="◈" accent="#00E0C4" spark={vertexHistory.length > 1 ? vertexHistory : undefined}
          value={<><AnimCounter target={round} /> <span style={{ fontSize: 11, color: 'var(--dag-text-faint)', fontWeight: 400 }}>~5s</span></>}
          sub={`Finalized: ${finalized}`}
        />
        <Card label="Total Supply" icon="◎" accent="#0066FF"
          value={<AnimCounter target={supplyUdag} decimals={2} suffix=" UDAG" />}
          sub={`${supplyPct.toFixed(2)}% of 21M`}
        >
          <div style={{ marginTop: 10, height: 3, borderRadius: 2, background: 'var(--dag-input-bg)' }}>
            <div style={{ height: '100%', borderRadius: 2, background: 'linear-gradient(90deg,#0066FF,#00E0C4)', width: `${supplyPct}%`, boxShadow: '0 0 6px rgba(0,102,255,0.3)' }} />
          </div>
        </Card>
        <Card label="Network" icon="⬡" accent="#A855F7"
          value={<><AnimCounter target={validators} /> <span style={{ fontSize: 13, fontWeight: 400, color: 'var(--dag-text-muted)' }}>validators</span></>}
          sub={`${peers} peers connected`}
        />
        <Card label="Treasury" icon="♛" accent="#FFB800"
          value={<AnimCounter target={treasuryUdag} decimals={2} suffix=" UDAG" />}
          sub="10% emission, council-controlled"
        />
      </div>

      {/* DAG + Emission */}
      <div style={{ display: 'grid', gridTemplateColumns: m ? '1fr' : '1.4fr 1fr', gap: 12, marginBottom: 16, animation: 'slideUp 0.6s ease' }}>
        <div style={{ background: 'var(--dag-card)', border: '1px solid var(--dag-border)', borderRadius: 14, padding: '16px 18px', overflow: 'hidden' }}>
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 10 }}>
            <span style={{ fontSize: 12.5, fontWeight: 600, color: 'var(--dag-text-secondary)' }}>Live DAG Topology</span>
            <div style={{ display: 'flex', gap: 7, alignItems: 'center' }}>
              <span style={{ fontSize: 10.5, color: 'var(--dag-text-faint)', fontFamily: "'DM Mono',monospace" }}>R{recentRounds.length > 0 ? recentRounds[0].round : Math.max(0, round - 7)}–{recentRounds.length > 0 ? recentRounds[recentRounds.length - 1].round : round}</span>
              <div style={{ width: 5, height: 5, borderRadius: '50%', background: '#00E0C4', animation: 'pulse 1.5s ease-in-out infinite' }} />
            </div>
          </div>
          <DagVis roundData={recentRounds} />
        </div>
        <div style={{ background: 'var(--dag-card)', border: '1px solid var(--dag-border)', borderRadius: 14, padding: '16px 18px' }}>
          <div style={{ fontSize: 12.5, fontWeight: 600, color: 'var(--dag-text-secondary)', marginBottom: 14 }}>Emission Progress</div>
          <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', marginBottom: 18, position: 'relative' }}>
            <RingChart value={supplyUdag} max={MAX_SUPPLY} />
            <div style={{ position: 'absolute', textAlign: 'center' }}>
              <div style={{ fontSize: 19, fontWeight: 700, color: 'var(--dag-text)' }}>{supplyPct.toFixed(1)}%</div>
              <div style={{ fontSize: 9.5, color: 'var(--dag-text-muted)' }}>of 21M</div>
            </div>
          </div>
          <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 10, marginBottom: 14 }}>
            {[{ l: 'EMITTED', v: supplyUdag }, { l: 'REMAINING', v: MAX_SUPPLY - supplyUdag }].map((x, i) => (
              <div key={i} style={{ background: 'var(--dag-card)', borderRadius: 8, padding: '9px 11px' }}>
                <div style={{ fontSize: 9.5, color: 'var(--dag-text-muted)', marginBottom: 3, letterSpacing: 1 }}>{x.l}</div>
                <div style={{ fontSize: 13.5, fontWeight: 600, color: 'var(--dag-text)' }}><AnimCounter target={x.v} /></div>
              </div>
            ))}
          </div>
          <EmBar splits={[75, 10, 10, 5]} colors={['#00E0C4', '#0066FF', '#FFB800', '#A855F7']} />
        </div>
      </div>

      {/* Network Vitals */}
      <div style={{ marginBottom: 16, animation: 'slideUp 0.7s ease' }}>
        <div style={{ fontSize: 10, fontWeight: 600, color: 'var(--dag-text-faint)', letterSpacing: 2, marginBottom: 10 }}>NETWORK VITALS</div>
        <div style={{ display: 'grid', gridTemplateColumns: m ? 'repeat(2,1fr)' : 'repeat(6,1fr)', gap: 10 }}>
          {[
            { l: 'Accounts', v: accounts, a: '#00E0C4' },
            { l: 'Mempool', v: mempoolCount, a: '#00E0C4' },
            { l: 'DAG Vertices', v: vertices.toLocaleString(), a: '#0066FF' },
            { l: 'Finalized', v: finalized.toLocaleString(), a: '#00E0C4' },
            { l: 'Memory', v: `${memoryMB.toFixed(1)} MB`, a: '#A855F7' },
            { l: 'Uptime', v: uptime, a: '#FFB800' },
          ].map((v, i) => (
            <div key={i} style={{ background: 'var(--dag-card)', border: '1px solid var(--dag-border)', borderRadius: 10, padding: '12px 14px' }}>
              <div style={{ fontSize: 9.5, color: 'var(--dag-text-muted)', letterSpacing: 1.5, marginBottom: 7 }}>{v.l.toUpperCase()}</div>
              <div style={{ fontSize: 20, fontWeight: 700, color: 'var(--dag-text)' }}>{typeof v.v === 'number' ? <AnimCounter target={v.v} /> : v.v}</div>
            </div>
          ))}
        </div>
      </div>

      {/* Bottom: Checkpoints + DAG Status + Recent Rounds */}
      <div style={{ display: 'grid', gridTemplateColumns: m ? '1fr' : '1fr 1fr 1.5fr', gap: 12, animation: 'slideUp 0.8s ease' }}>
        <div style={{ background: 'var(--dag-card)', border: '1px solid var(--dag-border)', borderRadius: 14, padding: '16px 18px' }}>
          <div style={{ fontSize: 12.5, fontWeight: 600, color: 'var(--dag-text-secondary)', marginBottom: 12 }}>◈ Checkpoints</div>
          {[
            { l: 'Last checkpoint', v: `Round ${(ck?.last_checkpoint_round ?? 0).toLocaleString()}` },
            { l: 'Age', v: ck ? `${Math.floor(ck.checkpoint_age_seconds / 60)}m` : '-' },
            { l: 'On disk', v: String(ck?.disk_count ?? 0) },
            { l: 'Pending', v: String(ck?.pending_checkpoints ?? 0) },
          ].map((r, i) => (
            <div key={i} style={{ display: 'flex', justifyContent: 'space-between', padding: '6px 0', borderBottom: i < 3 ? '1px solid var(--dag-row-border)' : 'none' }}>
              <span style={{ fontSize: 11.5, color: 'var(--dag-text-muted)' }}>{r.l}</span>
              <span style={{ fontSize: 11.5, fontWeight: 600, color: 'var(--dag-value-text)', fontFamily: "'DM Mono',monospace" }}>{r.v}</span>
            </div>
          ))}
        </div>
        <div style={{ background: 'var(--dag-card)', border: '1px solid var(--dag-border)', borderRadius: 14, padding: '16px 18px' }}>
          <div style={{ fontSize: 12.5, fontWeight: 600, color: 'var(--dag-text-secondary)', marginBottom: 12 }}>⚡ DAG Status</div>
          {[
            { l: 'Pruning floor', v: String(dag?.pruning_floor ?? 0) },
            { l: 'Tips', v: String(dag?.tips_count ?? 0) },
            { l: 'Sync', v: net?.sync_complete ? 'Complete' : 'Syncing...', c: net?.sync_complete ? '#00E0C4' : '#FFB800' },
            { l: 'Finality lag', v: `${fin?.finality_lag ?? 0} rounds`, c: (fin?.finality_lag ?? 0) <= 3 ? '#00E0C4' : '#FFB800' },
          ].map((r, i) => (
            <div key={i} style={{ display: 'flex', justifyContent: 'space-between', padding: '6px 0', borderBottom: i < 3 ? '1px solid var(--dag-row-border)' : 'none' }}>
              <span style={{ fontSize: 11.5, color: 'var(--dag-text-muted)' }}>{r.l}</span>
              <span style={{ fontSize: 11.5, fontWeight: 600, color: r.c || 'var(--dag-value-text)', fontFamily: "'DM Mono',monospace" }}>{r.v}</span>
            </div>
          ))}
        </div>
        <div style={{ background: 'var(--dag-card)', border: '1px solid var(--dag-border)', borderRadius: 14, padding: '16px 18px' }}>
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 12 }}>
            <span style={{ fontSize: 12.5, fontWeight: 600, color: 'var(--dag-text-secondary)' }}>◉ Recent Finalized Rounds</span>
            <Link to="/explorer" style={{ fontSize: 10.5, color: 'var(--dag-text-faint)', textDecoration: 'none' }}>View all →</Link>
          </div>
          <div style={{ display: 'grid', gridTemplateColumns: 'auto 1fr 1fr', gap: '0 14px' }}>
            {['ROUND', 'VERTICES', 'TXS'].map((h, i) => (
              <div key={i} style={{ fontSize: 8.5, fontWeight: 600, color: 'var(--dag-text-faint)', letterSpacing: 1.5, paddingBottom: 7, borderBottom: '1px solid var(--dag-table-border)' }}>{h}</div>
            ))}
            {recentRounds.slice(0, 6).map((r, i) => [
              <div key={`r${i}`} style={{ fontSize: 11.5, fontWeight: 600, color: '#00E0C4', padding: '5px 0', fontFamily: "'DM Mono',monospace", borderBottom: '1px solid var(--dag-row-border)' }}>{r.round}</div>,
              <div key={`v${i}`} style={{ fontSize: 11.5, color: 'var(--dag-cell-text)', padding: '5px 0', borderBottom: '1px solid var(--dag-row-border)' }}>{r.vertices?.length ?? 0}</div>,
              <div key={`t${i}`} style={{ fontSize: 11.5, color: 'var(--dag-cell-text)', padding: '5px 0', borderBottom: '1px solid var(--dag-row-border)' }}>{(r.vertices ?? []).reduce((s, v) => s + v.tx_count, 0)}</div>,
            ]).flat()}
          </div>
        </div>
      </div>
      <div style={{ height: 16 }} />
    </div>
  );
}
