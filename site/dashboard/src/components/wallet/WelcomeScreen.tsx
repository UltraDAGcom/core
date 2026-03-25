import { useState } from 'react';
import { Plus, Key, ChevronRight, Shield, Zap, Globe, ArrowRight, Eye, EyeOff, Copy, Check, Fingerprint, Lock, Sparkles, Wallet, ArrowDown } from 'lucide-react';
import { deriveAddress, generateKeypair } from '../../lib/keygen';

interface WelcomeScreenProps {
  onCreateWallet: (password: string, name: string, secretKey: string, address: string) => Promise<boolean>;
  onImportBlob: (json: string) => boolean;
  onUnlock: (password: string) => Promise<boolean>;
  onUnlockWithWebAuthn?: () => Promise<boolean>;
  onEnrollWebAuthn?: () => Promise<boolean>;
  webauthnAvailable?: boolean;
  webauthnEnrolled?: boolean;
  hasExisting: boolean;
  onFinishOnboarding?: () => void;
  isPostCreate?: boolean;
}

type Step = 'landing' | 'backup' | 'import' | 'secure' | 'biometrics' | 'success' | 'unlock' | 'restore';

const TOTAL_CREATE_STEPS = 4; // backup, secure, biometrics, success (landing not counted)

function StepIndicator({ current, total, labels }: { current: number; total: number; labels: string[] }) {
  return (
    <div className="flex items-center justify-center gap-1 mb-8">
      {Array.from({ length: total }, (_, i) => {
        const stepNum = i + 1;
        const isActive = stepNum === current;
        const isDone = stepNum < current;
        return (
          <div key={i} className="flex items-center gap-1">
            <div className="flex flex-col items-center">
              <div className={`
                w-8 h-8 rounded-full flex items-center justify-center text-xs font-semibold transition-all duration-300
                ${isActive ? 'bg-dag-accent text-white shadow-lg shadow-dag-accent/30 scale-110' : ''}
                ${isDone ? 'bg-dag-green/20 text-dag-green' : ''}
                ${!isActive && !isDone ? 'bg-slate-800 text-slate-500' : ''}
              `}>
                {isDone ? <Check className="w-3.5 h-3.5" /> : stepNum}
              </div>
              <span className={`text-[9px] mt-1 transition-colors ${isActive ? 'text-dag-accent' : isDone ? 'text-dag-green' : 'text-slate-600'}`}>
                {labels[i]}
              </span>
            </div>
            {i < total - 1 && (
              <div className={`w-8 h-px mb-4 transition-colors ${isDone ? 'bg-dag-green/40' : 'bg-slate-800'}`} />
            )}
          </div>
        );
      })}
    </div>
  );
}

