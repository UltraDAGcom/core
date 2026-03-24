import { useState } from 'react';
import { X, Plus, Key, ChevronLeft, Wallet } from 'lucide-react';
import { deriveAddress, generateKeypair } from '../../lib/keygen';

interface CreateKeystoreModalProps {
  open: boolean;
  onClose: () => void;
  onCreateOrUnlock: (password: string) => Promise<boolean>;
  onCreateWithKey: (password: string, name: string, secretKey: string, address: string) => Promise<boolean>;
  onImport: (json: string) => boolean;
  hasExisting: boolean;
}

type Screen = 'welcome' | 'unlock' | 'create' | 'import' | 'backup' | 'restore';

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

  const inputCls = "w-full px-3 py-2.5 bg-slate-800 border border-dag-border rounded-lg text-sm text-slate-200 placeholder-slate-500 focus:outline-none focus:border-dag-accent";
  const btnPrimary = "w-full py-2.5 rounded-lg bg-dag-accent text-white font-medium text-sm hover:bg-dag-accent/80 disabled:opacity-50 disabled:cursor-not-allowed transition-colors";
  const btnSecondary = "w-full py-2.5 rounded-lg bg-slate-700 text-slate-200 font-medium text-sm hover:bg-slate-600 transition-colors";

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 modal-backdrop bg-black/70">
      <div className="modal-content bg-dag-card border border-dag-border rounded-2xl shadow-2xl w-full max-w-md">
        {/* Header */}
        <div className="flex items-center justify-between p-5 border-b border-dag-border">
          <div className="flex items-center gap-2">
            {screen !== 'welcome' && screen !== 'unlock' && (
              <button onClick={() => goTo(hasExisting ? 'unlock' : 'welcome')} className="p-1 rounded text-slate-400 hover:text-white">
                <ChevronLeft className="w-4 h-4" />
              </button>
            )}
            <Wallet className="w-5 h-5 text-dag-accent" />
            <h2 className="text-lg font-semibold text-white">
              {screen === 'welcome' ? 'Welcome' : screen === 'unlock' ? 'Welcome Back' : screen === 'create' ? 'Create Wallet' : screen === 'import' ? 'Import Wallet' : screen === 'backup' ? 'Generated Wallet' : 'Restore Backup'}
            </h2>
          </div>
          <button onClick={onClose} className="p-1.5 rounded-lg text-slate-400 hover:text-white hover:bg-slate-700">
            <X className="w-4 h-4" />
          </button>
        </div>

        <div className="p-5 space-y-4">

          {/* ===== WELCOME (first time) ===== */}
          {screen === 'welcome' && (
            <>
              <p className="text-sm text-dag-muted text-center">
                Create a new wallet or import an existing one to get started.
              </p>
              <div className="space-y-3 pt-2">
                <button onClick={() => { handleGenerate(); goTo('create'); }} className={btnPrimary}>
                  <span className="flex items-center justify-center gap-2">
                    <Plus className="w-4 h-4" />
                    Create New Wallet
                  </span>
                </button>
                <button onClick={() => goTo('import')} className={btnSecondary}>
                  <span className="flex items-center justify-center gap-2">
                    <Key className="w-4 h-4" />
                    Import Private Key
                  </span>
                </button>
                <button onClick={() => goTo('restore')} className="w-full py-2 text-xs text-slate-500 hover:text-slate-300 transition-colors">
                  Restore from backup
                </button>
              </div>
            </>
          )}

          {/* ===== UNLOCK (returning user) ===== */}
          {screen === 'unlock' && (
            <>
              <p className="text-sm text-dag-muted">Enter your password to unlock your wallet.</p>
              <input
                type="password" value={password} onChange={(e) => setPassword(e.target.value)}
                placeholder="Password" className={inputCls}
                onKeyDown={(e) => e.key === 'Enter' && handleUnlock()}
                autoFocus
              />
              {error && <p className="text-sm text-red-400">{error}</p>}
              <button onClick={handleUnlock} disabled={loading} className={btnPrimary}>
                {loading ? 'Unlocking...' : 'Unlock'}
              </button>
              <button onClick={() => goTo('restore')} className="w-full py-2 text-xs text-slate-500 hover:text-slate-300 transition-colors">
                Restore from backup
              </button>
            </>
          )}

          {/* ===== CREATE NEW WALLET ===== */}
          {screen === 'create' && (
            <>
              {generatedKey && (
                <div className="bg-slate-800/50 rounded-lg p-3 border border-dag-accent/30 space-y-2">
                  <p className="text-[10px] text-dag-accent uppercase tracking-wider font-semibold">Your New Wallet</p>
                  <div>
                    <p className="text-[10px] text-dag-muted uppercase tracking-wider">Address</p>
                    <p className="text-xs font-mono text-dag-green break-all">{generatedKey.address}</p>
                  </div>
                  <div>
                    <p className="text-[10px] text-dag-muted uppercase tracking-wider">Private Key</p>
                    <p className="text-xs font-mono text-slate-400 break-all blur-sm hover:blur-none transition-all cursor-pointer" title="Hover to reveal">{generatedKey.secret_key}</p>
                  </div>
                  <p className="text-[10px] text-amber-400">Save your private key somewhere safe. It cannot be recovered.</p>
                </div>
              )}
              <input
                type="text" value={walletName} onChange={(e) => setWalletName(e.target.value)}
                placeholder="Wallet name" className={inputCls} autoFocus
              />
              <input
                type="password" value={password} onChange={(e) => setPassword(e.target.value)}
                placeholder="Set a password (min 8 characters)" className={inputCls}
              />
              <input
                type="password" value={confirmPassword} onChange={(e) => setConfirmPassword(e.target.value)}
                placeholder="Confirm password" className={inputCls}
                onKeyDown={(e) => e.key === 'Enter' && handleCreateWallet()}
              />
              <p className="text-[10px] text-dag-muted">Your wallet is encrypted locally with AES-256. Only you can access it.</p>
              {error && <p className="text-sm text-red-400">{error}</p>}
              <button onClick={handleCreateWallet} disabled={loading || !generatedKey} className={btnPrimary}>
                {loading ? 'Creating...' : 'Create Wallet'}
              </button>
            </>
          )}

          {/* ===== IMPORT PRIVATE KEY ===== */}
          {screen === 'import' && (
            <>
              <p className="text-sm text-dag-muted">Paste your private key to import an existing wallet.</p>
              <input
                type="text" value={walletName} onChange={(e) => setWalletName(e.target.value)}
                placeholder="Wallet name" className={inputCls} autoFocus
              />
              <input
                type="password" value={importKeyHex} onChange={(e) => handleKeyChange(e.target.value)}
                placeholder="Private key (64 hex characters)" className={inputCls + ' text-xs font-mono'}
              />
              {derivedAddress && (
                <div className="bg-slate-800/50 rounded-lg px-3 py-2 border border-dag-border/50">
                  <p className="text-[10px] text-dag-muted uppercase tracking-wider">Address</p>
                  <p className="text-xs font-mono text-dag-green break-all">{derivedAddress}</p>
                </div>
              )}
              <input
                type="password" value={password} onChange={(e) => setPassword(e.target.value)}
                placeholder="Set a password (min 8 characters)" className={inputCls}
              />
              <input
                type="password" value={confirmPassword} onChange={(e) => setConfirmPassword(e.target.value)}
                placeholder="Confirm password" className={inputCls}
                onKeyDown={(e) => e.key === 'Enter' && handleCreateWallet()}
              />
              {error && <p className="text-sm text-red-400">{error}</p>}
              <button onClick={handleCreateWallet} disabled={loading} className={btnPrimary}>
                {loading ? 'Importing...' : 'Import Wallet'}
              </button>
            </>
          )}

          {/* ===== RESTORE FROM BACKUP ===== */}
          {screen === 'restore' && (
            <>
              <p className="text-sm text-dag-muted">Paste a previously exported wallet backup.</p>
              <textarea
                value={importJson} onChange={(e) => setImportJson(e.target.value)}
                placeholder='Paste your backup JSON here...'
                rows={5}
                className={inputCls + ' text-xs font-mono resize-none'}
                autoFocus
              />
              {error && <p className="text-sm text-red-400">{error}</p>}
              <button onClick={handleRestore} disabled={loading} className={btnPrimary}>
                Restore
              </button>
            </>
          )}

        </div>
      </div>
    </div>
  );
}
