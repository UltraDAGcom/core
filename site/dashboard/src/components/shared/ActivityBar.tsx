import { useState, useRef, useEffect } from 'react';

interface ActivityBarProps {
  rounds: Array<{ round: number; vertexCount: number; txCount: number }>;
  maxRounds?: number;
}

export function ActivityBar({ rounds, maxRounds = 20 }: ActivityBarProps) {
  const [hoveredIndex, setHoveredIndex] = useState<number | null>(null);
  // Rounds that should play their entry animation (one-shot, cleared after animation ends)
  const [enteringRounds, setEnteringRounds] = useState<Set<number>>(new Set());
  // All rounds we've ever rendered — persists across re-renders
  const knownRef = useRef<Set<number>>(new Set());
  // Whether this is the very first render (don't animate initial batch)
  const isFirstRender = useRef(true);

  if (!rounds || rounds.length === 0) return null;

  const display = rounds
    .slice(-maxRounds)
    .sort((a, b) => a.round - b.round);

  const maxVertices = Math.max(...display.map(r => r.vertexCount), 1);

  // Detect new rounds on data change
  const roundIds = display.map(r => r.round).join(',');
  
  useEffect(() => {
    const currentRounds = display.map(r => r.round);
    const brandNew: number[] = [];

    for (const round of currentRounds) {
      if (!knownRef.current.has(round)) {
        knownRef.current.add(round);
        // Don't animate on first load — only on subsequent data refreshes
        if (!isFirstRender.current) {
          brandNew.push(round);
        }
      }
    }

    isFirstRender.current = false;

    if (brandNew.length > 0) {
      setEnteringRounds(new Set(brandNew));
      // Clear after animation completes (500ms)
      const timer = setTimeout(() => setEnteringRounds(new Set()), 500);
      return () => clearTimeout(timer);
    }

    // Prune old entries from known set
    if (knownRef.current.size > maxRounds * 3) {
      const sorted = [...knownRef.current].sort((a, b) => a - b);
      knownRef.current = new Set(sorted.slice(-maxRounds * 2));
    }
  }, [roundIds]);

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

        const isEntering = enteringRounds.has(r.round);

        return (
          <div
            key={r.round}
            className="relative flex-1 flex flex-col items-center justify-end cursor-pointer"
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
            {/* Bar */}
            <div
              className={`w-full rounded-sm ${barColor} min-w-[4px] transition-opacity duration-200`}
              style={{
                height: `${heightPercent}%`,
                opacity: hoveredIndex === i ? 1 : 0.7,
                transformOrigin: 'bottom',
                animation: isEntering ? 'activityBarIn 0.4s ease-out' : undefined,
              }}
            />
          </div>
        );
      })}
      <style>{`
        @keyframes activityBarIn {
          from { transform: scaleY(0); opacity: 0.2; }
          to { transform: scaleY(1); opacity: 0.7; }
        }
      `}</style>
    </div>
  );
}
