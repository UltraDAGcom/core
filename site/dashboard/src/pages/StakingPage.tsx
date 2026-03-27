import { useState, useEffect, useCallback } from 'react';
import { Coins, TrendingUp, Shield, ChevronDown, ChevronUp, Server, ExternalLink } from 'lucide-react';
import { getValidators, getStake, getDelegation, formatUdag, shortAddr, postDelegate, postUndelegate, getNodeUrl } from '../lib/api';
import { useKeystore } from '../hooks/useKeystore';
import { hasPasskeyWallet, getPasskeyWallet } from '../lib/passkey-wallet';
import { signAndSubmitSmartOp } from '../lib/webauthn-sign';
import { Card } from '../components/shared/Card';
import { ValidatorCard } from '../components/staking/ValidatorCard';
import { Pagination } from '../components/shared/Pagination';

const PER_PAGE = 10;
const SATS = 100_000_000;

interface ValidatorInfo {
  address: string;
  staked: number;
  effective_stake: number;
  delegator_count: number;
  commission_percent: number;
  is_active?: boolean;
}

interface DelegationInfo {
  address: string;
  name: string;
  delegated: number;
  validator: string;
  is_undelegating: boolean;
  unlock_at_round: number | null;
}

/** Pick the best validator for auto-delegation:
 *  1. Active validators only
 *  2. Lowest commission first
 *  3. Tie-break: lowest effective_stake (helps decentralization)
 */
function pickBestValidator(validators: ValidatorInfo[]): ValidatorInfo | null {
  const active = validators.filter(v => v.is_active);
  if (active.length === 0) return null;
  const sorted = [...active].sort((a, b) => {
    if (a.commission_percent !== b.commission_percent) return a.commission_percent - b.commission_percent;
    return a.effective_stake - b.effective_stake; // prefer less concentrated
  });
  return sorted[0];
}

