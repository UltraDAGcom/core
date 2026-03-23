import { useState, useRef } from 'react';

interface ActivityBarProps {
  rounds: Array<{ round: number; vertexCount: number; txCount: number }>;
  maxRounds?: number;
}

export function ActivityBar({ rounds, maxRounds = 20 }: ActivityBarProps) {
  const [hoveredIndex, setHoveredIndex] = useState<number | null>(null);
  // Track which rounds we've already seen so we can animate new arrivals once
  const seenRoundsRef = useRef<Set<number>>(new Set());

  if (!rounds || rounds.length === 0) return null;

  // Take last N rounds, sorted ascending (oldest left, newest right)
  const display = rounds
    .slice(-maxRounds)
    .sort((a, b) => a.round - b.round);

  const maxVertices = Math.max(...display.map(r => r.vertexCount), 1);

  // Determine which bars are new (never seen before)
  const newThisRender: Set<number> = new Set();
  for (const r of display) {
    if (!seenRoundsRef.current.has(r.round)) {
      newThisRender.add(r.round);
    }
  }
  // Mark all current rounds as seen (after checking)
  // Use a microtask so the render sees them as "new" first
  if (newThisRender.size > 0) {
    Promise.resolve().then(() => {
      for (const r of newThisRender) {
        seenRoundsRef.current.add(r);
      }
      // Keep the set bounded
      if (seenRoundsRef.current.size > maxRounds * 2) {
        const arr = [...seenRoundsRef.current].sort((a, b) => a - b);
        seenRoundsRef.current = new Set(arr.slice(-maxRounds));
      }
    });
  }

  return (
    <div className="relative flex items-end gap-[3px] h-12 px-1 mb-3">
      {display.map((r, i) => {
        const heightPercent = Math.max((r.vertexCount / maxVertices) * 100, 8);
        const barColor =
          r.vertexCount >= 4
            ? 'bg-dag-green'
            : r.vertexCount >= 2
              ? 'bg-dag-yellow'
              : 'bg-dag-red';

        const isNew = newThisRender.has(r.round);

        return (
          <div
            key={r.round}
            className="relative flex-1 flex flex-col items-center justify-end cursor-pointer group"
            style={{ height: '100%' }}
            onMouseEnter={() => setHoveredIndex(i)}
            onMouseLeave={() => setHoveredIndex(null)}
          >
            {/* Tooltip */}
            {hoveredIndex === i && (
              <div className="absolute bottom-full mb-2 left-1/2 -translate-x-1/2 z-10 whitespace-nowrap pointer-events-none">
                <div className="bg-dag-card border border-dag-border rounded-md px-2.5 py-1.5 text-[10px] text-dag-muted shadow-lg">
                  <div className="flex items-center gap-1.5">
                    <span className="text-white font-mono font-semibold">#{r.round.toLocaleString()}</span>
                    <span className="text-dag-border">|</span>
                    <span>{r.vertexCount} {r.vertexCount === 1 ? 'vertex' : 'vertices'}</span>
                    {r.txCount > 0 && (
                      <>
                        <span className="text-dag-border">|</span>
                        <span className="text-dag-blue">{r.txCount} tx</span>
                      </>
                    )}
                  </div>
                </div>
              </div>
            )}
            {/* Tx indicator dot */}
            {r.txCount > 0 && (
              <div className="w-1.5 h-1.5 rounded-full bg-dag-blue mb-0.5 shrink-0" />
            )}
            {/* Bar — new bars animate in once via CSS animation, then stay */}
            <div
              className={`w-full rounded-sm ${barColor} min-w-[4px]`}
              style={{
                height: `${heightPercent}%`,
                opacity: hoveredIndex === i ? 1 : 0.7,
                transition: isNew ? 'none' : 'all 0.3s ease',
                animation: isNew ? `activityBarIn 0.4s ease-out` : undefined,
              }}
            />
          </div>
        );
      })}
      <style>{`
        @keyframes activityBarIn {
          from { transform: scaleY(0); opacity: 0; }
          to { transform: scaleY(1); opacity: 0.7; }
        }
      `}</style>
    </div>
  );
}
