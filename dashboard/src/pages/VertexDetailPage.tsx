import { useEffect, useState } from 'react';
import { useParams, Link } from 'react-router-dom';
import { getVertex, connectToNode, isConnected, shortHash, formatUdag } from '../lib/api.ts';
import { CopyButton } from '../components/shared/CopyButton.tsx';
import { Badge } from '../components/shared/Badge.tsx';
import { DisplayIdentity } from '../components/shared/DisplayIdentity.tsx';
import { PageHeader } from '../components/shared/PageHeader.tsx';
import { useIsMobile } from '../hooks/useIsMobile';

export function VertexDetailPage() {
  const { hash } = useParams<{ hash: string }>();
  const m = useIsMobile();
  const [vertex, setVertex] = useState<Record<string, unknown> | null>(null);
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
  }, [hash, switchCount]);

  if (loading) return <div style={{ color: 'var(--dag-text-faint)', padding: '32px 0', textAlign: 'center' }}>Loading vertex...</div>;
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
    <div style={{ padding: m ? '12px 14px' : '18px 26px', fontFamily: "'DM Sans',sans-serif" }}>
      <PageHeader title="Vertex Detail" subtitle={hash ? `${shortHash(hash)}` : undefined} />
      <div style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>

      {/* Vertex info */}
      <div style={{ background: 'var(--dag-card)', border: '1px solid var(--dag-border)', borderRadius: 10, padding: 16, display: 'flex', flexDirection: 'column', gap: 12 }}>
        <InfoRow label="Hash" value={hash ?? ''} mono copy />
        <InfoRow label="Round">
          <Link to={`/round/${round}`} style={{ fontFamily: "'DM Mono',monospace", color: '#00E0C4', textDecoration: 'none' }}>
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
        <div style={{ background: 'var(--dag-card)', border: '1px solid var(--dag-border)', borderRadius: 10, padding: 16 }}>
          <h2 style={{ fontSize: 12, fontWeight: 600, color: 'var(--dag-text)', marginBottom: 12 }}>Parent Vertices ({parentCount})</h2>
          <p style={{ fontSize: 12, color: 'var(--dag-text-faint)' }}>{parentCount} parent{parentCount !== 1 ? 's' : ''} (hashes not available in this view)</p>
        </div>
      )}

      {/* Transactions */}
      {transactions.length > 0 && (
        <div style={{ background: 'var(--dag-card)', border: '1px solid var(--dag-border)', borderRadius: 10, padding: 16 }}>
          <h2 style={{ fontSize: 12, fontWeight: 600, color: 'var(--dag-text)', marginBottom: 12 }}>Transactions ({transactions.length})</h2>
          <div style={{ overflowX: 'auto' }}>
            <table style={{ width: '100%', fontSize: 12, borderCollapse: 'collapse' }}>
              <thead>
                <tr style={{ textAlign: 'left', color: 'var(--dag-text-muted)', borderBottom: '1px solid var(--dag-border)' }}>
                  <th style={{ padding: '8px 12px', fontWeight: 500 }}>Hash</th>
                  <th style={{ padding: '8px 12px', fontWeight: 500 }}>Type</th>
                  <th style={{ padding: '8px 12px', fontWeight: 500 }}>From</th>
                  <th style={{ padding: '8px 12px', fontWeight: 500 }}>Amount</th>
                  <th style={{ padding: '8px 12px', fontWeight: 500 }}>Fee</th>
                </tr>
              </thead>
              <tbody>
                {transactions.map((tx, i) => (
                  <tr key={String(tx.hash ?? i)} style={{ borderBottom: '1px solid var(--dag-border)' }}>
                    <td style={{ padding: '8px 12px' }}>
                      {tx.hash ? (
                        <div style={{ display: 'flex', alignItems: 'center', gap: 4 }}>
                          <Link to={`/tx/${tx.hash}`} style={{ fontFamily: "'DM Mono',monospace", color: '#00E0C4', fontSize: 10, textDecoration: 'none' }}>
                            {shortHash(String(tx.hash))}
                          </Link>
                          <CopyButton text={String(tx.hash)} />
                        </div>
                      ) : (
                        <span style={{ color: 'var(--dag-text-faint)', fontSize: 10 }}>--</span>
                      )}
                    </td>
                    <td style={{ padding: '8px 12px' }}>
                      <Badge label={String(tx.tx_type ?? tx.type ?? 'unknown')} variant="blue" />
                    </td>
                    <td style={{ padding: '8px 12px' }}>
                      {tx.from ? (
                        <DisplayIdentity address={String(tx.from)} link size="xs" />
                      ) : (
                        <span style={{ color: 'var(--dag-text-faint)', fontSize: 10 }}>--</span>
                      )}
                    </td>
                    <td style={{ padding: '8px 12px', fontFamily: "'DM Mono',monospace", fontSize: 10, color: 'var(--dag-text-secondary)' }}>
                      {tx.amount != null ? `${formatUdag(Number(tx.amount))} UDAG` : '--'}
                    </td>
                    <td style={{ padding: '8px 12px', fontFamily: "'DM Mono',monospace", fontSize: 10, color: 'var(--dag-text-secondary)' }}>
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
      <span style={{ color: 'var(--dag-text-faint)', fontSize: 12, width: 144, flexShrink: 0 }}>{label}</span>
      {children ?? (
        <div style={{ display: 'flex', alignItems: 'center', gap: 4, minWidth: 0 }}>
          <span style={{ fontSize: 12, color: 'var(--dag-text)', wordBreak: 'break-all', fontFamily: mono ? "'DM Mono',monospace" : undefined }}>{value}</span>
          {copy && value && <CopyButton text={value} />}
        </div>
      )}
    </div>
  );
}