export function WelcomeScreen({ onCreateWallet, onImportBlob, onUnlock, onUnlockWithWebAuthn, onEnrollWebAuthn, webauthnAvailable, webauthnEnrolled, hasExisting, onFinishOnboarding, isPostCreate }: WelcomeScreenProps) {
  // If we're returning after wallet creation (isPostCreate), start at biometrics step
  const initialStep: Step = isPostCreate
    ? (webauthnAvailable ? 'biometrics' : 'success')
    : (hasExisting ? 'unlock' : 'landing');

  const [step, setStep] = useState<Step>(initialStep);
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
  const [confirmedBackup, setConfirmedBackup] = useState(false);
  const [isImportFlow, setIsImportFlow] = useState(false);
  const [biometricsDone, setBiometricsDone] = useState(false);

  const resetFields = () => {
    setPassword(''); setConfirmPassword(''); setWalletName('');
    setImportKeyHex(''); setDerivedAddress(''); setGeneratedKey(null);
    setImportJson(''); setError(''); setConfirmedBackup(false);
    setIsImportFlow(false); setBiometricsDone(false);
  };

  const goTo = (s: Step) => { setError(''); setStep(s); };

  const handleKeyChange = async (hex: string) => {
    setImportKeyHex(hex);
    setDerivedAddress('');
    const clean = hex.replace(/\s/g, '').toLowerCase();
    if (/^[0-9a-f]{64}$/.test(clean)) {
      try { setDerivedAddress(await deriveAddress(clean)); } catch {}
    }
  };

  const handleSecureSubmit = async () => {
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
      // Wallet is now created and unlocked — move to biometrics or success
      if (webauthnAvailable) {
        goTo('biometrics');
      } else {
        goTo('success');
      }
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

  const handleBiometricEnroll = async () => {
    if (!onEnrollWebAuthn) return;
    setError(''); setLoading(true);
    try {
      const ok = await onEnrollWebAuthn();
      if (ok) {
        setBiometricsDone(true);
        // Brief pause to show success, then advance
        setTimeout(() => goTo('success'), 800);
      } else {
        setError('Biometric setup was cancelled. You can enable it later in Settings.');
      }
    } catch {
      setError('Biometric setup failed. You can enable it later in Settings.');
    } finally { setLoading(false); }
  };

  const currentStepNum = step === 'backup' ? 1 : step === 'secure' ? 2 : step === 'biometrics' ? 3 : step === 'success' ? 4 : 0;
  const stepLabels = ['Backup', 'Secure', 'Biometrics', 'Done'];

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
              The fast, lightweight network for instant payments. Create a wallet to get started.
            </p>
          </div>

          {/* Action cards */}
          <div className="space-y-3">
            <button
              onClick={async () => {
                const kp = await generateKeypair();
                setGeneratedKey(kp);
                setDerivedAddress(kp.address);
                setIsImportFlow(false);
                setStep('backup');
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
              onClick={() => { setIsImportFlow(true); goTo('import'); }}
              className="w-full group relative overflow-hidden rounded-xl border border-slate-700 bg-slate-800/30 hover:bg-slate-800/60 p-5 text-left transition-all hover:border-slate-600"
            >
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-4">
                  <div className="w-11 h-11 rounded-xl bg-slate-700/50 flex items-center justify-center">
                    <Key className="w-5 h-5 text-slate-300" />
                  </div>
                  <div>
                    <p className="font-semibold text-white">Import Existing Wallet</p>
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

  // ===== STEP 1: BACKUP KEY (create flow only) =====
  if (step === 'backup') {
    return (
      <div className="min-h-[calc(100vh-3.5rem)] flex items-center justify-center p-6">
        <div className="max-w-md w-full">
          <StepIndicator current={1} total={TOTAL_CREATE_STEPS} labels={stepLabels} />

          <div className="space-y-5">
            <div className="flex items-center gap-3">
              <button onClick={() => { resetFields(); goTo('landing'); }} className="p-2 rounded-lg text-slate-400 hover:text-white hover:bg-slate-800 transition-colors">
                <ArrowRight className="w-4 h-4 rotate-180" />
              </button>
              <div>
                <h1 className="text-xl font-bold text-white">Back Up Your Key</h1>
                <p className="text-xs text-dag-muted">This is the only way to recover your wallet</p>
              </div>
            </div>

            {generatedKey && (
              <div className="rounded-xl border border-dag-accent/20 bg-dag-accent/5 p-4 space-y-4">
                {/* Address */}
                <div>
                  <p className="text-[10px] text-dag-muted uppercase tracking-wider mb-1.5">Your Wallet Address</p>
                  <div className="flex items-center gap-2 bg-slate-800/60 rounded-lg p-3">
                    <Wallet className="w-4 h-4 text-dag-green flex-shrink-0" />
                    <p className="text-sm font-mono text-dag-green break-all flex-1">{generatedKey.address}</p>
                  </div>
                </div>

                {/* Private key */}
                <div>
                  <div className="flex items-center justify-between mb-1.5">
                    <p className="text-[10px] text-dag-muted uppercase tracking-wider">Private Key</p>
                    <div className="flex items-center gap-1">
                      <button onClick={() => setShowKey(!showKey)} className="p-1.5 rounded-lg text-slate-500 hover:text-white hover:bg-slate-700/50 transition-colors" title={showKey ? 'Hide' : 'Reveal'}>
                        {showKey ? <EyeOff className="w-3.5 h-3.5" /> : <Eye className="w-3.5 h-3.5" />}
                      </button>
                      <button
                        onClick={() => { navigator.clipboard.writeText(generatedKey.secret_key); setCopied(true); setTimeout(() => setCopied(false), 2000); }}
                        className={`flex items-center gap-1.5 px-2.5 py-1 rounded-lg text-xs font-medium transition-all ${
                          copied
                            ? 'bg-dag-green/15 text-dag-green'
                            : 'bg-slate-700/50 text-slate-300 hover:bg-slate-700 hover:text-white'
                        }`}
                      >
                        {copied ? <Check className="w-3 h-3" /> : <Copy className="w-3 h-3" />}
                        {copied ? 'Copied!' : 'Copy'}
                      </button>
                    </div>
                  </div>
                  <div className="bg-slate-800/60 rounded-lg p-3">
                    <p className={`text-xs font-mono text-slate-400 break-all select-all ${showKey ? '' : 'blur-sm'} transition-all duration-200`}>
                      {generatedKey.secret_key}
                    </p>
                  </div>
                </div>

                {/* Warning */}
                <div className="flex items-start gap-2.5 p-3 rounded-lg bg-amber-500/10 border border-amber-500/20">
                  <Shield className="w-4 h-4 text-amber-400 mt-0.5 flex-shrink-0" />
                  <div>
                    <p className="text-[11px] text-amber-300 font-semibold">Save this key now</p>
                    <p className="text-[10px] text-amber-300/70 mt-0.5">Store it somewhere safe and private. If you lose this key, your funds are gone forever. We cannot recover it for you.</p>
                  </div>
                </div>

                {/* Confirmation */}
                <label className="flex items-center gap-3 mt-2 cursor-pointer select-none p-2.5 rounded-lg hover:bg-slate-800/30 transition-colors -mx-1">
                  <input
                    type="checkbox"
                    checked={confirmedBackup}
                    onChange={e => setConfirmedBackup(e.target.checked)}
                    className="w-4.5 h-4.5 rounded border-slate-600 bg-slate-800 text-dag-accent focus:ring-dag-accent/30 flex-shrink-0"
                  />
                  <span className="text-sm text-slate-300">I have saved my private key securely</span>
                </label>
              </div>
            )}

            <button
              onClick={() => goTo('secure')}
              disabled={!confirmedBackup}
              className="w-full py-3.5 rounded-xl bg-dag-accent text-white font-semibold text-sm hover:bg-dag-accent/80 disabled:opacity-30 disabled:cursor-not-allowed transition-all flex items-center justify-center gap-2"
            >
              Continue
              <ArrowRight className="w-4 h-4" />
            </button>
          </div>
        </div>
      </div>
    );
  }

  // ===== IMPORT KEY =====
  if (step === 'import') {
    return (
      <div className="min-h-[calc(100vh-3.5rem)] flex items-center justify-center p-6">
        <div className="max-w-md w-full">
          <StepIndicator current={1} total={TOTAL_CREATE_STEPS} labels={stepLabels} />

          <div className="space-y-5">
            <div className="flex items-center gap-3">
              <button onClick={() => { resetFields(); goTo('landing'); }} className="p-2 rounded-lg text-slate-400 hover:text-white hover:bg-slate-800 transition-colors">
                <ArrowRight className="w-4 h-4 rotate-180" />
              </button>
              <div>
                <h1 className="text-xl font-bold text-white">Import Your Key</h1>
                <p className="text-xs text-dag-muted">Paste your existing private key to restore access</p>
              </div>
            </div>

            <div className="space-y-3">
              <div>
                <label className="text-[10px] text-dag-muted uppercase tracking-wider mb-1.5 block">Private Key</label>
                <input type="password" value={importKeyHex} onChange={(e) => handleKeyChange(e.target.value)}
                  placeholder="Paste your 64-character hex private key"
                  className={inputCls + ' font-mono text-xs'}
                  autoFocus
                />
              </div>
              {derivedAddress && (
                <div className="rounded-xl bg-dag-green/5 border border-dag-green/20 p-3">
                  <div className="flex items-center gap-2">
                    <Check className="w-4 h-4 text-dag-green flex-shrink-0" />
                    <div>
                      <p className="text-[10px] text-dag-muted uppercase tracking-wider">Wallet Found</p>
                      <p className="text-sm font-mono text-dag-green break-all">{derivedAddress}</p>
                    </div>
                  </div>
                </div>
              )}
            </div>

            {error && <p className="text-sm text-red-400 text-center">{error}</p>}

            <button
              onClick={() => goTo('secure')}
              disabled={!derivedAddress}
              className="w-full py-3.5 rounded-xl bg-dag-accent text-white font-semibold text-sm hover:bg-dag-accent/80 disabled:opacity-30 disabled:cursor-not-allowed transition-all flex items-center justify-center gap-2"
            >
              Continue
              <ArrowRight className="w-4 h-4" />
            </button>
          </div>
        </div>
      </div>
    );
  }

  // ===== STEP 2: SECURE (name + password) =====
  if (step === 'secure') {
    return (
      <div className="min-h-[calc(100vh-3.5rem)] flex items-center justify-center p-6">
        <div className="max-w-md w-full">
          <StepIndicator current={2} total={TOTAL_CREATE_STEPS} labels={stepLabels} />

          <div className="space-y-5">
            <div className="flex items-center gap-3">
              <button onClick={() => goTo(isImportFlow ? 'import' : 'backup')} className="p-2 rounded-lg text-slate-400 hover:text-white hover:bg-slate-800 transition-colors">
                <ArrowRight className="w-4 h-4 rotate-180" />
              </button>
              <div>
                <h1 className="text-xl font-bold text-white">Secure Your Wallet</h1>
                <p className="text-xs text-dag-muted">Choose a name and set a password to encrypt your keys</p>
              </div>
            </div>

            <div className="space-y-4">
              <div>
                <label className="text-[10px] text-dag-muted uppercase tracking-wider mb-1.5 block">Wallet Name</label>
                <input
                  type="text" value={walletName} onChange={(e) => setWalletName(e.target.value)}
                  placeholder="e.g. My Wallet, Savings, Trading..."
                  className={inputCls}
                  autoFocus
                />
              </div>
              <div>
                <label className="text-[10px] text-dag-muted uppercase tracking-wider mb-1.5 block">Password</label>
                <input
                  type="password" value={password} onChange={(e) => setPassword(e.target.value)}
                  placeholder="At least 8 characters"
                  className={inputCls}
                />
                {password.length > 0 && password.length < 8 && (
                  <p className="text-[10px] text-amber-400 mt-1.5 ml-1">{8 - password.length} more characters needed</p>
                )}
                {password.length >= 8 && (
                  <p className="text-[10px] text-dag-green mt-1.5 ml-1 flex items-center gap-1">
                    <Check className="w-3 h-3" /> Strong enough
                  </p>
                )}
              </div>
              <div>
                <label className="text-[10px] text-dag-muted uppercase tracking-wider mb-1.5 block">Confirm Password</label>
                <input
                  type="password" value={confirmPassword} onChange={(e) => setConfirmPassword(e.target.value)}
                  placeholder="Re-enter your password"
                  className={inputCls}
                  onKeyDown={(e) => e.key === 'Enter' && handleSecureSubmit()}
                />
                {confirmPassword.length > 0 && password !== confirmPassword && (
                  <p className="text-[10px] text-red-400 mt-1.5 ml-1">Passwords don't match</p>
                )}
                {confirmPassword.length > 0 && password === confirmPassword && password.length >= 8 && (
                  <p className="text-[10px] text-dag-green mt-1.5 ml-1 flex items-center gap-1">
                    <Check className="w-3 h-3" /> Passwords match
                  </p>
                )}
              </div>
            </div>

            <div className="flex items-start gap-2.5 p-3 rounded-lg bg-slate-800/50 border border-slate-700/50">
              <Lock className="w-4 h-4 text-slate-400 mt-0.5 flex-shrink-0" />
              <p className="text-[10px] text-slate-400">Your wallet is encrypted and stored only in this browser. We never see your keys or password.</p>
            </div>

            {error && <p className="text-sm text-red-400 text-center">{error}</p>}

            <button
              onClick={handleSecureSubmit}
              disabled={loading || !walletName.trim() || password.length < 8 || password !== confirmPassword}
              className="w-full py-3.5 rounded-xl bg-dag-accent text-white font-semibold text-sm hover:bg-dag-accent/80 disabled:opacity-30 disabled:cursor-not-allowed transition-all flex items-center justify-center gap-2"
            >
              {loading ? (
                <span className="flex items-center gap-2">
                  <span className="w-4 h-4 border-2 border-white/30 border-t-white rounded-full animate-spin" />
                  Creating Wallet...
                </span>
              ) : (
                <>
                  Create Wallet
                  <ArrowRight className="w-4 h-4" />
                </>
              )}
            </button>
          </div>
        </div>
      </div>
    );
  }

  // ===== STEP 3: BIOMETRICS =====
  if (step === 'biometrics') {
    return (
      <div className="min-h-[calc(100vh-3.5rem)] flex items-center justify-center p-6">
        <div className="max-w-md w-full">
          <StepIndicator current={3} total={TOTAL_CREATE_STEPS} labels={stepLabels} />

          <div className="space-y-6 text-center">
            <div className="space-y-3">
              <div className={`w-20 h-20 rounded-2xl flex items-center justify-center mx-auto shadow-lg transition-all duration-500 ${
                biometricsDone
                  ? 'bg-gradient-to-br from-dag-green to-emerald-500 shadow-dag-green/20'
                  : 'bg-gradient-to-br from-dag-accent to-purple-500 shadow-dag-accent/20'
              }`}>
                {biometricsDone ? (
                  <Check className="w-10 h-10 text-white" />
                ) : (
                  <Fingerprint className="w-10 h-10 text-white" />
                )}
              </div>
              <h1 className="text-2xl font-bold text-white">
                {biometricsDone ? 'Biometrics Enabled!' : 'Quick Unlock'}
              </h1>
              <p className="text-dag-muted text-sm max-w-xs mx-auto">
                {biometricsDone
                  ? 'You can now unlock your wallet with Face ID or fingerprint.'
                  : 'Enable biometrics to unlock your wallet instantly — no password needed every time.'}
              </p>
            </div>

            {!biometricsDone && (
              <>
                {/* Visual benefit cards */}
                <div className="grid grid-cols-2 gap-3 text-left">
                  <div className="p-3 rounded-xl bg-slate-800/50 border border-slate-700/50">
                    <Zap className="w-4 h-4 text-dag-yellow mb-2" />
                    <p className="text-xs text-white font-medium">Instant access</p>
                    <p className="text-[10px] text-slate-500 mt-0.5">Open your wallet in under a second</p>
                  </div>
                  <div className="p-3 rounded-xl bg-slate-800/50 border border-slate-700/50">
                    <Shield className="w-4 h-4 text-dag-green mb-2" />
                    <p className="text-xs text-white font-medium">Same security</p>
                    <p className="text-[10px] text-slate-500 mt-0.5">Your password still protects your keys</p>
                  </div>
                </div>

                {error && <p className="text-sm text-red-400">{error}</p>}

                <button
                  onClick={handleBiometricEnroll}
                  disabled={loading}
                  className="w-full py-3.5 rounded-xl bg-gradient-to-r from-dag-accent to-purple-500 text-white font-semibold text-sm hover:opacity-90 disabled:opacity-50 transition-all flex items-center justify-center gap-2"
                >
                  {loading ? (
                    <span className="flex items-center gap-2">
                      <span className="w-4 h-4 border-2 border-white/30 border-t-white rounded-full animate-spin" />
                      Waiting for biometrics...
                    </span>
                  ) : (
                    <>
                      <Fingerprint className="w-5 h-5" />
                      Enable Biometrics
                    </>
                  )}
                </button>

                <button
                  onClick={() => goTo('success')}
                  className="w-full text-center text-sm text-slate-500 hover:text-slate-300 transition-colors py-1"
                >
                  Skip for now
                </button>
              </>
            )}
          </div>
        </div>
      </div>
    );
  }

  // ===== STEP 4: SUCCESS =====
  if (step === 'success') {
    return (
      <div className="min-h-[calc(100vh-3.5rem)] flex items-center justify-center p-6">
        <div className="max-w-md w-full">
          <StepIndicator current={4} total={TOTAL_CREATE_STEPS} labels={stepLabels} />

          <div className="space-y-6 text-center">
            <div className="space-y-3">
              <div className="w-20 h-20 rounded-2xl bg-gradient-to-br from-dag-green to-emerald-500 flex items-center justify-center mx-auto shadow-lg shadow-dag-green/20">
                <Sparkles className="w-10 h-10 text-white" />
              </div>
              <h1 className="text-2xl font-bold text-white">You're All Set!</h1>
              <p className="text-dag-muted text-sm max-w-xs mx-auto">
                Your wallet is ready. Here's what you can do next.
              </p>
            </div>

            {/* Quick start cards */}
            <div className="space-y-2.5 text-left">
              <div className="flex items-center gap-3 p-3.5 rounded-xl bg-slate-800/50 border border-slate-700/50">
                <div className="w-9 h-9 rounded-lg bg-dag-yellow/15 flex items-center justify-center flex-shrink-0">
                  <ArrowDown className="w-4.5 h-4.5 text-dag-yellow" />
                </div>
                <div className="flex-1 min-w-0">
                  <p className="text-sm text-white font-medium">Get testnet UDAG</p>
                  <p className="text-[10px] text-slate-500 mt-0.5">Use the faucet on the Dashboard to receive free test tokens</p>
                </div>
              </div>

              <div className="flex items-center gap-3 p-3.5 rounded-xl bg-slate-800/50 border border-slate-700/50">
                <div className="w-9 h-9 rounded-lg bg-dag-accent/15 flex items-center justify-center flex-shrink-0">
                  <Zap className="w-4.5 h-4.5 text-dag-accent" />
                </div>
                <div className="flex-1 min-w-0">
                  <p className="text-sm text-white font-medium">Send a payment</p>
                  <p className="text-[10px] text-slate-500 mt-0.5">Transfer UDAG to any address in seconds</p>
                </div>
              </div>

              <div className="flex items-center gap-3 p-3.5 rounded-xl bg-slate-800/50 border border-slate-700/50">
                <div className="w-9 h-9 rounded-lg bg-purple-500/15 flex items-center justify-center flex-shrink-0">
                  <Globe className="w-4.5 h-4.5 text-purple-400" />
                </div>
                <div className="flex-1 min-w-0">
                  <p className="text-sm text-white font-medium">Explore the network</p>
                  <p className="text-[10px] text-slate-500 mt-0.5">View live rounds, transactions, and validators</p>
                </div>
              </div>
            </div>

            <button
              onClick={onFinishOnboarding}
              className="w-full py-3.5 rounded-xl bg-dag-accent text-white font-semibold text-sm hover:bg-dag-accent/80 transition-all flex items-center justify-center gap-2"
            >
              Go to Dashboard
              <ArrowRight className="w-4 h-4" />
            </button>
          </div>
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
