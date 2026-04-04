import { useState } from 'react';
import { Eye, EyeOff, Copy, Check, AlertTriangle } from 'lucide-react';
import { deriveAddress, generateKeypair } from '../../lib/keygen';
import { primaryButtonStyle, secondaryButtonStyle, inputStyle as themeInputStyle, buttonStyle as themeButtonStyle } from '../../lib/theme';

interface CreateKeystoreModalProps {
  open: boolean;
  onClose: () => void;
  onCreateOrUnlock: (password: string) => Promise<boolean>;
  onCreateWithKey: (password: string, name: string, secretKey: string, address: string) => Promise<boolean>;
  onImport: (json: string) => boolean;
  hasExisting: boolean;
}

type Screen = 'welcome' | 'unlock' | 'create' | 'import' | 'backup' | 'restore';

const modalInputStyle: React.CSSProperties = {
  ...themeInputStyle,
  padding: '10px 12px',
};

export function CreateKeystoreModal({
  open,
  onClose,
  onCreateOrUnlock,
  onCreateWithKey,
  onImport,
  hasExisting,
}: CreateKeystoreModalProps) {
  const [screen, setScreen] = useState<Screen>(hasExisting ? 'unlock' : 'welcome');
  const [password, setPassword] = useState('');
  const [confirmPassword, setConfirmPassword] = useState('');
  const [walletName, setWalletName] = useState('');
  const [importKeyHex, setImportKeyHex] = useState('');
  const [derivedAddress, setDerivedAddress] = useState('');
  const [generatedKey, setGeneratedKey] = useState<{ secret_key: string; address: string } | null>(null);
  const [importJson, setImportJson] = useState('');
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);

  if (!open) return null;

  const resetState = () => {
    setPassword('');
    setConfirmPassword('');
    setWalletName('');
    setImportKeyHex('');
    setDerivedAddress('');
    setGeneratedKey(null);
    setImportJson('');
    setError('');
  };

  const goTo = (s: Screen) => { resetState(); setScreen(s); };

  const handleKeyChange = async (hex: string) => {
    setImportKeyHex(hex);
    setDerivedAddress('');
    const clean = hex.replace(/\s/g, '').toLowerCase();
    if (/^[0-9a-f]{64}$/.test(clean)) {
      try { setDerivedAddress(await deriveAddress(clean)); } catch {}
    }
  };

  const handleGenerate = async () => {
    setLoading(true);
    try {
      const kp = await generateKeypair();
      setGeneratedKey(kp);
      setDerivedAddress(kp.address);
    } finally { setLoading(false); }
  };

  const validatePassword = (): boolean => {
    if (password.length < 8) { setError('Password must be at least 8 characters'); return false; }
    if (password !== confirmPassword) { setError('Passwords do not match'); return false; }
    return true;
  };

  const handleCreateWallet = async () => {
    setError('');
    if (!walletName.trim()) { setError('Please enter a wallet name'); return; }
    if (!validatePassword()) return;

    const key = generatedKey?.secret_key || importKeyHex.replace(/\s/g, '').toLowerCase();
    if (!/^[0-9a-f]{64}$/.test(key)) { setError('Invalid private key'); return; }

    setLoading(true);
    try {
      const addr = generatedKey?.address || await deriveAddress(key);
      const ok = await onCreateWithKey(password, walletName.trim(), key, addr);
      if (ok !== false) onClose();
    } catch (err) { setError(String(err)); } finally { setLoading(false); }
  };

  const handleUnlock = async () => {
    setError('');
    setLoading(true);
    try {
      const ok = await onCreateOrUnlock(password);
      if (!ok) setError('Incorrect password');
      else onClose();
    } catch (err) { setError(String(err)); } finally { setLoading(false); }
  };

  const handleRestore = async () => {
    setError('');
    const ok = onImport(importJson);
    if (!ok) { setError('Invalid backup data'); return; }
    goTo('unlock');
  };

  const linkBtnStyle: React.CSSProperties = {
    background: 'none', border: 'none', color: 'var(--dag-text-faint)',
    fontSize: 11, cursor: 'pointer', width: '100%', padding: '8px 0',
    transition: 'color 0.2s',
  };

  return (
    <div style={{
      position: 'fixed', inset: 0, zIndex: 50, display: 'flex', alignItems: 'center',
      justifyContent: 'center', padding: 16, background: 'rgba(0,0,0,0.7)',
      backdropFilter: 'blur(6px)',
    }}>
      <div style={{
        background: 'var(--dag-card)', border: '1px solid var(--dag-border)',
        borderRadius: 16, boxShadow: '0 24px 60px rgba(0,0,0,0.6)',
        width: '100%', maxWidth: 420,
      }}>
        {/* Header */}
        <div style={{
          display: 'flex', alignItems: 'center', justifyContent: 'space-between',
          padding: '16px 20px', borderBottom: '1px solid var(--dag-border)',
        }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
            {screen !== 'welcome' && screen !== 'unlock' && (
              <button onClick={() => goTo(hasExisting ? 'unlock' : 'welcome')} style={{
                background: 'none', border: 'none', color: 'var(--dag-text-muted)',
                cursor: 'pointer', padding: 4, borderRadius: 4, fontSize: 14,
              }}>
                ←
              </button>
            )}
            <span style={{ color: '#00E0C4', fontSize: 16 }}>◇</span>
            <h2 style={{ fontSize: 16, fontWeight: 600, color: 'var(--dag-text)' }}>
              {screen === 'welcome' ? 'Welcome' : screen === 'unlock' ? 'Welcome Back' : screen === 'create' ? 'Create Wallet' : screen === 'import' ? 'Import Wallet' : screen === 'backup' ? 'Generated Wallet' : 'Restore Backup'}
            </h2>
          </div>
          <button onClick={onClose} style={{
            background: 'none', border: 'none', color: 'var(--dag-text-faint)',
            cursor: 'pointer', padding: '4px 8px', borderRadius: 6, fontSize: 16,
          }}>
            ✕
          </button>
        </div>

        <div style={{ padding: 20, display: 'flex', flexDirection: 'column', gap: 14 }}>

          {/* ===== WELCOME (first time) ===== */}
          {screen === 'welcome' && (
            <>
              <p style={{ fontSize: 12, color: 'var(--dag-text-muted)', textAlign: 'center' }}>
                Create a new wallet or import an existing one to get started.
              </p>
              <div style={{ display: 'flex', flexDirection: 'column', gap: 10, paddingTop: 8 }}>
                <button onClick={() => { handleGenerate(); goTo('create'); }} style={{ ...primaryButtonStyle, width: '100%', padding: '11px 0', display: 'flex', alignItems: 'center', justifyContent: 'center', gap: 8 }}>
                  + Create New Wallet
                </button>
                <button onClick={() => goTo('import')} style={{ ...secondaryButtonStyle, width: '100%', padding: '11px 0', display: 'flex', alignItems: 'center', justifyContent: 'center', gap: 8 }}>
                  Import Private Key
                </button>
                <button onClick={() => goTo('restore')} style={linkBtnStyle}>
                  Restore from backup
                </button>
              </div>
            </>
          )}

          {/* ===== UNLOCK (returning user) ===== */}
          {screen === 'unlock' && (
            <>
              <p style={{ fontSize: 12, color: 'var(--dag-text-muted)' }}>Enter your password to unlock your wallet.</p>
              <input
                type="password" value={password} onChange={(e) => setPassword(e.target.value)}
                placeholder="Password" style={modalInputStyle}
                onKeyDown={(e) => e.key === 'Enter' && handleUnlock()}
                autoFocus
              />
              {error && <p style={{ fontSize: 12, color: '#EF4444' }}>{error}</p>}
              <button onClick={handleUnlock} disabled={loading} style={{ ...primaryButtonStyle, width: '100%', padding: '11px 0', opacity: loading ? 0.5 : 1 }}>
                {loading ? 'Unlocking...' : 'Unlock'}
              </button>
              <button onClick={() => goTo('restore')} style={linkBtnStyle}>
                Restore from backup
              </button>
            </>
          )}

          {/* ===== CREATE NEW WALLET ===== */}
          {screen === 'create' && (
            <>
              {generatedKey && (
                <KeystoreKeyDisplay address={generatedKey.address} secretKey={generatedKey.secret_key} />
              )}
              <input
                type="text" value={walletName} onChange={(e) => setWalletName(e.target.value)}
                placeholder="Wallet name" style={modalInputStyle} autoFocus
              />
              <input
                type="password" value={password} onChange={(e) => setPassword(e.target.value)}
                placeholder="Set a password (min 8 characters)" style={modalInputStyle}
              />
              <input
                type="password" value={confirmPassword} onChange={(e) => setConfirmPassword(e.target.value)}
                placeholder="Confirm password" style={modalInputStyle}
                onKeyDown={(e) => e.key === 'Enter' && handleCreateWallet()}
              />
              <p style={{ fontSize: 10, color: 'var(--dag-text-muted)' }}>Your wallet is encrypted locally with AES-256. Only you can access it.</p>
              {error && <p style={{ fontSize: 12, color: '#EF4444' }}>{error}</p>}
              <button onClick={handleCreateWallet} disabled={loading || !generatedKey} style={{ ...primaryButtonStyle, width: '100%', padding: '11px 0', opacity: loading || !generatedKey ? 0.5 : 1 }}>
                {loading ? 'Creating...' : 'Create Wallet'}
              </button>
            </>
          )}

          {/* ===== IMPORT PRIVATE KEY ===== */}
          {screen === 'import' && (
            <>
              <p style={{ fontSize: 12, color: 'var(--dag-text-muted)' }}>Paste your private key to import an existing wallet.</p>
              <input
                type="text" value={walletName} onChange={(e) => setWalletName(e.target.value)}
                placeholder="Wallet name" style={modalInputStyle} autoFocus
              />
              <input
                type="password" value={importKeyHex} onChange={(e) => handleKeyChange(e.target.value)}
                placeholder="Private key (64 hex characters)" style={{ ...modalInputStyle, fontSize: 11, fontFamily: "'DM Mono',monospace" }}
              />
              {derivedAddress && (
                <div style={{ background: 'var(--dag-input-bg)', borderRadius: 10, padding: '8px 12px', border: '1px solid var(--dag-border)' }}>
                  <p style={{ fontSize: 10, color: 'var(--dag-text-muted)', letterSpacing: 1, textTransform: 'uppercase' }}>Detected Address</p>
                  <p style={{ fontSize: 11, fontFamily: "'DM Mono',monospace", color: '#00E0C4' }}>{derivedAddress.slice(0, 8)}...{derivedAddress.slice(-6)}</p>
                </div>
              )}
              <input
                type="password" value={password} onChange={(e) => setPassword(e.target.value)}
                placeholder="Set a password (min 8 characters)" style={modalInputStyle}
              />
              <input
                type="password" value={confirmPassword} onChange={(e) => setConfirmPassword(e.target.value)}
                placeholder="Confirm password" style={modalInputStyle}
                onKeyDown={(e) => e.key === 'Enter' && handleCreateWallet()}
              />
              {error && <p style={{ fontSize: 12, color: '#EF4444' }}>{error}</p>}
              <button onClick={handleCreateWallet} disabled={loading} style={{ ...primaryButtonStyle, width: '100%', padding: '11px 0', opacity: loading ? 0.5 : 1 }}>
                {loading ? 'Importing...' : 'Import Wallet'}
              </button>
            </>
          )}

          {/* ===== RESTORE FROM BACKUP ===== */}
          {screen === 'restore' && (
            <>
              <p style={{ fontSize: 12, color: 'var(--dag-text-muted)' }}>Paste a previously exported wallet backup.</p>
              <textarea
                value={importJson} onChange={(e) => setImportJson(e.target.value)}
                placeholder='Paste your backup JSON here...'
                rows={5}
                style={{ ...modalInputStyle, fontSize: 11, fontFamily: "'DM Mono',monospace", resize: 'none' } as React.CSSProperties}
                autoFocus
              />
              {error && <p style={{ fontSize: 12, color: '#EF4444' }}>{error}</p>}
              <button onClick={handleRestore} disabled={loading} style={{ ...primaryButtonStyle, width: '100%', padding: '11px 0', opacity: loading ? 0.5 : 1 }}>
                Restore
              </button>
            </>
          )}

        </div>
      </div>
    </div>
  );
}

