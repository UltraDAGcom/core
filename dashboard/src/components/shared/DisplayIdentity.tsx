import { useState } from 'react';
import { Link } from 'react-router-dom';
import { ChevronDown, ChevronUp } from 'lucide-react';
import { useName } from '../../contexts/NameCacheContext';
import { shortAddr, fullAddr } from '../../lib/api';
import { CopyButton } from './CopyButton';

interface DisplayIdentityProps {
  /** Hex address (40 chars) — the canonical internal format */
  address: string;
  /** Link to /address/{hex} on click */
  link?: boolean;
  /** Show Advanced expand with full bech32m + hex */
  advanced?: boolean;
  /** Show copy button (copies bech32m by default) */
  copyable?: boolean;
  /** Override: force display of a known name (skips cache lookup) */
  knownName?: string | null;
  /** Additional CSS classes on the outer wrapper */
  className?: string;
  /** Font size override (default: text-sm for name, text-xs for address) */
  size?: 'xs' | 'sm' | 'md';
}

const sizeMap = {
  xs: { name: 'text-xs', addr: 'text-[10px]' },
  sm: { name: 'text-sm', addr: 'text-xs' },
  md: { name: 'text-base', addr: 'text-xs' },
};

export function DisplayIdentity({
  address,
  link = false,
  advanced = false,
  copyable = false,
  knownName,
  className = '',
  size = 'sm',
}: DisplayIdentityProps) {
  const [expanded, setExpanded] = useState(false);
  const { name: cachedName, loading } = useName(address);
  const ultraId = knownName !== undefined ? knownName : cachedName;
  const s = sizeMap[size];

  const bech = fullAddr(address);
  const short = shortAddr(address);

  // Primary display content
  const primaryContent = ultraId ? (
    <span className={`${s.name} font-semibold text-dag-accent`}>@{ultraId}</span>
  ) : loading ? (
    <span className={`${s.addr} font-mono text-dag-muted animate-pulse`}>{short}</span>
  ) : (
    <span className={`${s.addr} font-mono text-slate-300`}>{short}</span>
  );

  // Wrap in link if requested — link to profile if ULTRA ID exists, otherwise address page
  const linkTarget = ultraId ? `/profile/@${ultraId}` : `/address/${address}`;
  const display = link ? (
    <Link to={linkTarget} className="hover:underline decoration-dag-accent/40">
      {primaryContent}
    </Link>
  ) : primaryContent;

  if (!advanced) {
    return (
      <span className={`inline-flex items-center gap-1 ${className}`}>
        {display}
        {copyable && <CopyButton text={bech} />}
      </span>
    );
  }

  return (
    <div className={className}>
      <div className="inline-flex items-center gap-1">
        {display}
        {copyable && <CopyButton text={bech} />}
        <button
          onClick={() => setExpanded(!expanded)}
          className="p-0.5 rounded text-dag-muted hover:text-white transition-colors"
          title={expanded ? 'Hide address details' : 'Show address details'}
        >
          {expanded ? <ChevronUp className="w-3.5 h-3.5" /> : <ChevronDown className="w-3.5 h-3.5" />}
        </button>
      </div>
      {expanded && (
        <div className="mt-2 space-y-1.5 pl-1 border-l-2 border-dag-border ml-1">
          <div>
            <p className="text-[9px] uppercase text-dag-muted tracking-wider">Bech32m Address</p>
            <div className="flex items-center gap-1">
              <p className="text-[11px] font-mono text-slate-300 break-all">{bech}</p>
              <CopyButton text={bech} />
            </div>
          </div>
          <div>
            <p className="text-[9px] uppercase text-dag-muted tracking-wider">Hex Address</p>
            <div className="flex items-center gap-1">
              <p className="text-[11px] font-mono text-slate-400 break-all">{address}</p>
              <CopyButton text={address} />
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
