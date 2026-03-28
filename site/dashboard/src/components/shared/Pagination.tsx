import { useIsMobile } from '../../hooks/useIsMobile';

interface PaginationProps {
  page: number;
  totalPages: number;
  onPageChange: (page: number) => void;
  pageSize?: number;
  onPageSizeChange?: (size: number) => void;
  totalItems?: number;
}

const S = {
  wrap: {
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'space-between',
    marginTop: 12,
    gap: 8,
    flexWrap: 'wrap' as const,
  },
  btn: (disabled: boolean) => ({
    padding: '4px 10px',
    borderRadius: 6,
    background: 'var(--dag-card)',
    border: '1px solid var(--dag-border)',
    color: disabled ? 'var(--dag-text-faint)' : 'var(--dag-text-muted)',
    fontSize: 10.5,
    fontWeight: 500 as const,
    cursor: disabled ? 'default' : 'pointer',
    opacity: disabled ? 0.4 : 1,
    transition: 'all 0.15s',
  }),
  pageBtn: (active: boolean) => ({
    padding: '4px 9px',
    borderRadius: 6,
    background: active ? 'rgba(0,102,255,0.15)' : 'var(--dag-card)',
    border: active ? '1px solid rgba(0,102,255,0.3)' : '1px solid var(--dag-border)',
    color: active ? '#4D9AFF' : 'var(--dag-text-muted)',
    fontSize: 10.5,
    fontWeight: active ? 600 : (500 as const),
    cursor: active ? 'default' : 'pointer',
    transition: 'all 0.15s',
  }),
  info: {
    fontSize: 10,
    color: 'var(--dag-text-faint)',
    fontFamily: "'DM Sans',sans-serif",
    whiteSpace: 'nowrap' as const,
  },
  select: {
    padding: '3px 6px',
    borderRadius: 5,
    background: 'var(--dag-input-bg)',
    border: '1px solid var(--dag-border)',
    color: 'var(--dag-text-muted)',
    fontSize: 10,
    outline: 'none',
    cursor: 'pointer',
    fontFamily: "'DM Sans',sans-serif",
  },
};

export function Pagination({ page, totalPages, onPageChange, pageSize, onPageSizeChange, totalItems }: PaginationProps) {
  const m = useIsMobile();

  if (totalPages <= 1 && !onPageSizeChange) return null;

  // Build page numbers with ellipsis
  const pages: (number | '...')[] = [];
  const delta = m ? 1 : 2;
  for (let i = 1; i <= totalPages; i++) {
    if (i === 1 || i === totalPages || (i >= page - delta && i <= page + delta)) {
      pages.push(i);
    } else if (pages[pages.length - 1] !== '...') {
      pages.push('...');
    }
  }

  // "Showing X-Y of Z" range
  let rangeText = '';
  if (totalItems != null && pageSize) {
    const from = Math.min((page - 1) * pageSize + 1, totalItems);
    const to = Math.min(page * pageSize, totalItems);
    rangeText = totalItems === 0 ? '0 items' : `${from}–${to} of ${totalItems}`;
  }

  return (
    <div style={S.wrap}>
      {/* Left: showing range */}
      <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
        {rangeText && <span style={S.info}>{rangeText}</span>}
        {onPageSizeChange && pageSize && (
          <select
            value={pageSize}
            onChange={e => onPageSizeChange(Number(e.target.value))}
            style={S.select}
          >
            {[10, 25, 50].map(n => (
              <option key={n} value={n} style={{ background: '#0B1120' }}>{n}/page</option>
            ))}
          </select>
        )}
      </div>

      {/* Right: page controls */}
      {totalPages > 1 && (
        <div style={{ display: 'flex', alignItems: 'center', gap: 3 }}>
          <button
            onClick={() => onPageChange(page - 1)}
            disabled={page === 1}
            style={S.btn(page === 1)}
          >
            Prev
          </button>
          {!m && pages.map((p, i) =>
            p === '...' ? (
              <span key={`dots-${i}`} style={{ padding: '0 3px', fontSize: 10, color: 'var(--dag-text-faint)' }}>...</span>
            ) : (
              <button
                key={p}
                onClick={() => onPageChange(p)}
                style={S.pageBtn(p === page)}
              >
                {p}
              </button>
            )
          )}
          {m && (
            <span style={{ ...S.info, padding: '0 6px' }}>{page}/{totalPages}</span>
          )}
          <button
            onClick={() => onPageChange(page + 1)}
            disabled={page === totalPages}
            style={S.btn(page === totalPages)}
          >
            Next
          </button>
        </div>
      )}
    </div>
  );
}