function KeystoreKeyDisplay({ address, secretKey }: { address: string; secretKey: string }) {
  const [showKey, setShowKey] = useState(false);
  const [copied, setCopied] = useState(false);

  const truncAddr = `${address.slice(0, 8)}...${address.slice(-6)}`;

  const handleCopyKey = async () => {
    try {
      await navigator.clipboard.writeText(secretKey);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch { /* clipboard unavailable */ }
  };

  const smallBtnStyle: React.CSSProperties = {
    ...themeButtonStyle(),
    padding: '4px 8px', fontSize: 10, display: 'inline-flex', alignItems: 'center', gap: 4,
  };

  return (
    <div style={{ background: 'var(--dag-input-bg)', borderRadius: 10, padding: 12, border: '1px solid rgba(0,224,196,0.2)', display: 'flex', flexDirection: 'column', gap: 10 }}>
      <p style={{ fontSize: 10, color: '#00E0C4', letterSpacing: 1, textTransform: 'uppercase', fontWeight: 600 }}>Your New Wallet</p>
      <div>
        <p style={{ fontSize: 10, color: 'var(--dag-text-muted)', letterSpacing: 1, textTransform: 'uppercase' }}>Address</p>
        <p style={{ fontSize: 11, fontFamily: "'DM Mono',monospace", color: '#00E0C4' }}>{truncAddr}</p>
      </div>

      <div style={{ borderRadius: 8, border: '1px solid rgba(255,184,0,0.2)', background: 'rgba(255,184,0,0.04)', padding: 10, display: 'flex', flexDirection: 'column', gap: 8 }}>
        <div style={{ display: 'flex', alignItems: 'flex-start', gap: 6 }}>
          <AlertTriangle style={{ width: 12, height: 12, color: '#FFB800', flexShrink: 0, marginTop: 1 }} />
          <p style={{ fontSize: 10, color: '#FFB800', fontWeight: 500 }}>Save your private key somewhere safe. It cannot be recovered.</p>
        </div>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          <button onClick={() => setShowKey(!showKey)} style={smallBtnStyle}>
            {showKey ? <EyeOff style={{ width: 12, height: 12 }} /> : <Eye style={{ width: 12, height: 12 }} />}
            {showKey ? 'Hide Key' : 'Show Private Key'}
          </button>
          <button onClick={handleCopyKey} style={{
            ...smallBtnStyle,
            ...(copied ? { background: 'rgba(0,224,196,0.1)', borderColor: 'rgba(0,224,196,0.2)', color: '#00E0C4' } : {}),
          }}>
            {copied ? <Check style={{ width: 12, height: 12 }} /> : <Copy style={{ width: 12, height: 12 }} />}
            {copied ? 'Copied!' : 'Copy Key'}
          </button>
        </div>
        {showKey && (
          <p style={{ fontSize: 11, fontFamily: "'DM Mono',monospace", color: '#FFB800', background: 'var(--dag-input-bg)', padding: '8px 10px', borderRadius: 6, wordBreak: 'break-all', border: '1px solid rgba(255,184,0,0.1)' }}>
            {secretKey}
          </p>
        )}
      </div>
    </div>
  );
}
