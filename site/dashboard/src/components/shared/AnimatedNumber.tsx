import { useState, useEffect, useRef } from 'react';

interface AnimatedNumberProps {
  value: number;
  format?: (n: number) => string;
  className?: string;
  duration?: number; // ms, default 600
}

export function AnimatedNumber({ value, format, className = '', duration = 600 }: AnimatedNumberProps) {
  const [displayValue, setDisplayValue] = useState(value);
  const prevValue = useRef(value);
  const startTime = useRef(0);
  const rafRef = useRef(0);

  useEffect(() => {
    if (prevValue.current === value) return;
    const from = prevValue.current;
    const to = value;
    prevValue.current = value;
    startTime.current = performance.now();

    function animate(now: number) {
      const elapsed = now - startTime.current;
      const progress = Math.min(elapsed / duration, 1);
      // Ease-out cubic
      const eased = 1 - Math.pow(1 - progress, 3);
      setDisplayValue(Math.round(from + (to - from) * eased));
      if (progress < 1) rafRef.current = requestAnimationFrame(animate);
    }
    rafRef.current = requestAnimationFrame(animate);
    return () => cancelAnimationFrame(rafRef.current);
  }, [value, duration]);

  const formatted = format ? format(displayValue) : displayValue.toLocaleString();
  return <span className={className}>{formatted}</span>;
}
