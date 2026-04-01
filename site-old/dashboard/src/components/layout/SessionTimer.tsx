import { Timer } from 'lucide-react';

interface SessionTimerProps {
  secondsLeft: number;
  totalSeconds: number;
}

export function SessionBar({ secondsLeft, totalSeconds }: SessionTimerProps) {
  const fraction = secondsLeft / totalSeconds;
  const percent = fraction * 100;
  const urgent = secondsLeft <= 120;
  const critical = secondsLeft <= 30;

  // Gradient and glow color based on urgency
  const gradient = critical
    ? 'from-red-500 via-rose-400 to-red-500'
    : urgent
      ? 'from-amber-500 via-yellow-400 to-amber-500'
      : 'from-indigo-500 via-dag-accent to-purple-500';

  const glow = critical
    ? 'shadow-[0_0_12px_rgba(239,68,68,0.6)]'
    : urgent
      ? 'shadow-[0_0_8px_rgba(245,158,11,0.4)]'
      : 'shadow-[0_0_6px_rgba(99,102,241,0.3)]';

  return (
    <div className="h-[3px] w-full bg-slate-800/50 relative overflow-hidden">
      <div
        className={`h-full bg-gradient-to-r ${gradient} ${glow} transition-all duration-1000 ease-linear ${critical ? 'animate-pulse' : ''}`}
        style={{ width: `${percent}%` }}
      >
        {/* Shimmer effect on the leading edge */}
        <div className="absolute right-0 top-0 h-full w-8 bg-gradient-to-l from-white/30 to-transparent" />
      </div>
    </div>
  );
}

export function SessionBadge({ secondsLeft }: { secondsLeft: number }) {
  if (secondsLeft > 180) return null;

  const mins = Math.floor(secondsLeft / 60);
  const secs = secondsLeft % 60;
  const critical = secondsLeft <= 30;
  const urgent = secondsLeft <= 120;

  const timeStr = mins > 0
    ? `${mins}:${secs.toString().padStart(2, '0')}`
    : `${secs}s`;

  const style = critical
    ? 'text-red-400 bg-red-500/10 border-red-500/30 shadow-[0_0_8px_rgba(239,68,68,0.2)]'
    : urgent
      ? 'text-amber-400 bg-amber-500/10 border-amber-500/30'
      : 'text-slate-400 bg-slate-800/50 border-slate-600/30';

  return (
    <div className={`flex items-center gap-1.5 px-2.5 py-1 rounded-lg border text-[11px] font-mono tabular-nums transition-all ${style} ${critical ? 'animate-pulse' : ''}`}>
      <Timer className="w-3 h-3" />
      {timeStr}
    </div>
  );
}
