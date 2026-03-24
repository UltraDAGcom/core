import { useState, useRef, useEffect } from 'react';

interface ActivityBarProps {
  rounds: Array<{ round: number; vertexCount: number; txCount: number }>;
  maxRounds?: number;
}

export function ActivityBar({ rounds, maxRounds = 20 }: ActivityBarProps) {
  const [hoveredIndex, setHoveredIndex] = useState<number | null>(null);
  // Track the last known max round to detect new rounds
  const lastMaxRoundRef = useRef<number>(0);
  // Whether a new round just arrived (triggers wave animation)
  const [waveRound, setWaveRound] = useState<number>(0);
  // Track previous vertex counts to detect changes
  const prevVerticesRef = useRef<Map<number, number>>(new Map());

  if (!rounds || rounds.length === 0) return null;

  const display = rounds
    .slice(-maxRounds)
    .sort((a, b) => a.round - b.round);

  const maxVertices = Math.max(...display.map(r => r.vertexCount), 1);
  const currentMaxRound = display[display.length - 1]?.round || 0;

  // Detect new rounds and height changes
  useEffect(() => {
    const prevVertices = prevVerticesRef.current;
    let hasNewRound = currentMaxRound > lastMaxRoundRef.current;
    let hasHeightChanges = false;

    // Check for height changes on existing bars
    for (const r of display) {
      const prevHeight = prevVertices.get(r.round);
      if (prevHeight !== undefined && prevHeight !== r.vertexCount) {
        hasHeightChanges = true;
        break;
      }
    }

    if (hasNewRound) {
      lastMaxRoundRef.current = currentMaxRound;
      setWaveRound(prev => prev + 1);
    }

    // Update stored vertex counts
    for (const r of display) {
      prevVertices.set(r.round, r.vertexCount);
    }
  }, [currentMaxRound, display.map(r => `${r.round}-${r.vertexCount}`).join(',')]);

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

        const isLatest = r.round === currentMaxRound;
        const prevHeight = prevVerticesRef.current.get(r.round) ?? r.vertexCount;
        const heightChanged = prevHeight !== r.vertexCount;

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
              className={`w-full rounded-sm ${barColor} min-w-[4px]`}
              style={{
                height: `${heightPercent}%`,
                opacity: hoveredIndex === i ? 1 : isLatest ? 0.85 : 0.7,
                transformOrigin: 'bottom',
                transition: 'height 0.5s ease, opacity 0.2s ease, box-shadow 0.3s ease',
                boxShadow: isLatest ? '0 0 8px rgba(59, 130, 246, 0.4)' : 'none',
              }}
            />
          </div>
        );
      })}
    </div>
  );
}
