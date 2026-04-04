import { useEffect, useState } from 'react';
import { useParams, Link } from 'react-router-dom';
import { ChevronLeft, ChevronRight } from 'lucide-react';
import { getRound, getStatus, connectToNode, isConnected } from '../lib/api.ts';
import { VertexCard } from '../components/explorer/VertexCard.tsx';
import { Badge } from '../components/shared/Badge.tsx';
import { PageHeader } from '../components/shared/PageHeader.tsx';
import { useIsMobile } from '../hooks/useIsMobile';

export function RoundDetailPage() {
  const { round: roundStr } = useParams<{ round: string }>();
  const round = Number(roundStr);
  const m = useIsMobile();
  const [vertices, setVertices] = useState<Array<Record<string, unknown>>>([]);
  const [finalized, setFinalized] = useState(false);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');
  const [switchCount, setSwitchCount] = useState(0);

  useEffect(() => {
    const handler = () => setSwitchCount(n => n + 1);
    window.addEventListener('ultradag-network-switch', handler);
    return () => window.removeEventListener('ultradag-network-switch', handler);
  }, []);

  useEffect(() => {
    if (isNaN(round)) return;

    const fetchRound = async () => {
      setLoading(true);
      try {
        if (!isConnected()) await connectToNode();
        const [roundData, statusData] = await Promise.all([getRound(round), getStatus()]);
        const verts = Array.isArray(roundData) ? roundData : roundData?.vertices ?? [];
        setVertices(verts);
        setFinalized(round <= Number(statusData.last_finalized_round ?? 0));
        setError('');
      } catch (e) {
        setError(e instanceof Error ? e.message : 'Failed to fetch round');
      } finally {
        setLoading(false);
      }
    };

    fetchRound();
  }, [round, switchCount]);

  if (isNaN(round)) {
    return <div style={{ color: '#EF4444', padding: '32px 0' }}>Invalid round number</div>;
  }

  return (
    <div style={{ padding: m ? '12px 14px' : '18px 26px', fontFamily: "'DM Sans',sans-serif" }}>
      <PageHeader
        title={`Round #${round.toLocaleString()}`}
        subtitle={finalized ? 'Finalized' : 'Pending'}
        right={<>
          <Badge label={finalized ? 'Finalized' : 'Pending'} variant={finalized ? 'green' : 'yellow'} />
          <div style={{ display: 'flex', gap: 4 }}>
            {round > 1 && (
              <Link to={`/round/${round - 1}`} style={{ padding: 8, borderRadius: 6, background: 'var(--dag-card)', border: '1px solid var(--dag-border)', color: 'var(--dag-text-muted)', display: 'flex', alignItems: 'center', textDecoration: 'none' }}>
                <ChevronLeft style={{ width: 16, height: 16 }} />
              </Link>
            )}
            <Link to={`/round/${round + 1}`} style={{ padding: 8, borderRadius: 6, background: 'var(--dag-card)', border: '1px solid var(--dag-border)', color: 'var(--dag-text-muted)', display: 'flex', alignItems: 'center', textDecoration: 'none' }}>
              <ChevronRight style={{ width: 16, height: 16 }} />
            </Link>
          </div>
        </>}
      />
      <div style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>

      {error && <div style={{ background: 'rgba(239,68,68,0.06)', border: '1px solid rgba(239,68,68,0.15)', borderRadius: 10, padding: 12, fontSize: 12, color: '#EF4444' }}>{error}</div>}

      {loading ? (
        <div style={{ color: 'var(--dag-text-faint)', fontSize: 12, padding: '32px 0', textAlign: 'center' }}>Loading...</div>
      ) : vertices.length === 0 ? (
        <div style={{ color: 'var(--dag-text-faint)', fontSize: 12, padding: '32px 0', textAlign: 'center' }}>No vertices found in this round</div>
      ) : (
        <div>
          <p style={{ fontSize: 12, color: 'var(--dag-text-muted)', marginBottom: 12 }}>{vertices.length} vertices in this round</p>
          <div style={{ display: 'grid', gridTemplateColumns: m ? '1fr' : '1fr 1fr', gap: 12 }}>
            {vertices.map((v) => (
              <VertexCard
                key={String(v.hash)}
                hash={String(v.hash ?? '')}
                validator={String(v.validator ?? '')}
                reward={v.reward as number | undefined}
                reward_udag={v.reward_udag as number | undefined}
                tx_count={Number(v.tx_count ?? 0)}
                parent_count={Number(v.parent_count ?? 0)}
              />
            ))}
          </div>
        </div>
      )}
      </div>
    </div>
  );
}
