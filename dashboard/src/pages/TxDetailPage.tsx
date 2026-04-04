import { useEffect, useState } from 'react';
import { useParams, Link } from 'react-router-dom';
import { getTx, connectToNode, isConnected, shortHash, formatUdag } from '../lib/api.ts';
import { CopyButton } from '../components/shared/CopyButton.tsx';
import { Badge, StatusBadge } from '../components/shared/Badge.tsx';
import { DisplayIdentity } from '../components/shared/DisplayIdentity.tsx';
import { PageHeader } from '../components/shared/PageHeader.tsx';
import { useIsMobile } from '../hooks/useIsMobile';

export function TxDetailPage() {
  const { hash } = useParams<{ hash: string }>();
  const m = useIsMobile();
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

  if (loading) return <div style={{ color: 'var(--dag-text-faint)', padding: '32px 0', textAlign: 'center' }}>Loading transaction...</div>;
  if (error) {
    return (
      <div style={{ display: 'flex', flexDirection: 'column', gap: 16, padding: m ? '12px 14px' : '18px 26px' }}>
        <Link to="/explorer" style={{ display: 'inline-flex', alignItems: 'center', gap: 4, color: 'var(--dag-text-muted)', fontSize: 12, textDecoration: 'none' }}>
          Back to Explorer
        </Link>
        <div style={{ background: 'rgba(239,68,68,0.06)', border: '1px solid rgba(239,68,68,0.15)', borderRadius: 10, padding: 12, fontSize: 12, color: '#EF4444' }}>{error}</div>
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
    <div style={{ padding: m ? '12px 14px' : '18px 26px', fontFamily: "'DM Sans',sans-serif" }}>
      <PageHeader title="Transaction Detail" subtitle={hash ? `${shortHash(hash)}` : undefined} right={<StatusBadge status={status} />} />
      <div style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>

      <div style={{ background: 'var(--dag-card)', border: '1px solid var(--dag-border)', borderRadius: 10, padding: 16, display: 'flex', flexDirection: 'column', gap: 12 }}>
        <InfoRow label="Hash" value={hash ?? ''} mono copy />
        <InfoRow label="Status">
          <StatusBadge status={status} />
        </InfoRow>
        {round != null && (
          <InfoRow label="Round">
            <Link to={`/round/${round}`} style={{ fontFamily: "'DM Mono',monospace", color: '#00E0C4', textDecoration: 'none' }}>
              #{round.toLocaleString()}
            </Link>
          </InfoRow>
        )}
        {vertexHash && (
          <InfoRow label="Vertex">
            <div style={{ display: 'flex', alignItems: 'center', gap: 4 }}>
              <Link to={`/vertex/${vertexHash}`} style={{ fontFamily: "'DM Mono',monospace", color: '#00E0C4', fontSize: 12, textDecoration: 'none' }}>
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
    <div style={{ display: 'flex', alignItems: 'flex-start', gap: 12 }}>
      <span style={{ color: 'var(--dag-text-faint)', fontSize: 12, width: 112, flexShrink: 0 }}>{label}</span>
      {children ?? (
        <div style={{ display: 'flex', alignItems: 'center', gap: 4, minWidth: 0 }}>
          <span style={{ fontSize: 12, color: 'var(--dag-text)', wordBreak: 'break-all', fontFamily: mono ? "'DM Mono',monospace" : undefined }}>{value}</span>
          {copy && value && <CopyButton text={value} />}
        </div>
      )}
    </div>
  );
}
