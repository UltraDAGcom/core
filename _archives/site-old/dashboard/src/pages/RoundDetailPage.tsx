import { useEffect, useState } from 'react';
import { useParams, Link } from 'react-router-dom';
import { ChevronLeft, ChevronRight } from 'lucide-react';
import { getRound, getStatus, connectToNode, isConnected } from '../lib/api.ts';
import { VertexCard } from '../components/explorer/VertexCard.tsx';
import { Badge } from '../components/shared/Badge.tsx';

export function RoundDetailPage() {
  const { round: roundStr } = useParams<{ round: string }>();
  const round = Number(roundStr);
  const [vertices, setVertices] = useState<Array<Record<string, unknown>>>([]);
  const [finalized, setFinalized] = useState(false);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');

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
  }, [round]);

  if (isNaN(round)) {
    return <div className="text-red-400 py-8">Invalid round number</div>;
  }

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Link to="/explorer" className="text-slate-400 hover:text-slate-200">
          <ChevronLeft className="w-5 h-5" />
        </Link>
        <div className="flex items-center gap-3">
          <h1 className="text-2xl font-bold text-white font-mono">Round #{round.toLocaleString()}</h1>
          <Badge label={finalized ? 'Finalized' : 'Pending'} variant={finalized ? 'green' : 'yellow'} />
        </div>
        <div className="ml-auto flex items-center gap-2">
          {round > 1 && (
            <Link to={`/round/${round - 1}`} className="p-2 rounded bg-slate-800 border border-slate-700 text-slate-400 hover:text-white transition-colors">
              <ChevronLeft className="w-4 h-4" />
            </Link>
          )}
          <Link to={`/round/${round + 1}`} className="p-2 rounded bg-slate-800 border border-slate-700 text-slate-400 hover:text-white transition-colors">
            <ChevronRight className="w-4 h-4" />
          </Link>
        </div>
      </div>

      {error && <div className="bg-red-500/10 border border-red-500/30 rounded-lg p-3 text-sm text-red-400">{error}</div>}

      {loading ? (
        <div className="text-slate-500 text-sm py-8 text-center">Loading...</div>
      ) : vertices.length === 0 ? (
        <div className="text-slate-500 text-sm py-8 text-center">No vertices found in this round</div>
      ) : (
        <div>
          <p className="text-sm text-slate-400 mb-3">{vertices.length} vertices in this round</p>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
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
  );
}
