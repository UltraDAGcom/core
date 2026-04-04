const SEAT_CONFIG: { category: string; key: string; max: number; color: string }[] = [
  { category: 'Engineering', key: 'engineering', max: 5, color: '#3B82F6' },
  { category: 'Growth', key: 'growth', max: 3, color: '#10B981' },
  { category: 'Legal', key: 'legal', max: 2, color: '#EAB308' },
  { category: 'Research', key: 'research', max: 2, color: '#A855F7' },
  { category: 'Community', key: 'community', max: 4, color: '#EC4899' },
  { category: 'Operations', key: 'operations', max: 3, color: '#F97316' },
  { category: 'Security', key: 'security', max: 2, color: '#EF4444' },
];

interface CouncilMember {
  address: string;
  category: string;
}

interface SeatInfo {
  available: number;
  filled: number;
  max: number;
}

interface CouncilSeatGridProps {
  members: CouncilMember[];
  seats?: Record<string, SeatInfo>;
}

export function CouncilSeatGrid({ members, seats }: CouncilSeatGridProps) {
  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
      {SEAT_CONFIG.map(({ category, key, max: defaultMax, color }) => {
        const seatData = seats?.[key];
        const filled = seatData ? seatData.filled : members.filter(m => m.category === category).length;
        const max = seatData ? seatData.max : defaultMax;
        return (
          <div key={category} style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
            <span style={{ fontSize: 12, color: 'var(--dag-text-muted)', width: 96 }}>{category}</span>
            <div style={{ display: 'flex', gap: 6 }}>
              {Array.from({ length: max }).map((_, i) => (
                <div
                  key={i}
                  style={{
                    width: 16, height: 16, borderRadius: '50%',
                    transition: 'all 0.3s',
                    ...(i < filled
                      ? { background: color, border: '1px solid transparent', boxShadow: '0 0 6px rgba(255,255,255,0.15)' }
                      : { background: 'var(--dag-input-bg)', border: '1px solid var(--dag-border)' }),
                  }}
                  title={i < filled ? `Seat ${i + 1} filled` : `Seat ${i + 1} empty`}
                />
              ))}
            </div>
            <span style={{ fontSize: 10, color: 'var(--dag-text-muted)', fontFamily: 'monospace' }}>{filled}/{max}</span>
          </div>
        );
      })}
    </div>
  );
}
