import { useState, useEffect, useCallback } from 'react';
import { Key, Shield, Clock, Users, Wallet, Plus, Trash2, AlertTriangle, CheckCircle, Loader2, Tag, RefreshCw } from 'lucide-react';
import { getNodeUrl } from '../lib/api';
import { getPasskeyWallet } from '../lib/passkey-wallet';
import { signAndSubmitWithPasskey } from '../lib/webauthn-sign';

interface AuthorizedKey {
  key_id: string;
  key_type: string;
  label: string;
  daily_limit: number | null;
}

interface SmartAccountInfo {
  address: string;
  created_at_round: number;
  authorized_keys: AuthorizedKey[];
  has_recovery: boolean;
  guardian_count: number | null;
  recovery_threshold: number | null;
  has_pending_recovery: boolean;
  has_policy: boolean;
  instant_limit: number | null;
  vault_threshold: number | null;
  daily_limit: number | null;
  pending_vault_count: number;
  pending_key_removal: { key_id: string; executes_at_round: number } | null;
}

interface NameInfo {
  name: string;
  address: string;
  expiry_round: number | null;
}

const COIN = 100_000_000;
function satsToUdag(sats: number): string { return (sats / COIN).toFixed(4); }

function Card({ title, icon, children, action }: { title: string; icon: React.ReactNode; children: React.ReactNode; action?: React.ReactNode }) {
  return (
    <div className="bg-dag-card border border-dag-border rounded-xl p-5">
      <div className="flex items-center justify-between mb-4">
        <div className="flex items-center gap-2">
          {icon}
          <h3 className="text-white font-semibold">{title}</h3>
        </div>
        {action}
      </div>
      {children}
    </div>
  );
}

function ActionButton({ onClick, loading, children, variant = 'primary' }: { onClick: () => void; loading?: boolean; children: React.ReactNode; variant?: 'primary' | 'danger' | 'secondary' }) {
  const colors = variant === 'danger' ? 'bg-red-500/20 text-red-400 hover:bg-red-500/30 border-red-500/30' :
    variant === 'secondary' ? 'bg-slate-700/50 text-white hover:bg-slate-700 border-slate-600' :
    'bg-dag-accent/20 text-dag-accent hover:bg-dag-accent/30 border-dag-accent/30';
  return (
    <button onClick={onClick} disabled={loading} className={`px-3 py-1.5 rounded-lg text-xs font-medium border transition-all disabled:opacity-50 ${colors}`}>
      {loading ? <Loader2 className="w-3 h-3 animate-spin inline" /> : children}
    </button>
  );
}

