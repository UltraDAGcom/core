import { useState, useEffect, useCallback, memo } from 'react';
import { Plus, Key, ChevronRight, Shield, Zap, Globe, ArrowRight, Eye, EyeOff, Copy, Check, Fingerprint, Lock, Sparkles, Wallet, ArrowDown, Download, TestTube, Rocket, AlertTriangle, Trash2, RefreshCw, Timer } from 'lucide-react';
import { deriveAddress } from '../../lib/keygen';
import { generateWithMnemonic, mnemonicToKeypair, isValidMnemonic } from '../../lib/mnemonic';
import { PasskeyOnboarding } from './PasskeyOnboarding';
import type { NetworkType } from '../../lib/api';

// ─── Props & Types ───────────────────────────────────────────────────────────

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

// ─── Shared Components ───────────────────────────────────────────────────────

const StepIndicator = memo(function StepIndicator({ current, total, labels }: { current: number; total: number; labels: string[] }) {
  return (
    <>
      {/* Desktop: numbered circles + labels */}
      <div className="hidden sm:flex items-center justify-center gap-1 mb-8">
        {Array.from({ length: total }, (_, i) => {
          const s = i + 1;
          const isActive = s === current;
          const isDone = s < current;
          return (
            <div key={i} className="flex items-center gap-1">
              <div className="flex flex-col items-center">
                <div className={`w-8 h-8 rounded-full flex items-center justify-center text-xs font-semibold transition-all duration-300 ${
                  isActive ? 'bg-dag-accent text-white shadow-lg shadow-dag-accent/30 scale-110' :
                  isDone ? 'bg-dag-green/20 text-dag-green' : 'bg-slate-800 text-slate-500'
                }`}>
                  {isDone ? <Check className="w-3.5 h-3.5" /> : s}
                </div>
                <span className={`text-[9px] mt-1 whitespace-nowrap transition-colors ${
                  isActive ? 'text-dag-accent' : isDone ? 'text-dag-green' : 'text-slate-600'
                }`}>{labels[i]}</span>
              </div>
              {i < total - 1 && <div className={`w-6 h-px mb-4 ${isDone ? 'bg-dag-green/40' : 'bg-slate-800'}`} />}
            </div>
          );
        })}
      </div>
      {/* Mobile: compact bar */}
      <div className="flex sm:hidden items-center justify-center gap-1.5 mb-6">
        {Array.from({ length: total }, (_, i) => (
          <div key={i} className={`h-1 rounded-full transition-all duration-300 ${
            i + 1 <= current ? 'bg-dag-accent w-7' : 'bg-slate-700 w-3'
          }`} />
        ))}
      </div>
    </>
  );
});

function NetworkBadge({ network }: { network: NetworkType }) {
  return network === 'testnet' ? (
    <span className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full text-[10px] font-semibold bg-dag-yellow/15 text-dag-yellow border border-dag-yellow/20">
      <TestTube className="w-3 h-3" /> Testnet
    </span>
  ) : (
    <span className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full text-[10px] font-semibold bg-dag-green/15 text-dag-green border border-dag-green/20">
      <Rocket className="w-3 h-3" /> Mainnet
    </span>
  );
}

function PasswordStrength({ password }: { password: string }) {
  if (!password) return null;
  const len = password.length;
  const hasUpper = /[A-Z]/.test(password);
  const hasLower = /[a-z]/.test(password);
  const hasNum = /[0-9]/.test(password);
  const hasSymbol = /[^a-zA-Z0-9]/.test(password);
  const variety = [hasUpper, hasLower, hasNum, hasSymbol].filter(Boolean).length;

  let strength = 0;
  if (len >= 8) strength++;
  if (len >= 12) strength++;
  if (variety >= 3) strength++;
  if (variety >= 4 && len >= 10) strength++;

  const label = strength <= 1 ? 'Weak' : strength === 2 ? 'Fair' : strength === 3 ? 'Good' : 'Strong';
  const color = strength <= 1 ? 'bg-red-400' : strength === 2 ? 'bg-amber-400' : strength === 3 ? 'bg-dag-green' : 'bg-dag-green';
  const textColor = strength <= 1 ? 'text-red-400' : strength === 2 ? 'text-amber-400' : 'text-dag-green';

  return (
    <div className="mt-2 space-y-1">
      <div className="flex gap-1">
        {[0, 1, 2, 3].map(i => (
          <div key={i} className={`h-1 flex-1 rounded-full transition-all ${i < strength ? color : 'bg-slate-700'}`} />
        ))}
      </div>
      <p className={`text-[10px] ${textColor}`}>{label}{len < 8 && ` — ${8 - len} more characters needed`}</p>
    </div>
  );
}

// ─── Step config per flow ────────────────────────────────────────────────────

const CREATE_LABELS = ['Network', 'Backup', 'Secure', 'Biometrics', 'Done'];
const IMPORT_LABELS = ['Network', 'Import', 'Biometrics', 'Done'];

// ─── Main Component ──────────────────────────────────────────────────────────

