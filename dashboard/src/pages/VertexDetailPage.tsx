import { useEffect, useState } from 'react';
import { useParams, Link } from 'react-router-dom';
import { ChevronLeft } from 'lucide-react';
import { getVertex, connectToNode, isConnected, shortHash, formatUdag } from '../lib/api.ts';
import { CopyButton } from '../components/shared/CopyButton.tsx';
import { Badge } from '../components/shared/Badge.tsx';
import { DisplayIdentity } from '../components/shared/DisplayIdentity.tsx';

export function VertexDetailPage() {
  const { hash } = useParams<{ hash: string }>();
  const [vertex, setVertex] = useState<Record<string, unknown> | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');

  useEffect(() => {
    if (!hash) return;

    const fetchVertex = async () => {
      setLoading(true);
      try {
        if (!isConnected()) await connectToNode();
        const data = await getVertex(hash);
        setVertex(data);
        setError('');
      } catch (e) {
        setError(e instanceof Error ? e.message : 'Vertex not found');
      } finally {
        setLoading(false);
      }
    };

    fetchVertex();
  }, [hash]);

  if (loading) return <div className="text-slate-500 py-8 text-center">Loading vertex...</div>;
  if (error) {
    return (
      <div className="space-y-4">
        <Link to="/explorer" className="inline-flex items-center gap-1 text-slate-400 hover:text-slate-200 text-sm">
          <ChevronLeft className="w-4 h-4" /> Back to Explorer
        </Link>
        <div className="bg-red-500/10 border border-red-500/30 rounded-lg p-3 text-sm text-red-400">{error}</div>
      </div>
    );
  }
  if (!vertex) return null;

  const round = Number(vertex.round ?? 0);
  const validator = String(vertex.validator ?? '');
  const parentCount = Number(vertex.parent_count ?? 0);
  const transactions = (vertex.transactions ?? []) as Array<Record<string, unknown>>;
  // Reward: API returns coinbase.amount (sats), fall back to legacy vertex.reward fields
  const coinbase = vertex.coinbase as Record<string, unknown> | undefined;
  const rewardSats = coinbase?.amount != null ? Number(coinbase.amount) : vertex.reward != null ? Number(vertex.reward) : null;
  const rewardDisplay = rewardSats != null ? `${formatUdag(rewardSats)} UDAG` : '--';

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Link to={`/round/${round}`} className="text-slate-400 hover:text-slate-200">
          <ChevronLeft className="w-5 h-5" />
        </Link>
        <h1 className="text-xl font-bold text-white">Vertex Detail</h1>
      </div>

      {/* Vertex info */}
      <div className="bg-slate-800/50 border border-slate-700 rounded-lg p-4 space-y-3">
        <InfoRow label="Hash" value={hash ?? ''} mono copy />
        <InfoRow label="Round">
          <Link to={`/round/${round}`} className="font-mono text-blue-400 hover:text-blue-300">
            #{round.toLocaleString()}
          </Link>
        </InfoRow>
        <InfoRow label="Validator">
          <DisplayIdentity address={validator} link size="sm" />
        </InfoRow>
        <InfoRow label="Reward" value={rewardDisplay} mono />
        <InfoRow label="Parent Count" value={String(parentCount)} mono />
        <InfoRow label="Transaction Count" value={String(transactions.length)} mono />
      </div>

      {/* Parents */}
      {parentCount > 0 && (
        <div className="bg-slate-800/50 border border-slate-700 rounded-lg p-4">
          <h2 className="text-sm font-semibold text-slate-200 mb-3">Parent Vertices ({parentCount})</h2>
          <p className="text-sm text-slate-500">{parentCount} parent{parentCount !== 1 ? 's' : ''} (hashes not available in this view)</p>
        </div>
      )}

      {/* Transactions */}
      {transactions.length > 0 && (
        <div className="bg-slate-800/50 border border-slate-700 rounded-lg p-4">
          <h2 className="text-sm font-semibold text-slate-200 mb-3">Transactions ({transactions.length})</h2>
          <div className="overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="text-left text-slate-400 border-b border-slate-700">
                  <th className="py-2 px-3 font-medium">Hash</th>
                  <th className="py-2 px-3 font-medium">Type</th>
                  <th className="py-2 px-3 font-medium">From</th>
                  <th className="py-2 px-3 font-medium">Amount</th>
                  <th className="py-2 px-3 font-medium">Fee</th>
                </tr>
              </thead>
              <tbody>
                {transactions.map((tx, i) => (
                  <tr key={String(tx.hash ?? i)} className="border-b border-slate-800 hover:bg-slate-800/50 transition-colors">
                    <td className="py-2 px-3">
                      {tx.hash ? (
                        <div className="flex items-center gap-1">
                          <Link to={`/tx/${tx.hash}`} className="font-mono text-blue-400 hover:text-blue-300 text-xs">
                            {shortHash(String(tx.hash))}
                          </Link>
                          <CopyButton text={String(tx.hash)} />
                        </div>
                      ) : (
                        <span className="text-slate-500 text-xs">--</span>
                      )}
                    </td>
                    <td className="py-2 px-3">
                      <Badge label={String(tx.tx_type ?? tx.type ?? 'unknown')} variant="blue" />
                    </td>
                    <td className="py-2 px-3">
                      {tx.from ? (
                        <DisplayIdentity address={String(tx.from)} link size="xs" />
                      ) : (
                        <span className="text-slate-500 text-xs">--</span>
                      )}
                    </td>
                    <td className="py-2 px-3 font-mono text-xs text-slate-300">
                      {tx.amount != null ? `${formatUdag(Number(tx.amount))} UDAG` : '--'}
                    </td>
                    <td className="py-2 px-3 font-mono text-xs text-slate-300">
                      {tx.fee != null ? `${formatUdag(Number(tx.fee))} UDAG` : '--'}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}
    </div>
  );
}

function InfoRow({
  label,
  value,
  mono,
  copy,
  children,
}: {
  label: string;
  value?: string;
  mono?: boolean;
  copy?: boolean;
  children?: React.ReactNode;
}) {
  return (
    <div className="flex items-start gap-3">
      <span className="text-slate-500 text-sm w-36 shrink-0">{label}</span>
      {children ?? (
        <div className="flex items-center gap-1 min-w-0">
          <span className={`text-sm text-slate-200 break-all ${mono ? 'font-mono' : ''}`}>{value}</span>
          {copy && value && <CopyButton text={value} />}
        </div>
      )}
    </div>
  );
}
