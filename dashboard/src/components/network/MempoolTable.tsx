import { Link } from 'react-router-dom';
import { shortHash, formatUdag } from '../../lib/api.ts';
import { CopyButton } from '../shared/CopyButton.tsx';
import { DisplayIdentity } from '../shared/DisplayIdentity.tsx';
import { Badge } from '../shared/Badge.tsx';
import { Pagination } from '../shared/Pagination.tsx';
import { tableHeaderStyle, tableCellStyle } from '../../lib/theme';
import { useState } from 'react';

interface MempoolTx {
  hash: string;
  from?: string;
  to?: string;
  amount?: number;
  fee?: number;
  type?: string;
  tx_type?: string;
}

interface MempoolTableProps {
  transactions: MempoolTx[];
}

const PAGE_SIZE = 10;

function txTypeLabel(tx: MempoolTx): string {
  return tx.tx_type ?? tx.type ?? 'unknown';
}

const thStyle: React.CSSProperties = {
  ...tableHeaderStyle,
  padding: '8px 12px',
  textAlign: 'left',
};

const tdStyle: React.CSSProperties = {
  ...tableCellStyle,
  padding: '8px 12px',
};

export function MempoolTable({ transactions }: MempoolTableProps) {
  const [page, setPage] = useState(1);
  const totalPages = Math.ceil(transactions.length / PAGE_SIZE);
  const paged = transactions.slice((page - 1) * PAGE_SIZE, page * PAGE_SIZE);

  if (transactions.length === 0) {
    return <p style={{ color: 'var(--dag-text-faint)', fontSize: 12, padding: '16px 0', textAlign: 'center' }}>Mempool is empty</p>;
  }

  return (
    <div>
      <div style={{ overflowX: 'auto' }}>
        <table style={{ width: '100%', fontSize: 12, borderCollapse: 'collapse' }}>
          <thead>
            <tr>
              <th style={thStyle}>Hash</th>
              <th style={thStyle}>Type</th>
              <th style={thStyle}>From</th>
              <th style={thStyle}>Fee</th>
            </tr>
          </thead>
          <tbody>
            {paged.map((tx, i) => (
              <tr key={tx.hash ?? i} style={{ transition: 'background 0.15s' }}>
                <td style={tdStyle}>
                  <div style={{ display: 'flex', alignItems: 'center', gap: 4 }}>
                    {tx.hash ? (
                      <>
                        <Link to={`/tx/${tx.hash}`} style={{ fontFamily: "'DM Mono',monospace", color: '#00E0C4', textDecoration: 'none', fontSize: 11 }}>
                          {shortHash(tx.hash)}
                        </Link>
                        <CopyButton text={tx.hash} />
                      </>
                    ) : (
                      <span style={{ color: 'var(--dag-text-faint)', fontSize: 11 }}>--</span>
                    )}
                  </div>
                </td>
                <td style={tdStyle}>
                  <Badge label={txTypeLabel(tx)} variant="blue" />
                </td>
                <td style={tdStyle}>
                  {tx.from ? (
                    <DisplayIdentity address={tx.from} link size="xs" />
                  ) : (
                    <span style={{ color: 'var(--dag-text-faint)', fontSize: 11 }}>--</span>
                  )}
                </td>
                <td style={{ ...tdStyle, fontFamily: "'DM Mono',monospace", color: 'var(--dag-text-secondary)', fontSize: 11 }}>
                  {tx.fee != null ? formatUdag(tx.fee) : '--'}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <Pagination page={page} totalPages={totalPages} onPageChange={setPage} />
    </div>
  );
}
