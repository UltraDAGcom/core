interface SparklineProps {
  data: number[];
  width?: number;
  height?: number;
  color?: string;
  fillOpacity?: number;
  strokeWidth?: number;
  className?: string;
}

export function Sparkline({
  data,
  width = 120,
  height = 32,
  color = '#3d9be9',
  fillOpacity = 0.1,
  strokeWidth = 1.5,
  className = '',
}: SparklineProps) {
  if (!data || data.length < 2) return null;

  const padding = height * 0.1;
  const min = Math.min(...data);
  const max = Math.max(...data);
  const range = max - min || 1;

  const points = data.map((v, i) => {
    const x = (i / (data.length - 1)) * width;
    const y = padding + (1 - (v - min) / range) * (height - 2 * padding);
    return `${x},${y}`;
  });

  const polylinePoints = points.join(' ');
  const polygonPoints = `0,${height} ${polylinePoints} ${width},${height}`;

  // Approximate total path length for dash animation
  const totalLength = width * 1.5;

  return (
    <svg
      viewBox={`0 0 ${width} ${height}`}
      width={width}
      height={height}
      className={className}
      preserveAspectRatio="none"
    >
      <polygon
        points={polygonPoints}
        fill={color}
        opacity={fillOpacity}
      />
      <polyline
        points={polylinePoints}
        fill="none"
        stroke={color}
        strokeWidth={strokeWidth}
        strokeLinecap="round"
        strokeLinejoin="round"
        style={{
          strokeDasharray: totalLength,
          strokeDashoffset: totalLength,
          animation: 'sparkline-draw 0.8s ease-out forwards',
        }}
      />
    </svg>
  );
}
