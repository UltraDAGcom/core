import { Link } from 'react-router-dom';
import { shortHash, formatUdag } from '../../lib/api.ts';
import { CopyButton } from '../shared/CopyButton.tsx';
import { DisplayIdentity } from '../shared/DisplayIdentity.tsx';

interface VertexCardProps {
  hash: string;
  validator: string;
  reward?: number;
  reward_udag?: number;
  tx_count: number;
  parent_count: number;
  showLink?: boolean;
}

const cardStyle: React.CSSProperties = {
  background: 'var(--dag-card)',
  border: '1px solid var(--dag-border)',
  borderRadius: 10,
  padding: 16,
  transition: 'border-color 0.2s',
};

const labelStyle: React.CSSProperties = {
  fontSize: 10, color: 'var(--dag-text-faint)', letterSpacing: 1,
  textTransform: 'uppercase',
};

const valueStyle: React.CSSProperties = {
  fontFamily: "'DM Mono',monospace", fontSize: 12,
  color: 'var(--dag-text-secondary)',
};

export function VertexCard({ hash, validator, reward, reward_udag, tx_count, parent_count, showLink = true }: VertexCardProps) {
  const rewardDisplay = reward_udag != null ? `${reward_udag} UDAG` : reward != null ? formatUdag(reward) + ' UDAG' : '--';

  return (
    <div style={cardStyle}>
      <div style={{ display: 'flex', alignItems: 'flex-start', justifyContent: 'space-between', marginBottom: 12 }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          <span style={{ fontSize: 10, color: 'var(--dag-text-faint)', letterSpacing: 1, textTransform: 'uppercase' }}>Vertex</span>
          {showLink ? (
            <Link to={`/vertex/${hash}`} style={{ fontFamily: "'DM Mono',monospace", fontSize: 12, color: '#00E0C4', textDecoration: 'none' }}>
              {shortHash(hash)}
            </Link>
          ) : (
            <span style={{ fontFamily: "'DM Mono',monospace", fontSize: 12, color: 'var(--dag-text)' }}>{shortHash(hash)}</span>
          )}
          <CopyButton text={hash} />
        </div>
      </div>
      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 12, fontSize: 12 }}>
        <div>
          <span style={labelStyle}>Validator</span>
          <DisplayIdentity address={validator} link size="xs" />
        </div>
        <div>
          <span style={labelStyle}>Reward</span>
          <p style={valueStyle}>{rewardDisplay}</p>
        </div>
        <div>
          <span style={labelStyle}>Transactions</span>
          <p style={valueStyle}>{tx_count}</p>
        </div>
        <div>
          <span style={labelStyle}>Parents</span>
          <p style={valueStyle}>{parent_count}</p>
        </div>
      </div>
    </div>
  );
}