export function SmartAccountPage({ walletAddress, nodeUrl }: { walletAddress?: string; nodeUrl: string }) {
  const [info, setInfo] = useState<SmartAccountInfo | null>(null);
  const [nameInfo, setNameInfo] = useState<NameInfo | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');
  const [success, setSuccess] = useState('');

  // Form states
  const [showNameForm, setShowNameForm] = useState(false);
  const [nameInput, setNameInput] = useState('');
  const [nameAvailable, setNameAvailable] = useState<boolean | null>(null);
  const [nameChecking, setNameChecking] = useState(false);
  const [nameFee, setNameFee] = useState(0);
  const [nameWarning, setNameWarning] = useState('');

  const [showRecoveryForm, setShowRecoveryForm] = useState(false);
  const [guardianInputs, setGuardianInputs] = useState(['', '', '']);
  const [recoveryThreshold, setRecoveryThreshold] = useState(2);

  const [showPolicyForm, setShowPolicyForm] = useState(false);
  const [policyInstantLimit, setPolicyInstantLimit] = useState('1');
  const [policyVaultThreshold, setPolicyVaultThreshold] = useState('100');
  const [policyDailyLimit, setPolicyDailyLimit] = useState('10');

  const [submitting, setSubmitting] = useState(false);

  const fetchInfo = useCallback(async () => {
    if (!walletAddress) return;
    setLoading(true);
    try {
      const [saRes, nameRes] = await Promise.all([
        fetch(`${nodeUrl}/smart-account/${walletAddress}`, { signal: AbortSignal.timeout(5000) }).catch(() => null),
        fetch(`${nodeUrl}/name/reverse/${walletAddress}`, { signal: AbortSignal.timeout(5000) }).catch(() => null),
      ]);
      if (saRes?.ok) setInfo(await saRes.json()); else setInfo(null);
      if (nameRes?.ok) setNameInfo(await nameRes.json()); else setNameInfo(null);
    } catch { setError('Failed to fetch SmartAccount info'); }
    finally { setLoading(false); }
  }, [walletAddress, nodeUrl]);

  useEffect(() => { fetchInfo(); }, [fetchInfo]);

  const checkName = useCallback(async (name: string) => {
    setNameInput(name);
    setNameAvailable(null);
    setNameWarning('');
    if (name.length < 3) return;
    setNameChecking(true);
    try {
      const res = await fetch(`${nodeUrl}/name/available/${encodeURIComponent(name)}`, { signal: AbortSignal.timeout(5000) });
      if (res.ok) {
        const data = await res.json();
        setNameAvailable(data.available);
        setNameFee(data.annual_fee || 0);
        if (data.similar_warning) setNameWarning(data.similar_warning);
        if (!data.valid) setNameWarning('Invalid name format');
      }
    } catch {} finally { setNameChecking(false); }
  }, [nodeUrl]);

  const submitNameRegistration = useCallback(async () => {
    if (!walletAddress || !nameInput || !nameAvailable) return;
    setSubmitting(true);
    setError('');
    try {
      // For now, show instruction — full WebAuthn-signed RegisterNameTx
      // requires building signable_bytes client-side
      setSuccess(`Name "${nameInput}" registration ready. Fee: ${satsToUdag(nameFee)} UDAG/year. Submit via /tx/submit with a signed RegisterNameTx.`);
      setShowNameForm(false);
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : 'Failed');
    } finally { setSubmitting(false); }
  }, [walletAddress, nameInput, nameAvailable, nameFee]);

  if (!walletAddress) {
    return <div className="p-6 text-dag-muted">Create or unlock a wallet to manage your SmartAccount.</div>;
  }
  if (loading) {
    return <div className="p-6 text-dag-muted animate-pulse">Loading SmartAccount info...</div>;
  }

  return (
    <div className="p-4 lg:p-6 space-y-6 max-w-4xl">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-white">SmartAccount</h1>
          <p className="text-dag-muted text-sm mt-1">Manage your keys, recovery, spending limits, and name.</p>
        </div>
        <button onClick={fetchInfo} className="p-2 rounded-lg bg-slate-800 hover:bg-slate-700 transition-colors">
          <RefreshCw className="w-4 h-4 text-dag-muted" />
        </button>
      </div>

      {error && <div className="bg-red-500/10 border border-red-500/30 rounded-lg p-3 text-red-400 text-sm">{error}</div>}
      {success && <div className="bg-green-500/10 border border-green-500/30 rounded-lg p-3 text-green-400 text-sm">{success}</div>}

      {/* ── Name ── */}
      <Card title="Your Name" icon={<Tag className="w-5 h-5 text-dag-accent" />}
        action={!nameInfo && !showNameForm ? <ActionButton onClick={() => setShowNameForm(true)}>Claim Name</ActionButton> : undefined}>
        {nameInfo ? (
          <div className="space-y-2">
            <div className="flex items-center gap-3">
              <span className="text-2xl font-bold text-dag-accent">{nameInfo.name}</span>
              <span className="text-xs bg-green-500/20 text-green-400 px-2 py-0.5 rounded-full">Active</span>
            </div>
            {nameInfo.expiry_round && <p className="text-dag-muted text-sm">Expires at round {nameInfo.expiry_round.toLocaleString()}</p>}
          </div>
        ) : showNameForm ? (
          <div className="space-y-3">
            <div className="relative">
              <input type="text" value={nameInput} onChange={e => checkName(e.target.value.toLowerCase().replace(/[^a-z0-9-]/g, ''))}
                placeholder="john29" maxLength={20} autoFocus
                className="w-full px-4 py-3 bg-dag-bg border border-dag-border rounded-xl text-white placeholder-slate-600 focus:border-dag-accent focus:outline-none" />
              {nameChecking && <Loader2 className="absolute right-3 top-1/2 -translate-y-1/2 w-4 h-4 animate-spin text-dag-muted" />}
              {nameAvailable === true && <CheckCircle className="absolute right-3 top-1/2 -translate-y-1/2 w-4 h-4 text-green-400" />}
            </div>
            {nameWarning && <p className="text-yellow-400 text-xs">{nameWarning}</p>}
            {nameAvailable && <p className="text-green-400 text-xs">{nameInput} is available! ({satsToUdag(nameFee)} UDAG/year)</p>}
            {nameAvailable === false && <p className="text-red-400 text-xs">Name taken</p>}
            <div className="flex gap-2">
              <ActionButton onClick={submitNameRegistration} loading={submitting}>Register</ActionButton>
              <ActionButton onClick={() => setShowNameForm(false)} variant="secondary">Cancel</ActionButton>
            </div>
          </div>
        ) : (
          <p className="text-dag-muted text-sm">No name registered. Claim a name like <span className="text-dag-accent">alice</span> or <span className="text-dag-accent">john29</span>.</p>
        )}
      </Card>

      {/* ── Keys ── */}
      <Card title="Authorized Keys" icon={<Key className="w-5 h-5 text-blue-400" />}>
        {info && info.authorized_keys.length > 0 ? (
          <div className="space-y-3">
            {info.authorized_keys.map(key => (
              <div key={key.key_id} className="flex items-center justify-between bg-dag-bg rounded-lg p-3">
                <div className="flex items-center gap-3">
                  <div className={`w-8 h-8 rounded-lg flex items-center justify-center ${key.key_type === 'p256' ? 'bg-purple-500/20' : 'bg-blue-500/20'}`}>
                    <Key className={`w-4 h-4 ${key.key_type === 'p256' ? 'text-purple-400' : 'text-blue-400'}`} />
                  </div>
                  <div>
                    <p className="text-white text-sm font-medium">{key.label}</p>
                    <p className="text-dag-muted text-xs font-mono">{key.key_id.slice(0, 12)}... ({key.key_type === 'p256' ? 'Passkey' : 'Ed25519'})</p>
                  </div>
                </div>
                {key.daily_limit ? <p className="text-dag-muted text-xs">{satsToUdag(key.daily_limit)} UDAG/day</p> : <p className="text-dag-muted text-xs">No limit</p>}
              </div>
            ))}
            {info.pending_key_removal && (
              <div className="bg-yellow-500/10 border border-yellow-500/30 rounded-lg p-3 text-yellow-400 text-sm flex items-center gap-2">
                <Clock className="w-4 h-4" /> Key removal pending (round {info.pending_key_removal.executes_at_round})
              </div>
            )}
            <p className="text-dag-muted text-xs">To add a backup device, scan a QR code from your other device's wallet app.</p>
          </div>
        ) : (
          <p className="text-dag-muted text-sm">
            {info ? 'No keys registered yet. Your key will auto-register on first transaction.' : 'SmartAccount not yet created. Send or receive funds to activate.'}
          </p>
        )}
      </Card>

      {/* ── Recovery ── */}
      <Card title="Social Recovery" icon={<Users className="w-5 h-5 text-green-400" />}
        action={!info?.has_recovery && !showRecoveryForm ? <ActionButton onClick={() => setShowRecoveryForm(true)}>Set Up</ActionButton> : undefined}>
        {info?.has_recovery ? (
          <div className="space-y-2">
            <div className="flex items-center gap-2">
              <CheckCircle className="w-4 h-4 text-green-400" />
              <span className="text-green-400 text-sm font-medium">Recovery configured</span>
            </div>
            <p className="text-dag-muted text-sm">{info.recovery_threshold}-of-{info.guardian_count} guardians</p>
            {info.has_pending_recovery && (
              <div className="bg-yellow-500/10 border border-yellow-500/30 rounded-lg p-3 text-yellow-400 text-sm flex items-center gap-2">
                <AlertTriangle className="w-4 h-4" /> Recovery attempt pending — cancel if you didn't initiate it!
              </div>
            )}
          </div>
        ) : showRecoveryForm ? (
          <div className="space-y-3">
            <p className="text-dag-muted text-xs">Enter addresses of trusted people who can help recover your account.</p>
            {guardianInputs.map((g, i) => (
              <input key={i} type="text" value={g} onChange={e => { const n = [...guardianInputs]; n[i] = e.target.value; setGuardianInputs(n); }}
                placeholder={`Guardian ${i + 1} address or name`}
                className="w-full px-3 py-2 bg-dag-bg border border-dag-border rounded-lg text-white text-sm placeholder-slate-600 focus:border-dag-accent focus:outline-none" />
            ))}
            <div className="flex items-center gap-2">
              <span className="text-dag-muted text-xs">Required to recover:</span>
              <select value={recoveryThreshold} onChange={e => setRecoveryThreshold(Number(e.target.value))}
                className="bg-dag-bg border border-dag-border rounded px-2 py-1 text-white text-xs">
                {[1,2,3].map(n => <option key={n} value={n}>{n} of {guardianInputs.filter(g => g).length || 3}</option>)}
              </select>
            </div>
            <div className="flex gap-2">
              <ActionButton onClick={() => { setSuccess('Submit SetRecoveryTx via /tx/submit to configure guardians.'); setShowRecoveryForm(false); }}>Save</ActionButton>
              <ActionButton onClick={() => setShowRecoveryForm(false)} variant="secondary">Cancel</ActionButton>
            </div>
          </div>
        ) : (
          <div className="text-dag-muted text-sm">
            <p>No recovery configured.</p>
            <p className="text-xs mt-1">Set up trusted guardians who can help recover your account if all devices are lost.</p>
          </div>
        )}
      </Card>

      {/* ── Spending Policy ── */}
      <Card title="Spending Policy" icon={<Shield className="w-5 h-5 text-orange-400" />}
        action={!info?.has_policy && !showPolicyForm ? <ActionButton onClick={() => setShowPolicyForm(true)}>Set Up</ActionButton> : undefined}>
        {info?.has_policy ? (
          <div className="grid grid-cols-2 gap-3">
            {info.instant_limit != null && (
              <div className="bg-dag-bg rounded-lg p-3">
                <p className="text-xs text-dag-muted">Instant limit</p>
                <p className="text-white font-semibold">{satsToUdag(info.instant_limit)} UDAG</p>
              </div>
            )}
            {info.vault_threshold != null && info.vault_threshold > 0 && (
              <div className="bg-dag-bg rounded-lg p-3">
                <p className="text-xs text-dag-muted">Vault threshold</p>
                <p className="text-white font-semibold">{satsToUdag(info.vault_threshold)} UDAG</p>
              </div>
            )}
            {info.daily_limit != null && (
              <div className="bg-dag-bg rounded-lg p-3">
                <p className="text-xs text-dag-muted">Daily limit</p>
                <p className="text-white font-semibold">{satsToUdag(info.daily_limit)} UDAG</p>
              </div>
            )}
            <div className="bg-dag-bg rounded-lg p-3">
              <p className="text-xs text-dag-muted">Pending vaults</p>
              <p className="text-white font-semibold">{info.pending_vault_count}</p>
            </div>
          </div>
        ) : showPolicyForm ? (
          <div className="space-y-3">
            <div>
              <label className="text-xs text-dag-muted">Instant transfer limit (UDAG)</label>
              <input type="number" value={policyInstantLimit} onChange={e => setPolicyInstantLimit(e.target.value)}
                className="w-full px-3 py-2 bg-dag-bg border border-dag-border rounded-lg text-white text-sm focus:border-dag-accent focus:outline-none" />
            </div>
            <div>
              <label className="text-xs text-dag-muted">Vault threshold — large transfers held (UDAG)</label>
              <input type="number" value={policyVaultThreshold} onChange={e => setPolicyVaultThreshold(e.target.value)}
                className="w-full px-3 py-2 bg-dag-bg border border-dag-border rounded-lg text-white text-sm focus:border-dag-accent focus:outline-none" />
            </div>
            <div>
              <label className="text-xs text-dag-muted">Daily spending cap (UDAG)</label>
              <input type="number" value={policyDailyLimit} onChange={e => setPolicyDailyLimit(e.target.value)}
                className="w-full px-3 py-2 bg-dag-bg border border-dag-border rounded-lg text-white text-sm focus:border-dag-accent focus:outline-none" />
            </div>
            <div className="flex gap-2">
              <ActionButton onClick={() => { setSuccess('Submit SetPolicyTx via /tx/submit. Policy takes effect after ~2.8 hours time-lock.'); setShowPolicyForm(false); }}>Save</ActionButton>
              <ActionButton onClick={() => setShowPolicyForm(false)} variant="secondary">Cancel</ActionButton>
            </div>
          </div>
        ) : (
          <div className="text-dag-muted text-sm">
            <p>No spending policy configured.</p>
            <p className="text-xs mt-1">Set daily limits and vault thresholds to protect against key theft.</p>
          </div>
        )}
      </Card>
    </div>
  );
}
