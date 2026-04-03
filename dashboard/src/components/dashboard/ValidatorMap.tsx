import { useRef, useEffect, useState, useCallback } from 'react';
import { WORLD_POLYGONS, WORLD_LINES } from '../../lib/world-outline';
import { shortAddr, formatUdag } from '../../lib/api';
import { useName } from '../../contexts/NameCacheContext';
import type { GeoLocatedPeer } from '../../hooks/useGeoLocatedPeers';

// Stable color per validator address
const VALIDATOR_COLORS = ['#00E0C4', '#0066FF', '#A855F7', '#FFB800', '#34d399', '#f472b6', '#60a5fa', '#fbbf24', '#c084fc', '#fb923c'];
function validatorColor(addr: string): string {
  let h = 0;
  for (let i = 0; i < addr.length; i++) h = ((h << 5) - h + addr.charCodeAt(i)) | 0;
  return VALIDATOR_COLORS[Math.abs(h) % VALIDATOR_COLORS.length];
}

function hexToRgb(hex: string) {
  const r = parseInt(hex.slice(1, 3), 16);
  const g = parseInt(hex.slice(3, 5), 16);
  const b = parseInt(hex.slice(5, 7), 16);
  return { r, g, b };
}

// Equirectangular projection
function project(lng: number, lat: number, w: number, h: number, padding = 20): [number, number] {
  const x = padding + ((lng + 180) / 360) * (w - padding * 2);
  const y = padding + ((90 - lat) / 180) * (h - padding * 2);
  return [x, y];
}

interface TooltipData {
  x: number;
  y: number;
  peer: GeoLocatedPeer;
}

function Tooltip({ data }: { data: TooltipData }) {
  const addr = data.peer.validator?.address;
  const { name } = useName(addr);
  const displayName = name ? `@${name}` : addr ? shortAddr(addr) : null;
  const v = data.peer.validator;

  return (
    <div style={{
      position: 'absolute',
      left: data.x + 12,
      top: data.y - 10,
      background: 'rgba(10, 14, 26, 0.95)',
      border: '1px solid rgba(255,255,255,0.08)',
      borderRadius: 8,
      padding: '8px 10px',
      pointerEvents: 'none',
      zIndex: 10,
      minWidth: 120,
      backdropFilter: 'blur(8px)',
    }}>
      <div style={{ fontSize: 10, fontWeight: 600, color: '#fff', marginBottom: 4 }}>
        {data.peer.city}{data.peer.city && data.peer.country ? ', ' : ''}{data.peer.country}
      </div>
      {displayName && (
        <div style={{ fontSize: 9.5, color: v ? validatorColor(v.address) : '#00E0C4', fontWeight: 600, marginBottom: 4 }}>
          {displayName}
        </div>
      )}
      {v && (
        <div style={{ display: 'flex', gap: 8, fontSize: 9, color: 'rgba(255,255,255,0.5)' }}>
          <span>{formatUdag(v.effective_stake)} UDAG</span>
          <span>{v.delegator_count} del.</span>
          <span>{v.commission_percent}%</span>
        </div>
      )}
      {!v && (
        <div style={{ fontSize: 9, color: 'rgba(255,255,255,0.4)' }}>Peer node</div>
      )}
    </div>
  );
}

interface ValidatorMapProps {
  peers: GeoLocatedPeer[];
}

