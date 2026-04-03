import { severityInfo, statusColor, timeAgo, type ParsedBounty } from '../../lib/github';

interface BountyCardProps {
  bounty: ParsedBounty;
  active: boolean;
  onClick: () => void;
}

export function BountyCard({ bounty, active, onClick }: BountyCardProps) {
  const sev = severityInfo(bounty.category);

  return (
    <div onClick={onClick} style={{
      padding: '10px 12px', cursor: 'pointer', transition: 'background 0.15s',
      borderBottom: '1px solid var(--dag-row-border)',
      background: active ? 'rgba(0,102,255,0.04)' : 'transparent',
      borderLeft: active ? '2px solid #0066FF' : '2px solid transparent',
    }}>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 4 }}>
        <div style={{ display: 'flex', gap: 6, alignItems: 'center' }}>
          <span style={{
            fontSize: 9, fontWeight: 700, padding: '1px 6px', borderRadius: 3,
            background: `${sev.color}15`, color: sev.color, letterSpacing: 0.5,
          }}>{sev.label.toUpperCase()}</span>
          <span style={{
            fontSize: 9, fontWeight: 600, padding: '1px 6px', borderRadius: 3,
            background: `${statusColor(bounty.status)}15`, color: statusColor(bounty.status),
          }}>{bounty.status.toUpperCase()}</span>
        </div>
        {bounty.reward > 0 && (
          <span style={{ fontSize: 13, fontWeight: 700, color: '#00E0C4', fontFamily: "'DM Mono',monospace" }}>
            {bounty.reward.toLocaleString()} UDAG
          </span>
        )}
      </div>
      <div style={{
        fontSize: 12, fontWeight: 600, color: 'var(--dag-text)',
        overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', marginBottom: 4,
      }}>
        {bounty.issue.title.replace(/\[\d+(?:\.\d+)?\s*UDAG\]/i, '').trim()}
      </div>
      <div style={{ fontSize: 10, color: 'var(--dag-text-faint)', display: 'flex', gap: 8 }}>
        {bounty.issue.user && <span>{bounty.issue.user.login}</span>}
        <span>{bounty.issue.comments} comments</span>
        <span>{timeAgo(bounty.issue.updated_at)}</span>
      </div>
    </div>
  );
}
