import type { ParsedBounty } from '../../lib/github';

interface BountyFiltersProps {
  category: string;
  status: string;
  onCategoryChange: (cat: string) => void;
  onStatusChange: (status: string) => void;
  bounties: ParsedBounty[];
}

const CATS = [
  { key: 'all', label: 'All' },
  { key: 'security', label: 'Security' },
  { key: 'bug', label: 'Bugs' },
  { key: 'feature', label: 'Features' },
];

const STATUSES = [
  { key: 'open', label: 'Open' },
  { key: 'all', label: 'All' },
];

const pill = (active: boolean): React.CSSProperties => ({
  padding: '4px 10px', borderRadius: 6, fontSize: 10.5, fontWeight: 600, cursor: 'pointer',
  border: 'none', transition: 'all 0.15s',
  background: active ? 'rgba(0,224,196,0.1)' : 'transparent',
  color: active ? '#00E0C4' : 'var(--dag-text-faint)',
});

export function BountyFilters({ category, status, onCategoryChange, onStatusChange, bounties }: BountyFiltersProps) {
  const count = (key: string) => {
    if (key === 'all') return bounties.length;
    if (key === 'security') return bounties.filter(b => b.category.startsWith('security')).length;
    return bounties.filter(b => b.category === key).length;
  };

  return (
    <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 12 }}>
      <div style={{ display: 'flex', gap: 4 }}>
        {CATS.map(c => (
          <button key={c.key} onClick={() => onCategoryChange(c.key)} style={pill(category === c.key)}>
            {c.label} ({count(c.key)})
          </button>
        ))}
      </div>
      <div style={{ display: 'flex', gap: 4 }}>
        {STATUSES.map(s => (
          <button key={s.key} onClick={() => onStatusChange(s.key)} style={pill(status === s.key)}>
            {s.label}
          </button>
        ))}
      </div>
    </div>
  );
}
