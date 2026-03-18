import { Link } from 'react-router-dom';
import { shortHash, shortAddr, formatUdag } from '../../lib/api.ts';
import { CopyButton } from '../shared/CopyButton.tsx';
import { Badge } from '../shared/Badge.tsx';
import { Pagination } from '../shared/Pagination.tsx';
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

export function MempoolTable({ transactions }: MempoolTableProps) {
  const [page, setPage] = useState(1);
  const totalPages = Math.ceil(transactions.length / PAGE_SIZE);
  const paged = transactions.slice((page - 1) * PAGE_SIZE, page * PAGE_SIZE);

  if (transactions.length === 0) {
    return <p className="text-slate-500 text-sm py-4 text-center">Mempool is empty</p>;
  }

  return (
    <div>
      <div className="overflow-x-auto">
        <table className="w-full text-sm">
          <thead>
            <tr className="text-left text-slate-400 border-b border-slate-700">
              <th className="py-2 px-3 font-medium">Hash</th>
              <th className="py-2 px-3 font-medium">Type</th>
              <th className="py-2 px-3 font-medium">From</th>
              <th className="py-2 px-3 font-medium">Fee</th>
            </tr>
          </thead>
          <tbody>
            {paged.map((tx, i) => (
              <tr key={tx.hash ?? i} className="border-b border-slate-800 hover:bg-slate-800/50 transition-colors">
                <td className="py-2 px-3">
                  <div className="flex items-center gap-1">
                    {tx.hash ? (
                      <>
                        <Link to={`/tx/${tx.hash}`} className="font-mono text-blue-400 hover:text-blue-300 text-xs">
                          {shortHash(tx.hash)}
                        </Link>
                        <CopyButton text={tx.hash} />
                      </>
                    ) : (
                      <span className="text-slate-500 text-xs">--</span>
                    )}
                  </div>
                </td>
                <td className="py-2 px-3">
                  <Badge label={txTypeLabel(tx)} variant="blue" />
                </td>
                <td className="py-2 px-3">
                  {tx.from ? (
                    <div className="flex items-center gap-1">
                      <Link to={`/address/${tx.from}`} className="font-mono text-slate-300 hover:text-blue-400 text-xs">
                        {shortAddr(tx.from)}
                      </Link>
                      <CopyButton text={tx.from} />
                    </div>
                  ) : (
                    <span className="text-slate-500 text-xs">--</span>
                  )}
                </td>
                <td className="py-2 px-3 font-mono text-slate-300 text-xs">
                  {tx.fee != null ? formatUdag(tx.fee) : '--'}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <Pagination currentPage={page} totalPages={totalPages} onPageChange={setPage} />
    </div>
  );
}