export function ValidatorMap({ peers }: ValidatorMapProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const frameRef = useRef<number>(0);
  const timeRef = useRef(0);
  const [tooltip, setTooltip] = useState<TooltipData | null>(null);

  const W = 540, H = 260;

  // Handle hover
  const handleMouseMove = useCallback((e: React.MouseEvent<HTMLDivElement>) => {
    const rect = containerRef.current?.getBoundingClientRect();
    if (!rect) return;
    const scaleX = W / rect.width;
    const scaleY = H / rect.height;
    const mx = (e.clientX - rect.left) * scaleX;
    const my = (e.clientY - rect.top) * scaleY;

    let closest: { peer: GeoLocatedPeer; dist: number; px: number; py: number } | null = null;
    for (const peer of peers) {
      const [px, py] = project(peer.lng, peer.lat, W, H);
      const dx = mx - px, dy = my - py;
      const dist = Math.sqrt(dx * dx + dy * dy);
      if (dist < 20 && (!closest || dist < closest.dist)) {
        closest = { peer, dist, px, py };
      }
    }

    if (closest) {
      // Convert canvas coords back to screen coords
      setTooltip({
        x: closest.px / scaleX,
        y: closest.py / scaleY,
        peer: closest.peer,
      });
    } else {
      setTooltip(null);
    }
  }, [peers]);

  // Canvas animation loop
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;
    canvas.width = W * 2; canvas.height = H * 2;
    ctx.scale(2, 2);

    const draw = () => {
      timeRef.current += 0.016;
      const t = timeRef.current;
      ctx.clearRect(0, 0, W, H);

      // Draw world polygons (land masses)
      for (const poly of WORLD_POLYGONS) {
        ctx.beginPath();
        for (let i = 0; i < poly.length; i++) {
          const [x, y] = project(poly[i][0], poly[i][1], W, H);
          if (i === 0) ctx.moveTo(x, y); else ctx.lineTo(x, y);
        }
        ctx.closePath();
        ctx.fillStyle = 'rgba(255,255,255,0.015)';
        ctx.fill();
        ctx.strokeStyle = 'rgba(255,255,255,0.06)';
        ctx.lineWidth = 0.5;
        ctx.stroke();
      }

      // Draw additional coastline lines
      for (const line of WORLD_LINES) {
        ctx.beginPath();
        for (let i = 0; i < line.length; i++) {
          const [x, y] = project(line[i][0], line[i][1], W, H);
          if (i === 0) ctx.moveTo(x, y); else ctx.lineTo(x, y);
        }
        ctx.strokeStyle = 'rgba(255,255,255,0.04)';
        ctx.lineWidth = 0.5;
        ctx.stroke();
      }

      // Draw subtle grid lines
      ctx.strokeStyle = 'rgba(255,255,255,0.015)';
      ctx.lineWidth = 0.5;
      for (let lat = -60; lat <= 80; lat += 30) {
        ctx.beginPath();
        const [x0, y0] = project(-180, lat, W, H);
        const [x1, y1] = project(180, lat, W, H);
        ctx.moveTo(x0, y0); ctx.lineTo(x1, y1);
        ctx.stroke();
      }
      for (let lng = -180; lng <= 180; lng += 60) {
        ctx.beginPath();
        const [x0, y0] = project(lng, -80, W, H);
        const [x1, y1] = project(lng, 80, W, H);
        ctx.moveTo(x0, y0); ctx.lineTo(x1, y1);
        ctx.stroke();
      }

      // Draw peer/validator dots
      for (const peer of peers) {
        const [px, py] = project(peer.lng, peer.lat, W, H);
        const color = peer.validator ? validatorColor(peer.validator.address) : '#00E0C4';
        const { r, g, b } = hexToRgb(color);
        const isValidator = !!peer.validator;
        const baseSz = isValidator ? 3.5 + Math.min((peer.validator!.effective_stake / 1e10), 4) : 2.5;
        const pulse = Math.sin(t * 2 + px * 0.1 + py * 0.1) * 1.2;

        // Outer glow
        const grad = ctx.createRadialGradient(px, py, 0, px, py, baseSz + 10 + pulse);
        grad.addColorStop(0, `rgba(${r},${g},${b},0.2)`);
        grad.addColorStop(1, `rgba(${r},${g},${b},0)`);
        ctx.beginPath();
        ctx.arc(px, py, baseSz + 10 + pulse, 0, Math.PI * 2);
        ctx.fillStyle = grad;
        ctx.fill();

        // Core dot
        ctx.beginPath();
        ctx.arc(px, py, baseSz, 0, Math.PI * 2);
        ctx.fillStyle = color;
        ctx.fill();

        // Active validator ring
        if (isValidator && peer.validator!.is_active) {
          ctx.beginPath();
          ctx.arc(px, py, baseSz + 3, 0, Math.PI * 2);
          ctx.strokeStyle = `rgba(${r},${g},${b},${0.3 + Math.sin(t * 3 + px) * 0.15})`;
          ctx.lineWidth = 0.8;
          ctx.stroke();
        }
      }

      // Ping animation for new peers (using time-based index cycling)
      if (peers.length > 0) {
        const pingIdx = Math.floor(t / 3) % peers.length;
        const peer = peers[pingIdx];
        const [px, py] = project(peer.lng, peer.lat, W, H);
        const pingPhase = (t % 3) / 3; // 0→1 over 3 seconds
        if (pingPhase < 0.6) {
          const radius = 4 + pingPhase * 30;
          const alpha = 0.3 * (1 - pingPhase / 0.6);
          ctx.beginPath();
          ctx.arc(px, py, radius, 0, Math.PI * 2);
          ctx.strokeStyle = `rgba(0,224,196,${alpha})`;
          ctx.lineWidth = 1;
          ctx.stroke();
        }
      }

      frameRef.current = requestAnimationFrame(draw);
    };

    frameRef.current = requestAnimationFrame(draw);
    return () => cancelAnimationFrame(frameRef.current);
  }, [peers]);

  return (
    <div
      ref={containerRef}
      onMouseMove={handleMouseMove}
      onMouseLeave={() => setTooltip(null)}
      style={{ position: 'relative', cursor: 'crosshair' }}
    >
      <canvas ref={canvasRef} style={{ width: '100%', height: H, borderRadius: 12, display: 'block' }} />
      {tooltip && <Tooltip data={tooltip} />}
    </div>
  );
}
