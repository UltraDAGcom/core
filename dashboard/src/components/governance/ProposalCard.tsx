import { StatusBadge } from '../shared/StatusBadge';

interface ProposalCardProps {
  id: number;
  title: string;
  status: string;
  proposal_type: string;
  votes_for: number;
  votes_against: number;
  council_size: number;
  onClick: () => void;
}

const cardStyle: React.CSSProperties = {
  width: '100%',
  textAlign: 'left' as const,
  borderRadius: 8,
  background: 'var(--dag-input-bg)',
  border: '1px solid var(--dag-border)',
  padding: 16,
  cursor: 'pointer',
  transition: 'all 0.15s',
};

export function ProposalCard({
  id,
  title,
  status,
  proposal_type,
  votes_for,
  votes_against,
  council_size,
  onClick,
}: ProposalCardProps) {
  const totalVotes = votes_for + votes_against;
  const approvalPct = totalVotes > 0 ? Math.round((votes_for / totalVotes) * 100) : 0;
  const quorumPct = council_size > 0 ? Math.round((totalVotes / council_size) * 100) : 0;

  return (
    <button onClick={onClick} style={cardStyle}>
      <div style={{ display: 'flex', alignItems: 'flex-start', justifyContent: 'space-between', gap: 12 }}>
        <div style={{ minWidth: 0, flex: 1 }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 4, flexWrap: 'wrap' }}>
            <span style={{ fontSize: 10, color: 'var(--dag-text-muted)' }}>#{id}</span>
            <StatusBadge status={status} />
            <span style={{
              fontSize: 10, color: 'var(--dag-text-muted)',
              padding: '2px 6px', borderRadius: 4,
              background: 'var(--dag-card)', border: '1px solid var(--dag-border)',
              overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap',
              maxWidth: 200,
            }}>
              {proposal_type}
            </span>
          </div>
          <h4 style={{
            color: 'var(--dag-text)', fontWeight: 500,
            overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap',
          }}>
            {title}
          </h4>
        </div>
      </div>

      <div style={{ marginTop: 12, display: 'grid', gridTemplateColumns: 'repeat(3, 1fr)', gap: 12, fontSize: 12 }}>
        <div>
          <span style={{ color: 'var(--dag-text-muted)', display: 'block', fontSize: 10 }}>For</span>
          <span style={{ color: '#00E0C4' }}>{votes_for} ({approvalPct}%)</span>
        </div>
        <div>
          <span style={{ color: 'var(--dag-text-muted)', display: 'block', fontSize: 10 }}>Against</span>
          <span style={{ color: '#EF4444' }}>{votes_against}</span>
        </div>
        <div>
          <span style={{ color: 'var(--dag-text-muted)', display: 'block', fontSize: 10 }}>Quorum</span>
          <span style={{ color: 'var(--dag-text)' }}>{quorumPct}% of {council_size}</span>
        </div>
      </div>
    </button>
  );
}
