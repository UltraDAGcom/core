import { Link } from 'react-router-dom';
import { Badge } from '../shared/Badge.tsx';
import { DisplayIdentity } from '../shared/DisplayIdentity.tsx';
import { tableHeaderStyle, tableCellStyle } from '../../lib/theme';

interface Vertex {
  hash: string;
  validator: string;
  reward_udag?: number;
  reward?: number;
  tx_count: number;
  parent_count: number;
}

interface RoundTableProps {
  rounds: Array<{
    round: number;
    vertices: Vertex[];
    finalized: boolean;
  }>;
}

const thStyle: React.CSSProperties = {
  ...tableHeaderStyle,
  padding: '8px 12px',
  textAlign: 'left',
};

const tdStyle: React.CSSProperties = {
  ...tableCellStyle,
  padding: '10px 12px',
};

export function RoundTable({ rounds }: RoundTableProps) {
  if (rounds.length === 0) {
    return <p style={{ color: 'var(--dag-text-faint)', fontSize: 12, padding: '32px 0', textAlign: 'center' }}>No rounds to display</p>;
  }

  return (
    <div style={{ overflowX: 'auto' }}>
      <table style={{ width: '100%', fontSize: 12, borderCollapse: 'collapse' }}>
        <thead>
          <tr>
            <th style={{ ...thStyle, width: 112 }}>Round</th>
            <th style={{ ...thStyle, width: 80, textAlign: 'center' }}>Vertices</th>
            <th style={{ ...thStyle, width: 64, textAlign: 'center' }}>Txns</th>
            <th style={thStyle}>Validators</th>
            <th style={{ ...thStyle, width: 96 }}>Status</th>
          </tr>
        </thead>
        <tbody>
          {rounds.map((r) => {
            const totalTxs = r.vertices.reduce((sum, v) => sum + v.tx_count, 0);
            return (
              <tr key={r.round} style={{ transition: 'background 0.15s' }}>
                <td style={tdStyle}>
                  <Link to={`/round/${r.round}`} style={{ color: '#00E0C4', textDecoration: 'none', fontFamily: "'DM Mono',monospace", fontWeight: 700, fontSize: 14 }}>
                    #{r.round.toLocaleString()}
                  </Link>
                </td>
                <td style={{ ...tdStyle, fontFamily: "'DM Mono',monospace", textAlign: 'center', color: 'var(--dag-text-secondary)' }}>{r.vertices.length}</td>
                <td style={{ ...tdStyle, fontFamily: "'DM Mono',monospace", textAlign: 'center', color: 'var(--dag-text-secondary)' }}>
                  {totalTxs > 0 ? totalTxs : <span style={{ color: 'var(--dag-text-faint)' }}>0</span>}
                </td>
                <td style={tdStyle}>
                  <div style={{ display: 'flex', flexWrap: 'wrap', gap: 4 }}>
                    {r.vertices.slice(0, 3).map((v) => (
                      <span key={v.hash} style={{ display: 'inline-flex', alignItems: 'center', gap: 4, fontSize: 11 }}>
                        <DisplayIdentity address={v.validator} link size="xs" />
                      </span>
                    ))}
                    {r.vertices.length > 3 && (
                      <span style={{ fontSize: 10, color: 'var(--dag-text-faint)' }}>+{r.vertices.length - 3} more</span>
                    )}
                  </div>
                </td>
                <td style={tdStyle}>
                  <Badge label={r.finalized ? 'Finalized' : 'Pending'} variant={r.finalized ? 'green' : 'yellow'} />
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}
