import { severityInfo, statusColor, bountyDescription, timeAgo, type ParsedBounty } from '../../lib/github';
import { DisplayIdentity } from '../shared/DisplayIdentity';
import { primaryButtonStyle, secondaryButtonStyle } from '../../lib/theme';

interface BountyDetailProps {
  bounty: ParsedBounty | null;
  unlocked: boolean;
  onPayClick: () => void;
}

const S = {
  statBox: { background: 'var(--dag-card)', borderRadius: 8, padding: '8px 10px' } as React.CSSProperties,
  statLabel: { fontSize: 9, color: 'var(--dag-text-faint)', letterSpacing: 1, marginBottom: 2 } as React.CSSProperties,
  statValue: { fontSize: 13, fontWeight: 600, color: '#fff' } as React.CSSProperties,
};

export function BountyDetail({ bounty, unlocked, onPayClick }: BountyDetailProps) {
  if (!bounty) {
    return (
      <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center', height: '100%', padding: '40px 20px', textAlign: 'center' }}>
        <div style={{ fontSize: 36, color: 'var(--dag-text-faint)', marginBottom: 12, opacity: 0.3 }}>⚡</div>
        <p style={{ fontSize: 12, color: 'var(--dag-text-faint)' }}>Select a bounty to view details</p>
      </div>
    );
  }

  const sev = severityInfo(bounty.category);
  const desc = bountyDescription(bounty.issue.body);
  const canPay = unlocked && (bounty.status === 'approved' || bounty.status === 'open');

  return (
    <div style={{ animation: 'slideUp 0.3s ease' }}>
      {/* Header */}
      <div style={{ display: 'flex', gap: 6, alignItems: 'center', marginBottom: 8 }}>
        <span style={{
          fontSize: 9, fontWeight: 700, padding: '2px 8px', borderRadius: 4,
          background: `${sev.color}15`, color: sev.color, letterSpacing: 0.5,
        }}>{sev.label.toUpperCase()}</span>
        <span style={{
          fontSize: 9, fontWeight: 600, padding: '2px 8px', borderRadius: 4,
          background: `${statusColor(bounty.status)}15`, color: statusColor(bounty.status),
        }}>{bounty.status.toUpperCase()}</span>
        <span style={{ fontSize: 10, color: 'var(--dag-text-faint)', marginLeft: 'auto' }}>#{bounty.issue.number}</span>
      </div>

      <h3 style={{ fontSize: 15, fontWeight: 700, color: 'var(--dag-text)', marginBottom: 12, lineHeight: 1.4 }}>
        {bounty.issue.title.replace(/\[\d+(?:\.\d+)?\s*UDAG\]/i, '').trim()}
      </h3>

      {/* Description */}
      {desc && (
        <div style={{
          fontSize: 11.5, color: 'var(--dag-text-muted)', lineHeight: 1.6, marginBottom: 14,
          whiteSpace: 'pre-wrap', maxHeight: 200, overflowY: 'auto',
          padding: '10px 12px', background: 'var(--dag-input-bg)', borderRadius: 8,
        }}>
          {desc}
        </div>
      )}

      {/* Stats grid */}
      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 8, marginBottom: 14 }}>
        <div style={S.statBox}>
          <div style={S.statLabel}>REWARD</div>
          <div style={{ ...S.statValue, color: '#00E0C4', fontFamily: "'DM Mono',monospace" }}>
            {bounty.reward > 0 ? `${bounty.reward.toLocaleString()} UDAG` : 'TBD'}
          </div>
        </div>
        <div style={S.statBox}>
          <div style={S.statLabel}>CATEGORY</div>
          <div style={{ ...S.statValue, color: sev.color }}>{sev.label}</div>
        </div>
        {bounty.creatorAddress && (
          <div style={S.statBox}>
            <div style={S.statLabel}>CREATOR</div>
            <DisplayIdentity address={bounty.creatorAddress} link size="xs" />
          </div>
        )}
        <div style={S.statBox}>
          <div style={S.statLabel}>COMMENTS</div>
          <div style={S.statValue}>{bounty.issue.comments}</div>
        </div>
        <div style={S.statBox}>
          <div style={S.statLabel}>CREATED</div>
          <div style={{ fontSize: 11, color: 'var(--dag-text-muted)' }}>{timeAgo(bounty.issue.created_at)}</div>
        </div>
        <div style={S.statBox}>
          <div style={S.statLabel}>UPDATED</div>
          <div style={{ fontSize: 11, color: 'var(--dag-text-muted)' }}>{timeAgo(bounty.issue.updated_at)}</div>
        </div>
      </div>

      {/* Actions */}
      <div style={{ display: 'flex', gap: 8, paddingTop: 12, borderTop: '1px solid var(--dag-table-border)' }}>
        <a
          href={bounty.issue.html_url}
          target="_blank"
          rel="noopener noreferrer"
          style={{
            ...secondaryButtonStyle, flex: 1, padding: '10px 0', textAlign: 'center' as const,
            textDecoration: 'none',
          }}
        >
          View on GitHub →
        </a>
        {canPay && (
          <button onClick={onPayClick} style={{
            ...primaryButtonStyle, flex: 1, padding: '10px 0',
          }}>
            Pay Bounty
          </button>
        )}
      </div>
    </div>
  );
}
