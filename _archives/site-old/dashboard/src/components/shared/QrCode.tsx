import { useEffect, useRef } from 'react';
import QRCode from 'qrcode';

interface QrCodeProps {
  value: string;
  size?: number;
}

export function QrCode({ value, size = 256 }: QrCodeProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || !value) return;

    QRCode.toCanvas(canvas, value, {
      width: size,
      margin: 2,
      color: {
        dark: '#ffffff',
        light: '#0a0f1a',
      },
      errorCorrectionLevel: 'M',
    }).catch(() => {
      // QR generation failed — clear canvas
      const ctx = canvas.getContext('2d');
      if (ctx) {
        ctx.clearRect(0, 0, canvas.width, canvas.height);
      }
    });
  }, [value, size]);

  return (
    <div className="flex items-center justify-center">
      <div className="rounded-xl border border-dag-border bg-dag-surface p-4">
        <canvas ref={canvasRef} />
      </div>
    </div>
  );
}
