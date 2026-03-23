import { useState, useEffect, useRef } from 'react';

interface ActivityBarProps {
  rounds: Array<{ round: number; vertexCount: number; txCount: number }>;
  maxRounds?: number;
}

export function ActivityBar({ rounds, maxRounds = 20 }: ActivityBarProps) {
  const [hoveredIndex, setHoveredIndex] = useState<number | null>(null);
  const [animatedBars, setAnimatedBars] = useState<Set<number>>(new Set());
  const prevRoundsRef = useRef<number[]>([]);

  if (!rounds || rounds.length === 0) return null;

  // Take last N rounds, sorted ascending (oldest left, newest right)
  const display = rounds
    .slice(-maxRounds)
    .sort((a, b) => a.round - b.round);

  const maxVertices = Math.max(...display.map(r => r.vertexCount), 1);

  // Detect newly arrived rounds and trigger entry animation
  useEffect(() => {
    const currentRoundNums = display.map(r => r.round);
    const prevRoundNums = prevRoundsRef.current;
    const newRounds = new Set<number>();
    for (const r of currentRoundNums) {
      if (!prevRoundNums.includes(r)) {
        newRounds.add(r);
      }
    }
    if (newRounds.size > 0) {
      setAnimatedBars(newRounds);
      // Clear animation class after animation completes
      const timer = setTimeout(() => setAnimatedBars(new Set()), 800);
      prevRoundsRef.current = currentRoundNums;
      return () => clearTimeout(timer);
    }
    prevRoundsRef.current = currentRoundNums;
  }, [display.map(r => r.round).join(',')]);

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

        const isNew = animatedBars.has(r.round);
        const isNewest = i === display.length - 1;

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
            {/* Tx indicator — pulsing dot for rounds with transactions */}
            {r.txCount > 0 && (
              <div className={`w-1.5 h-1.5 rounded-full bg-dag-blue mb-0.5 shrink-0 ${isNewest ? 'animate-pulse' : ''}`}
                style={{ boxShadow: isNewest ? '0 0 6px rgba(59,130,246,0.5)' : undefined }}
              />
            )}
            {/* Bar with entry animation */}
            <div
              className={`w-full rounded-sm ${barColor} min-w-[4px] transition-all duration-300`}
              style={{
                height: isNew ? '0%' : `${heightPercent}%`,
                opacity: hoveredIndex === i ? 1 : isNewest ? 0.9 : 0.65,
                animation: isNew ? `barGrow 0.5s ease-out forwards` : undefined,
                ['--target-height' as string]: `${heightPercent}%`,
                boxShadow: isNewest ? `0 0 8px ${r.vertexCount >= 4 ? 'rgba(34,197,94,0.3)' : r.vertexCount >= 2 ? 'rgba(234,179,8,0.3)' : 'rgba(248,113,113,0.3)'}` : undefined,
              }}
            />
          </div>
        );
      })}
      <style>{`
        @keyframes barGrow {
          from { height: 0%; opacity: 0.3; }
          to { height: var(--target-height); opacity: 0.65; }
        }
      `}</style>
    </div>
  );
}
