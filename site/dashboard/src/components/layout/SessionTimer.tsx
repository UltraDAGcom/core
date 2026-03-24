import { Timer } from 'lucide-react';

interface SessionTimerProps {
  secondsLeft: number;
  totalSeconds: number;
}

export function SessionBar({ secondsLeft, totalSeconds }: SessionTimerProps) {
  const fraction = secondsLeft / totalSeconds;
  const urgent = secondsLeft <= 120; // 2 minutes
  const critical = secondsLeft <= 30;

  // Color transitions: accent → yellow → red
  const barColor = critical
    ? 'bg-red-500'
    : urgent
      ? 'bg-amber-400'
      : 'bg-dag-accent/40';

  return (
    <div className="h-[2px] w-full bg-transparent relative overflow-hidden">
      <div
        className={`h-full ${barColor} transition-all duration-1000 ease-linear ${critical ? 'animate-pulse' : ''}`}
        style={{ width: `${fraction * 100}%` }}
      />
    </div>
  );
}

export function SessionBadge({ secondsLeft }: { secondsLeft: number }) {
  // Only show when < 3 minutes remain
  if (secondsLeft > 180) return null;

  const mins = Math.floor(secondsLeft / 60);
  const secs = secondsLeft % 60;
  const critical = secondsLeft <= 30;
  const urgent = secondsLeft <= 120;

  const timeStr = mins > 0
    ? `${mins}:${secs.toString().padStart(2, '0')}`
    : `${secs}s`;

  const color = critical
    ? 'text-red-400 bg-red-500/15 border-red-500/30'
    : urgent
      ? 'text-amber-400 bg-amber-500/15 border-amber-500/30'
      : 'text-slate-400 bg-slate-700/50 border-slate-600/30';

  return (
    <div className={`flex items-center gap-1.5 px-2 py-1 rounded-md border text-[11px] font-mono tabular-nums ${color} ${critical ? 'animate-pulse' : ''}`}>
      <Timer className="w-3 h-3" />
      {timeStr}
    </div>
  );
}
