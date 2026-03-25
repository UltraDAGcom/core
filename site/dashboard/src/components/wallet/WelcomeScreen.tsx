import { useState, useEffect } from 'react';
import { Plus, Key, ChevronRight, Shield, Zap, Globe, ArrowRight, Eye, EyeOff, Copy, Check, Fingerprint, Lock, Sparkles, Wallet, ArrowDown, Download, TestTube, Rocket, AlertTriangle, Trash2 } from 'lucide-react';
import { deriveAddress } from '../../lib/keygen';
import { generateWithMnemonic, mnemonicToKeypair, isValidMnemonic } from '../../lib/mnemonic';
import type { NetworkType } from '../../lib/api';

interface WelcomeScreenProps {
  onCreateWallet: (password: string, name: string, secretKey: string, address: string) => Promise<boolean>;
  onImportBlob: (json: string) => boolean;
  onUnlock: (password: string) => Promise<boolean>;
  onUnlockWithWebAuthn?: () => Promise<boolean>;
  onEnrollWebAuthn?: () => Promise<boolean>;
  onExportBlob?: () => string | null;
  onResetWallet?: () => void;
  webauthnAvailable?: boolean;
  webauthnEnrolled?: boolean;
  hasExisting: boolean;
  onFinishOnboarding?: () => void;
  isPostCreate?: boolean;
  network: NetworkType;
  onSwitchNetwork: (net: NetworkType) => void;
}

type Step = 'landing' | 'network' | 'backup' | 'import' | 'secure' | 'biometrics' | 'success' | 'unlock' | 'restore';

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
              <span className={`text-[9px] mt-1 transition-colors whitespace-nowrap ${isActive ? 'text-dag-accent' : isDone ? 'text-dag-green' : 'text-slate-600'}`}>
                {labels[i]}
              </span>
            </div>
            {i < total - 1 && (
              <div className={`w-6 h-px mb-4 transition-colors ${isDone ? 'bg-dag-green/40' : 'bg-slate-800'}`} />
            )}
          </div>
        );
      })}
    </div>
  );
}

function NetworkBadge({ network }: { network: NetworkType }) {
  if (network === 'testnet') {
    return (
      <span className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full text-[10px] font-semibold bg-dag-yellow/15 text-dag-yellow border border-dag-yellow/20">
        <TestTube className="w-3 h-3" /> Testnet
      </span>
    );
  }
  return (
    <span className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full text-[10px] font-semibold bg-dag-green/15 text-dag-green border border-dag-green/20">
      <Rocket className="w-3 h-3" /> Mainnet
    </span>
  );
}

// Create flow: Network → Backup → Secure → Biometrics → Done
const CREATE_STEPS = 5;
const CREATE_LABELS = ['Network', 'Backup', 'Secure', 'Biometrics', 'Done'];
// Import flow: Network → Import → Biometrics → Done
const IMPORT_STEPS = 4;
const IMPORT_LABELS = ['Network', 'Import', 'Biometrics', 'Done'];

