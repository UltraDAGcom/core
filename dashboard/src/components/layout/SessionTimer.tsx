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

  const barColor = critical
    ? 'linear-gradient(90deg, #EF4444, #FB7185, #EF4444)'
    : urgent
      ? 'linear-gradient(90deg, #F59E0B, #FBBF24, #F59E0B)'
      : 'linear-gradient(90deg, #6366F1, var(--dag-subheading, #7C3AED), #A855F7)';

  const glowShadow = critical
    ? '0 0 12px rgba(239,68,68,0.6)'
    : urgent
      ? '0 0 8px rgba(245,158,11,0.4)'
      : '0 0 6px rgba(99,102,241,0.3)';

  return (
    <div style={{
      height: 3, width: '100%', background: 'rgba(30,41,59,0.5)',
      position: 'relative', overflow: 'hidden',
    }}>
      <div style={{
        height: '100%',
        background: barColor,
        boxShadow: glowShadow,
        transition: 'all 1s linear',
        width: `${percent}%`,
        position: 'relative',
        ...(critical ? { animation: 'pulse 2s ease-in-out infinite' } : {}),
      }}>
        {/* Shimmer effect on the leading edge */}
        <div style={{
          position: 'absolute', right: 0, top: 0, height: '100%', width: 32,
          background: 'linear-gradient(to left, rgba(255,255,255,0.3), transparent)',
        }} />
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

  const badgeStyle: React.CSSProperties = critical
    ? { color: '#F87171', background: 'rgba(239,68,68,0.1)', borderColor: 'rgba(239,68,68,0.3)', boxShadow: '0 0 8px rgba(239,68,68,0.2)' }
    : urgent
      ? { color: '#FBBF24', background: 'rgba(245,158,11,0.1)', borderColor: 'rgba(245,158,11,0.3)' }
      : { color: 'var(--dag-text-muted)', background: 'rgba(30,41,59,0.5)', borderColor: 'rgba(71,85,105,0.3)' };

  return (
    <div style={{
      display: 'flex', alignItems: 'center', gap: 6,
      padding: '4px 10px', borderRadius: 8,
      border: '1px solid', fontSize: 11,
      fontFamily: 'monospace', fontVariantNumeric: 'tabular-nums',
      transition: 'all 0.15s',
      ...badgeStyle,
      ...(critical ? { animation: 'pulse 2s ease-in-out infinite' } : {}),
    }}>
      <Timer size={12} />
      {timeStr}
    </div>
  );
}
