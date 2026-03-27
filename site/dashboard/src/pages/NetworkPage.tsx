import { useEffect, useState, useCallback } from 'react';
import { getStatus, getPeers, getMempool, getMetrics, getHealthDetailed, connectToNode, isConnected, getNodeUrl } from '../lib/api';

const S = {
  card: { background: 'rgba(255,255,255,0.018)', border: '1px solid rgba(255,255,255,0.055)', borderRadius: 14, padding: '18px 20px' } as React.CSSProperties,
  stat: { background: 'rgba(255,255,255,0.025)', borderRadius: 10, padding: '10px 13px' } as React.CSSProperties,
  mono: { fontFamily: "'DM Mono',monospace" },
  th: { fontSize: 8.5, fontWeight: 600, color: 'rgba(255,255,255,0.18)', letterSpacing: 1.5, paddingBottom: 7, borderBottom: '1px solid rgba(255,255,255,0.03)' } as React.CSSProperties,
  td: { fontSize: 11, color: 'rgba(255,255,255,0.4)', padding: '6px 0', borderBottom: '1px solid rgba(255,255,255,0.015)' } as React.CSSProperties,
};

const healthColor: Record<string, string> = { healthy: '#00E0C4', warning: '#FFB800', unhealthy: '#EF4444', degraded: '#FFB800' };

