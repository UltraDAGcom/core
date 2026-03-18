import { useState } from 'react';
import { Copy, Check } from 'lucide-react';

export function CopyButton({ text }: { text: string }) {
  const [copied, setCopied] = useState(false);

  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      // clipboard not available
    }
  };

  return (
    <button
      onClick={handleCopy}
      className="relative inline-flex items-center p-1 rounded text-slate-400 hover:text-slate-200 hover:bg-slate-700 transition-all active:scale-110"
      title="Copy"
    >
      {copied ? <Check className="w-3.5 h-3.5 text-green-400" /> : <Copy className="w-3.5 h-3.5" />}
      {copied && (
        <span className="absolute -top-7 left-1/2 -translate-x-1/2 px-2 py-0.5 rounded bg-dag-card border border-dag-border text-[10px] text-dag-green whitespace-nowrap shadow-lg animate-copy-tooltip pointer-events-none">
          Copied!
        </span>
      )}
    </button>
  );
}
