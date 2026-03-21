const SEAT_CONFIG: { category: string; key: string; max: number; color: string }[] = [
  { category: 'Technical', key: 'technical', max: 7, color: 'bg-dag-blue' },
  { category: 'Business', key: 'business', max: 4, color: 'bg-dag-green' },
  { category: 'Legal', key: 'legal', max: 3, color: 'bg-dag-yellow' },
  { category: 'Academic', key: 'academic', max: 3, color: 'bg-dag-purple' },
  { category: 'Community', key: 'community', max: 2, color: 'bg-dag-red' },
  { category: 'Foundation', key: 'foundation', max: 2, color: 'bg-[#f97316]' },
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
