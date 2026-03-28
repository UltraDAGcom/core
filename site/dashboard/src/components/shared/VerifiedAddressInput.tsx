import { useAddressVerify } from '../../hooks/useAddressVerify';

const SATS = 100_000_000;

interface Props {
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  style?: React.CSSProperties;
}

/**
 * Address input with real-time on-chain verification.
 * Shows: ✓ green checkmark + balance when found, ⚠ warning when invalid,
 * spinner when checking, and name resolution.
 */
export function VerifiedAddressInput({ value, onChange, placeholder = 'Address, bech32m, or name', style }: Props) {
  const status = useAddressVerify(value);

  return (
    <div>
      <div style={{ position: 'relative' }}>
        <input
          type="text"
          value={value}
          onChange={e => onChange(e.target.value)}
          placeholder={placeholder}
          style={{
            width: '100%', padding: '10px 14px', paddingRight: 36, borderRadius: 10,
            background: 'var(--dag-input-bg)',
            border: `1px solid ${status.exists ? 'rgba(0,224,196,0.2)' : status.error ? 'rgba(239,68,68,0.2)' : 'var(--dag-border)'}`,
            color: 'var(--dag-text)', fontSize: 13, outline: 'none', fontFamily: "'DM Sans',sans-serif",
            transition: 'border-color 0.2s',
            ...style,
          }}
        />
        {/* Status indicator */}
        <span style={{ position: 'absolute', right: 12, top: '50%', transform: 'translateY(-50%)', fontSize: 14 }}>
          {status.checking ? (
            <span style={{ color: 'var(--dag-text-faint)', fontSize: 11 }}>...</span>
          ) : status.exists ? (
            <span style={{ color: '#00E0C4' }}>✓</span>
          ) : status.error ? (
            <span style={{ color: '#EF4444', fontSize: 12 }}>✕</span>
          ) : value.trim().length >= 3 ? (
            <span style={{ color: 'var(--dag-text-faint)', fontSize: 12 }}>?</span>
          ) : null}
        </span>
      </div>

      {/* Detail line below input */}
      {value.trim().length >= 3 && !status.checking && (
        <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginTop: 4, minHeight: 16 }}>
          {status.exists ? (
            <>
              <span style={{ fontSize: 10, color: '#00E0C4' }}>
                Found on-chain
              </span>
              {status.name && (
                <span style={{ fontSize: 10, color: '#A855F7', fontWeight: 600 }}>
                  {status.name}
                </span>
              )}
              {status.balance != null && (
                <span style={{ fontSize: 10, color: 'var(--dag-text-faint)', fontFamily: "'DM Mono',monospace" }}>
                  {(status.balance / SATS).toFixed(4)} UDAG
                </span>
              )}
            </>
          ) : status.error ? (
            <span style={{ fontSize: 10, color: '#EF4444' }}>{status.error}</span>
          ) : (
            <span style={{ fontSize: 10, color: 'var(--dag-text-faint)' }}>Address not found on-chain (new or invalid)</span>
          )}
        </div>
      )}
    </div>
  );
}
