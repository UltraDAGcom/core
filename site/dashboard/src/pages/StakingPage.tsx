import { useState, useEffect, useCallback } from 'react';
import { Coins, Server, Users, ArrowRight, Info, ExternalLink } from 'lucide-react';
import { getValidators, getStake, getDelegation, formatUdag, shortAddr, postDelegate, postUndelegate } from '../lib/api';
import { useKeystore } from '../hooks/useKeystore';
import { Card } from '../components/shared/Card';
import { Pagination } from '../components/shared/Pagination';
import { ValidatorCard } from '../components/staking/ValidatorCard';

const PER_PAGE = 10;

interface ValidatorInfo {
  address: string;
  staked: number;
  effective_stake: number;
  delegator_count: number;
  commission_percent: number;
  is_active?: boolean;
}

interface StakeInfo {
  address: string;
  name: string;
  staked: number;
  staked_udag: number;
  effective_stake: number;
  commission_percent: number;
  is_active_validator: boolean;
  unlock_at_round: number | null;
}

interface DelegationInfo {
  address: string;
  name: string;
  delegated: number;
  validator: string;
  is_undelegating: boolean;
  unlock_at_round: number | null;
}

export function StakingPage() {
  const { wallets, unlocked } = useKeystore();
  const [validators, setValidators] = useState<ValidatorInfo[]>([]);
  const [totalStaked, setTotalStaked] = useState(0);
  const [totalDelegated, setTotalDelegated] = useState(0);
  const [validatorCount, setValidatorCount] = useState(0);
  const [stakes, setStakes] = useState<StakeInfo[]>([]);
  const [delegations, setDelegations] = useState<DelegationInfo[]>([]);
  const [page, setPage] = useState(1);
  const [loading, setLoading] = useState(true);
  const [delegateTarget, setDelegateTarget] = useState<string | null>(null);
  const [delegateAmount, setDelegateAmount] = useState('');
  const [delegateWalletIdx, setDelegateWalletIdx] = useState(0);
  const [delegateLoading, setDelegateLoading] = useState(false);
  const [delegateError, setDelegateError] = useState('');
  const [delegateSuccess, setDelegateSuccess] = useState('');
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
    } catch {
      /* ignore */
    }

    if (unlocked && wallets.length > 0) {
      const stakeResults: StakeInfo[] = [];
      const delResults: DelegationInfo[] = [];
      for (const w of wallets) {
        try {
          const s = await getStake(w.address);
          if (s.staked > 0 || s.unlock_at_round) {
            stakeResults.push({ address: w.address, name: w.name, ...s });
          }
        } catch {
          /* no stake */
        }
        try {
          const d = await getDelegation(w.address);
          if (d.delegated > 0 || d.is_undelegating) {
            delResults.push({ address: w.address, name: w.name, ...d });
          }
        } catch {
          /* no delegation */
        }
      }
      setStakes(stakeResults);
      setDelegations(delResults);
    }
    setLoading(false);
  }, [wallets, unlocked]);

  useEffect(() => {
    refresh();
    const interval = setInterval(refresh, 30000);
    return () => clearInterval(interval);
  }, [refresh]);

  const totalPages = Math.max(1, Math.ceil(validators.length / PER_PAGE));
  const pageValidators = validators.slice((page - 1) * PER_PAGE, page * PER_PAGE);

  const handleDelegate = async () => {
    const wallet = wallets[delegateWalletIdx];
    if (!wallet || !delegateTarget) return;
    const sats = Math.floor(parseFloat(delegateAmount) * 100_000_000);
    if (isNaN(sats) || sats < 100 * 100_000_000) {
      setDelegateError('Minimum delegation is 100 UDAG');
      return;
    }
    setDelegateLoading(true);
    setDelegateError('');
    setDelegateSuccess('');
    try {
      await postDelegate({ secret_key: wallet.secret_key, validator: delegateTarget, amount: sats });
      setDelegateSuccess('Delegation submitted');
      setDelegateTarget(null);
      setDelegateAmount('');
      refresh();
    } catch (e: unknown) {
      setDelegateError(e instanceof Error ? e.message : 'Delegation failed');
    } finally {
      setDelegateLoading(false);
    }
  };

  const handleUndelegate = async (walletAddr: string) => {
    const wallet = wallets.find(w => w.address === walletAddr);
    if (!wallet) return;
    setUndelegateLoading(walletAddr);
    setUndelegateError('');
    try {
      await postUndelegate({ secret_key: wallet.secret_key });
      refresh();
    } catch (e: unknown) {
      setUndelegateError(e instanceof Error ? e.message : 'Undelegate failed');
    } finally {
      setUndelegateLoading('');
    }
  };

  return (
    <div className="space-y-6 animate-page-enter">
      <div>
        <h1 className="text-2xl font-bold text-white">Earn Rewards</h1>
        <p className="text-sm text-dag-muted mt-1">Delegate UDAG to validators and earn passive staking rewards</p>
      </div>

      {/* Stats row */}
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

      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        {/* Left column: delegate + info */}
        <div className="space-y-4">
          {/* Delegate form — shown when a validator is selected */}
          {delegateTarget ? (
            <Card title={`Delegate to ${shortAddr(delegateTarget)}`}>
              <div className="space-y-3">
                <label className="block">
                  <span className="text-sm text-dag-muted">Wallet</span>
                  <select
                    value={delegateWalletIdx}
                    onChange={e => setDelegateWalletIdx(Number(e.target.value))}
                    className="mt-1 block w-full rounded bg-dag-surface border border-dag-border px-3 py-2 text-sm text-white"
                  >
                    {wallets.map((w, i) => (
                      <option key={i} value={i}>{w.name} ({shortAddr(w.address)})</option>
                    ))}
                  </select>
                </label>
                <label className="block">
                  <span className="text-sm text-dag-muted">Amount (UDAG, min 100)</span>
                  <input
                    type="number"
                    min="100"
                    step="1"
                    value={delegateAmount}
                    onChange={e => setDelegateAmount(e.target.value)}
                    placeholder="100"
                    className="mt-1 block w-full rounded bg-dag-surface border border-dag-border px-3 py-2 text-sm text-white"
                  />
                </label>
                {delegateError && <p className="text-sm text-dag-red">{delegateError}</p>}
                {delegateSuccess && <p className="text-sm text-dag-green">{delegateSuccess}</p>}
                <div className="flex gap-2">
                  <button
                    onClick={handleDelegate}
                    disabled={delegateLoading}
                    className="flex-1 py-2 rounded bg-dag-accent text-white font-medium text-sm hover:bg-dag-accent/90 disabled:opacity-50"
                  >
                    {delegateLoading ? 'Submitting...' : 'Delegate'}
                  </button>
                  <button
                    onClick={() => { setDelegateTarget(null); setDelegateError(''); setDelegateSuccess(''); }}
                    className="px-4 py-2 rounded bg-dag-surface border border-dag-border text-dag-muted text-sm hover:text-white"
                  >
                    Cancel
                  </button>
                </div>
              </div>
            </Card>
          ) : (
            /* How it works — shown when no validator selected */
            <Card title="How Delegation Works">
              <div className="space-y-4">
                <div className="flex items-start gap-3">
                  <div className="w-8 h-8 rounded-lg bg-dag-accent/10 flex items-center justify-center flex-shrink-0 mt-0.5">
                    <Users className="w-4 h-4 text-dag-accent" />
                  </div>
                  <div>
                    <p className="text-sm text-white font-medium">Choose a validator</p>
                    <p className="text-xs text-dag-muted mt-0.5">Select from the list and click "Delegate". Your UDAG backs their stake.</p>
                  </div>
                </div>
                <div className="flex items-start gap-3">
                  <div className="w-8 h-8 rounded-lg bg-dag-green/10 flex items-center justify-center flex-shrink-0 mt-0.5">
                    <Coins className="w-4 h-4 text-dag-green" />
                  </div>
                  <div>
                    <p className="text-sm text-white font-medium">Earn passive rewards</p>
                    <p className="text-xs text-dag-muted mt-0.5">Rewards are proportional to your delegation, minus the validator's commission.</p>
                  </div>
                </div>
                <div className="flex items-start gap-3">
                  <div className="w-8 h-8 rounded-lg bg-dag-yellow/10 flex items-center justify-center flex-shrink-0 mt-0.5">
                    <Info className="w-4 h-4 text-dag-yellow" />
                  </div>
                  <div>
                    <p className="text-sm text-white font-medium">Minimum 100 UDAG</p>
                    <p className="text-xs text-dag-muted mt-0.5">Undelegating has a ~2.8 hour cooldown before funds are returned.</p>
                  </div>
                </div>
              </div>
            </Card>
          )}

          {/* Run a validator info */}
          <div className="rounded-lg bg-dag-surface/50 border border-dag-border/50 p-4">
            <div className="flex items-start gap-3">
              <Server className="w-4 h-4 text-dag-muted mt-0.5 flex-shrink-0" />
              <div>
                <p className="text-xs text-dag-muted">
                  <span className="text-slate-300 font-medium">Want to run a validator?</span> Staking requires running a node with the CLI.
                  Use <code className="text-dag-accent text-[10px]">--auto-stake</code> when starting your node.
                </p>
                <a href="/docs.html#staking" className="text-xs text-dag-accent hover:underline mt-1 inline-flex items-center gap-1">
                  Validator setup guide <ExternalLink className="w-3 h-3" />
                </a>
              </div>
            </div>
          </div>
        </div>

        {/* Right columns: delegations + validators */}
        <div className="lg:col-span-2 space-y-6">
          {/* Your validator stake (read-only) */}
          {unlocked && stakes.length > 0 && (
            <Card title="Your Validator Stake">
              <div className="space-y-3">
                {stakes.map(s => (
                  <div key={s.address} className="rounded bg-dag-surface border border-dag-border p-3">
                    <div className="flex items-center justify-between mb-2">
                      <span className="text-sm font-medium text-white">{s.name}</span>
                      {s.is_active_validator ? (
                        <span className="text-xs px-2 py-0.5 rounded bg-dag-green/20 text-dag-green border border-dag-green/40">
                          Active Validator
                        </span>
                      ) : (
                        <span className="text-xs px-2 py-0.5 rounded bg-slate-700 text-dag-muted">
                          {s.unlock_at_round ? 'Unstaking' : 'Observer'}
                        </span>
                      )}
                    </div>
                    <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-2 text-sm">
                      <div>
                        <span className="text-dag-muted block text-xs">Staked</span>
                        <span className="text-white">{formatUdag(s.staked)} UDAG</span>
                      </div>
                      <div>
                        <span className="text-dag-muted block text-xs">Effective Stake</span>
                        <span className="text-white">{formatUdag(s.effective_stake ?? 0)} UDAG</span>
                      </div>
                      <div>
                        <span className="text-dag-muted block text-xs">Commission</span>
                        <span className="text-white">{s.commission_percent ?? 10}%</span>
                      </div>
                      <div>
                        <span className="text-dag-muted block text-xs">Status</span>
                        <span className={s.unlock_at_round ? 'text-dag-yellow' : 'text-dag-green'}>
                          {s.unlock_at_round ? `Cooldown (round ${s.unlock_at_round})` : 'Active'}
                        </span>
                      </div>
                    </div>
                    <p className="text-[10px] text-dag-muted mt-2">Manage your validator stake via the CLI — <code className="text-dag-accent">ultradag-node --unstake</code></p>
                  </div>
                ))}
              </div>
            </Card>
          )}

          {/* Your delegations */}
          {unlocked && delegations.length > 0 && (
            <Card title="Your Delegations">
              <div className="space-y-3">
                {delegations.map(d => (
                  <div key={d.address} className="rounded bg-dag-surface border border-dag-border p-3">
                    <div className="flex items-center justify-between mb-2">
                      <span className="text-sm font-medium text-white">{d.name}</span>
                      <span className={`text-xs ${d.is_undelegating ? 'text-dag-yellow' : 'text-dag-accent'}`}>
                        {d.is_undelegating ? `Undelegating (round ${d.unlock_at_round})` : 'Earning rewards'}
                      </span>
                    </div>
                    <div className="grid grid-cols-2 gap-2 text-sm">
                      <div>
                        <span className="text-dag-muted block text-xs">Delegated</span>
                        <span className="text-white">{formatUdag(d.delegated)} UDAG</span>
                      </div>
                      <div>
                        <span className="text-dag-muted block text-xs">Validator</span>
                        <span className="text-white font-mono text-xs">{shortAddr(d.validator)}</span>
                      </div>
                    </div>
                    {!d.is_undelegating && (
                      <button
                        onClick={() => handleUndelegate(d.address)}
                        disabled={undelegateLoading === d.address}
                        className="mt-2 text-sm px-3 py-1 rounded bg-dag-red/20 text-dag-red border border-dag-red/40 hover:bg-dag-red/30 disabled:opacity-50"
                      >
                        {undelegateLoading === d.address ? 'Submitting...' : 'Undelegate'}
                      </button>
                    )}
                  </div>
                ))}
                {undelegateError && <p className="text-sm text-dag-red mt-2">{undelegateError}</p>}
              </div>
            </Card>
          )}

          {/* Validator list */}
          <Card title={`Validators (${validators.length})`}>
            {loading ? (
              <p className="text-dag-muted text-sm">Loading validators...</p>
            ) : validators.length === 0 ? (
              <div className="flex flex-col items-center justify-center py-12 text-center">
                <div className="w-14 h-14 rounded-2xl bg-purple-500/10 border border-purple-500/20 flex items-center justify-center mb-4">
                  <Coins className="w-7 h-7 text-purple-400" />
                </div>
                <h4 className="text-white font-medium mb-1">No validators yet</h4>
                <p className="text-sm text-dag-muted max-w-xs">Validators join by running a node and staking via the CLI. Once active, you can delegate to them here.</p>
              </div>
            ) : (
              <>
                <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
                  {pageValidators.map(v => (
                    <ValidatorCard
                      key={v.address}
                      address={v.address}
                      effective_stake={v.effective_stake}
                      delegator_count={v.delegator_count}
                      commission_percent={v.commission_percent}
                      is_active={v.is_active || false}
                      onDelegate={unlocked && wallets.length > 0 ? () => setDelegateTarget(v.address) : undefined}
                    />
                  ))}
                </div>
                <Pagination currentPage={page} totalPages={totalPages} onPageChange={setPage} />
              </>
            )}
          </Card>
        </div>
      </div>
    </div>
  );
}
