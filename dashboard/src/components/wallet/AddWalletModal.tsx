import { useState } from 'react';
import { X, Plus, Eye, EyeOff, Copy, Check, AlertTriangle } from 'lucide-react';

interface AddWalletModalProps {
  open: boolean;
  onClose: () => void;
  onGenerate: () => Promise<{ secret_key: string; address: string } | null>;
  onAdd: (name: string, secretKey: string, address: string) => Promise<void>;
}

const overlayStyle: React.CSSProperties = {
  position: 'fixed', inset: 0, zIndex: 50,
  display: 'flex', alignItems: 'center', justifyContent: 'center',
  padding: 16, background: 'var(--dag-overlay)',
};

const cardStyle: React.CSSProperties = {
  background: 'var(--dag-card)', border: '1px solid var(--dag-border)',
  borderRadius: 14, boxShadow: '0 25px 50px rgba(0,0,0,0.25)',
  width: '100%', maxWidth: 448,
};

const headerStyle: React.CSSProperties = {
  display: 'flex', alignItems: 'center', justifyContent: 'space-between',
  padding: 20, borderBottom: '1px solid var(--dag-border)',
};

const closeBtnStyle: React.CSSProperties = {
  padding: 6, borderRadius: 8, color: 'var(--dag-text-muted)',
  background: 'none', border: 'none', cursor: 'pointer', transition: 'all 0.15s',
};

const tabBorderStyle: React.CSSProperties = {
  display: 'flex', borderBottom: '1px solid var(--dag-border)',
};

const bodyStyle: React.CSSProperties = {
  padding: 20, display: 'flex', flexDirection: 'column', gap: 16,
};

const labelStyle: React.CSSProperties = {
  fontSize: 10, color: 'var(--dag-text-muted)', textTransform: 'uppercase',
  letterSpacing: 1.2, fontWeight: 600,
};

const inputStyle: React.CSSProperties = {
  marginTop: 4, width: '100%', padding: '10px 12px',
  background: 'var(--dag-input-bg)', border: '1px solid var(--dag-border)',
  borderRadius: 10, fontSize: 12, color: 'var(--dag-text-secondary)',
  outline: 'none',
};

const monoInputStyle: React.CSSProperties = {
  ...inputStyle, fontSize: 10, fontFamily: 'monospace',
};

const primaryBtnStyle: React.CSSProperties = {
  width: '100%', padding: '10px 0', borderRadius: 10,
  background: 'var(--dag-subheading)', color: 'var(--dag-text)', fontWeight: 500,
  fontSize: 12, border: 'none', cursor: 'pointer', transition: 'all 0.15s',
};

const secondaryBtnStyle: React.CSSProperties = {
  width: '100%', padding: '10px 0', borderRadius: 10,
  background: 'var(--dag-input-bg)', color: 'var(--dag-text-secondary)', fontWeight: 500,
  fontSize: 12, border: 'none', cursor: 'pointer', transition: 'all 0.15s',
};

const smallBtnStyle: React.CSSProperties = {
  display: 'flex', alignItems: 'center', gap: 6,
  padding: '6px 10px', borderRadius: 6, fontSize: 10, fontWeight: 500,
  background: 'rgba(51,65,85,0.6)', color: 'var(--dag-text-secondary)',
  border: 'none', cursor: 'pointer', transition: 'all 0.15s',
};

