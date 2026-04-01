const SEAT_CONFIG: { category: string; key: string; max: number; color: string }[] = [
  { category: 'Engineering', key: 'engineering', max: 5, color: 'bg-blue-500' },
  { category: 'Growth', key: 'growth', max: 3, color: 'bg-emerald-500' },
  { category: 'Legal', key: 'legal', max: 2, color: 'bg-yellow-500' },
  { category: 'Research', key: 'research', max: 2, color: 'bg-purple-500' },
  { category: 'Community', key: 'community', max: 4, color: 'bg-pink-500' },
  { category: 'Operations', key: 'operations', max: 3, color: 'bg-orange-500' },
  { category: 'Security', key: 'security', max: 2, color: 'bg-red-500' },
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
    <div className="space-y-3">
      {SEAT_CONFIG.map(({ category, key, max: defaultMax, color }) => {
        const seatData = seats?.[key];
        const filled = seatData ? seatData.filled : members.filter(m => m.category === category).length;
        const max = seatData ? seatData.max : defaultMax;
        return (
          <div key={category} className="flex items-center gap-3">
            <span className="text-sm text-dag-muted w-24">{category}</span>
            <div className="flex gap-1.5">
              {Array.from({ length: max }).map((_, i) => (
                <div
                  key={i}
                  className={`w-4 h-4 rounded-full border transition-all duration-300 ${
                    i < filled
                      ? `${color} border-transparent shadow-[0_0_6px_rgba(255,255,255,0.15)]`
                      : 'bg-dag-surface border-dag-border'
                  }`}
                  title={i < filled ? `Seat ${i + 1} filled` : `Seat ${i + 1} empty`}
                />
              ))}
            </div>
            <span className="text-xs text-dag-muted font-mono">{filled}/{max}</span>
          </div>
        );
      })}
    </div>
  );
}