export function NetworkPage() {
  const [status, setStatus] = useState<Record<string, unknown> | null>(null);
  const [peers, setPeers] = useState<string[]>([]);
  const [bootstrap, setBootstrap] = useState<Array<{ addr: string; connected: boolean }>>([]);
  const [mempool, setMempool] = useState<Array<Record<string, unknown>>>([]);
  const [metrics, setMetrics] = useState<Record<string, unknown> | null>(null);
  const [health, setHealth] = useState<{ status: string; components: Record<string, { available?: boolean; [k: string]: unknown }> } | null>(null);

  const fetchAll = useCallback(async () => {
    try {
      if (!isConnected()) await connectToNode();
      const [s, p, m, mt, h] = await Promise.allSettled([getStatus(), getPeers(), getMempool(), getMetrics(), getHealthDetailed()]);
      if (s.status === 'fulfilled') setStatus(s.value);
      if (p.status === 'fulfilled') { setPeers(p.value.peers ?? []); setBootstrap(p.value.bootstrap_nodes ?? []); }
      if (m.status === 'fulfilled') setMempool(Array.isArray(m.value) ? m.value : m.value?.transactions ?? []);
      if (mt.status === 'fulfilled') setMetrics(mt.value);
      if (h.status === 'fulfilled') setHealth(h.value);
    } catch {}
  }, []);

  useEffect(() => { fetchAll(); const iv = setInterval(fetchAll, 5000); return () => clearInterval(iv); }, [fetchAll]);

  useEffect(() => {
    const handler = () => { setStatus(null); setPeers([]); setBootstrap([]); setMempool([]); setMetrics(null); setHealth(null); fetchAll(); };
    window.addEventListener('ultradag-network-switch', handler);
    return () => window.removeEventListener('ultradag-network-switch', handler);
  }, [fetchAll]);

  const hc = health?.status ?? 'unknown';
  const components = health?.components ? Object.entries(health.components) : [];

  return (
    <div style={{ padding: '18px 26px', fontFamily: "'DM Sans',sans-serif" }}>
      <style>{`@keyframes slideUp{from{opacity:0;transform:translateY(10px)}to{opacity:1;transform:translateY(0)}} @keyframes pulse{0%,100%{opacity:1}50%{opacity:.4}}`}</style>

      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 22, animation: 'slideUp 0.3s ease' }}>
        <div>
          <h1 style={{ fontSize: 21, fontWeight: 700, color: '#fff' }}>Node Status</h1>
          <p style={{ fontSize: 11.5, color: 'rgba(255,255,255,0.25)', marginTop: 2 }}>
            Connected to <span style={S.mono}>{getNodeUrl()}</span>
          </p>
        </div>
        <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
          <div style={{ width: 6, height: 6, borderRadius: '50%', background: '#00E0C4', animation: 'pulse 2s infinite' }} />
          <span style={{ fontSize: 10.5, color: 'rgba(255,255,255,0.22)' }}>Auto-refresh 5s</span>
        </div>
      </div>

      {/* Health */}
      <div style={{ ...S.card, marginBottom: 14, animation: 'slideUp 0.4s ease' }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 12 }}>
          <span style={{ fontSize: 14, color: healthColor[hc] || '#888' }}>♡</span>
          <span style={{ fontSize: 13, fontWeight: 600, color: 'rgba(255,255,255,0.55)' }}>Health</span>
          <span style={{ fontSize: 11, fontWeight: 600, color: healthColor[hc] || '#888', textTransform: 'capitalize' }}>{hc}</span>
        </div>
        {components.length > 0 && (
          <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8 }}>
            {components.map(([name, comp]) => (
              <div key={name} style={{ display: 'flex', alignItems: 'center', gap: 5, background: 'rgba(255,255,255,0.025)', borderRadius: 6, padding: '5px 10px' }}>
                <div style={{ width: 7, height: 7, borderRadius: '50%', background: comp.available ? '#00E0C4' : comp.available === false ? '#EF4444' : 'rgba(255,255,255,0.15)' }} />
                <span style={{ fontSize: 10.5, color: 'rgba(255,255,255,0.45)', textTransform: 'capitalize' }}>{name}</span>
              </div>
            ))}
          </div>
        )}
      </div>

      {/* Status Grid */}
      {status && (
        <div style={{ display: 'grid', gridTemplateColumns: 'repeat(4,1fr)', gap: 10, marginBottom: 14, animation: 'slideUp 0.5s ease' }}>
          {[
            { l: 'DAG ROUND', v: String(status.dag_round ?? 0), c: '#00E0C4' },
            { l: 'FINALIZED', v: String(status.last_finalized_round ?? 0), c: '#0066FF' },
            { l: 'PEERS', v: String(peers.length), c: '#A855F7' },
            { l: 'MEMPOOL', v: String(status.mempool_size ?? mempool.length), c: '#FFB800' },
            { l: 'ACCOUNTS', v: String(status.accounts ?? 0), c: '#fff' },
            { l: 'VALIDATORS', v: String(status.validator_count ?? status.active_stakers ?? 0), c: '#00E0C4' },
            { l: 'DAG TIPS', v: String(status.dag_tips ?? 0), c: '#0066FF' },
            { l: 'VERTICES', v: Number(status.dag_vertices ?? 0).toLocaleString(), c: '#A855F7' },
          ].map((s2, i) => (
            <div key={i} style={S.stat}>
              <div style={{ fontSize: 9, color: 'rgba(255,255,255,0.22)', letterSpacing: 1, marginBottom: 3 }}>{s2.l}</div>
              <div style={{ fontSize: 17, fontWeight: 700, color: s2.c, ...S.mono }}>{s2.v}</div>
            </div>
          ))}
        </div>
      )}

      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 14, animation: 'slideUp 0.6s ease' }}>
        {/* Peers */}
        <div style={S.card}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 7, marginBottom: 12 }}>
            <span style={{ color: '#00E0C4', fontSize: 14 }}>◎</span>
            <span style={{ fontSize: 13, fontWeight: 600, color: 'rgba(255,255,255,0.55)' }}>Connected Peers ({peers.length})</span>
          </div>
          {peers.length === 0 ? (
            <p style={{ fontSize: 11, color: 'rgba(255,255,255,0.2)' }}>No peers connected</p>
          ) : (
            <div style={{ display: 'flex', flexDirection: 'column', gap: 2 }}>
              {peers.map((p, i) => (
                <div key={p} style={{ display: 'flex', alignItems: 'center', gap: 6, padding: '4px 0', borderBottom: '1px solid rgba(255,255,255,0.015)' }}>
                  <span style={{ fontSize: 10, color: 'rgba(255,255,255,0.15)', width: 16 }}>{i + 1}</span>
                  <span style={{ fontSize: 10.5, color: 'rgba(255,255,255,0.4)', ...S.mono, wordBreak: 'break-all' }}>{p}</span>
                </div>
              ))}
            </div>
          )}
          {bootstrap.length > 0 && bootstrap.some(n => n.connected) && (
            <div style={{ marginTop: 12, paddingTop: 10, borderTop: '1px solid rgba(255,255,255,0.03)' }}>
              <div style={{ fontSize: 9, color: 'rgba(255,255,255,0.2)', letterSpacing: 1, marginBottom: 6 }}>BOOTSTRAP NODES</div>
              {bootstrap.map(n => (
                <div key={n.addr} style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', padding: '3px 0' }}>
                  <span style={{ fontSize: 10, ...S.mono, color: 'rgba(255,255,255,0.3)' }}>{n.addr}</span>
                  <span style={{ display: 'flex', alignItems: 'center', gap: 4, fontSize: 9.5 }}>
                    <span style={{ width: 5, height: 5, borderRadius: '50%', background: n.connected ? '#00E0C4' : 'rgba(255,255,255,0.1)' }} />
                    <span style={{ color: n.connected ? '#00E0C4' : 'rgba(255,255,255,0.15)' }}>{n.connected ? 'Connected' : 'Down'}</span>
                  </span>
                </div>
              ))}
            </div>
          )}
        </div>

        {/* Mempool */}
        <div style={S.card}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 7, marginBottom: 12 }}>
            <span style={{ color: '#FFB800', fontSize: 14 }}>◈</span>
            <span style={{ fontSize: 13, fontWeight: 600, color: 'rgba(255,255,255,0.55)' }}>Mempool ({mempool.length})</span>
          </div>
          {mempool.length === 0 ? (
            <p style={{ fontSize: 11, color: 'rgba(255,255,255,0.2)' }}>Mempool empty</p>
          ) : (
            <div style={{ display: 'flex', flexDirection: 'column', gap: 2 }}>
              {mempool.slice(0, 15).map((tx, i) => (
                <div key={i} style={{ display: 'flex', justifyContent: 'space-between', padding: '4px 0', borderBottom: '1px solid rgba(255,255,255,0.015)' }}>
                  <span style={{ fontSize: 10, color: 'rgba(255,255,255,0.4)' }}>{String(tx.type ?? 'tx')}</span>
                  <span style={{ fontSize: 10, ...S.mono, color: 'rgba(255,255,255,0.22)' }}>{String(tx.hash ?? '').slice(0, 12)}…</span>
                </div>
              ))}
              {mempool.length > 15 && <p style={{ fontSize: 10, color: 'rgba(255,255,255,0.15)', marginTop: 4 }}>+{mempool.length - 15} more</p>}
            </div>
          )}
        </div>
      </div>

      {/* Metrics */}
      {metrics && (
        <div style={{ ...S.card, marginTop: 14, animation: 'slideUp 0.7s ease' }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 7, marginBottom: 14 }}>
            <span style={{ color: '#0066FF', fontSize: 14 }}>◉</span>
            <span style={{ fontSize: 13, fontWeight: 600, color: 'rgba(255,255,255,0.55)' }}>Metrics</span>
          </div>
          <div style={{ display: 'grid', gridTemplateColumns: 'repeat(4,1fr)', gap: 8 }}>
            {Object.entries(metrics).flatMap(([section, data]) => {
              if (!data || typeof data !== 'object') return [];
              return Object.entries(data as Record<string, unknown>).map(([key, value]) => (
                <div key={`${section}.${key}`} style={S.stat}>
                  <div style={{ fontSize: 8.5, color: 'rgba(255,255,255,0.2)', letterSpacing: 0.8, marginBottom: 2 }}>{key.replace(/_/g, ' ')}</div>
                  <div style={{ fontSize: 13, fontWeight: 600, color: '#fff', ...S.mono }}>{typeof value === 'number' ? value.toLocaleString() : String(value)}</div>
                </div>
              ));
            })}
          </div>
        </div>
      )}
    </div>
  );
}
