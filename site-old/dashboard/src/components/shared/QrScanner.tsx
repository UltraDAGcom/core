import { useEffect, useRef, useState, useCallback } from 'react';
import { X, Camera } from 'lucide-react';
import jsQR from 'jsqr';

interface QrScannerProps {
  open: boolean;
  onClose: () => void;
  onScan: (data: string) => void;
}

export function QrScanner({ open, onClose, onScan }: QrScannerProps) {
  const videoRef = useRef<HTMLVideoElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const streamRef = useRef<MediaStream | null>(null);
  const animFrameRef = useRef<number>(0);
  const [error, setError] = useState('');

  const stopCamera = useCallback(() => {
    if (animFrameRef.current) {
      cancelAnimationFrame(animFrameRef.current);
      animFrameRef.current = 0;
    }
    if (streamRef.current) {
      for (const track of streamRef.current.getTracks()) {
        track.stop();
      }
      streamRef.current = null;
    }
    if (videoRef.current) {
      videoRef.current.srcObject = null;
    }
  }, []);

  const handleClose = useCallback(() => {
    stopCamera();
    setError('');
    onClose();
  }, [stopCamera, onClose]);

  // Try native BarcodeDetector first, fall back to jsQR
  const scanFrame = useCallback(() => {
    const video = videoRef.current;
    const canvas = canvasRef.current;
    if (!video || !canvas || video.readyState !== video.HAVE_ENOUGH_DATA) {
      animFrameRef.current = requestAnimationFrame(scanFrame);
      return;
    }

    canvas.width = video.videoWidth;
    canvas.height = video.videoHeight;
    const ctx = canvas.getContext('2d', { willReadFrequently: true });
    if (!ctx) {
      animFrameRef.current = requestAnimationFrame(scanFrame);
      return;
    }

    ctx.drawImage(video, 0, 0, canvas.width, canvas.height);
    const imageData = ctx.getImageData(0, 0, canvas.width, canvas.height);

    const code = jsQR(imageData.data, imageData.width, imageData.height, {
      inversionAttempts: 'dontInvert',
    });

    if (code && code.data) {
      onScan(code.data);
      handleClose();
      return;
    }

    animFrameRef.current = requestAnimationFrame(scanFrame);
  }, [onScan, handleClose]);

  useEffect(() => {
    if (!open) return;

    let cancelled = false;

    async function startCamera() {
      setError('');
      try {
        const stream = await navigator.mediaDevices.getUserMedia({
          video: { facingMode: 'environment' },
        });
        if (cancelled) {
          for (const track of stream.getTracks()) track.stop();
          return;
        }
        streamRef.current = stream;
        if (videoRef.current) {
          videoRef.current.srcObject = stream;
          await videoRef.current.play();
          // Start scanning after video is playing
          animFrameRef.current = requestAnimationFrame(scanFrame);
        }
      } catch {
        if (!cancelled) {
          setError('Camera access denied or not available.');
        }
      }
    }

    startCamera();

    return () => {
      cancelled = true;
      stopCamera();
    };
  }, [open, scanFrame, stopCamera]);

  if (!open) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 backdrop-blur-sm">
      <div className="bg-dag-card border border-dag-border rounded-xl shadow-2xl w-full max-w-md overflow-hidden">
        <div className="flex items-center justify-between px-4 py-3 border-b border-dag-border">
          <div className="flex items-center gap-2 text-white font-medium text-sm">
            <Camera className="w-4 h-4" />
            Scan QR Code
          </div>
          <button
            onClick={handleClose}
            className="text-dag-muted hover:text-white transition-colors"
          >
            <X className="w-5 h-5" />
          </button>
        </div>

        <div className="relative aspect-square bg-black">
          <video
            ref={videoRef}
            className="w-full h-full object-cover"
            playsInline
            muted
          />
          <canvas ref={canvasRef} className="hidden" />

          {/* Scanning overlay with corner markers */}
          <div className="absolute inset-0 flex items-center justify-center pointer-events-none">
            <div className="w-56 h-56 relative">
              {/* Top-left corner */}
              <div className="absolute top-0 left-0 w-6 h-6 border-t-2 border-l-2 border-dag-accent rounded-tl" />
              {/* Top-right corner */}
              <div className="absolute top-0 right-0 w-6 h-6 border-t-2 border-r-2 border-dag-accent rounded-tr" />
              {/* Bottom-left corner */}
              <div className="absolute bottom-0 left-0 w-6 h-6 border-b-2 border-l-2 border-dag-accent rounded-bl" />
              {/* Bottom-right corner */}
              <div className="absolute bottom-0 right-0 w-6 h-6 border-b-2 border-r-2 border-dag-accent rounded-br" />
            </div>
          </div>

          {error && (
            <div className="absolute inset-0 flex items-center justify-center bg-black/80">
              <p className="text-red-400 text-sm text-center px-6">{error}</p>
            </div>
          )}
        </div>

        <div className="px-4 py-3 text-center">
          <p className="text-xs text-dag-muted">
            Point your camera at a QR code containing an UltraDAG address.
          </p>
        </div>
      </div>
    </div>
  );
}
