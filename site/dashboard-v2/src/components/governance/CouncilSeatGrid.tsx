const SEAT_CONFIG: { category: string; max: number; color: string }[] = [
  { category: 'Technical', max: 7, color: 'bg-dag-blue' },
  { category: 'Business', max: 4, color: 'bg-dag-green' },
  { category: 'Legal', max: 3, color: 'bg-dag-yellow' },
  { category: 'Academic', max: 3, color: 'bg-dag-purple' },
  { category: 'Community', max: 2, color: 'bg-dag-red' },
  { category: 'Foundation', max: 2, color: 'bg-[#f97316]' },
];

interface CouncilMember {
  address: string;
  category: string;
}

interface CouncilSeatGridProps {
  members: CouncilMember[];
}

export function CouncilSeatGrid({ members }: CouncilSeatGridProps) {
  return (
    <div className="space-y-3">
      {SEAT_CONFIG.map(({ category, max, color }) => {
        const filled = members.filter(m => m.category === category).length;
        return (
          <div key={category} className="flex items-center gap-3">
            <span className="text-sm text-dag-muted w-24">{category}</span>
            <div className="flex gap-1.5">
              {Array.from({ length: max }).map((_, i) => (
                <div
                  key={i}
                  className={`w-4 h-4 rounded-full border ${
                    i < filled
                      ? `${color} border-transparent`
                      : 'bg-dag-surface border-dag-border'
                  }`}
                  title={i < filled ? `Seat ${i + 1} filled` : `Seat ${i + 1} empty`}
                />
              ))}
            </div>
            <span className="text-xs text-dag-muted">{filled}/{max}</span>
          </div>
        );
      })}
    </div>
  );
}