export function WelcomeScreen({
  onCreateWallet, onImportBlob, onUnlock, onUnlockWithWebAuthn, onEnrollWebAuthn,
  onExportBlob, onResetWallet, webauthnAvailable, webauthnEnrolled, hasExisting,
  onFinishOnboarding, isPostCreate, network, onSwitchNetwork,
}: WelcomeScreenProps) {
  const initialStep: Step = isPostCreate
    ? (webauthnAvailable ? 'biometrics' : 'success')
    : (hasExisting ? 'unlock' : 'landing');

  const [step, setStep] = useState<Step>(initialStep);
  const [showAdvancedCreate, setShowAdvancedCreate] = useState(false);

  // Form state
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

  // Mnemonic verification state
  const [verifyIndices, setVerifyIndices] = useState<number[]>([]);
  const [verifyAnswers, setVerifyAnswers] = useState<string[]>(['', '', '']);
  const [verifyError, setVerifyError] = useState('');
  const [verifyPassed, setVerifyPassed] = useState(false);

  // Unlock rate limiting
  const [failedAttempts, setFailedAttempts] = useState(0);
  const [lockedUntil, setLockedUntil] = useState(0);
  const [lockCountdown, setLockCountdown] = useState(0);

  // When keystore is destroyed, return to landing
  useEffect(() => {
    if (!hasExisting && !isPostCreate && step === 'unlock') {
      setStep('landing');
      setShowResetConfirm(false);
      setError('');
      setPassword('');
    }
  }, [hasExisting]);

  // Auto-blur recovery phrase after 30 seconds
  useEffect(() => {
    if (showKey && step === 'backup') {
      const t = setTimeout(() => setShowKey(false), 30000);
      return () => clearTimeout(t);
    }
  }, [showKey, step]);

  // Clear biometricsDone when entering biometrics step
  useEffect(() => {
    if (step === 'biometrics') setBiometricsDone(false);
  }, [step]);

  // Lock countdown timer
  useEffect(() => {
    if (lockedUntil <= Date.now()) { setLockCountdown(0); return; }
    const interval = setInterval(() => {
      const remaining = Math.ceil((lockedUntil - Date.now()) / 1000);
      if (remaining <= 0) { setLockCountdown(0); clearInterval(interval); }
      else setLockCountdown(remaining);
    }, 1000);
    return () => clearInterval(interval);
  }, [lockedUntil]);

  // Pick 3 random indices for verification when mnemonic is generated
  useEffect(() => {
    if (generatedKey?.mnemonic && !isImportFlow) {
      const indices: number[] = [];
      while (indices.length < 3) {
        const idx = Math.floor(Math.random() * 12);
        if (!indices.includes(idx)) indices.push(idx);
      }
      setVerifyIndices(indices.sort((a, b) => a - b));
      setVerifyAnswers(['', '', '']);
      setVerifyPassed(false);
      setVerifyError('');
    }
  }, [generatedKey?.mnemonic, isImportFlow]);

  const goTo = (s: Step) => { setError(''); setStep(s); };

  const resetFormState = useCallback(() => {
    setPassword(''); setConfirmPassword(''); setWalletName('');
    setImportKeyHex(''); setImportMnemonic(''); setDerivedAddress('');
    setGeneratedKey(null); setImportJson(''); setError('');
    setConfirmedBackup(false); setShowKey(false); setCopied(null);
    setVerifyAnswers(['', '', '']); setVerifyPassed(false); setVerifyError('');
    setBiometricsDone(false); setKeystoreDownloaded(false);
  }, []);

  const startFlow = useCallback((importFlow: boolean) => {
    resetFormState();
    setIsImportFlow(importFlow);
    goTo('network');
  }, [resetFormState]);

  const copyText = (text: string, label: string) => {
    navigator.clipboard.writeText(text);
    setCopied(label);
    setTimeout(() => setCopied(null), 2500);
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

  // Verify mnemonic words
  const handleVerify = () => {
    const words = generatedKey?.mnemonic?.split(' ') ?? [];
    const correct = verifyIndices.every((idx, i) =>
      verifyAnswers[i].trim().toLowerCase() === words[idx]
    );
    if (correct) {
      setVerifyPassed(true);
      setVerifyError('');
    } else {
      setVerifyError('One or more words are incorrect. Check your recovery phrase and try again.');
    }
  };

  // Create flow submit
  const handleSecureSubmit = async () => {
    setError('');
    if (!walletName.trim()) { setError('Please enter a wallet name'); return; }
    if (password.length < 8) { setError('Password must be at least 8 characters'); return; }
    if (password !== confirmPassword) { setError('Passwords do not match'); return; }
    if (!generatedKey) { setError('No key generated'); return; }
    setLoading(true);
    try {
      await onCreateWallet(password, walletName.trim(), generatedKey.secret_key, generatedKey.address);
      goTo(webauthnAvailable ? 'biometrics' : 'success');
    } catch (err) { setError(String(err)); } finally { setLoading(false); }
  };

  // Import flow submit
  const handleImportSubmit = async () => {
    setError('');
    if (!walletName.trim()) { setError('Please enter a wallet name'); return; }
    if (password.length < 8) { setError('Password must be at least 8 characters'); return; }
    if (password !== confirmPassword) { setError('Passwords do not match'); return; }
    let key: string, addr: string;
    if (importMode === 'mnemonic' && generatedKey) {
      key = generatedKey.secret_key; addr = generatedKey.address;
    } else if (importMode === 'hex') {
      key = importKeyHex.replace(/\s/g, '').toLowerCase(); addr = derivedAddress;
    } else { setError('No valid key entered'); return; }
    if (!/^[0-9a-f]{64}$/.test(key) || !addr) { setError('Invalid key'); return; }
    setLoading(true);
    try {
      await onCreateWallet(password, walletName.trim(), key, addr);
      goTo(webauthnAvailable ? 'biometrics' : 'success');
    } catch (err) { setError(String(err)); } finally { setLoading(false); }
  };

  // Unlock with rate limiting
  const handleUnlock = async () => {
    if (lockedUntil > Date.now()) return;
    setError(''); setLoading(true);
    try {
      const ok = await onUnlock(password);
      if (!ok) {
        const attempts = failedAttempts + 1;
        setFailedAttempts(attempts);
        if (attempts >= 5) {
          const lockMs = Math.min(30000 * Math.pow(2, attempts - 5), 300000); // 30s, 60s, 120s... max 5min
          setLockedUntil(Date.now() + lockMs);
          setError(`Too many failed attempts. Locked for ${Math.ceil(lockMs / 1000)}s.`);
        } else {
          setError(`Incorrect password (${5 - attempts} attempts remaining)`);
        }
      } else {
        setFailedAttempts(0);
        setLockedUntil(0);
      }
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
        setError('cancelled');
      }
    } catch {
      setError('failed');
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

  // Dynamic step config
  const totalSteps = isImportFlow ? IMPORT_LABELS.length : CREATE_LABELS.length;
  const stepLabels = isImportFlow ? IMPORT_LABELS : CREATE_LABELS;
  const stepNum = (() => {
    if (step === 'network') return 1;
    if (step === 'backup' || step === 'import') return 2;
    if (step === 'secure') return 3; // create only
    if (step === 'biometrics') return isImportFlow ? 3 : 4;
    if (step === 'success') return isImportFlow ? 4 : 5;
    return 0;
  })();

  const inputCls = "w-full px-4 py-3 bg-slate-800/80 border border-slate-700 rounded-xl text-sm text-slate-200 placeholder-slate-500 focus:outline-none focus:border-dag-accent focus:ring-1 focus:ring-dag-accent/30 transition-all";

  // ═══════════════════════════════════════════════════════════════════════════
  // LANDING
  // ═══════════════════════════════════════════════════════════════════════════
  if (step === 'landing') {
    // Primary flow: Passkey-first onboarding (fingerprint → username → done)
    if (!showAdvancedCreate) {
      return (
        <div className="min-h-[calc(100vh-3.5rem)] flex items-center justify-center p-6">
          <PasskeyOnboarding
            onComplete={(_address, _name) => {
              // PasskeyOnboarding already saved to passkey-wallet.ts via savePasskeyWallet().
              // Just close the onboarding overlay — App.tsx will detect pk.unlocked and show dashboard.
              onFinishOnboarding?.();
            }}
            onFallbackToAdvanced={() => setShowAdvancedCreate(true)}
          />
        </div>
      );
    }

    // Advanced flow: traditional Ed25519 seed phrase / import
    return (
      <div className="min-h-[calc(100vh-3.5rem)] flex items-center justify-center p-6">
        <div className="max-w-lg w-full space-y-8">
          <div className="text-center space-y-3">
            <div className="w-16 h-16 rounded-2xl bg-gradient-to-br from-dag-accent to-purple-500 flex items-center justify-center mx-auto shadow-lg shadow-dag-accent/20">
              <Key className="w-8 h-8 text-white" />
            </div>
            <h1 className="text-3xl font-bold text-white">Advanced Setup</h1>
            <p className="text-dag-muted text-sm max-w-sm mx-auto">
              Create or import a wallet using a traditional private key.
            </p>
          </div>

          <div className="space-y-3">
            <button onClick={() => startFlow(false)}
              className="w-full group rounded-xl border border-dag-accent/30 bg-dag-accent/5 hover:bg-dag-accent/10 p-5 text-left transition-all hover:border-dag-accent/50">
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

            <button onClick={() => startFlow(true)}
              className="w-full group rounded-xl border border-slate-700 bg-slate-800/30 hover:bg-slate-800/60 p-5 text-left transition-all hover:border-slate-600">
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

          <button onClick={() => setShowAdvancedCreate(false)}
            className="w-full text-center text-sm text-dag-accent hover:text-dag-accent/80 transition-colors py-2">
            <Fingerprint className="w-4 h-4 inline mr-1" />
            Back to passkey setup
          </button>

          <button onClick={() => goTo('restore')} className="w-full text-center text-xs text-slate-500 hover:text-slate-300 transition-colors py-2">
            Restore from backup file
          </button>
        </div>
      </div>
    );
  }

  // ═══════════════════════════════════════════════════════════════════════════
  // UNLOCK
  // ═══════════════════════════════════════════════════════════════════════════
  if (step === 'unlock') {
    const isLocked = lockCountdown > 0;
    const isPasskeyWallet = !!localStorage.getItem('ultradag_passkey');

    // Passkey wallet: show biometric unlock instead of password
    if (isPasskeyWallet) {
      return (
        <div className="min-h-[calc(100vh-3.5rem)] flex items-center justify-center p-6">
          <div className="max-w-md w-full space-y-6 text-center">
            <div className="w-20 h-20 rounded-2xl bg-gradient-to-br from-dag-accent to-purple-500 flex items-center justify-center mx-auto shadow-lg shadow-dag-accent/20">
              <Fingerprint className="w-10 h-10 text-white" />
            </div>
            <h1 className="text-2xl font-bold text-white">Welcome Back</h1>
            <p className="text-dag-muted text-sm">Verify your identity to unlock</p>

            {error && (
              <div className="bg-red-500/10 border border-red-500/30 rounded-lg p-3 text-red-400 text-sm">
                {error}
              </div>
            )}

            <button
              onClick={async () => {
                setError(''); setLoading(true);
                try {
                  const passkeyRaw = localStorage.getItem('ultradag_passkey');
                  const passkey = passkeyRaw ? JSON.parse(passkeyRaw) : null;
                  if (!passkey?.credentialId) throw new Error('No passkey found');

                  // Trigger biometric verification via WebAuthn
                  // Use discoverable credential (no allowCredentials) so the browser
                  // shows the passkey picker — works regardless of credential ID encoding
                  const challenge = crypto.getRandomValues(new Uint8Array(32));
                  const credential = await navigator.credentials.get({
                    publicKey: {
                      challenge,
                      rpId: window.location.hostname,
                      userVerification: 'required',
                      timeout: 60000,
                    },
                  });

                  if (!credential) {
                    setError('Biometric verification cancelled.');
                    return;
                  }

                  // Biometric verified — unlock the keystore with placeholder password
                  const ok = await onUnlock('passkey-wallet');
                  if (!ok) setError('Failed to unlock wallet.');
                } catch (e: unknown) {
                  const msg = e instanceof Error ? e.message : 'Verification failed';
                  setError(msg);
                } finally {
                  setLoading(false);
                }
              }}
              disabled={loading}
              className="w-full py-4 rounded-xl bg-gradient-to-r from-dag-accent to-purple-500 text-white font-semibold text-lg hover:opacity-90 disabled:opacity-50 transition-all flex items-center justify-center gap-2"
            >
              <Fingerprint className="w-6 h-6" />
              {loading ? 'Verifying...' : 'Unlock with Biometrics'}
            </button>

            <button onClick={() => { if (onResetWallet) onResetWallet(); }} className="text-xs text-slate-500 hover:text-slate-300 transition-colors">
              Start Fresh
            </button>
          </div>
        </div>
      );
    }

    return (
      <div className="min-h-[calc(100vh-3.5rem)] flex items-center justify-center p-6">
        <div className="max-w-md w-full space-y-6">
          <div className="text-center space-y-3">
            <div className="w-14 h-14 rounded-2xl bg-gradient-to-br from-dag-accent to-purple-500 flex items-center justify-center mx-auto shadow-lg shadow-dag-accent/20">
              <Zap className="w-7 h-7 text-white" />
            </div>
            <h1 className="text-2xl font-bold text-white">Welcome Back</h1>
            <p className="text-dag-muted text-sm">Unlock your wallet to continue</p>
          </div>

          {/* Primary unlock card */}
          <div className="rounded-xl border border-slate-700/50 bg-slate-800/30 p-5 space-y-4">
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

            <input type="password" value={password}
              onChange={(e) => { setPassword(e.target.value); setError(''); }}
              placeholder="Enter your password" className={inputCls}
              onKeyDown={(e) => e.key === 'Enter' && !isLocked && handleUnlock()}
              autoFocus={!webauthnEnrolled} disabled={isLocked}
            />

            {error && (
              <div className="flex items-center gap-2 p-2.5 rounded-lg bg-red-500/10 border border-red-500/15">
                <AlertTriangle className="w-3.5 h-3.5 text-red-400 flex-shrink-0" />
                <p className="text-[11px] text-red-400">{error}</p>
              </div>
            )}

            {isLocked && (
              <div className="flex items-center gap-2 p-2.5 rounded-lg bg-amber-500/10 border border-amber-500/15">
                <Timer className="w-3.5 h-3.5 text-amber-400 flex-shrink-0" />
                <p className="text-[11px] text-amber-400">Try again in {lockCountdown}s</p>
              </div>
            )}

            <button onClick={handleUnlock} disabled={loading || !password || isLocked}
              className={`w-full py-3 rounded-xl font-semibold text-sm disabled:opacity-40 transition-colors ${
                webauthnEnrolled ? 'bg-slate-700 text-slate-200 hover:bg-slate-600' : 'bg-dag-accent text-white hover:bg-dag-accent/80'
              }`}>
              {loading ? 'Unlocking...' : 'Unlock'}
            </button>
          </div>

          {/* Alternative actions */}
          {!showResetConfirm && (
            <div className="space-y-2">
              <p className="text-[10px] text-slate-600 uppercase tracking-wider text-center">Other options</p>
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
            <div className="rounded-xl border border-red-500/25 bg-red-500/5 p-4 space-y-3 animate-in fade-in">
              <div className="flex items-start gap-3">
                <div className="w-9 h-9 rounded-lg bg-red-500/15 flex items-center justify-center flex-shrink-0">
                  <AlertTriangle className="w-5 h-5 text-red-400" />
                </div>
                <div>
                  <p className="text-sm font-semibold text-red-400">Remove wallet from this browser?</p>
                  <p className="text-[11px] text-red-300/70 mt-1.5 leading-relaxed">
                    This will permanently delete the encrypted wallet.
                    <span className="text-red-300 font-medium"> Without your 12-word recovery phrase or a backup file, your funds will be lost forever.</span>
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
                  <Trash2 className="w-3.5 h-3.5" /> Remove Wallet
                </button>
              </div>
            </div>
          )}
        </div>
      </div>
    );
  }

  // ═══════════════════════════════════════════════════════════════════════════
  // STEP 1: NETWORK
  // ═══════════════════════════════════════════════════════════════════════════
  if (step === 'network') {
    return (
      <div className="min-h-[calc(100vh-3.5rem)] flex items-center justify-center p-6">
        <div className="max-w-md w-full">
          <StepIndicator current={stepNum} total={totalSteps} labels={stepLabels} />
          <div className="space-y-5">
            <div className="flex items-center gap-3">
              <button onClick={() => { resetFormState(); goTo('landing'); }} className="p-2 rounded-lg text-slate-400 hover:text-white hover:bg-slate-800 transition-colors">
                <ArrowRight className="w-4 h-4 rotate-180" />
              </button>
              <div>
                <h1 className="text-xl font-bold text-white">Choose Your Network</h1>
                <p className="text-xs text-dag-muted">This determines which blockchain your wallet connects to</p>
              </div>
            </div>

            <div className="space-y-3">
              {([
                { net: 'testnet' as const, icon: TestTube, label: 'Testnet', badge: 'Recommended',
                  desc: 'Free tokens for testing. No real money. Perfect for getting started.',
                  borderActive: 'border-dag-yellow bg-dag-yellow/5 shadow-lg shadow-dag-yellow/5',
                  iconActive: 'bg-dag-yellow/15', iconColor: 'text-dag-yellow',
                  checkBg: 'bg-dag-yellow/20', checkColor: 'text-dag-yellow', badgeCls: 'bg-dag-yellow/15 text-dag-yellow' },
                { net: 'mainnet' as const, icon: Rocket, label: 'Mainnet',
                  desc: 'Real UDAG tokens with actual value. For real transactions and staking.',
                  borderActive: 'border-dag-green bg-dag-green/5 shadow-lg shadow-dag-green/5',
                  iconActive: 'bg-dag-green/15', iconColor: 'text-dag-green',
                  checkBg: 'bg-dag-green/20', checkColor: 'text-dag-green' },
              ] as const).map(({ net, icon: Icon, label, badge, desc, borderActive, iconActive, iconColor, checkBg, checkColor, badgeCls }) => (
                <button key={net} onClick={() => onSwitchNetwork(net)}
                  className={`w-full text-left p-5 rounded-xl border-2 transition-all ${
                    network === net ? borderActive : 'border-slate-700 bg-slate-800/30 hover:border-slate-600'
                  }`}>
                  <div className="flex items-start gap-4">
                    <div className={`w-11 h-11 rounded-xl flex items-center justify-center flex-shrink-0 ${network === net ? iconActive : 'bg-slate-700/50'}`}>
                      <Icon className={`w-5 h-5 ${network === net ? iconColor : 'text-slate-400'}`} />
                    </div>
                    <div className="flex-1">
                      <div className="flex items-center gap-2">
                        <p className="font-semibold text-white">{label}</p>
                        {badge && <span className={`text-[9px] px-1.5 py-0.5 rounded font-medium ${badgeCls}`}>{badge}</span>}
                      </div>
                      <p className="text-xs text-dag-muted mt-1">{desc}</p>
                    </div>
                    {network === net && (
                      <div className={`w-6 h-6 rounded-full ${checkBg} flex items-center justify-center flex-shrink-0`}>
                        <Check className={`w-3.5 h-3.5 ${checkColor}`} />
                      </div>
                    )}
                  </div>
                </button>
              ))}
            </div>

            <button
              onClick={async () => {
                if (isImportFlow) { goTo('import'); return; }
                setLoading(true);
                try {
                  const kp = await generateWithMnemonic();
                  setGeneratedKey(kp);
                  setDerivedAddress(kp.address);
                  goTo('backup');
                } catch (err) { setError(String(err)); }
                finally { setLoading(false); }
              }}
              disabled={loading}
              className="w-full py-3.5 rounded-xl bg-dag-accent text-white font-semibold text-sm hover:bg-dag-accent/80 disabled:opacity-50 transition-all flex items-center justify-center gap-2">
              {loading ? (
                <><span className="w-4 h-4 border-2 border-white/30 border-t-white rounded-full animate-spin" /> Generating...</>
              ) : (<>Continue <ArrowRight className="w-4 h-4" /></>)}
            </button>
          </div>
        </div>
      </div>
    );
  }

  // ═══════════════════════════════════════════════════════════════════════════
  // STEP 2 (CREATE): BACKUP + VERIFY
  // ═══════════════════════════════════════════════════════════════════════════
  if (step === 'backup') {
    const words = generatedKey?.mnemonic?.split(' ') ?? [];
    const showVerify = confirmedBackup && !verifyPassed;
    const canContinue = confirmedBackup && verifyPassed;

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
                  <h1 className="text-xl font-bold text-white">{showVerify ? 'Verify Your Phrase' : 'Your Recovery Phrase'}</h1>
                  <p className="text-xs text-dag-muted">{showVerify ? 'Confirm you wrote it down correctly' : 'Write these 12 words down in order'}</p>
                </div>
              </div>
              <NetworkBadge network={network} />
            </div>

            {/* Phase 1: Show mnemonic */}
            {!showVerify && words.length > 0 && (
              <div className="rounded-xl border border-dag-accent/20 bg-dag-accent/5 p-4 space-y-4">
                <div className={`grid grid-cols-3 gap-2 transition-all duration-300 select-all ${showKey ? '' : 'blur-sm'}`}>
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
                    aria-label="Copy recovery phrase"
                    className={`flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium transition-all ${
                      copied === 'mnemonic' ? 'bg-dag-green/15 text-dag-green' : 'bg-slate-700/50 text-slate-300 hover:bg-slate-700 hover:text-white'
                    }`}>
                    {copied === 'mnemonic' ? <Check className="w-3 h-3" /> : <Copy className="w-3 h-3" />}
                    {copied === 'mnemonic' ? 'Copied!' : 'Copy'}
                  </button>
                </div>

                {copied === 'mnemonic' && (
                  <div className="flex items-start gap-2 p-2.5 rounded-lg bg-amber-500/10 border border-amber-500/15">
                    <Shield className="w-3.5 h-3.5 text-amber-400 mt-0.5 flex-shrink-0" />
                    <p className="text-[10px] text-amber-300/80">Never paste your recovery phrase into websites or messages. We will never ask for it.</p>
                  </div>
                )}

                <div className="pt-2 border-t border-slate-700/50">
                  <p className="text-[10px] text-dag-muted uppercase tracking-wider mb-1.5">Wallet Address</p>
                  <div className="flex items-center gap-2 bg-slate-800/60 rounded-lg p-2.5">
                    <Wallet className="w-3.5 h-3.5 text-dag-green flex-shrink-0" />
                    <p className="text-xs font-mono text-dag-green flex-1">
                      {generatedKey?.address ? `${generatedKey.address.slice(0, 8)}...${generatedKey.address.slice(-6)}` : ''}
                    </p>
                    {generatedKey?.address && (
                      <button onClick={() => copyText(generatedKey.address, 'address')}
                        className={`flex items-center gap-1 px-2 py-1 rounded text-[10px] font-medium transition-all flex-shrink-0 ${
                          copied === 'address' ? 'bg-dag-green/15 text-dag-green' : 'bg-slate-700/50 text-slate-400 hover:text-white'
                        }`}>
                        {copied === 'address' ? <Check className="w-3 h-3" /> : <Copy className="w-3 h-3" />}
                        {copied === 'address' ? 'Copied!' : 'Copy'}
                      </button>
                    )}
                  </div>
                </div>

                <div className="flex items-start gap-2.5 p-3 rounded-lg bg-amber-500/10 border border-amber-500/20">
                  <Shield className="w-4 h-4 text-amber-400 mt-0.5 flex-shrink-0" />
                  <div>
                    <p className="text-[11px] text-amber-300 font-semibold">Write this down and keep it safe</p>
                    <p className="text-[10px] text-amber-300/70 mt-0.5">Anyone with these words can access your funds. We cannot recover them for you.</p>
                  </div>
                </div>

                <label className="flex items-center gap-3 cursor-pointer select-none p-2.5 rounded-lg hover:bg-slate-800/30 transition-colors -mx-1">
                  <input type="checkbox" checked={confirmedBackup} onChange={e => setConfirmedBackup(e.target.checked)}
                    className="w-4 h-4 rounded border-slate-600 bg-slate-800 text-dag-accent focus:ring-dag-accent/30 flex-shrink-0" />
                  <span className="text-sm text-slate-300">I have written down my recovery phrase</span>
                </label>
              </div>
            )}

            {/* Phase 2: Verify 3 random words */}
            {showVerify && (
              <div className="rounded-xl border border-dag-accent/20 bg-dag-accent/5 p-4 space-y-4">
                <p className="text-xs text-slate-300">Enter the following words from your recovery phrase to verify you saved it correctly.</p>
                <div className="space-y-3">
                  {verifyIndices.map((wordIdx, i) => (
                    <div key={wordIdx}>
                      <label htmlFor={`verify-${i}`} className="text-[10px] text-dag-muted uppercase tracking-wider mb-1 block">
                        Word #{wordIdx + 1}
                      </label>
                      <input
                        id={`verify-${i}`}
                        type="text"
                        value={verifyAnswers[i]}
                        onChange={e => {
                          const next = [...verifyAnswers];
                          next[i] = e.target.value;
                          setVerifyAnswers(next);
                          setVerifyError('');
                        }}
                        placeholder={`Enter word #${wordIdx + 1}`}
                        className={inputCls + ' font-mono'}
                        autoFocus={i === 0}
                        onKeyDown={e => { if (e.key === 'Enter' && verifyAnswers.every(a => a.trim())) handleVerify(); }}
                      />
                    </div>
                  ))}
                </div>
                {verifyError && (
                  <div className="flex items-start gap-2 p-2.5 rounded-lg bg-red-500/10 border border-red-500/15">
                    <AlertTriangle className="w-3.5 h-3.5 text-red-400 mt-0.5 flex-shrink-0" />
                    <p className="text-[11px] text-red-400">{verifyError}</p>
                  </div>
                )}
                <button onClick={handleVerify}
                  disabled={!verifyAnswers.every(a => a.trim())}
                  className="w-full py-3 rounded-xl bg-dag-accent text-white font-semibold text-sm hover:bg-dag-accent/80 disabled:opacity-30 disabled:cursor-not-allowed transition-all">
                  Verify
                </button>
                <button onClick={() => { setConfirmedBackup(false); setVerifyAnswers(['', '', '']); setVerifyError(''); }}
                  className="w-full text-center text-xs text-slate-500 hover:text-slate-300 transition-colors py-1">
                  Show recovery phrase again
                </button>
              </div>
            )}

            {/* Verified success */}
            {canContinue && (
              <div className="flex items-center gap-3 p-3.5 rounded-xl bg-dag-green/5 border border-dag-green/20">
                <Check className="w-5 h-5 text-dag-green flex-shrink-0" />
                <p className="text-sm text-dag-green font-medium">Recovery phrase verified successfully</p>
              </div>
            )}

            <button onClick={() => goTo('secure')} disabled={!canContinue}
              className="w-full py-3.5 rounded-xl bg-dag-accent text-white font-semibold text-sm hover:bg-dag-accent/80 disabled:opacity-30 disabled:cursor-not-allowed transition-all flex items-center justify-center gap-2">
              Continue <ArrowRight className="w-4 h-4" />
            </button>
          </div>
        </div>
      </div>
    );
  }

  // ═══════════════════════════════════════════════════════════════════════════
  // STEP 2 (IMPORT): Key input + name + password — all in one
  // ═══════════════════════════════════════════════════════════════════════════
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

            {/* Tabs */}
            <div className="flex bg-slate-800/60 rounded-xl p-1 border border-slate-700/50">
              <button onClick={() => { setImportMode('mnemonic'); setDerivedAddress(''); setError(''); }}
                className={`flex-1 py-2 rounded-lg text-xs font-medium transition-all flex items-center justify-center gap-1.5 ${
                  importMode === 'mnemonic' ? 'bg-dag-accent/15 text-dag-accent border border-dag-accent/20' : 'text-slate-400 hover:text-white'
                }`}>
                <Key className="w-3 h-3" /> Recovery Phrase
              </button>
              <button onClick={() => { setImportMode('hex'); setDerivedAddress(''); setGeneratedKey(null); setError(''); }}
                className={`flex-1 py-2 rounded-lg text-xs font-medium transition-all flex items-center justify-center gap-1.5 ${
                  importMode === 'hex' ? 'bg-dag-accent/15 text-dag-accent border border-dag-accent/20' : 'text-slate-400 hover:text-white'
                }`}>
                <Lock className="w-3 h-3" /> Private Key
              </button>
            </div>

            {/* Mnemonic */}
            {importMode === 'mnemonic' && (
              <div className="rounded-xl border border-slate-700/50 bg-slate-800/30 p-4 space-y-3">
                <div className="flex items-center justify-between">
                  <p className="text-[10px] text-dag-muted uppercase tracking-wider">Enter your 12 words</p>
                  <span className={`text-[10px] font-medium px-2 py-0.5 rounded-full ${
                    mnemonicValid ? 'bg-dag-green/15 text-dag-green' :
                    mnemonicComplete ? 'bg-red-500/15 text-red-400' : 'bg-slate-700/50 text-slate-500'
                  }`}>{wordCount}/12</span>
                </div>
                <div className="grid grid-cols-3 gap-2">
                  {Array.from({ length: 12 }, (_, i) => {
                    const word = mnemonicWords[i] || '';
                    const hasWord = word.length > 0;
                    return (
                      <div key={i} className={`flex items-center gap-2 rounded-lg px-3 py-2 transition-all ${
                        hasWord && mnemonicValid ? 'bg-dag-green/8 border border-dag-green/20' :
                        hasWord && mnemonicComplete && !mnemonicValid ? 'bg-red-500/8 border border-red-500/20' :
                        hasWord ? 'bg-dag-accent/8 border border-dag-accent/20' :
                        i === wordCount ? 'bg-slate-700/30 border border-dag-accent/30 border-dashed' :
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
                  rows={2} className="w-full px-3 py-2.5 bg-slate-900/60 border border-slate-700/50 rounded-lg text-sm text-slate-200 placeholder-slate-600 focus:outline-none focus:border-dag-accent/40 focus:ring-1 focus:ring-dag-accent/20 transition-all resize-none font-mono"
                  autoFocus />
                {mnemonicPartial && (
                  <p className="text-[10px] text-slate-500 flex items-center gap-1.5">
                    <span className="w-1.5 h-1.5 rounded-full bg-dag-accent/50 animate-pulse" />
                    {12 - wordCount} more {12 - wordCount === 1 ? 'word' : 'words'} needed
                  </p>
                )}
                {mnemonicComplete && !mnemonicValid && (
                  <div className="flex items-start gap-2 p-2.5 rounded-lg bg-red-500/10 border border-red-500/15">
                    <Shield className="w-3.5 h-3.5 text-red-400 mt-0.5 flex-shrink-0" />
                    <p className="text-[11px] text-red-400">Invalid recovery phrase. Check each word carefully.</p>
                  </div>
                )}
              </div>
            )}

            {/* Hex */}
            {importMode === 'hex' && (
              <div className="rounded-xl border border-slate-700/50 bg-slate-800/30 p-4 space-y-3">
                <p className="text-[10px] text-dag-muted uppercase tracking-wider">Private Key (64 hex characters)</p>
                <input type="password" value={importKeyHex} onChange={(e) => handleHexKeyChange(e.target.value)}
                  placeholder="Enter or paste your private key..."
                  className="w-full px-3 py-2.5 bg-slate-900/60 border border-slate-700/50 rounded-lg text-sm text-slate-200 placeholder-slate-600 focus:outline-none focus:border-dag-accent/40 focus:ring-1 focus:ring-dag-accent/20 transition-all font-mono"
                  autoFocus />
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
                    <p className="text-xs text-dag-green font-semibold flex items-center gap-1.5"><Check className="w-3.5 h-3.5" /> Wallet Found</p>
                    <p className="text-xs font-mono text-dag-green/80 mt-0.5">{derivedAddress.slice(0, 8)}...{derivedAddress.slice(-6)}</p>
                  </div>
                </div>
              </div>
            )}

            {/* Protect wallet — shown once address is found */}
            {derivedAddress && (
              <div className="rounded-xl border border-slate-700/50 bg-slate-800/30 p-4 space-y-4">
                <div className="flex items-start gap-2.5 pb-3 border-b border-slate-700/30">
                  <Lock className="w-4 h-4 text-slate-400 mt-0.5 flex-shrink-0" />
                  <div>
                    <p className="text-xs text-white font-medium">Protect this wallet</p>
                    <p className="text-[10px] text-slate-500 mt-0.5">Set a name and password to encrypt your keys in this browser.</p>
                  </div>
                </div>
                <div className="space-y-3">
                  <div>
                    <label htmlFor="import-name" className="text-[10px] text-dag-muted uppercase tracking-wider mb-1 block">Wallet Name</label>
                    <input id="import-name" type="text" value={walletName} onChange={(e) => setWalletName(e.target.value)}
                      placeholder="e.g. My Wallet" className={inputCls} />
                  </div>
                  <div>
                    <label htmlFor="import-pw" className="text-[10px] text-dag-muted uppercase tracking-wider mb-1 block">Password</label>
                    <input id="import-pw" type="password" value={password} onChange={(e) => setPassword(e.target.value)}
                      placeholder="At least 8 characters" className={inputCls} />
                    <PasswordStrength password={password} />
                  </div>
                  <div>
                    <label htmlFor="import-pw2" className="text-[10px] text-dag-muted uppercase tracking-wider mb-1 block">Confirm Password</label>
                    <input id="import-pw2" type="password" value={confirmPassword} onChange={(e) => setConfirmPassword(e.target.value)}
                      placeholder="Re-enter password" className={inputCls}
                      onKeyDown={(e) => e.key === 'Enter' && importReady && handleImportSubmit()} />
                    {confirmPassword && password !== confirmPassword && <p className="text-[10px] text-red-400 mt-1.5 ml-1">Passwords don't match</p>}
                    {confirmPassword && password === confirmPassword && password.length >= 8 && <p className="text-[10px] text-dag-green mt-1.5 ml-1 flex items-center gap-1"><Check className="w-3 h-3" /> Ready</p>}
                  </div>
                </div>
              </div>
            )}

            {error && <p className="text-sm text-red-400 text-center">{error}</p>}

            <button onClick={handleImportSubmit} disabled={loading || !importReady}
              className="w-full py-3.5 rounded-xl bg-dag-accent text-white font-semibold text-sm hover:bg-dag-accent/80 disabled:opacity-30 disabled:cursor-not-allowed transition-all flex items-center justify-center gap-2">
              {loading ? (<><span className="w-4 h-4 border-2 border-white/30 border-t-white rounded-full animate-spin" /> Importing...</>) : (<>Import Wallet <ArrowRight className="w-4 h-4" /></>)}
            </button>

            <button onClick={() => goTo('restore')} className="w-full text-center text-xs text-slate-500 hover:text-slate-300 transition-colors py-1">
              Or restore from a keystore backup file
            </button>
          </div>
        </div>
      </div>
    );
  }

  // ═══════════════════════════════════════════════════════════════════════════
  // STEP 3 (CREATE ONLY): SECURE
  // ═══════════════════════════════════════════════════════════════════════════
  if (step === 'secure') {
    return (
      <div className="min-h-[calc(100vh-3.5rem)] flex items-center justify-center p-6">
        <div className="max-w-md w-full">
          <StepIndicator current={stepNum} total={totalSteps} labels={stepLabels} />
          <div className="space-y-5">
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-3">
                <button onClick={() => goTo('backup')} className="p-2 rounded-lg text-slate-400 hover:text-white hover:bg-slate-800 transition-colors">
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
                <label htmlFor="create-name" className="text-[10px] text-dag-muted uppercase tracking-wider mb-1.5 block">Wallet Name</label>
                <input id="create-name" type="text" value={walletName} onChange={(e) => setWalletName(e.target.value)}
                  placeholder="e.g. My Wallet, Savings, Trading..." className={inputCls} autoFocus />
              </div>
              <div>
                <label htmlFor="create-pw" className="text-[10px] text-dag-muted uppercase tracking-wider mb-1.5 block">Password</label>
                <input id="create-pw" type="password" value={password} onChange={(e) => setPassword(e.target.value)}
                  placeholder="At least 8 characters" className={inputCls} />
                <PasswordStrength password={password} />
              </div>
              <div>
                <label htmlFor="create-pw2" className="text-[10px] text-dag-muted uppercase tracking-wider mb-1.5 block">Confirm Password</label>
                <input id="create-pw2" type="password" value={confirmPassword} onChange={(e) => setConfirmPassword(e.target.value)}
                  placeholder="Re-enter your password" className={inputCls}
                  onKeyDown={(e) => e.key === 'Enter' && handleSecureSubmit()} />
                {confirmPassword && password !== confirmPassword && <p className="text-[10px] text-red-400 mt-1.5 ml-1">Passwords don't match</p>}
                {confirmPassword && password === confirmPassword && password.length >= 8 && <p className="text-[10px] text-dag-green mt-1.5 ml-1 flex items-center gap-1"><Check className="w-3 h-3" /> Passwords match</p>}
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
              {loading ? (<><span className="w-4 h-4 border-2 border-white/30 border-t-white rounded-full animate-spin" /> Creating Wallet...</>) : (<>Create Wallet <ArrowRight className="w-4 h-4" /></>)}
            </button>
          </div>
        </div>
      </div>
    );
  }

  // ═══════════════════════════════════════════════════════════════════════════
  // BIOMETRICS
  // ═══════════════════════════════════════════════════════════════════════════
  if (step === 'biometrics') {
    const hasBioError = error === 'cancelled' || error === 'failed';
    return (
      <div className="min-h-[calc(100vh-3.5rem)] flex items-center justify-center p-6">
        <div className="max-w-md w-full">
          <StepIndicator current={stepNum} total={totalSteps} labels={stepLabels} />
          <div className="space-y-6 text-center">
            <div className="space-y-3">
              <div className={`w-20 h-20 rounded-2xl flex items-center justify-center mx-auto shadow-lg transition-all duration-500 ${
                biometricsDone ? 'bg-gradient-to-br from-dag-green to-emerald-500 shadow-dag-green/20 scale-110' : 'bg-gradient-to-br from-dag-accent to-purple-500 shadow-dag-accent/20'
              }`}>
                {biometricsDone ? <Check className="w-10 h-10 text-white" /> : <Fingerprint className="w-10 h-10 text-white" />}
              </div>
              <h1 className="text-2xl font-bold text-white">
                {biometricsDone ? 'Biometrics Enabled!' : 'Enable Quick Unlock'}
              </h1>
              <p className="text-dag-muted text-sm max-w-xs mx-auto">
                {biometricsDone ? 'Next time, unlock with just a glance or a touch.' : 'Use Face ID, Touch ID, or fingerprint to unlock instantly.'}
              </p>
            </div>

            {!biometricsDone && (
              <>
                <div className="grid grid-cols-2 gap-3 text-left">
                  <div className="p-3 rounded-xl bg-slate-800/50 border border-slate-700/50">
                    <Zap className="w-4 h-4 text-dag-yellow mb-2" />
                    <p className="text-xs text-white font-medium">Instant access</p>
                    <p className="text-[10px] text-slate-500 mt-0.5">Unlock in under a second</p>
                  </div>
                  <div className="p-3 rounded-xl bg-slate-800/50 border border-slate-700/50">
                    <Shield className="w-4 h-4 text-dag-green mb-2" />
                    <p className="text-xs text-white font-medium">Same security</p>
                    <p className="text-[10px] text-slate-500 mt-0.5">Password is always a fallback</p>
                  </div>
                </div>

                {hasBioError && (
                  <div className="space-y-3">
                    <p className="text-sm text-slate-400">
                      {error === 'cancelled' ? 'Biometric setup was cancelled.' : 'Biometric setup failed.'}
                    </p>
                    <div className="flex gap-2">
                      <button onClick={handleBiometricEnroll} disabled={loading}
                        className="flex-1 py-2.5 rounded-lg bg-slate-700 text-slate-200 text-xs font-medium hover:bg-slate-600 transition-colors flex items-center justify-center gap-1.5">
                        <RefreshCw className="w-3.5 h-3.5" /> Try Again
                      </button>
                      <button onClick={() => goTo('success')}
                        className="flex-1 py-2.5 rounded-lg bg-slate-800/50 text-slate-400 text-xs font-medium hover:text-slate-200 transition-colors">
                        Continue Without
                      </button>
                    </div>
                  </div>
                )}

                {!hasBioError && (
                  <>
                    <button onClick={handleBiometricEnroll} disabled={loading}
                      className="w-full py-3.5 rounded-xl bg-gradient-to-r from-dag-accent to-purple-500 text-white font-semibold text-sm hover:opacity-90 disabled:opacity-50 transition-all flex items-center justify-center gap-2">
                      {loading ? (<><span className="w-4 h-4 border-2 border-white/30 border-t-white rounded-full animate-spin" /> Waiting for biometrics...</>) : (<><Fingerprint className="w-5 h-5" /> Enable Biometrics</>)}
                    </button>
                    <button onClick={() => goTo('success')} className="w-full text-center text-sm text-slate-500 hover:text-slate-300 transition-colors py-1">
                      Skip for now
                    </button>
                  </>
                )}
              </>
            )}
          </div>
        </div>
      </div>
    );
  }

  // ═══════════════════════════════════════════════════════════════════════════
  // SUCCESS
  // ═══════════════════════════════════════════════════════════════════════════
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
              <p className="text-dag-muted text-sm">Your <NetworkBadge network={network} /> wallet is ready.</p>
            </div>

            {onExportBlob && (
              <button onClick={handleDownloadKeystore}
                className={`w-full flex items-center gap-3 p-4 rounded-xl border transition-all text-left ${
                  keystoreDownloaded ? 'border-dag-green/30 bg-dag-green/5' : 'border-dag-accent/30 bg-dag-accent/5 hover:border-dag-accent/50 hover:bg-dag-accent/10'
                }`}>
                <div className={`w-10 h-10 rounded-lg flex items-center justify-center flex-shrink-0 ${keystoreDownloaded ? 'bg-dag-green/15' : 'bg-dag-accent/15'}`}>
                  {keystoreDownloaded ? <Check className="w-5 h-5 text-dag-green" /> : <Download className="w-5 h-5 text-dag-accent" />}
                </div>
                <div className="flex-1 min-w-0">
                  <p className="text-sm font-medium text-white">{keystoreDownloaded ? 'Keystore Downloaded' : 'Download Keystore Backup'}</p>
                  <p className="text-[10px] text-dag-muted mt-0.5">
                    {keystoreDownloaded ? `Saved as ultradag-${network}-keystore.json` : 'Encrypted backup — restore on any device'}
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

  // ═══════════════════════════════════════════════════════════════════════════
  // RESTORE FROM BACKUP FILE
  // ═══════════════════════════════════════════════════════════════════════════
  const restoreBack = () => goTo(hasExisting ? 'unlock' : isImportFlow ? 'import' : 'landing');
  const jsonLooksValid = (() => { try { const p = JSON.parse(importJson); return p && p.version && p.ciphertext; } catch { return false; } })();

  return (
    <div className="min-h-[calc(100vh-3.5rem)] flex items-center justify-center p-6">
      <div className="max-w-md w-full space-y-5">
        <div className="flex items-center gap-3">
          <button onClick={restoreBack} className="p-2 rounded-lg text-slate-400 hover:text-white hover:bg-slate-800 transition-colors">
            <ArrowRight className="w-4 h-4 rotate-180" />
          </button>
          <div>
            <h1 className="text-xl font-bold text-white">Restore from Backup</h1>
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
            rows={5} className="w-full px-3 py-2.5 bg-slate-900/60 border border-slate-700/50 rounded-lg text-sm text-slate-200 placeholder-slate-600 focus:outline-none focus:border-dag-accent/40 focus:ring-1 focus:ring-dag-accent/20 transition-all resize-none font-mono text-xs"
            autoFocus />
          {importJson.trim().length > 0 && (
            <p className={`text-[10px] flex items-center gap-1.5 ${jsonLooksValid ? 'text-dag-green' : 'text-slate-500'}`}>
              {jsonLooksValid && <Check className="w-3 h-3" />}
              {jsonLooksValid ? 'Valid keystore format detected' : 'Paste the full JSON contents'}
            </p>
          )}
        </div>

        <div className="flex items-start gap-2.5 p-3 rounded-lg bg-slate-800/40 border border-slate-700/30">
          <Shield className="w-3.5 h-3.5 text-slate-500 mt-0.5 flex-shrink-0" />
          <p className="text-[10px] text-slate-500">Your backup is encrypted. You'll need the password you set when the wallet was created.</p>
        </div>

        {error && <p className="text-sm text-red-400 text-center">{error}</p>}

        <button onClick={handleRestore} disabled={loading || !jsonLooksValid}
          className="w-full py-3.5 rounded-xl bg-dag-accent text-white font-semibold text-sm hover:bg-dag-accent/80 disabled:opacity-30 disabled:cursor-not-allowed transition-all">
          Restore Wallet
        </button>
      </div>
    </div>
  );
}