export function StakingPage() {
  const { wallets, unlocked } = useKeystore();
  const [validators, setValidators] = useState<ValidatorInfo[]>([]);
  const [totalStaked, setTotalStaked] = useState(0);
  const [totalDelegated, setTotalDelegated] = useState(0);
  const [validatorCount, setValidatorCount] = useState(0);
  const [delegations, setDelegations] = useState<DelegationInfo[]>([]);
  const [loading, setLoading] = useState(true);

  // Simple stake form
  const [amount, setAmount] = useState('');
  const [walletIdx, setWalletIdx] = useState(0);
  const [stakeLoading, setStakeLoading] = useState(false);
  const [stakeError, setStakeError] = useState('');
  const [stakeSuccess, setStakeSuccess] = useState('');

  // Advanced mode
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [customValidator, setCustomValidator] = useState<string | null>(null);
  const [advancedPage, setAdvancedPage] = useState(1);

  // Undelegate
  const [undelegateLoading, setUndelegateLoading] = useState('');
  const [undelegateError, setUndelegateError] = useState('');

  const refresh = useCallback(async () => {
    try {
      const v = await getValidators();
      const raw: ValidatorInfo[] = Array.isArray(v) ? v : (v.validators ?? []);
      const list = raw.map((vi: ValidatorInfo) => ({ ...vi, is_active: vi.is_active ?? true }));
      setValidators(list);
      if (!Array.isArray(v)) {
        setTotalStaked(v.total_staked ?? 0);
        setTotalDelegated(v.total_delegated ?? 0);
        setValidatorCount(v.count ?? list.length);
      } else {
        setValidatorCount(list.length);
      }
    } catch { /* ignore */ }

    if (unlocked && wallets.length > 0) {
      const delResults: DelegationInfo[] = [];
      for (const w of wallets) {
        try {
          const d = await getDelegation(w.address);
          if (d.delegated > 0 || d.is_undelegating) {
            delResults.push({ address: w.address, name: w.name, ...d });
          }
        } catch { /* no delegation */ }
      }
      setDelegations(delResults);
    }
    setLoading(false);
  }, [wallets, unlocked]);

  useEffect(() => {
    refresh();
    const interval = setInterval(refresh, 30000);
    return () => clearInterval(interval);
  }, [refresh]);

  const bestValidator = pickBestValidator(validators);
  const targetValidator = customValidator || bestValidator?.address || null;

  const handleStake = async () => {
    const wallet = wallets[walletIdx];
    if (!wallet || !targetValidator) return;
    const sats = Math.floor(parseFloat(amount) * SATS);
    if (isNaN(sats) || sats < 100 * SATS) {
      setStakeError('Minimum is 100 UDAG');
      return;
    }
    setStakeLoading(true);
    setStakeError('');
    setStakeSuccess('');
    try {
      if (hasPasskeyWallet() && !wallet.secret_key) {
        // Passkey wallet: use SmartOp with WebAuthn signing
        const balRes = await fetch(`${getNodeUrl()}/balance/${wallet.address}`, { signal: AbortSignal.timeout(5000) });
        const balData = await balRes.json();
        await signAndSubmitSmartOp(
          { Delegate: { validator: targetValidator, amount: sats } },
          0, // fee-exempt
          balData.nonce ?? 0,
        );
      } else {
        await postDelegate({ secret_key: wallet.secret_key, validator: targetValidator, amount: sats });
      }
      setStakeSuccess(`${amount} UDAG staked successfully!`);
      setAmount('');
      setCustomValidator(null);
      refresh();
    } catch (e: unknown) {
      setStakeError(e instanceof Error ? e.message : 'Staking failed');
    } finally {
      setStakeLoading(false);
    }
  };

  const handleUndelegate = async (walletAddr: string) => {
    const wallet = wallets.find(w => w.address === walletAddr);
    if (!wallet) return;
    setUndelegateLoading(walletAddr);
    setUndelegateError('');
    try {
      if (hasPasskeyWallet() && !wallet.secret_key) {
        const balRes = await fetch(`${getNodeUrl()}/balance/${wallet.address}`, { signal: AbortSignal.timeout(5000) });
        const balData = await balRes.json();
        await signAndSubmitSmartOp({ Undelegate: {} }, 0, balData.nonce ?? 0);
      } else {
        await postUndelegate({ secret_key: wallet.secret_key });
      }
      refresh();
    } catch (e: unknown) {
      setUndelegateError(e instanceof Error ? e.message : 'Unstaking failed');
    } finally {
      setUndelegateLoading('');
    }
  };

  const totalPages = Math.max(1, Math.ceil(validators.length / PER_PAGE));
  const pageValidators = validators.slice((advancedPage - 1) * PER_PAGE, advancedPage * PER_PAGE);

  return (
    <div className="space-y-6 animate-page-enter">
      <div>
        <h1 className="text-2xl font-bold text-white">Staking</h1>
        <p className="text-sm text-dag-muted mt-1">Stake UDAG to earn passive rewards — no node required</p>
      </div>

      {/* Stats */}
      <div className="grid grid-cols-1 sm:grid-cols-2 md:grid-cols-3 gap-4">
        <div className="rounded-lg bg-dag-card border border-dag-border p-4">
          <span className="text-dag-muted text-xs block mb-1">Total Staked</span>
          <span className="text-white font-bold text-lg font-mono">{formatUdag(totalStaked)} <span className="text-dag-muted text-sm font-normal">UDAG</span></span>
        </div>
        <div className="rounded-lg bg-dag-card border border-dag-border p-4">
          <span className="text-dag-muted text-xs block mb-1">Total Delegated</span>
          <span className="text-white font-bold text-lg font-mono">{formatUdag(totalDelegated)} <span className="text-dag-muted text-sm font-normal">UDAG</span></span>
        </div>
        <div className="rounded-lg bg-dag-card border border-dag-border p-4">
          <span className="text-dag-muted text-xs block mb-1">Active Validators</span>
          <span className="text-dag-green font-bold text-xl font-mono">{validatorCount}</span>
        </div>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-5 gap-6">
        {/* ── LEFT: STAKE FORM ── */}
        <div className="lg:col-span-2 space-y-4">
          {/* Primary one-click staking card */}
          <div className="rounded-xl bg-gradient-to-b from-dag-accent/5 to-transparent border border-dag-accent/20 p-6 space-y-5">
            <div className="space-y-1">
              <h2 className="text-lg font-bold text-white">Stake UDAG</h2>
              <p className="text-xs text-dag-muted">Enter an amount and we'll stake it with the best validator automatically.</p>
            </div>

            {!unlocked ? (
              <p className="text-sm text-dag-muted py-4 text-center">Unlock your wallet to stake.</p>
            ) : (
              <div className="space-y-4">
                {/* Wallet selector (only if multiple) */}
                {wallets.length > 1 && (
                  <div>
                    <label htmlFor="stake-wallet" className="text-[10px] text-dag-muted uppercase tracking-wider block mb-1">Wallet</label>
                    <select id="stake-wallet" value={walletIdx} onChange={e => setWalletIdx(Number(e.target.value))}
                      className="w-full rounded-lg bg-dag-surface border border-dag-border px-3 py-2.5 text-sm text-white">
                      {wallets.map((w, i) => (
                        <option key={i} value={i}>{w.name} ({shortAddr(w.address)})</option>
                      ))}
                    </select>
                  </div>
                )}

                {/* Amount input */}
                <div>
                  <label htmlFor="stake-amount" className="text-[10px] text-dag-muted uppercase tracking-wider block mb-1">Amount</label>
                  <div className="relative">
                    <input
                      id="stake-amount"
                      type="number"
                      min="100"
                      step="1"
                      value={amount}
                      onChange={e => { setAmount(e.target.value); setStakeError(''); setStakeSuccess(''); }}
                      placeholder="100"
                      className="w-full rounded-lg bg-dag-surface border border-dag-border px-3 py-3 pr-16 text-white text-lg font-mono"
                    />
                    <span className="absolute right-3 top-1/2 -translate-y-1/2 text-dag-muted text-sm">UDAG</span>
                  </div>
                  <p className="text-[10px] text-dag-muted mt-1.5">Minimum 100 UDAG</p>
                </div>

                {/* Auto-selected validator info */}
                {targetValidator && (
                  <div className="rounded-lg bg-dag-surface/50 border border-dag-border/50 p-3">
                    <div className="flex items-center justify-between">
                      <div>
                        <p className="text-[10px] text-dag-muted uppercase tracking-wider">
                          {customValidator ? 'Selected validator' : 'Auto-selected validator'}
                        </p>
                        <p className="text-sm text-white font-mono mt-0.5">{shortAddr(targetValidator)}</p>
                      </div>
                      {bestValidator && !customValidator && (
                        <span className="text-[10px] px-2 py-0.5 rounded bg-dag-green/15 text-dag-green border border-dag-green/20">
                          {bestValidator.commission_percent}% commission
                        </span>
                      )}
                    </div>
                    {customValidator && (
                      <button onClick={() => setCustomValidator(null)} className="text-[10px] text-dag-accent hover:underline mt-1">
                        Use auto-selection instead
                      </button>
                    )}
                  </div>
                )}

                {!targetValidator && validators.length === 0 && !loading && (
                  <div className="rounded-lg bg-amber-500/5 border border-amber-500/20 p-3">
                    <p className="text-xs text-amber-400">No validators available yet. Staking will be enabled when validators join the network.</p>
                  </div>
                )}

                {stakeError && (
                  <div className="rounded-lg bg-red-500/10 border border-red-500/20 p-2.5">
                    <p className="text-sm text-red-400">{stakeError}</p>
                  </div>
                )}
                {stakeSuccess && (
                  <div className="rounded-lg bg-dag-green/10 border border-dag-green/20 p-2.5">
                    <p className="text-sm text-dag-green">{stakeSuccess}</p>
                  </div>
                )}

                <button
                  onClick={handleStake}
                  disabled={stakeLoading || !targetValidator || !amount}
                  className="w-full py-3 rounded-xl bg-dag-accent text-white font-semibold text-sm hover:bg-dag-accent/90 disabled:opacity-30 disabled:cursor-not-allowed transition-all"
                >
                  {stakeLoading ? 'Staking...' : 'Stake'}
                </button>
              </div>
            )}
          </div>

          {/* How it works */}
          <div className="rounded-lg bg-dag-surface/30 border border-dag-border/50 p-4 space-y-3">
            <div className="flex items-center gap-2">
              <TrendingUp className="w-4 h-4 text-dag-green" />
              <p className="text-xs text-white font-medium">How staking works</p>
            </div>
            <div className="space-y-2 text-[11px] text-dag-muted leading-relaxed">
              <p>Your UDAG is delegated to a validator who secures the network. You earn rewards proportional to your stake, minus the validator's commission.</p>
              <p>Unstaking has a ~2.8 hour cooldown before your funds are returned.</p>
            </div>
          </div>

          {/* Validator info */}
          <div className="rounded-lg bg-dag-surface/30 border border-dag-border/50 p-3">
            <div className="flex items-start gap-2.5">
              <Server className="w-3.5 h-3.5 text-dag-muted mt-0.5 flex-shrink-0" />
              <div>
                <p className="text-[11px] text-dag-muted">
                  <span className="text-slate-400">Run a validator?</span> Requires CLI setup with <code className="text-dag-accent text-[10px]">--auto-stake</code>.
                </p>
                <a href="/docs.html#staking" className="text-[11px] text-dag-accent hover:underline inline-flex items-center gap-1 mt-0.5">
                  Setup guide <ExternalLink className="w-2.5 h-2.5" />
                </a>
              </div>
            </div>
          </div>
        </div>

        {/* ── RIGHT: DELEGATIONS + VALIDATORS ── */}
        <div className="lg:col-span-3 space-y-6">
          {/* Your active delegations */}
          {unlocked && delegations.length > 0 && (
            <Card title="Your Staked UDAG">
              <div className="space-y-3">
                {delegations.map(d => (
                  <div key={d.address} className="rounded-lg bg-dag-surface border border-dag-border p-4">
                    <div className="flex items-center justify-between mb-3">
                      <div>
                        <span className="text-sm font-medium text-white">{d.name}</span>
                        <span className={`ml-2 text-[10px] px-2 py-0.5 rounded-full ${
                          d.is_undelegating ? 'bg-amber-400/15 text-amber-400' : 'bg-dag-green/15 text-dag-green'
                        }`}>
                          {d.is_undelegating ? 'Unstaking' : 'Earning rewards'}
                        </span>
                      </div>
                      <span className="text-lg font-bold text-white font-mono">{formatUdag(d.delegated)} <span className="text-sm text-dag-muted font-normal">UDAG</span></span>
                    </div>
                    <div className="flex items-center justify-between text-xs">
                      <span className="text-dag-muted">Validator: <span className="font-mono text-slate-400">{shortAddr(d.validator)}</span></span>
                      {d.is_undelegating ? (
                        <span className="text-amber-400">Cooldown until round {d.unlock_at_round}</span>
                      ) : (
                        <button
                          onClick={() => handleUndelegate(d.address)}
                          disabled={undelegateLoading === d.address}
                          className="px-3 py-1 rounded bg-red-500/10 text-red-400 border border-red-500/20 hover:bg-red-500/20 disabled:opacity-50 transition-colors"
                        >
                          {undelegateLoading === d.address ? 'Unstaking...' : 'Unstake'}
                        </button>
                      )}
                    </div>
                  </div>
                ))}
                {undelegateError && <p className="text-sm text-red-400 mt-2">{undelegateError}</p>}
              </div>
            </Card>
          )}

          {/* Advanced: choose your own validator */}
          <div className="rounded-lg bg-dag-surface/30 border border-dag-border/50 overflow-hidden">
            <button
              onClick={() => setShowAdvanced(!showAdvanced)}
              className="w-full flex items-center justify-between p-4 text-left hover:bg-dag-surface/50 transition-colors"
            >
              <div className="flex items-center gap-2">
                <Shield className="w-4 h-4 text-dag-muted" />
                <span className="text-sm text-slate-300 font-medium">Choose your own validator</span>
                <span className="text-[10px] text-dag-muted">Advanced</span>
              </div>
              {showAdvanced ? <ChevronUp className="w-4 h-4 text-dag-muted" /> : <ChevronDown className="w-4 h-4 text-dag-muted" />}
            </button>

            {showAdvanced && (
              <div className="px-4 pb-4 space-y-3">
                <p className="text-xs text-dag-muted">
                  Select a specific validator to stake with instead of automatic selection. Consider commission rate, uptime, and stake concentration.
                </p>
                {loading ? (
                  <p className="text-sm text-dag-muted py-4 text-center">Loading validators...</p>
                ) : validators.length === 0 ? (
                  <div className="text-center py-8">
                    <Coins className="w-8 h-8 text-purple-400/50 mx-auto mb-2" />
                    <p className="text-sm text-dag-muted">No validators yet</p>
                    <p className="text-xs text-dag-muted mt-1">Validators join by running a node with the CLI</p>
                  </div>
                ) : (
                  <>
                    <div className="grid grid-cols-1 sm:grid-cols-2 gap-2">
                      {pageValidators.map(v => (
                        <ValidatorCard
                          key={v.address}
                          address={v.address}
                          effective_stake={v.effective_stake}
                          delegator_count={v.delegator_count}
                          commission_percent={v.commission_percent}
                          is_active={v.is_active || false}
                          onDelegate={unlocked && wallets.length > 0 ? () => {
                            setCustomValidator(v.address);
                            setShowAdvanced(false);
                            window.scrollTo({ top: 0, behavior: 'smooth' });
                          } : undefined}
                        />
                      ))}
                    </div>
                    <Pagination currentPage={advancedPage} totalPages={totalPages} onPageChange={setAdvancedPage} />
                  </>
                )}
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
