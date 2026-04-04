import { useEffect, useState } from 'react';
import { useParams, Link } from 'react-router-dom';
import { ChevronLeft } from 'lucide-react';
import { getTx, connectToNode, isConnected, shortHash, formatUdag } from '../lib/api.ts';
import { CopyButton } from '../components/shared/CopyButton.tsx';
import { Badge, StatusBadge } from '../components/shared/Badge.tsx';
import { DisplayIdentity } from '../components/shared/DisplayIdentity.tsx';

export function TxDetailPage() {
  const { hash } = useParams<{ hash: string }>();
  const [tx, setTx] = useState<Record<string, unknown> | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');
  const [switchCount, setSwitchCount] = useState(0);

  useEffect(() => {
    const handler = () => setSwitchCount(n => n + 1);
    window.addEventListener('ultradag-network-switch', handler);
    return () => window.removeEventListener('ultradag-network-switch', handler);
  }, []);

  useEffect(() => {
    if (!hash) return;

    const fetchTx = async () => {
      setLoading(true);
      try {
        if (!isConnected()) await connectToNode();
        const data = await getTx(hash);
        setTx(data);
        setError('');
      } catch (e) {
        setError(e instanceof Error ? e.message : 'Transaction not found');
      } finally {
        setLoading(false);
      }
    };

    fetchTx();
  }, [hash, switchCount]);

  if (loading) return <div className="text-slate-500 py-8 text-center">Loading transaction...</div>;
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
  if (!tx) return null;

  const status = String(tx.status ?? 'unknown');
  const round = tx.round != null ? Number(tx.round) : null;
  const vertexHash = tx.vertex_hash ? String(tx.vertex_hash) : null;
  const validator = tx.validator ? String(tx.validator) : null;

  // Transaction details may be nested under "transaction" or flat at the top level
  const txData = (tx.transaction && typeof tx.transaction === 'object') ? tx.transaction as Record<string, unknown> : tx;

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Link to="/explorer" className="text-slate-400 hover:text-slate-200">
          <ChevronLeft className="w-5 h-5" />
        </Link>
        <h1 className="text-xl font-bold text-white">Transaction Detail</h1>
        <StatusBadge status={status} />
      </div>

      <div className="bg-slate-800/50 border border-slate-700 rounded-lg p-4 space-y-3">
        <InfoRow label="Hash" value={hash ?? ''} mono copy />
        <InfoRow label="Status">
          <StatusBadge status={status} />
        </InfoRow>
        {round != null && (
          <InfoRow label="Round">
            <Link to={`/round/${round}`} className="font-mono text-blue-400 hover:text-blue-300">
              #{round.toLocaleString()}
            </Link>
          </InfoRow>
        )}
        {vertexHash && (
          <InfoRow label="Vertex">
            <div className="flex items-center gap-1">
              <Link to={`/vertex/${vertexHash}`} className="font-mono text-blue-400 hover:text-blue-300 text-sm">
                {shortHash(vertexHash)}
              </Link>
              <CopyButton text={vertexHash} />
            </div>
          </InfoRow>
        )}
        {validator && (
          <InfoRow label="Validator">
            <DisplayIdentity address={validator} link size="sm" />
          </InfoRow>
        )}
        {(txData.tx_type ?? txData.type) != null && (
          <InfoRow label="Type">
            <Badge label={String(txData.tx_type ?? txData.type)} variant="blue" />
          </InfoRow>
        )}
        {txData.from != null && (
          <InfoRow label="From">
            <DisplayIdentity address={String(txData.from)} link size="sm" />
          </InfoRow>
        )}
        {txData.to != null && (
          <InfoRow label="To">
            <DisplayIdentity address={String(txData.to)} link size="sm" />
          </InfoRow>
        )}
        {txData.amount != null && <InfoRow label="Amount" value={`${formatUdag(Number(txData.amount))} UDAG`} mono />}
        {txData.fee != null && <InfoRow label="Fee" value={`${formatUdag(Number(txData.fee))} UDAG`} mono />}
        {txData.nonce != null && <InfoRow label="Nonce" value={String(txData.nonce)} mono />}
      </div>
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
      <span className="text-slate-500 text-sm w-28 shrink-0">{label}</span>
      {children ?? (
        <div className="flex items-center gap-1 min-w-0">
          <span className={`text-sm text-slate-200 break-all ${mono ? 'font-mono' : ''}`}>{value}</span>
          {copy && value && <CopyButton text={value} />}
        </div>
      )}
    </div>
  );
}
