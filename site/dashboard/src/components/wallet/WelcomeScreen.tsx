import { useState } from 'react';
import { Plus, Key, ChevronRight, Shield, Zap, Globe, ArrowRight, Eye, EyeOff, Copy, Check, Fingerprint } from 'lucide-react';
import { deriveAddress, generateKeypair } from '../../lib/keygen';

interface WelcomeScreenProps {
  onCreateWallet: (password: string, name: string, secretKey: string, address: string) => Promise<boolean>;
  onImportBlob: (json: string) => boolean;
  onUnlock: (password: string) => Promise<boolean>;
  onUnlockWithWebAuthn?: () => Promise<boolean>;
  webauthnAvailable?: boolean;
  webauthnEnrolled?: boolean;
  hasExisting: boolean;
}

type Step = 'landing' | 'create' | 'import' | 'unlock' | 'restore';

export function WelcomeScreen({ onCreateWallet, onImportBlob, onUnlock, onUnlockWithWebAuthn, webauthnAvailable: _webauthnAvailable, webauthnEnrolled, hasExisting }: WelcomeScreenProps) {
  const [step, setStep] = useState<Step>(hasExisting ? 'unlock' : 'landing');
  const [password, setPassword] = useState('');
  const [confirmPassword, setConfirmPassword] = useState('');
  const [walletName, setWalletName] = useState('');
  const [importKeyHex, setImportKeyHex] = useState('');
  const [derivedAddress, setDerivedAddress] = useState('');
  const [generatedKey, setGeneratedKey] = useState<{ secret_key: string; address: string } | null>(null);
  const [importJson, setImportJson] = useState('');
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);
  const [showKey, setShowKey] = useState(false);
  const [copied, setCopied] = useState(false);

  const resetFields = () => {
    setPassword(''); setConfirmPassword(''); setWalletName('');
    setImportKeyHex(''); setDerivedAddress(''); setGeneratedKey(null);
    setImportJson(''); setError('');
  };

  const goTo = (s: Step) => { resetFields(); setStep(s); };

  const handleKeyChange = async (hex: string) => {
    setImportKeyHex(hex);
    setDerivedAddress('');
    const clean = hex.replace(/\s/g, '').toLowerCase();
    if (/^[0-9a-f]{64}$/.test(clean)) {
      try { setDerivedAddress(await deriveAddress(clean)); } catch {}
    }
  };

  const handleCreate = async () => {
    setError('');
    if (!walletName.trim()) { setError('Please enter a wallet name'); return; }
    if (password.length < 8) { setError('Password must be at least 8 characters'); return; }
    if (password !== confirmPassword) { setError('Passwords do not match'); return; }
    const key = generatedKey?.secret_key || importKeyHex.replace(/\s/g, '').toLowerCase();
    if (!/^[0-9a-f]{64}$/.test(key)) { setError('Invalid private key'); return; }
    setLoading(true);
    try {
      const addr = generatedKey?.address || await deriveAddress(key);
      await onCreateWallet(password, walletName.trim(), key, addr);
    } catch (err) { setError(String(err)); } finally { setLoading(false); }
  };

  const handleUnlock = async () => {
    setError(''); setLoading(true);
    try {
      const ok = await onUnlock(password);
      if (!ok) setError('Incorrect password');
    } catch (err) { setError(String(err)); } finally { setLoading(false); }
  };

  const handleRestore = () => {
    setError('');
    if (!onImportBlob(importJson)) { setError('Invalid backup data'); return; }
    goTo('unlock');
  };

  const inputCls = "w-full px-4 py-3 bg-slate-800/80 border border-slate-700 rounded-xl text-sm text-slate-200 placeholder-slate-500 focus:outline-none focus:border-dag-accent focus:ring-1 focus:ring-dag-accent/30 transition-all";

  // ===== LANDING PAGE =====
  if (step === 'landing') {
    return (
      <div className="min-h-[calc(100vh-3.5rem)] flex items-center justify-center p-6">
        <div className="max-w-lg w-full space-y-8">
          {/* Hero */}
          <div className="text-center space-y-3">
            <div className="w-16 h-16 rounded-2xl bg-gradient-to-br from-dag-accent to-purple-500 flex items-center justify-center mx-auto shadow-lg shadow-dag-accent/20">
              <Zap className="w-8 h-8 text-white" />
            </div>
            <h1 className="text-3xl font-bold text-white">Welcome to UltraDAG</h1>
            <p className="text-dag-muted text-sm max-w-sm mx-auto">
              Create a wallet to send, receive, stake, and participate in governance on the UltraDAG network.
            </p>
          </div>

          {/* Action cards */}
          <div className="space-y-3">
            <button
              onClick={async () => {
                const kp = await generateKeypair();
                setGeneratedKey(kp);
                setDerivedAddress(kp.address);
                setStep('create');
              }}
              className="w-full group relative overflow-hidden rounded-xl border border-dag-accent/30 bg-dag-accent/5 hover:bg-dag-accent/10 p-5 text-left transition-all hover:border-dag-accent/50"
            >
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-4">
                  <div className="w-11 h-11 rounded-xl bg-dag-accent/15 flex items-center justify-center">
                    <Plus className="w-5 h-5 text-dag-accent" />
                  </div>
                  <div>
                    <p className="font-semibold text-white">Create New Wallet</p>
                    <p className="text-xs text-dag-muted mt-0.5">Instant setup — ready in seconds</p>
                  </div>
                </div>
                <ChevronRight className="w-5 h-5 text-dag-accent opacity-50 group-hover:opacity-100 transition-opacity" />
              </div>
            </button>

            <button
              onClick={() => goTo('import')}
              className="w-full group relative overflow-hidden rounded-xl border border-slate-700 bg-slate-800/30 hover:bg-slate-800/60 p-5 text-left transition-all hover:border-slate-600"
            >
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-4">
                  <div className="w-11 h-11 rounded-xl bg-slate-700/50 flex items-center justify-center">
                    <Key className="w-5 h-5 text-slate-300" />
                  </div>
                  <div>
                    <p className="font-semibold text-white">Import Private Key</p>
                    <p className="text-xs text-dag-muted mt-0.5">Already have a wallet? Import it here</p>
                  </div>
                </div>
                <ChevronRight className="w-5 h-5 text-slate-500 opacity-50 group-hover:opacity-100 transition-opacity" />
              </div>
            </button>
          </div>

          {/* Features */}
          <div className="grid grid-cols-3 gap-3 pt-2">
            <div className="text-center p-3 rounded-lg bg-slate-800/30 border border-slate-800">
              <Shield className="w-4 h-4 text-dag-green mx-auto mb-1.5" />
              <p className="text-[10px] text-dag-muted">Password protected</p>
            </div>
            <div className="text-center p-3 rounded-lg bg-slate-800/30 border border-slate-800">
              <Globe className="w-4 h-4 text-dag-accent mx-auto mb-1.5" />
              <p className="text-[10px] text-dag-muted">You hold the keys</p>
            </div>
            <div className="text-center p-3 rounded-lg bg-slate-800/30 border border-slate-800">
              <Zap className="w-4 h-4 text-dag-yellow mx-auto mb-1.5" />
              <p className="text-[10px] text-dag-muted">No account needed</p>
            </div>
          </div>

          <button onClick={() => goTo('restore')} className="w-full text-center text-xs text-slate-500 hover:text-slate-300 transition-colors py-2">
            Restore from backup
          </button>
        </div>
      </div>
    );
  }

  // ===== UNLOCK =====
  if (step === 'unlock') {
    return (
      <div className="min-h-[calc(100vh-3.5rem)] flex items-center justify-center p-6">
        <div className="max-w-sm w-full space-y-6">
          <div className="text-center space-y-3">
            <div className="w-14 h-14 rounded-2xl bg-gradient-to-br from-dag-accent to-purple-500 flex items-center justify-center mx-auto shadow-lg shadow-dag-accent/20">
              <Zap className="w-7 h-7 text-white" />
            </div>
            <h1 className="text-2xl font-bold text-white">Welcome Back</h1>
            <p className="text-dag-muted text-sm">
              {webauthnEnrolled ? 'Use biometrics or enter your password.' : 'Enter your password to unlock your wallet.'}
            </p>
          </div>
          <div className="space-y-3">
            {/* Biometric unlock — shown prominently when enrolled */}
            {webauthnEnrolled && onUnlockWithWebAuthn && (
              <button
                onClick={async () => {
                  setError(''); setLoading(true);
                  try {
                    const ok = await onUnlockWithWebAuthn();
                    if (!ok) setError('Biometric authentication failed. Try your password.');
                  } catch { setError('Biometric unavailable. Use your password.'); }
                  finally { setLoading(false); }
                }}
                disabled={loading}
                className="w-full py-3.5 rounded-xl bg-gradient-to-r from-dag-accent to-purple-500 text-white font-semibold text-sm hover:opacity-90 disabled:opacity-50 transition-all flex items-center justify-center gap-2"
              >
                <Fingerprint className="w-5 h-5" />
                {loading ? 'Verifying...' : 'Unlock with Biometrics'}
              </button>
            )}
            {webauthnEnrolled && <div className="flex items-center gap-3"><div className="flex-1 h-px bg-dag-border" /><span className="text-xs text-dag-muted">or</span><div className="flex-1 h-px bg-dag-border" /></div>}
            <input
              type="password" value={password} onChange={(e) => setPassword(e.target.value)}
              placeholder="Password" className={inputCls}
              onKeyDown={(e) => e.key === 'Enter' && handleUnlock()}
              autoFocus={!webauthnEnrolled}
            />
            {error && <p className="text-sm text-red-400 text-center">{error}</p>}
            <button onClick={handleUnlock} disabled={loading}
              className={`w-full py-3 rounded-xl font-semibold text-sm disabled:opacity-50 transition-colors ${
                webauthnEnrolled
                  ? 'bg-slate-700 text-slate-200 hover:bg-slate-600'
                  : 'bg-dag-accent text-white hover:bg-dag-accent/80'
              }`}>
              {loading ? 'Unlocking...' : 'Unlock with Password'}
            </button>
          </div>
          <button onClick={() => goTo('restore')} className="w-full text-center text-xs text-slate-500 hover:text-slate-300 transition-colors py-2">
            Restore from backup
          </button>
        </div>
      </div>
    );
  }

  // ===== CREATE / IMPORT FORM =====
  const isImport = step === 'import';

  if (step === 'create' || step === 'import') {
    return (
      <div className="min-h-[calc(100vh-3.5rem)] flex items-center justify-center p-6">
        <div className="max-w-md w-full space-y-5">
          <div className="flex items-center gap-3">
            <button onClick={() => goTo('landing')} className="p-2 rounded-lg text-slate-400 hover:text-white hover:bg-slate-800 transition-colors">
              <ArrowRight className="w-4 h-4 rotate-180" />
            </button>
            <div>
              <h1 className="text-xl font-bold text-white">{isImport ? 'Import Wallet' : 'Create Wallet'}</h1>
              <p className="text-xs text-dag-muted">{isImport ? 'Paste your existing private key' : 'Your new wallet is ready'}</p>
            </div>
          </div>

          {/* Generated key display */}
          {generatedKey && !isImport && (
            <div className="rounded-xl border border-dag-accent/20 bg-dag-accent/5 p-4 space-y-3">
              <div>
                <p className="text-[10px] text-dag-muted uppercase tracking-wider mb-1">Your Address</p>
                <p className="text-sm font-mono text-dag-green break-all">{generatedKey.address}</p>
              </div>
              <div>
                <div className="flex items-center justify-between mb-1">
                  <p className="text-[10px] text-dag-muted uppercase tracking-wider">Private Key</p>
                  <div className="flex items-center gap-1">
                    <button onClick={() => setShowKey(!showKey)} className="p-1 rounded text-slate-500 hover:text-white transition-colors" title={showKey ? 'Hide' : 'Reveal'}>
                      {showKey ? <EyeOff className="w-3.5 h-3.5" /> : <Eye className="w-3.5 h-3.5" />}
                    </button>
                    <button onClick={() => { navigator.clipboard.writeText(generatedKey.secret_key); setCopied(true); setTimeout(() => setCopied(false), 2000); }} className="p-1 rounded text-slate-500 hover:text-white transition-colors" title="Copy">
                      {copied ? <Check className="w-3.5 h-3.5 text-dag-green" /> : <Copy className="w-3.5 h-3.5" />}
                    </button>
                  </div>
                </div>
                <p className={`text-xs font-mono text-slate-400 break-all select-all ${showKey ? '' : 'blur-sm'} transition-all duration-200`}>{generatedKey.secret_key}</p>
              </div>
              <div className="flex items-start gap-2 mt-2 p-2 rounded-lg bg-amber-500/10 border border-amber-500/20">
                <Shield className="w-3.5 h-3.5 text-amber-400 mt-0.5 flex-shrink-0" />
                <p className="text-[11px] text-amber-300/90">Save your private key now. It cannot be recovered if lost.</p>
              </div>
            </div>
          )}

          {/* Import key input */}
          {isImport && (
            <>
              <input type="password" value={importKeyHex} onChange={(e) => handleKeyChange(e.target.value)}
                placeholder="Private key (64 hex characters)" className={inputCls + ' font-mono text-xs'}
                autoFocus
              />
              {derivedAddress && (
                <div className="rounded-lg bg-slate-800/50 border border-dag-border/50 px-4 py-2.5">
                  <p className="text-[10px] text-dag-muted uppercase tracking-wider">Address</p>
                  <p className="text-sm font-mono text-dag-green break-all">{derivedAddress}</p>
                </div>
              )}
            </>
          )}

          <div className="space-y-3">
            <input type="text" value={walletName} onChange={(e) => setWalletName(e.target.value)}
              placeholder="Wallet name" className={inputCls}
              autoFocus={!isImport}
            />
            <input type="password" value={password} onChange={(e) => setPassword(e.target.value)}
              placeholder="Set a password (min 8 characters)" className={inputCls}
            />
            <input type="password" value={confirmPassword} onChange={(e) => setConfirmPassword(e.target.value)}
              placeholder="Confirm password" className={inputCls}
              onKeyDown={(e) => e.key === 'Enter' && handleCreate()}
            />
          </div>

          <p className="text-[10px] text-dag-muted text-center">Your wallet is encrypted with AES-256-GCM and stored only in your browser.</p>

          {error && <p className="text-sm text-red-400 text-center">{error}</p>}

          <button onClick={handleCreate} disabled={loading}
            className="w-full py-3 rounded-xl bg-dag-accent text-white font-semibold text-sm hover:bg-dag-accent/80 disabled:opacity-50 transition-colors">
            {loading ? 'Creating...' : isImport ? 'Import Wallet' : 'Create Wallet'}
          </button>
        </div>
      </div>
    );
  }

  // ===== RESTORE =====
  return (
    <div className="min-h-[calc(100vh-3.5rem)] flex items-center justify-center p-6">
      <div className="max-w-md w-full space-y-5">
        <div className="flex items-center gap-3">
          <button onClick={() => goTo(hasExisting ? 'unlock' : 'landing')} className="p-2 rounded-lg text-slate-400 hover:text-white hover:bg-slate-800 transition-colors">
            <ArrowRight className="w-4 h-4 rotate-180" />
          </button>
          <div>
            <h1 className="text-xl font-bold text-white">Restore from Backup</h1>
            <p className="text-xs text-dag-muted">Paste a previously exported wallet backup</p>
          </div>
        </div>
        <textarea value={importJson} onChange={(e) => setImportJson(e.target.value)}
          placeholder="Paste your backup JSON here..."
          rows={6}
          className={inputCls + ' font-mono text-xs resize-none'}
          autoFocus
        />
        {error && <p className="text-sm text-red-400 text-center">{error}</p>}
        <button onClick={handleRestore} disabled={loading}
          className="w-full py-3 rounded-xl bg-dag-accent text-white font-semibold text-sm hover:bg-dag-accent/80 disabled:opacity-50 transition-colors">
          Restore
        </button>
      </div>
    </div>
  );
}
