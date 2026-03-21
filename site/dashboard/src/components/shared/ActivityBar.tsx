import { useState } from 'react';

interface ActivityBarProps {
  rounds: Array<{ round: number; vertexCount: number; txCount: number }>;
  maxRounds?: number;
}

export function ActivityBar({ rounds, maxRounds = 20 }: ActivityBarProps) {
  const [hoveredIndex, setHoveredIndex] = useState<number | null>(null);

  if (!rounds || rounds.length === 0) return null;

  // Take last N rounds, sorted ascending (oldest left, newest right)
  const display = rounds
    .slice(-maxRounds)
    .sort((a, b) => a.round - b.round);

  const maxVertices = Math.max(...display.map(r => r.vertexCount), 1);

  return (
    <div className="relative flex items-end gap-[3px] h-10 px-1 mb-3">
      {display.map((r, i) => {
        const heightPercent = Math.max((r.vertexCount / maxVertices) * 100, 8);
        const barColor =
          r.vertexCount >= 4
            ? 'bg-dag-green'
            : r.vertexCount >= 2
              ? 'bg-dag-yellow'
              : 'bg-dag-red';

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
                <div className="bg-dag-card border border-dag-border rounded-md px-2 py-1 text-[10px] text-dag-muted shadow-lg">
                  <span className="text-white font-mono">#{r.round.toLocaleString()}</span>
                  {' '}&mdash;{' '}
                  {r.vertexCount} {r.vertexCount === 1 ? 'vertex' : 'vertices'}, {r.txCount} tx
                </div>
              </div>
            )}
            {/* Tx dot */}
            {r.txCount > 0 && (
              <div className="w-1.5 h-1.5 rounded-full bg-dag-blue mb-0.5 shrink-0" />
            )}
            {/* Bar */}
            <div
              className={`w-full rounded-sm ${barColor} transition-all duration-300 min-w-[4px]`}
              style={{
                height: `${heightPercent}%`,
                opacity: hoveredIndex === i ? 1 : 0.7,
              }}
            />
          </div>
        );
      })}
    </div>
  );
}