export function AddWalletModal({ open, onClose, onGenerate, onAdd }: AddWalletModalProps) {
  const [tab, setTab] = useState<'generate' | 'import'>('generate');
  const [name, setName] = useState('');
  const [secretKey, setSecretKey] = useState('');
  const [address, setAddress] = useState('');
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);
  const [generated, setGenerated] = useState(false);

  if (!open) return null;

  const handleGenerate = async () => {
    setError('');
    setLoading(true);
    try {
      const result = await onGenerate();
      if (result) {
        setSecretKey(result.secret_key);
        setAddress(result.address);
        setGenerated(true);
      } else {
        setError('Failed to generate keypair. Check node connection.');
      }
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  };

  const handleSave = async () => {
    setError('');
    if (!name.trim()) {
      setError('Name is required');
      return;
    }
    if (!secretKey.trim() || !address.trim()) {
      setError('Secret key and address are required');
      return;
    }
    if (!/^[0-9a-fA-F]{64}$/.test(secretKey.trim())) {
      setError('Secret key must be 64 hex characters');
      return;
    }
    if (!/^[0-9a-fA-F]{40}$/.test(address.trim())) {
      setError('Address must be 40 hex characters');
      return;
    }
    setLoading(true);
    try {
      await onAdd(name.trim(), secretKey.trim().toLowerCase(), address.trim().toLowerCase());
      onClose();
      resetState();
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  };

  const resetState = () => {
    setName('');
    setSecretKey('');
    setAddress('');
    setError('');
    setGenerated(false);
  };

  const handleClose = () => {
    onClose();
    resetState();
  };

  return (
    <div style={overlayStyle} onClick={(e) => { if (e.target === e.currentTarget) handleClose(); }}>
      <div style={cardStyle}>
        {/* Header */}
        <div style={headerStyle}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
            <Plus size={20} style={{ color: 'var(--dag-subheading)' }} />
            <h2 style={{ fontSize: 16, fontWeight: 600, color: 'var(--dag-text)' }}>Add Wallet</h2>
          </div>
          <button onClick={handleClose} style={closeBtnStyle}>
            <X size={16} />
          </button>
        </div>

        {/* Tabs */}
        <div style={tabBorderStyle}>
          <TabBtn active={tab === 'generate'} onClick={() => setTab('generate')} label="Generate" />
          <TabBtn active={tab === 'import'} onClick={() => setTab('import')} label="Import Key" />
        </div>

        {/* Body */}
        <div style={bodyStyle}>
          <div>
            <label style={labelStyle}>Wallet Name</label>
            <input
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="My Wallet"
              style={inputStyle}
              autoFocus
            />
          </div>

          {tab === 'generate' && (
            <GenerateTab
              generated={generated}
              loading={loading}
              address={address}
              secretKey={secretKey}
              onGenerate={handleGenerate}
            />
          )}

          {tab === 'import' && (
            <>
              <div>
                <label style={labelStyle}>Secret Key (64 hex)</label>
                <input
                  type="password"
                  value={secretKey}
                  onChange={(e) => setSecretKey(e.target.value)}
                  placeholder="Enter 64-character hex secret key"
                  style={monoInputStyle}
                />
              </div>
              <div>
                <label style={labelStyle}>Address (64 hex)</label>
                <input
                  type="text"
                  value={address}
                  onChange={(e) => setAddress(e.target.value)}
                  placeholder="Enter 64-character hex address"
                  style={monoInputStyle}
                />
              </div>
              <p style={{ fontSize: 10, color: 'var(--dag-text-faint)' }}>
                Your key is stored encrypted on this device. It never leaves your browser.
              </p>
            </>
          )}

          {error && <p style={{ fontSize: 12, color: '#F87171' }}>{error}</p>}

          <button
            onClick={handleSave}
            disabled={loading || !name.trim() || !secretKey.trim() || !address.trim()}
            style={{
              ...primaryBtnStyle,
              ...(loading || !name.trim() || !secretKey.trim() || !address.trim()
                ? { opacity: 0.5, cursor: 'not-allowed' } : {}),
            }}
          >
            {loading ? 'Saving...' : 'Save Wallet'}
          </button>
        </div>
      </div>
    </div>
  );
}

function GenerateTab({ generated, loading, address, secretKey, onGenerate }: {
  generated: boolean; loading: boolean; address: string; secretKey: string; onGenerate: () => void;
}) {
  const [showKey, setShowKey] = useState(false);
  const [copied, setCopied] = useState(false);

  const truncAddr = address ? `${address.slice(0, 8)}...${address.slice(-6)}` : '';

  const handleCopyKey = async () => {
    try {
      await navigator.clipboard.writeText(secretKey);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch { /* clipboard unavailable */ }
  };

  if (!generated) {
    return (
      <button
        onClick={onGenerate}
        disabled={loading}
        style={{
          ...secondaryBtnStyle,
          ...(loading ? { opacity: 0.5, cursor: 'not-allowed' } : {}),
        }}
      >
        {loading ? 'Generating...' : 'Generate Keypair'}
      </button>
    );
  }

  return (
    <>
      <div>
        <label style={labelStyle}>Your Address</label>
        <p style={{
          marginTop: 4, fontSize: 10, fontFamily: 'monospace',
          color: 'var(--dag-text-secondary)', background: 'var(--dag-input-bg)',
          padding: '8px 12px', borderRadius: 10,
        }}>
          {truncAddr}
        </p>
      </div>
      <div style={{
        borderRadius: 10, border: '1px solid rgba(245,158,11,0.2)',
        background: 'rgba(245,158,11,0.04)', padding: 12,
        display: 'flex', flexDirection: 'column', gap: 8,
      }}>
        <div style={{ display: 'flex', alignItems: 'flex-start', gap: 8 }}>
          <AlertTriangle size={14} style={{ color: '#FBBF24', marginTop: 2, flexShrink: 0 }} />
          <p style={{ fontSize: 11, color: '#FBBF24', fontWeight: 500 }}>
            Save your private key. It cannot be recovered if lost.
          </p>
        </div>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          <button onClick={() => setShowKey(!showKey)} style={smallBtnStyle}>
            {showKey ? <EyeOff size={12} /> : <Eye size={12} />}
            {showKey ? 'Hide Key' : 'Show Private Key'}
          </button>
          <button
            onClick={handleCopyKey}
            style={{
              ...smallBtnStyle,
              ...(copied ? { background: 'rgba(34,197,94,0.12)', color: '#4ADE80' } : {}),
            }}
          >
            {copied ? <Check size={12} /> : <Copy size={12} />}
            {copied ? 'Copied!' : 'Copy Key'}
          </button>
        </div>
        {showKey && (
          <p style={{
            fontSize: 10, fontFamily: 'monospace', color: '#FCD34D',
            background: 'rgba(30,41,59,0.8)', padding: '8px 12px',
            borderRadius: 6, wordBreak: 'break-all',
            border: '1px solid rgba(245,158,11,0.1)',
          }}>
            {secretKey}
          </p>
        )}
      </div>
    </>
  );
}

function TabBtn({ active, onClick, label }: { active: boolean; onClick: () => void; label: string }) {
  return (
    <button
      onClick={onClick}
      style={{
        flex: 1, padding: '12px 0', fontSize: 12, fontWeight: 500,
        transition: 'all 0.15s', background: 'none', cursor: 'pointer',
        border: 'none',
        borderBottom: active ? '2px solid var(--dag-subheading)' : '2px solid transparent',
        color: active ? 'var(--dag-subheading)' : 'var(--dag-text-muted)',
      }}
    >
      {label}
    </button>
  );
}