export function WelcomeScreen({
  onCreateWallet, onImportBlob, onUnlock, onUnlockWithWebAuthn, onEnrollWebAuthn,
  onExportBlob, onResetWallet, webauthnAvailable, webauthnEnrolled, hasExisting, onFinishOnboarding,
  isPostCreate, network, onSwitchNetwork,
}: WelcomeScreenProps) {
  const initialStep: Step = isPostCreate
    ? (webauthnAvailable ? 'biometrics' : 'success')
    : (hasExisting ? 'unlock' : 'landing');

  const [step, setStep] = useState<Step>(initialStep);

  // When keystore is destroyed, go back to landing
  useEffect(() => {
    if (!hasExisting && !isPostCreate && step === 'unlock') {
      setStep('landing');
      setShowResetConfirm(false);
      setError('');
      setPassword('');
    }
  }, [hasExisting]);

  const [password, setPassword] = useState('');
  const [confirmPassword, setConfirmPassword] = useState('');
  const [walletName, setWalletName] = useState('');
  const [importKeyHex, setImportKeyHex] = useState('');
  const [importMnemonic, setImportMnemonic] = useState('');
  const [importMode, setImportMode] = useState<'mnemonic' | 'hex'>('mnemonic');
  const [derivedAddress, setDerivedAddress] = useState('');
  const [generatedKey, setGeneratedKey] = useState<{ secret_key: string; address: string; mnemonic?: string } | null>(null);
  const [importJson, setImportJson] = useState('');
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);
  const [showKey, setShowKey] = useState(false);
  const [copied, setCopied] = useState<string | null>(null);
  const [confirmedBackup, setConfirmedBackup] = useState(false);
  const [isImportFlow, setIsImportFlow] = useState(false);
  const [biometricsDone, setBiometricsDone] = useState(false);
  const [keystoreDownloaded, setKeystoreDownloaded] = useState(false);
  const [showResetConfirm, setShowResetConfirm] = useState(false);

  const goTo = (s: Step) => { setError(''); setStep(s); };

  const copyText = (text: string, label: string) => {
    navigator.clipboard.writeText(text);
    setCopied(label);
    setTimeout(() => setCopied(null), 2000);
  };

  const handleHexKeyChange = async (hex: string) => {
    setImportKeyHex(hex);
    setDerivedAddress('');
    const clean = hex.replace(/\s/g, '').toLowerCase();
    if (/^[0-9a-f]{64}$/.test(clean)) {
      try { setDerivedAddress(await deriveAddress(clean)); } catch {}
    }
  };

  const handleMnemonicChange = async (phrase: string) => {
    setImportMnemonic(phrase);
    setDerivedAddress('');
    if (isValidMnemonic(phrase)) {
      try {
        const kp = await mnemonicToKeypair(phrase);
        setDerivedAddress(kp.address);
        setGeneratedKey(kp);
      } catch {}
    }
  };

  // Create flow: save generated key with name + password
  const handleSecureSubmit = async () => {
    setError('');
    if (!walletName.trim()) { setError('Please enter a wallet name'); return; }
    if (password.length < 8) { setError('Password must be at least 8 characters'); return; }
    if (password !== confirmPassword) { setError('Passwords do not match'); return; }
    if (!generatedKey) { setError('No key generated'); return; }
    if (!/^[0-9a-f]{64}$/.test(generatedKey.secret_key)) { setError('Invalid key'); return; }
    setLoading(true);
    try {
      await onCreateWallet(password, walletName.trim(), generatedKey.secret_key, generatedKey.address);
      goTo(webauthnAvailable ? 'biometrics' : 'success');
    } catch (err) { setError(String(err)); } finally { setLoading(false); }
  };

  // Import flow: import existing key with name + password (all on one screen)
  const handleImportSubmit = async () => {
    setError('');
    if (!walletName.trim()) { setError('Please enter a wallet name'); return; }
    if (password.length < 8) { setError('Password must be at least 8 characters'); return; }
    if (password !== confirmPassword) { setError('Passwords do not match'); return; }

    let key: string;
    let addr: string;
    if (importMode === 'mnemonic' && generatedKey) {
      key = generatedKey.secret_key;
      addr = generatedKey.address;
    } else if (importMode === 'hex') {
      key = importKeyHex.replace(/\s/g, '').toLowerCase();
      addr = derivedAddress;
    } else {
      setError('No valid key entered'); return;
    }

    if (!/^[0-9a-f]{64}$/.test(key) || !addr) { setError('Invalid key'); return; }
    setLoading(true);
    try {
      await onCreateWallet(password, walletName.trim(), key, addr);
      goTo(webauthnAvailable ? 'biometrics' : 'success');
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
        setTimeout(() => goTo('success'), 800);
      } else {
        setError('Biometric setup was cancelled. You can enable it later in Settings.');
      }
    } catch {
      setError('Biometric setup failed. You can enable it later in Settings.');
    } finally { setLoading(false); }
  };

  const handleDownloadKeystore = () => {
    if (!onExportBlob) return;
    const json = onExportBlob();
    if (json) {
      const blob = new Blob([json], { type: 'application/json' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `ultradag-${network}-keystore.json`;
      a.click();
      URL.revokeObjectURL(url);
      setKeystoreDownloaded(true);
    }
  };

  const totalSteps = isImportFlow ? IMPORT_STEPS : CREATE_STEPS;
  const stepLabels = isImportFlow ? IMPORT_LABELS : CREATE_LABELS;
  const inputCls = "w-full px-4 py-3 bg-slate-800/80 border border-slate-700 rounded-xl text-sm text-slate-200 placeholder-slate-500 focus:outline-none focus:border-dag-accent focus:ring-1 focus:ring-dag-accent/30 transition-all";

  // Step numbers differ per flow
  const stepNum = (() => {
    if (step === 'network') return 1;
    if (step === 'backup') return 2; // create only
    if (step === 'import') return 2; // import only
    if (step === 'secure') return 3; // create only
    if (step === 'biometrics') return isImportFlow ? 3 : 4;
    if (step === 'success') return isImportFlow ? 4 : 5;
    return 0;
  })();

  // ===== LANDING =====
  if (step === 'landing') {
    return (
      <div className="min-h-[calc(100vh-3.5rem)] flex items-center justify-center p-6">
        <div className="max-w-lg w-full space-y-8">
          <div className="text-center space-y-3">
            <div className="w-16 h-16 rounded-2xl bg-gradient-to-br from-dag-accent to-purple-500 flex items-center justify-center mx-auto shadow-lg shadow-dag-accent/20">
              <Zap className="w-8 h-8 text-white" />
            </div>
            <h1 className="text-3xl font-bold text-white">Welcome to UltraDAG</h1>
            <p className="text-dag-muted text-sm max-w-sm mx-auto">
              The fast, lightweight network for instant payments. Create a wallet to get started.
            </p>
          </div>

          <div className="space-y-3">
            <button
              onClick={() => { setIsImportFlow(false); goTo('network'); }}
              className="w-full group relative overflow-hidden rounded-xl border border-dag-accent/30 bg-dag-accent/5 hover:bg-dag-accent/10 p-5 text-left transition-all hover:border-dag-accent/50"
            >
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-4">
                  <div className="w-11 h-11 rounded-xl bg-dag-accent/15 flex items-center justify-center">
                    <Plus className="w-5 h-5 text-dag-accent" />
                  </div>
                  <div>
                    <p className="font-semibold text-white">Create New Wallet</p>
                    <p className="text-xs text-dag-muted mt-0.5">Generate a 12-word recovery phrase</p>
                  </div>
                </div>
                <ChevronRight className="w-5 h-5 text-dag-accent opacity-50 group-hover:opacity-100 transition-opacity" />
              </div>
            </button>

            <button
              onClick={() => { setIsImportFlow(true); goTo('network'); }}
              className="w-full group relative overflow-hidden rounded-xl border border-slate-700 bg-slate-800/30 hover:bg-slate-800/60 p-5 text-left transition-all hover:border-slate-600"
            >
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-4">
                  <div className="w-11 h-11 rounded-xl bg-slate-700/50 flex items-center justify-center">
                    <Key className="w-5 h-5 text-slate-300" />
                  </div>
                  <div>
                    <p className="font-semibold text-white">Import Existing Wallet</p>
                    <p className="text-xs text-dag-muted mt-0.5">Recovery phrase or private key</p>
                  </div>
                </div>
                <ChevronRight className="w-5 h-5 text-slate-500 opacity-50 group-hover:opacity-100 transition-opacity" />
              </div>
            </button>
          </div>

          <div className="grid grid-cols-3 gap-3 pt-2">
            <div className="text-center p-3 rounded-lg bg-slate-800/30 border border-slate-800">
              <Shield className="w-4 h-4 text-dag-green mx-auto mb-1.5" />
              <p className="text-[10px] text-dag-muted">Encrypted locally</p>
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
            Restore from backup file
          </button>
        </div>
      </div>
    );
  }

  // ===== UNLOCK =====
  if (step === 'unlock') {
    return (
      <div className="min-h-[calc(100vh-3.5rem)] flex items-center justify-center p-6">
        <div className="max-w-md w-full space-y-6">
          {/* Header */}
          <div className="text-center space-y-3">
            <div className="w-14 h-14 rounded-2xl bg-gradient-to-br from-dag-accent to-purple-500 flex items-center justify-center mx-auto shadow-lg shadow-dag-accent/20">
              <Zap className="w-7 h-7 text-white" />
            </div>
            <h1 className="text-2xl font-bold text-white">Welcome Back</h1>
            <p className="text-dag-muted text-sm">Unlock your wallet to continue</p>
          </div>

          {/* Primary unlock card */}
          <div className="rounded-xl border border-slate-700/50 bg-slate-800/30 p-5 space-y-4">
            {/* Biometric unlock — big prominent option */}
            {webauthnEnrolled && onUnlockWithWebAuthn && (
              <>
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
                <div className="flex items-center gap-3">
                  <div className="flex-1 h-px bg-slate-700/50" />
                  <span className="text-[10px] text-slate-500 uppercase tracking-wider">or use password</span>
                  <div className="flex-1 h-px bg-slate-700/50" />
                </div>
              </>
            )}

            {/* Password field */}
            <div>
              <input type="password" value={password} onChange={(e) => { setPassword(e.target.value); setError(''); }}
                placeholder="Enter your password" className={inputCls}
                onKeyDown={(e) => e.key === 'Enter' && handleUnlock()}
                autoFocus={!webauthnEnrolled}
              />
            </div>

            {error && (
              <div className="flex items-center gap-2 p-2.5 rounded-lg bg-red-500/10 border border-red-500/15">
                <AlertTriangle className="w-3.5 h-3.5 text-red-400 flex-shrink-0" />
                <p className="text-[11px] text-red-400">{error}</p>
              </div>
            )}

            <button onClick={handleUnlock} disabled={loading || !password}
              className={`w-full py-3 rounded-xl font-semibold text-sm disabled:opacity-40 transition-colors ${
                webauthnEnrolled ? 'bg-slate-700 text-slate-200 hover:bg-slate-600' : 'bg-dag-accent text-white hover:bg-dag-accent/80'
              }`}>
              {loading ? 'Unlocking...' : 'Unlock'}
            </button>
          </div>

          {/* Alternative actions — proper cards, not just links */}
          {!showResetConfirm && (
            <div className="space-y-2">
              <p className="text-[10px] text-slate-600 uppercase tracking-wider text-center">Can't unlock?</p>
              <div className="grid grid-cols-2 gap-2">
                <button onClick={() => goTo('restore')}
                  className="group p-3.5 rounded-xl border border-slate-700/50 bg-slate-800/20 hover:bg-slate-800/50 hover:border-slate-600 transition-all text-left">
                  <Download className="w-4 h-4 text-slate-500 group-hover:text-dag-accent mb-2 transition-colors" />
                  <p className="text-xs text-white font-medium">Restore Backup</p>
                  <p className="text-[10px] text-slate-500 mt-0.5">From a keystore file</p>
                </button>
                {onResetWallet && (
                  <button onClick={() => setShowResetConfirm(true)}
                    className="group p-3.5 rounded-xl border border-slate-700/50 bg-slate-800/20 hover:bg-slate-800/50 hover:border-slate-600 transition-all text-left">
                    <Plus className="w-4 h-4 text-slate-500 group-hover:text-dag-accent mb-2 transition-colors" />
                    <p className="text-xs text-white font-medium">Start Fresh</p>
                    <p className="text-[10px] text-slate-500 mt-0.5">Create or import wallet</p>
                  </button>
                )}
              </div>
            </div>
          )}

          {/* Reset confirmation */}
          {showResetConfirm && onResetWallet && (
            <div className="rounded-xl border border-red-500/25 bg-red-500/5 p-4 space-y-3">
              <div className="flex items-start gap-3">
                <div className="w-9 h-9 rounded-lg bg-red-500/15 flex items-center justify-center flex-shrink-0">
                  <AlertTriangle className="w-5 h-5 text-red-400" />
                </div>
                <div>
                  <p className="text-sm font-semibold text-red-400">Remove wallet from this browser?</p>
                  <p className="text-[11px] text-red-300/70 mt-1.5 leading-relaxed">
                    This will permanently delete the encrypted wallet from this device.
                    <span className="text-red-300 font-medium"> If you don't have your 12-word recovery phrase or a backup file, your funds will be lost forever.</span>
                  </p>
                </div>
              </div>
              <div className="flex gap-2 pt-1">
                <button onClick={() => setShowResetConfirm(false)}
                  className="flex-1 py-2.5 rounded-lg bg-slate-700 text-slate-200 text-xs font-medium hover:bg-slate-600 transition-colors">
                  Cancel
                </button>
                <button onClick={onResetWallet}
                  className="flex-1 py-2.5 rounded-lg bg-red-500/20 text-red-400 text-xs font-medium hover:bg-red-500/30 transition-colors flex items-center justify-center gap-1.5 border border-red-500/20">
                  <Trash2 className="w-3.5 h-3.5" />
                  Remove Wallet
                </button>
              </div>
            </div>
          )}
        </div>
      </div>
    );
  }

  // ===== STEP 1: NETWORK CHOICE =====
  if (step === 'network') {
    return (
      <div className="min-h-[calc(100vh-3.5rem)] flex items-center justify-center p-6">
        <div className="max-w-md w-full">
          <StepIndicator current={stepNum} total={totalSteps} labels={stepLabels} />
          <div className="space-y-5">
            <div className="flex items-center gap-3">
              <button onClick={() => goTo('landing')} className="p-2 rounded-lg text-slate-400 hover:text-white hover:bg-slate-800 transition-colors">
                <ArrowRight className="w-4 h-4 rotate-180" />
              </button>
              <div>
                <h1 className="text-xl font-bold text-white">Choose Your Network</h1>
                <p className="text-xs text-dag-muted">This determines which blockchain your wallet connects to</p>
              </div>
            </div>

            <div className="space-y-3">
              <button
                onClick={() => onSwitchNetwork('testnet')}
                className={`w-full text-left p-5 rounded-xl border-2 transition-all ${
                  network === 'testnet'
                    ? 'border-dag-yellow bg-dag-yellow/5 shadow-lg shadow-dag-yellow/5'
                    : 'border-slate-700 bg-slate-800/30 hover:border-slate-600'
                }`}
              >
                <div className="flex items-start gap-4">
                  <div className={`w-11 h-11 rounded-xl flex items-center justify-center flex-shrink-0 ${
                    network === 'testnet' ? 'bg-dag-yellow/15' : 'bg-slate-700/50'
                  }`}>
                    <TestTube className={`w-5 h-5 ${network === 'testnet' ? 'text-dag-yellow' : 'text-slate-400'}`} />
                  </div>
                  <div className="flex-1">
                    <div className="flex items-center gap-2">
                      <p className="font-semibold text-white">Testnet</p>
                      <span className="text-[9px] px-1.5 py-0.5 rounded bg-dag-yellow/15 text-dag-yellow font-medium">Recommended</span>
                    </div>
                    <p className="text-xs text-dag-muted mt-1">Free tokens for testing. No real money involved. Perfect for getting started and exploring.</p>
                  </div>
                  {network === 'testnet' && (
                    <div className="w-6 h-6 rounded-full bg-dag-yellow/20 flex items-center justify-center flex-shrink-0">
                      <Check className="w-3.5 h-3.5 text-dag-yellow" />
                    </div>
                  )}
                </div>
              </button>

              <button
                onClick={() => onSwitchNetwork('mainnet')}
                className={`w-full text-left p-5 rounded-xl border-2 transition-all ${
                  network === 'mainnet'
                    ? 'border-dag-green bg-dag-green/5 shadow-lg shadow-dag-green/5'
                    : 'border-slate-700 bg-slate-800/30 hover:border-slate-600'
                }`}
              >
                <div className="flex items-start gap-4">
                  <div className={`w-11 h-11 rounded-xl flex items-center justify-center flex-shrink-0 ${
                    network === 'mainnet' ? 'bg-dag-green/15' : 'bg-slate-700/50'
                  }`}>
                    <Rocket className={`w-5 h-5 ${network === 'mainnet' ? 'text-dag-green' : 'text-slate-400'}`} />
                  </div>
                  <div className="flex-1">
                    <p className="font-semibold text-white">Mainnet</p>
                    <p className="text-xs text-dag-muted mt-1">Real UDAG tokens with actual value. For real transactions and staking.</p>
                  </div>
                  {network === 'mainnet' && (
                    <div className="w-6 h-6 rounded-full bg-dag-green/20 flex items-center justify-center flex-shrink-0">
                      <Check className="w-3.5 h-3.5 text-dag-green" />
                    </div>
                  )}
                </div>
              </button>
            </div>

            <button
              onClick={async () => {
                if (isImportFlow) {
                  goTo('import');
                } else {
                  setLoading(true);
                  try {
                    const kp = await generateWithMnemonic();
                    setGeneratedKey(kp);
                    setDerivedAddress(kp.address);
                    goTo('backup');
                  } catch (err) { setError(String(err)); }
                  finally { setLoading(false); }
                }
              }}
              disabled={loading}
              className="w-full py-3.5 rounded-xl bg-dag-accent text-white font-semibold text-sm hover:bg-dag-accent/80 disabled:opacity-50 transition-all flex items-center justify-center gap-2"
            >
              {loading ? (
                <span className="flex items-center gap-2">
                  <span className="w-4 h-4 border-2 border-white/30 border-t-white rounded-full animate-spin" />
                  Generating...
                </span>
              ) : (<>Continue <ArrowRight className="w-4 h-4" /></>)}
            </button>
          </div>
        </div>
      </div>
    );
  }

  // ===== STEP 2: BACKUP (create flow) =====
  if (step === 'backup') {
    const words = generatedKey?.mnemonic?.split(' ') ?? [];
    return (
      <div className="min-h-[calc(100vh-3.5rem)] flex items-center justify-center p-6">
        <div className="max-w-md w-full">
          <StepIndicator current={stepNum} total={totalSteps} labels={stepLabels} />
          <div className="space-y-5">
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-3">
                <button onClick={() => goTo('network')} className="p-2 rounded-lg text-slate-400 hover:text-white hover:bg-slate-800 transition-colors">
                  <ArrowRight className="w-4 h-4 rotate-180" />
                </button>
                <div>
                  <h1 className="text-xl font-bold text-white">Your Recovery Phrase</h1>
                  <p className="text-xs text-dag-muted">Write these 12 words down in order</p>
                </div>
              </div>
              <NetworkBadge network={network} />
            </div>

            {words.length > 0 && (
              <div className="rounded-xl border border-dag-accent/20 bg-dag-accent/5 p-4 space-y-4">
                <div className={`grid grid-cols-3 gap-2 ${showKey ? '' : 'blur-sm'} transition-all duration-200 select-all`}>
                  {words.map((word, i) => (
                    <div key={i} className="flex items-center gap-2 bg-slate-800/60 rounded-lg px-3 py-2">
                      <span className="text-[10px] text-slate-500 font-mono w-4 text-right">{i + 1}</span>
                      <span className="text-sm text-white font-medium">{word}</span>
                    </div>
                  ))}
                </div>

                <div className="flex items-center gap-2">
                  <button onClick={() => setShowKey(!showKey)}
                    className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium bg-slate-700/50 text-slate-300 hover:bg-slate-700 hover:text-white transition-all">
                    {showKey ? <EyeOff className="w-3 h-3" /> : <Eye className="w-3 h-3" />}
                    {showKey ? 'Hide' : 'Reveal'}
                  </button>
                  <button onClick={() => copyText(generatedKey?.mnemonic ?? '', 'mnemonic')}
                    className={`flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium transition-all ${
                      copied === 'mnemonic' ? 'bg-dag-green/15 text-dag-green' : 'bg-slate-700/50 text-slate-300 hover:bg-slate-700 hover:text-white'
                    }`}>
                    {copied === 'mnemonic' ? <Check className="w-3 h-3" /> : <Copy className="w-3 h-3" />}
                    {copied === 'mnemonic' ? 'Copied!' : 'Copy'}
                  </button>
                </div>

                <div className="pt-2 border-t border-slate-700/50">
                  <p className="text-[10px] text-dag-muted uppercase tracking-wider mb-1.5">Wallet Address</p>
                  <div className="flex items-center gap-2 bg-slate-800/60 rounded-lg p-2.5">
                    <Wallet className="w-3.5 h-3.5 text-dag-green flex-shrink-0" />
                    <p className="text-xs font-mono text-dag-green break-all flex-1">{generatedKey?.address}</p>
                  </div>
                </div>

                <div className="flex items-start gap-2.5 p-3 rounded-lg bg-amber-500/10 border border-amber-500/20">
                  <Shield className="w-4 h-4 text-amber-400 mt-0.5 flex-shrink-0" />
                  <div>
                    <p className="text-[11px] text-amber-300 font-semibold">Write this down and keep it safe</p>
                    <p className="text-[10px] text-amber-300/70 mt-0.5">Anyone with these words can access your funds. Never share them. We cannot recover them for you.</p>
                  </div>
                </div>

                <label className="flex items-center gap-3 cursor-pointer select-none p-2.5 rounded-lg hover:bg-slate-800/30 transition-colors -mx-1">
                  <input type="checkbox" checked={confirmedBackup} onChange={e => setConfirmedBackup(e.target.checked)}
                    className="w-4 h-4 rounded border-slate-600 bg-slate-800 text-dag-accent focus:ring-dag-accent/30 flex-shrink-0" />
                  <span className="text-sm text-slate-300">I have written down my recovery phrase</span>
                </label>
              </div>
            )}

            <button onClick={() => goTo('secure')} disabled={!confirmedBackup}
              className="w-full py-3.5 rounded-xl bg-dag-accent text-white font-semibold text-sm hover:bg-dag-accent/80 disabled:opacity-30 disabled:cursor-not-allowed transition-all flex items-center justify-center gap-2">
              Continue <ArrowRight className="w-4 h-4" />
            </button>
          </div>
        </div>
      </div>
    );
  }

  // ===== STEP 2: IMPORT (self-contained — key input + name + password + submit) =====
  if (step === 'import') {
    const mnemonicWords = importMnemonic.trim().split(/\s+/).filter(Boolean);
    const wordCount = mnemonicWords.length;
    const mnemonicComplete = wordCount === 12;
    const mnemonicValid = mnemonicComplete && isValidMnemonic(importMnemonic);
    const mnemonicPartial = wordCount > 0 && wordCount < 12;
    const importReady = !!derivedAddress && walletName.trim().length > 0 && password.length >= 8 && password === confirmPassword;

    return (
      <div className="min-h-[calc(100vh-3.5rem)] flex items-center justify-center p-6">
        <div className="max-w-md w-full">
          <StepIndicator current={stepNum} total={totalSteps} labels={stepLabels} />
          <div className="space-y-5">
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-3">
                <button onClick={() => goTo('network')} className="p-2 rounded-lg text-slate-400 hover:text-white hover:bg-slate-800 transition-colors">
                  <ArrowRight className="w-4 h-4 rotate-180" />
                </button>
                <div>
                  <h1 className="text-xl font-bold text-white">Import Your Wallet</h1>
                  <p className="text-xs text-dag-muted">Restore access with your recovery phrase or key</p>
                </div>
              </div>
              <NetworkBadge network={network} />
            </div>

            {/* Import method tabs */}
            <div className="flex bg-slate-800/60 rounded-xl p-1 border border-slate-700/50">
              <button onClick={() => { setImportMode('mnemonic'); setDerivedAddress(''); setError(''); }}
                className={`flex-1 py-2 rounded-lg text-xs font-medium transition-all flex items-center justify-center gap-1.5 ${
                  importMode === 'mnemonic' ? 'bg-dag-accent/15 text-dag-accent border border-dag-accent/20' : 'text-slate-400 hover:text-white'
                }`}>
                <Key className="w-3 h-3" />
                Recovery Phrase
              </button>
              <button onClick={() => { setImportMode('hex'); setDerivedAddress(''); setGeneratedKey(null); setError(''); }}
                className={`flex-1 py-2 rounded-lg text-xs font-medium transition-all flex items-center justify-center gap-1.5 ${
                  importMode === 'hex' ? 'bg-dag-accent/15 text-dag-accent border border-dag-accent/20' : 'text-slate-400 hover:text-white'
                }`}>
                <Lock className="w-3 h-3" />
                Private Key
              </button>
            </div>

            {/* ---- Mnemonic import ---- */}
            {importMode === 'mnemonic' && (
              <div className="rounded-xl border border-slate-700/50 bg-slate-800/30 p-4 space-y-3">
                <div className="flex items-center justify-between">
                  <p className="text-[10px] text-dag-muted uppercase tracking-wider">Enter your 12 words</p>
                  <span className={`text-[10px] font-medium px-2 py-0.5 rounded-full ${
                    mnemonicValid ? 'bg-dag-green/15 text-dag-green' :
                    mnemonicComplete ? 'bg-red-500/15 text-red-400' :
                    'bg-slate-700/50 text-slate-500'
                  }`}>{wordCount}/12</span>
                </div>
                <div className="grid grid-cols-3 gap-2">
                  {Array.from({ length: 12 }, (_, i) => {
                    const word = mnemonicWords[i] || '';
                    const hasWord = word.length > 0;
                    const isCurrent = i === wordCount || (i === 11 && wordCount === 12);
                    return (
                      <div key={i} className={`flex items-center gap-2 rounded-lg px-3 py-2 transition-all ${
                        hasWord && mnemonicValid ? 'bg-dag-green/8 border border-dag-green/20' :
                        hasWord && mnemonicComplete && !mnemonicValid ? 'bg-red-500/8 border border-red-500/20' :
                        hasWord ? 'bg-dag-accent/8 border border-dag-accent/20' :
                        isCurrent ? 'bg-slate-700/30 border border-dag-accent/30 border-dashed' :
                        'bg-slate-800/40 border border-slate-700/30'
                      }`}>
                        <span className="text-[10px] text-slate-500 font-mono w-4 text-right">{i + 1}</span>
                        <span className={`text-sm font-medium truncate ${hasWord ? 'text-white' : 'text-slate-600'}`}>
                          {word || '\u00B7\u00B7\u00B7'}
                        </span>
                      </div>
                    );
                  })}
                </div>
                <textarea value={importMnemonic} onChange={(e) => handleMnemonicChange(e.target.value)}
                  placeholder="Type or paste your 12-word recovery phrase..."
                  rows={2}
                  className="w-full px-3 py-2.5 bg-slate-900/60 border border-slate-700/50 rounded-lg text-sm text-slate-200 placeholder-slate-600 focus:outline-none focus:border-dag-accent/40 focus:ring-1 focus:ring-dag-accent/20 transition-all resize-none font-mono"
                  autoFocus
                />
                {mnemonicPartial && (
                  <p className="text-[10px] text-slate-500 flex items-center gap-1.5">
                    <span className="w-1.5 h-1.5 rounded-full bg-dag-accent/50 animate-pulse" />
                    {12 - wordCount} more {12 - wordCount === 1 ? 'word' : 'words'} needed
                  </p>
                )}
                {mnemonicComplete && !mnemonicValid && (
                  <div className="flex items-start gap-2 p-2.5 rounded-lg bg-red-500/10 border border-red-500/15">
                    <Shield className="w-3.5 h-3.5 text-red-400 mt-0.5 flex-shrink-0" />
                    <p className="text-[11px] text-red-400">Invalid recovery phrase. Check each word carefully — spelling and order matter.</p>
                  </div>
                )}
              </div>
            )}

            {/* ---- Hex key import ---- */}
            {importMode === 'hex' && (
              <div className="rounded-xl border border-slate-700/50 bg-slate-800/30 p-4 space-y-3">
                <p className="text-[10px] text-dag-muted uppercase tracking-wider">Private Key (64 hex characters)</p>
                <input type="password" value={importKeyHex} onChange={(e) => handleHexKeyChange(e.target.value)}
                  placeholder="Enter or paste your private key..."
                  className="w-full px-3 py-2.5 bg-slate-900/60 border border-slate-700/50 rounded-lg text-sm text-slate-200 placeholder-slate-600 focus:outline-none focus:border-dag-accent/40 focus:ring-1 focus:ring-dag-accent/20 transition-all font-mono"
                  autoFocus
                />
                {importKeyHex.length > 0 && importKeyHex.replace(/\s/g, '').length < 64 && /^[0-9a-fA-F\s]*$/.test(importKeyHex) && (
                  <p className="text-[10px] text-slate-500">{64 - importKeyHex.replace(/\s/g, '').length} characters remaining</p>
                )}
                {importKeyHex.length > 0 && !/^[0-9a-fA-F\s]*$/.test(importKeyHex) && (
                  <p className="text-[10px] text-red-400">Must contain only hex characters (0-9, a-f)</p>
                )}
              </div>
            )}

            {/* Wallet found */}
            {derivedAddress && (
              <div className="rounded-xl bg-dag-green/5 border border-dag-green/25 p-4">
                <div className="flex items-center gap-3">
                  <div className="w-10 h-10 rounded-lg bg-dag-green/15 flex items-center justify-center flex-shrink-0">
                    <Wallet className="w-5 h-5 text-dag-green" />
                  </div>
                  <div className="flex-1 min-w-0">
                    <p className="text-xs text-dag-green font-semibold flex items-center gap-1.5">
                      <Check className="w-3.5 h-3.5" /> Wallet Found
                    </p>
                    <p className="text-xs font-mono text-dag-green/80 break-all mt-0.5">{derivedAddress}</p>
                  </div>
                </div>
              </div>
            )}

            {/* Name + password — shown once wallet is found */}
            {derivedAddress && (
              <div className="rounded-xl border border-slate-700/50 bg-slate-800/30 p-4 space-y-4">
                <div className="flex items-start gap-2.5 pb-3 border-b border-slate-700/30">
                  <Lock className="w-4 h-4 text-slate-400 mt-0.5 flex-shrink-0" />
                  <div>
                    <p className="text-xs text-white font-medium">Protect this wallet on your device</p>
                    <p className="text-[10px] text-slate-500 mt-0.5">Set a name and password to encrypt your keys in this browser.</p>
                  </div>
                </div>
                <div className="space-y-3">
                  <input type="text" value={walletName} onChange={(e) => setWalletName(e.target.value)}
                    placeholder="Wallet name" className={inputCls} />
                  <input type="password" value={password} onChange={(e) => setPassword(e.target.value)}
                    placeholder="Password (min 8 characters)" className={inputCls} />
                  <input type="password" value={confirmPassword} onChange={(e) => setConfirmPassword(e.target.value)}
                    placeholder="Confirm password" className={inputCls}
                    onKeyDown={(e) => e.key === 'Enter' && importReady && handleImportSubmit()} />
                  {confirmPassword.length > 0 && password !== confirmPassword && (
                    <p className="text-[10px] text-red-400 ml-1">Passwords don't match</p>
                  )}
                  {confirmPassword.length > 0 && password === confirmPassword && password.length >= 8 && (
                    <p className="text-[10px] text-dag-green ml-1 flex items-center gap-1"><Check className="w-3 h-3" /> Ready</p>
                  )}
                </div>
              </div>
            )}

            {error && <p className="text-sm text-red-400 text-center">{error}</p>}

            <button onClick={handleImportSubmit} disabled={loading || !importReady}
              className="w-full py-3.5 rounded-xl bg-dag-accent text-white font-semibold text-sm hover:bg-dag-accent/80 disabled:opacity-30 disabled:cursor-not-allowed transition-all flex items-center justify-center gap-2">
              {loading ? (
                <span className="flex items-center gap-2">
                  <span className="w-4 h-4 border-2 border-white/30 border-t-white rounded-full animate-spin" />
                  Importing...
                </span>
              ) : (<>Import Wallet <ArrowRight className="w-4 h-4" /></>)}
            </button>

            <button onClick={() => goTo('restore')} className="w-full text-center text-xs text-slate-500 hover:text-slate-300 transition-colors py-1">
              Or restore from a keystore backup file
            </button>
          </div>
        </div>
      </div>
    );
  }

  // ===== STEP 3: SECURE =====
  if (step === 'secure') {
    return (
      <div className="min-h-[calc(100vh-3.5rem)] flex items-center justify-center p-6">
        <div className="max-w-md w-full">
          <StepIndicator current={stepNum} total={totalSteps} labels={stepLabels} />
          <div className="space-y-5">
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-3">
                <button onClick={() => goTo(isImportFlow ? 'import' : 'backup')} className="p-2 rounded-lg text-slate-400 hover:text-white hover:bg-slate-800 transition-colors">
                  <ArrowRight className="w-4 h-4 rotate-180" />
                </button>
                <div>
                  <h1 className="text-xl font-bold text-white">Secure Your Wallet</h1>
                  <p className="text-xs text-dag-muted">Choose a name and password to encrypt your keys</p>
                </div>
              </div>
              <NetworkBadge network={network} />
            </div>

            <div className="space-y-4">
              <div>
                <label className="text-[10px] text-dag-muted uppercase tracking-wider mb-1.5 block">Wallet Name</label>
                <input type="text" value={walletName} onChange={(e) => setWalletName(e.target.value)}
                  placeholder="e.g. My Wallet, Savings, Trading..." className={inputCls} autoFocus />
              </div>
              <div>
                <label className="text-[10px] text-dag-muted uppercase tracking-wider mb-1.5 block">Password</label>
                <input type="password" value={password} onChange={(e) => setPassword(e.target.value)}
                  placeholder="At least 8 characters" className={inputCls} />
                {password.length > 0 && password.length < 8 && (
                  <p className="text-[10px] text-amber-400 mt-1.5 ml-1">{8 - password.length} more characters needed</p>
                )}
                {password.length >= 8 && (
                  <p className="text-[10px] text-dag-green mt-1.5 ml-1 flex items-center gap-1"><Check className="w-3 h-3" /> Strong enough</p>
                )}
              </div>
              <div>
                <label className="text-[10px] text-dag-muted uppercase tracking-wider mb-1.5 block">Confirm Password</label>
                <input type="password" value={confirmPassword} onChange={(e) => setConfirmPassword(e.target.value)}
                  placeholder="Re-enter your password" className={inputCls}
                  onKeyDown={(e) => e.key === 'Enter' && handleSecureSubmit()} />
                {confirmPassword.length > 0 && password !== confirmPassword && (
                  <p className="text-[10px] text-red-400 mt-1.5 ml-1">Passwords don't match</p>
                )}
                {confirmPassword.length > 0 && password === confirmPassword && password.length >= 8 && (
                  <p className="text-[10px] text-dag-green mt-1.5 ml-1 flex items-center gap-1"><Check className="w-3 h-3" /> Passwords match</p>
                )}
              </div>
            </div>

            <div className="flex items-start gap-2.5 p-3 rounded-lg bg-slate-800/50 border border-slate-700/50">
              <Lock className="w-4 h-4 text-slate-400 mt-0.5 flex-shrink-0" />
              <p className="text-[10px] text-slate-400">Your wallet is encrypted and stored only in this browser. We never see your keys or password.</p>
            </div>

            {error && <p className="text-sm text-red-400 text-center">{error}</p>}

            <button onClick={handleSecureSubmit}
              disabled={loading || !walletName.trim() || password.length < 8 || password !== confirmPassword}
              className="w-full py-3.5 rounded-xl bg-dag-accent text-white font-semibold text-sm hover:bg-dag-accent/80 disabled:opacity-30 disabled:cursor-not-allowed transition-all flex items-center justify-center gap-2">
              {loading ? (
                <span className="flex items-center gap-2">
                  <span className="w-4 h-4 border-2 border-white/30 border-t-white rounded-full animate-spin" />
                  Creating Wallet...
                </span>
              ) : (<>Create Wallet <ArrowRight className="w-4 h-4" /></>)}
            </button>
          </div>
        </div>
      </div>
    );
  }

  // ===== STEP 4: BIOMETRICS =====
  if (step === 'biometrics') {
    return (
      <div className="min-h-[calc(100vh-3.5rem)] flex items-center justify-center p-6">
        <div className="max-w-md w-full">
          <StepIndicator current={stepNum} total={totalSteps} labels={stepLabels} />
          <div className="space-y-6 text-center">
            <div className="space-y-3">
              <div className={`w-20 h-20 rounded-2xl flex items-center justify-center mx-auto shadow-lg transition-all duration-500 ${
                biometricsDone
                  ? 'bg-gradient-to-br from-dag-green to-emerald-500 shadow-dag-green/20'
                  : 'bg-gradient-to-br from-dag-accent to-purple-500 shadow-dag-accent/20'
              }`}>
                {biometricsDone ? <Check className="w-10 h-10 text-white" /> : <Fingerprint className="w-10 h-10 text-white" />}
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
                <button onClick={handleBiometricEnroll} disabled={loading}
                  className="w-full py-3.5 rounded-xl bg-gradient-to-r from-dag-accent to-purple-500 text-white font-semibold text-sm hover:opacity-90 disabled:opacity-50 transition-all flex items-center justify-center gap-2">
                  {loading ? (
                    <span className="flex items-center gap-2">
                      <span className="w-4 h-4 border-2 border-white/30 border-t-white rounded-full animate-spin" />
                      Waiting for biometrics...
                    </span>
                  ) : (<><Fingerprint className="w-5 h-5" /> Enable Biometrics</>)}
                </button>
                <button onClick={() => goTo('success')} className="w-full text-center text-sm text-slate-500 hover:text-slate-300 transition-colors py-1">
                  Skip for now
                </button>
              </>
            )}
          </div>
        </div>
      </div>
    );
  }

  // ===== STEP 5: SUCCESS =====
  if (step === 'success') {
    return (
      <div className="min-h-[calc(100vh-3.5rem)] flex items-center justify-center p-6">
        <div className="max-w-md w-full">
          <StepIndicator current={stepNum} total={totalSteps} labels={stepLabels} />
          <div className="space-y-6 text-center">
            <div className="space-y-3">
              <div className="w-20 h-20 rounded-2xl bg-gradient-to-br from-dag-green to-emerald-500 flex items-center justify-center mx-auto shadow-lg shadow-dag-green/20">
                <Sparkles className="w-10 h-10 text-white" />
              </div>
              <h1 className="text-2xl font-bold text-white">You're All Set!</h1>
              <p className="text-dag-muted text-sm max-w-xs mx-auto">
                Your <NetworkBadge network={network} /> wallet is ready.
              </p>
            </div>

            {onExportBlob && (
              <button onClick={handleDownloadKeystore}
                className={`w-full flex items-center gap-3 p-4 rounded-xl border transition-all text-left ${
                  keystoreDownloaded
                    ? 'border-dag-green/30 bg-dag-green/5'
                    : 'border-dag-accent/30 bg-dag-accent/5 hover:border-dag-accent/50 hover:bg-dag-accent/10'
                }`}>
                <div className={`w-10 h-10 rounded-lg flex items-center justify-center flex-shrink-0 ${
                  keystoreDownloaded ? 'bg-dag-green/15' : 'bg-dag-accent/15'
                }`}>
                  {keystoreDownloaded ? <Check className="w-5 h-5 text-dag-green" /> : <Download className="w-5 h-5 text-dag-accent" />}
                </div>
                <div className="flex-1 min-w-0">
                  <p className="text-sm font-medium text-white">
                    {keystoreDownloaded ? 'Keystore Downloaded' : 'Download Keystore Backup'}
                  </p>
                  <p className="text-[10px] text-dag-muted mt-0.5">
                    {keystoreDownloaded
                      ? `Saved as ultradag-${network}-keystore.json`
                      : 'Encrypted backup file — restore your wallet on any device'}
                  </p>
                </div>
              </button>
            )}

            <div className="space-y-2.5 text-left">
              {network === 'testnet' && (
                <div className="flex items-center gap-3 p-3.5 rounded-xl bg-slate-800/50 border border-slate-700/50">
                  <div className="w-9 h-9 rounded-lg bg-dag-yellow/15 flex items-center justify-center flex-shrink-0">
                    <ArrowDown className="w-4 h-4 text-dag-yellow" />
                  </div>
                  <div className="flex-1 min-w-0">
                    <p className="text-sm text-white font-medium">Get free testnet UDAG</p>
                    <p className="text-[10px] text-slate-500 mt-0.5">Use the faucet on the Dashboard to receive test tokens</p>
                  </div>
                </div>
              )}
              <div className="flex items-center gap-3 p-3.5 rounded-xl bg-slate-800/50 border border-slate-700/50">
                <div className="w-9 h-9 rounded-lg bg-dag-accent/15 flex items-center justify-center flex-shrink-0">
                  <Zap className="w-4 h-4 text-dag-accent" />
                </div>
                <div className="flex-1 min-w-0">
                  <p className="text-sm text-white font-medium">Send a payment</p>
                  <p className="text-[10px] text-slate-500 mt-0.5">Transfer UDAG to any address in seconds</p>
                </div>
              </div>
              <div className="flex items-center gap-3 p-3.5 rounded-xl bg-slate-800/50 border border-slate-700/50">
                <div className="w-9 h-9 rounded-lg bg-purple-500/15 flex items-center justify-center flex-shrink-0">
                  <Globe className="w-4 h-4 text-purple-400" />
                </div>
                <div className="flex-1 min-w-0">
                  <p className="text-sm text-white font-medium">Explore the network</p>
                  <p className="text-[10px] text-slate-500 mt-0.5">View live rounds, transactions, and validators</p>
                </div>
              </div>
            </div>

            <button onClick={onFinishOnboarding}
              className="w-full py-3.5 rounded-xl bg-dag-accent text-white font-semibold text-sm hover:bg-dag-accent/80 transition-all flex items-center justify-center gap-2">
              Go to Dashboard <ArrowRight className="w-4 h-4" />
            </button>
          </div>
        </div>
      </div>
    );
  }

  // ===== RESTORE =====
  const restoreBack = () => goTo(hasExisting ? 'unlock' : isImportFlow ? 'import' : 'landing');
  const jsonLooksValid = (() => {
    try { const p = JSON.parse(importJson); return p && p.version && p.ciphertext; } catch { return false; }
  })();

  return (
    <div className="min-h-[calc(100vh-3.5rem)] flex items-center justify-center p-6">
      <div className="max-w-md w-full space-y-5">
        <div className="flex items-center gap-3">
          <button onClick={restoreBack} className="p-2 rounded-lg text-slate-400 hover:text-white hover:bg-slate-800 transition-colors">
            <ArrowRight className="w-4 h-4 rotate-180" />
          </button>
          <div>
            <h1 className="text-xl font-bold text-white">Restore from Backup File</h1>
            <p className="text-xs text-dag-muted">Use a previously downloaded keystore file</p>
          </div>
        </div>

        <div className="rounded-xl border border-slate-700/50 bg-slate-800/30 p-4 space-y-4">
          <div className="flex items-center gap-3 pb-3 border-b border-slate-700/30">
            <div className="w-10 h-10 rounded-lg bg-dag-accent/10 flex items-center justify-center flex-shrink-0">
              <Download className="w-5 h-5 text-dag-accent" />
            </div>
            <div>
              <p className="text-sm text-white font-medium">Keystore JSON</p>
              <p className="text-[10px] text-slate-500">Paste the contents of your <span className="font-mono text-slate-400">ultradag-*-keystore.json</span> file</p>
            </div>
          </div>

          <textarea value={importJson} onChange={(e) => { setImportJson(e.target.value); setError(''); }}
            placeholder='{"version":1,"kdf":"pbkdf2-sha256",...}'
            rows={5}
            className="w-full px-3 py-2.5 bg-slate-900/60 border border-slate-700/50 rounded-lg text-sm text-slate-200 placeholder-slate-600 focus:outline-none focus:border-dag-accent/40 focus:ring-1 focus:ring-dag-accent/20 transition-all resize-none font-mono text-xs"
            autoFocus
          />

          {importJson.trim().length > 0 && (
            <div className={`flex items-center gap-2 text-[10px] ${jsonLooksValid ? 'text-dag-green' : 'text-slate-500'}`}>
              {jsonLooksValid ? <Check className="w-3 h-3" /> : <span className="w-3 h-3" />}
              {jsonLooksValid ? 'Valid keystore format detected' : 'Paste the full JSON contents of your backup file'}
            </div>
          )}
        </div>

        <div className="flex items-start gap-2.5 p-3 rounded-lg bg-slate-800/40 border border-slate-700/30">
          <Shield className="w-3.5 h-3.5 text-slate-500 mt-0.5 flex-shrink-0" />
          <p className="text-[10px] text-slate-500">Your backup is encrypted. After restoring, you'll need to enter the password you used when the wallet was created.</p>
        </div>

        {error && <p className="text-sm text-red-400 text-center">{error}</p>}

        <button onClick={handleRestore} disabled={loading || !jsonLooksValid}
          className="w-full py-3.5 rounded-xl bg-dag-accent text-white font-semibold text-sm hover:bg-dag-accent/80 disabled:opacity-30 disabled:cursor-not-allowed transition-all flex items-center justify-center gap-2">
          Restore Wallet
        </button>
      </div>
    </div>
  );
}
