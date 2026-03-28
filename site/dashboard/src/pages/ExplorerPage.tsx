import { useEffect, useState, useCallback, useRef } from 'react';
import { Link } from 'react-router-dom';
import { getStatus, getRound, connectToNode, isConnected } from '../lib/api';
import { SearchBar } from '../components/explorer/SearchBar';

const PAGE_SIZE = 10;
const SATS = 100_000_000;

const S = {
  card: { background: 'var(--dag-card)', border: '1px solid var(--dag-border)', borderRadius: 14, padding: '18px 20px' } as React.CSSProperties,
  th: { fontSize: 8.5, fontWeight: 600, color: 'var(--dag-text-faint)', letterSpacing: 1.5, paddingBottom: 8, borderBottom: '1px solid var(--dag-table-border)' } as React.CSSProperties,
  td: { fontSize: 11.5, color: 'var(--dag-cell-text)', padding: '7px 0', borderBottom: '1px solid var(--dag-row-border)' } as React.CSSProperties,
  mono: { fontFamily: "'DM Mono',monospace" },
};

interface RoundData {
  round: number;
  vertices: Array<{ hash: string; validator: string; tx_count: number; parent_count: number }>;
  finalized: boolean;
}

export function ExplorerPage() {
  const [rounds, setRounds] = useState<RoundData[]>([]);
  const [dagRound, setDagRound] = useState(0);
  const [finalized, setFinalized] = useState(0);
  const [validators, setValidators] = useState(0);
  const [supply, setSupply] = useState(0);
  const [page, setPage] = useState(1);
  const [loading, setLoading] = useState(true);
  const ivRef = useRef<ReturnType<typeof setInterval>>(undefined);

  const fetchRounds = useCallback(async () => {
    try {
      if (!isConnected()) await connectToNode();
      const s = await getStatus();
      const dr = Number(s.dag_round ?? 0);
      const fin = Number(s.last_finalized_round ?? 0);
      setDagRound(dr); setFinalized(fin);
      setValidators(Number(s.validator_count ?? s.active_stakers ?? 0));
      setSupply(Number(s.total_supply ?? 0));
      if (fin <= 0) { setRounds([]); setLoading(false); return; }
      const start = fin - (page - 1) * PAGE_SIZE;
      const end = Math.max(1, start - PAGE_SIZE + 1);
      const results = await Promise.all(
        Array.from({ length: start - end + 1 }, (_, i) => start - i).map(r =>
          getRound(r).then(d => ({ round: r, vertices: Array.isArray(d) ? d : d?.vertices ?? [], finalized: r <= fin })).catch(() => null)
        )
      );
      setRounds(results.filter((r): r is RoundData => r !== null));
    } catch {} finally { setLoading(false); }
  }, [page]);

  useEffect(() => { setLoading(true); fetchRounds(); }, [fetchRounds]);
  useEffect(() => {
    if (ivRef.current) clearInterval(ivRef.current);
    if (page === 1) ivRef.current = setInterval(fetchRounds, 10000);
    return () => { if (ivRef.current) clearInterval(ivRef.current); };
  }, [page, fetchRounds]);

  useEffect(() => {
    const handler = () => { setRounds([]); setDagRound(0); setFinalized(0); setValidators(0); setSupply(0); setLoading(true); fetchRounds(); };
    window.addEventListener('ultradag-network-switch', handler);
    return () => window.removeEventListener('ultradag-network-switch', handler);
  }, [fetchRounds]);

  const totalPages = Math.max(1, Math.ceil(finalized / PAGE_SIZE));
  const lag = dagRound - finalized;

  return (
    <div style={{ padding: '18px 26px', fontFamily: "'DM Sans',sans-serif" }}>
      <style>{`@keyframes slideUp{from{opacity:0;transform:translateY(10px)}to{opacity:1;transform:translateY(0)}} @keyframes pulse{0%,100%{opacity:1}50%{opacity:.4}}`}</style>

      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 22, animation: 'slideUp 0.3s ease' }}>
        <div>
          <h1 style={{ fontSize: 21, fontWeight: 700, color: 'var(--dag-text)' }}>Explorer</h1>
          <p style={{ fontSize: 11.5, color: 'var(--dag-subheading)', marginTop: 2 }}>Search and browse the DAG</p>
        </div>
        <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
          {lag <= 3 && <div style={{ display: 'flex', alignItems: 'center', gap: 5 }}>
            <div style={{ width: 6, height: 6, borderRadius: '50%', background: '#00E0C4', boxShadow: '0 0 6px #00E0C4', animation: 'pulse 2s infinite' }} />
            <span style={{ fontSize: 10.5, color: '#00E0C4', fontWeight: 600 }}>LIVE</span>
          </div>}
          <span style={{ fontSize: 10.5, padding: '3px 10px', borderRadius: 6, background: lag <= 3 ? 'rgba(0,224,196,0.08)' : lag <= 10 ? 'rgba(255,184,0,0.08)' : 'rgba(239,68,68,0.08)', color: lag <= 3 ? '#00E0C4' : lag <= 10 ? '#FFB800' : '#EF4444', fontWeight: 600 }}>
            Lag: {lag}
          </span>
        </div>
      </div>

      {/* Stats */}
      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(4,1fr)', gap: 12, marginBottom: 16, animation: 'slideUp 0.4s ease' }}>
        {[
          { l: 'DAG ROUND', v: dagRound.toLocaleString(), c: '#fff', i: '◈' },
          { l: 'FINALIZED', v: finalized.toLocaleString(), c: '#00E0C4', i: '✓' },
          { l: 'VALIDATORS', v: String(validators), c: '#A855F7', i: '♛' },
          { l: 'SUPPLY', v: (supply / SATS).toLocaleString(undefined, { maximumFractionDigits: 0 }) + ' UDAG', c: '#0066FF', i: '◎' },
        ].map((s, i) => (
          <div key={i} style={S.card}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 6, marginBottom: 6 }}>
              <span style={{ color: s.c, fontSize: 13 }}>{s.i}</span>
              <span style={{ fontSize: 9, color: 'var(--dag-subheading)', letterSpacing: 1.2 }}>{s.l}</span>
            </div>
            <div style={{ fontSize: 21, fontWeight: 700, color: s.c, ...S.mono }}>{s.v}</div>
          </div>
        ))}
      </div>

      {/* Search */}
      <div style={{ marginBottom: 16, animation: 'slideUp 0.5s ease' }}>
        <SearchBar />
      </div>

      {/* Round Table */}
      <div style={{ ...S.card, animation: 'slideUp 0.6s ease' }}>
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 14 }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
            <span style={{ color: '#A855F7', fontSize: 14 }}>◉</span>
            <span style={{ fontSize: 13, fontWeight: 600, color: 'var(--dag-text-secondary)' }}>Finalized Rounds</span>
            {page === 1 && <span style={{ display: 'flex', alignItems: 'center', gap: 4, fontSize: 9.5, color: 'var(--dag-text-faint)' }}>
              <span style={{ width: 5, height: 5, borderRadius: '50%', background: '#00E0C4', animation: 'pulse 1.5s infinite' }} /> Auto-refresh
            </span>}
          </div>
          <span style={{ fontSize: 10, color: 'var(--dag-text-faint)', ...S.mono }}>Page {page}/{totalPages}</span>
        </div>

        {loading ? (
          <p style={{ fontSize: 12, color: 'var(--dag-text-faint)', textAlign: 'center', padding: '30px 0' }}>Loading rounds...</p>
        ) : rounds.length === 0 ? (
          <p style={{ fontSize: 12, color: 'var(--dag-text-faint)', textAlign: 'center', padding: '30px 0' }}>No rounds yet.</p>
        ) : (
          <>
            <div style={{ display: 'grid', gridTemplateColumns: 'auto 1fr 1fr 1fr 1fr', gap: '0 16px' }}>
              {['ROUND', 'VERTICES', 'TXS', 'PARENTS', 'STATUS'].map((h, i) => (
                <div key={i} style={S.th}>{h}</div>
              ))}
              {rounds.map(r => {
                const txs = r.vertices.reduce((s, v) => s + v.tx_count, 0);
                const parents = r.vertices.length > 0 ? Math.round(r.vertices.reduce((s, v) => s + v.parent_count, 0) / r.vertices.length) : 0;
                return [
                  <Link key={`r${r.round}`} to={`/round/${r.round}`} style={{ ...S.td, fontWeight: 600, color: '#00E0C4', ...S.mono, textDecoration: 'none' }}>{r.round.toLocaleString()}</Link>,
                  <div key={`v${r.round}`} style={S.td}>{r.vertices.length}</div>,
                  <div key={`t${r.round}`} style={{ ...S.td, color: txs > 0 ? '#FFB800' : undefined }}>{txs}</div>,
                  <div key={`p${r.round}`} style={S.td}>~{parents}</div>,
                  <div key={`s${r.round}`} style={S.td}>
                    <span style={{ fontSize: 9, padding: '2px 7px', borderRadius: 4, background: r.finalized ? 'rgba(0,224,196,0.08)' : 'rgba(255,184,0,0.08)', color: r.finalized ? '#00E0C4' : '#FFB800', fontWeight: 600 }}>
                      {r.finalized ? 'Finalized' : 'Pending'}
                    </span>
                  </div>,
                ];
              }).flat()}
            </div>
            {/* Pagination */}
            <div style={{ display: 'flex', justifyContent: 'center', gap: 6, marginTop: 14 }}>
              <button onClick={() => setPage(Math.max(1, page - 1))} disabled={page === 1}
                style={{ padding: '5px 12px', borderRadius: 6, background: 'var(--dag-input-bg)', border: '1px solid var(--dag-border)', color: page === 1 ? 'var(--dag-text-faint)' : 'var(--dag-text)', fontSize: 11, cursor: page === 1 ? 'default' : 'pointer' }}>← Prev</button>
              <span style={{ padding: '5px 10px', fontSize: 11, color: 'var(--dag-text-muted)', ...S.mono }}>{page} / {totalPages}</span>
              <button onClick={() => setPage(Math.min(totalPages, page + 1))} disabled={page === totalPages}
                style={{ padding: '5px 12px', borderRadius: 6, background: 'var(--dag-input-bg)', border: '1px solid var(--dag-border)', color: page === totalPages ? 'var(--dag-text-faint)' : 'var(--dag-text)', fontSize: 11, cursor: page === totalPages ? 'default' : 'pointer' }}>Next →</button>
            </div>
          </>
        )}
      </div>
    </div>
  );
}
