import { useEffect, useState } from 'react';
import { useParams, Link } from 'react-router-dom';
import { ChevronLeft, Wallet, Shield, Users, Crown, ChevronDown, ChevronUp } from 'lucide-react';
import { getBalance, getStake, getDelegation, getCouncil, connectToNode, isConnected, formatUdag } from '../lib/api.ts';
import { CopyButton } from '../components/shared/CopyButton.tsx';
import { Badge } from '../components/shared/Badge.tsx';
import { DisplayIdentity } from '../components/shared/DisplayIdentity.tsx';

function AddressHeader({ address, registeredName, isSmartAccount }: { address: string; registeredName: string | null; isSmartAccount: boolean }) {
  return (
    <div className="bg-slate-800/50 border border-slate-700 rounded-lg p-3 space-y-2">
      {registeredName && (
        <div className="flex items-center gap-2 mb-1">
          <span className="text-dag-accent font-bold text-lg">@{registeredName}</span>
          <span className="text-[10px] text-slate-500 uppercase tracking-wider">ULTRA ID</span>
          {isSmartAccount && <span className="text-xs bg-purple-500/20 text-purple-400 px-2 py-0.5 rounded-full">SmartAccount</span>}
        </div>
      )}

      <DisplayIdentity address={address} advanced copyable knownName={registeredName} size="sm" />
    </div>
  );
}

export function AddressPage() {
  const { address } = useParams<{ address: string }>();
  const [balance, setBalance] = useState<Record<string, unknown> | null>(null);
  const [stake, setStake] = useState<Record<string, unknown> | null>(null);
  const [delegation, setDelegation] = useState<Record<string, unknown> | null>(null);
  const [councilMember, setCouncilMember] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');

  useEffect(() => {
    if (!address) return;

    const fetchAll = async () => {
      setLoading(true);
      try {
        if (!isConnected()) await connectToNode();

        const results = await Promise.allSettled([
          getBalance(address),
          getStake(address),
          getDelegation(address),
          getCouncil(),
        ]);

        if (results[0].status === 'fulfilled') setBalance(results[0].value);
        else setError('Address not found');

        if (results[1].status === 'fulfilled') setStake(results[1].value);
        if (results[2].status === 'fulfilled') setDelegation(results[2].value);
        if (results[3].status === 'fulfilled') {
          const council = results[3].value;
          const members = (council?.members ?? []) as Array<Record<string, unknown>>;
          const member = members.find((m) => String(m.address) === address);
          if (member) setCouncilMember(String(member.category ?? 'Member'));
        }
      } catch (e) {
        setError(e instanceof Error ? e.message : 'Failed to fetch address info');
      } finally {
        setLoading(false);
      }
    };

    fetchAll();
  }, [address]);

  if (loading) return <div className="text-slate-500 py-8 text-center">Loading address...</div>;

  const balanceSats = Number(balance?.balance ?? 0);
  const balanceDelegated = Number(balance?.delegated ?? 0);
  const nonce = Number(balance?.nonce ?? 0);
  const registeredName = balance?.name ? String(balance.name) : null;
  const isSmartAccount = balance?.is_smart_account === true;
  const stakedSats = Number(stake?.staked ?? 0);
  const isActiveValidator = stake?.is_active_validator === true;
  const effectiveStake = Number(stake?.effective_stake ?? 0);
  const commission = stake?.commission_percent;
  const delegatedSats = Number(delegation?.delegated ?? 0);
  const delegationValidator = delegation?.validator ? String(delegation.validator) : null;

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Link to="/explorer" className="text-slate-400 hover:text-slate-200">
          <ChevronLeft className="w-5 h-5" />
        </Link>
        <h1 className="text-xl font-bold text-white">Address</h1>
      </div>

      <AddressHeader address={address ?? ''} registeredName={registeredName} isSmartAccount={isSmartAccount} />

      {error && <div className="bg-red-500/10 border border-red-500/30 rounded-lg p-3 text-sm text-red-400">{error}</div>}

      <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
        {/* Balance card */}
        <div className="bg-slate-800/50 border border-slate-700 rounded-lg p-4">
          <div className="flex items-center gap-2 mb-3">
            <Wallet className="w-4 h-4 text-blue-400" />
            <h2 className="text-sm font-semibold text-slate-200">Balance</h2>
          </div>
          <p className="text-2xl font-bold text-white font-mono">
            {formatUdag(balanceSats)} <span className="text-sm text-slate-400">UDAG</span>
          </p>
          <p className="text-xs text-slate-500 mt-1">{balanceSats.toLocaleString()} sats</p>
          <div className="mt-3 pt-3 border-t border-slate-700 space-y-2">
            <div className="flex justify-between text-sm">
              <span className="text-slate-500">Nonce</span>
              <span className="font-mono text-slate-300">{nonce}</span>
            </div>
            {balanceDelegated > 0 && (
              <div className="flex justify-between text-sm">
                <span className="text-slate-500">Delegated</span>
                <span className="font-mono text-slate-300">{formatUdag(balanceDelegated)} UDAG</span>
              </div>
            )}
          </div>
        </div>

        {/* Staking card */}
        <div className="bg-slate-800/50 border border-slate-700 rounded-lg p-4">
          <div className="flex items-center gap-2 mb-3">
            <Shield className="w-4 h-4 text-purple-400" />
            <h2 className="text-sm font-semibold text-slate-200">Staking</h2>
            {isActiveValidator && <Badge label="Active Validator" variant="green" />}
          </div>
          {stakedSats > 0 ? (
            <>
              <p className="text-2xl font-bold text-white font-mono">
                {formatUdag(stakedSats)} <span className="text-sm text-slate-400">UDAG</span>
              </p>
              <p className="text-xs text-slate-500 mt-1">{stakedSats.toLocaleString()} sats staked</p>
              <div className="mt-3 pt-3 border-t border-slate-700 space-y-2">
                {effectiveStake > 0 && (
                  <div className="flex justify-between text-sm">
                    <span className="text-slate-500">Effective Stake</span>
                    <span className="font-mono text-slate-300">{formatUdag(effectiveStake)} UDAG</span>
                  </div>
                )}
                {commission != null && (
                  <div className="flex justify-between text-sm">
                    <span className="text-slate-500">Commission</span>
                    <span className="font-mono text-slate-300">{String(commission)}%</span>
                  </div>
                )}
                {stake?.unlock_at_round != null && (
                  <div className="flex justify-between text-sm">
                    <span className="text-slate-500">Unstaking at</span>
                    <span className="font-mono text-yellow-400">Round {Number(stake.unlock_at_round).toLocaleString()}</span>
                  </div>
                )}
              </div>
            </>
          ) : (
            <p className="text-slate-500 text-sm">Not staking</p>
          )}
        </div>

        {/* Delegation card */}
        <div className="bg-slate-800/50 border border-slate-700 rounded-lg p-4">
          <div className="flex items-center gap-2 mb-3">
            <Users className="w-4 h-4 text-green-400" />
            <h2 className="text-sm font-semibold text-slate-200">Delegation</h2>
          </div>
          {delegatedSats > 0 ? (
            <>
              <p className="text-2xl font-bold text-white font-mono">
                {formatUdag(delegatedSats)} <span className="text-sm text-slate-400">UDAG</span>
              </p>
              {delegationValidator && (
                <div className="mt-3 pt-3 border-t border-slate-700">
                  <div className="flex justify-between items-center text-sm">
                    <span className="text-slate-500">Delegated to</span>
                    <DisplayIdentity address={delegationValidator} link size="xs" />
                  </div>
                  {delegation?.unlock_at_round != null && (
                    <div className="flex justify-between text-sm mt-1">
                      <span className="text-slate-500">Undelegating at</span>
                      <span className="font-mono text-yellow-400">Round {Number(delegation.unlock_at_round).toLocaleString()}</span>
                    </div>
                  )}
                </div>
              )}
            </>
          ) : (
            <p className="text-slate-500 text-sm">No delegation</p>
          )}
        </div>

        {/* Council card */}
        <div className="bg-slate-800/50 border border-slate-700 rounded-lg p-4">
          <div className="flex items-center gap-2 mb-3">
            <Crown className="w-4 h-4 text-yellow-400" />
            <h2 className="text-sm font-semibold text-slate-200">Council</h2>
          </div>
          {councilMember ? (
            <div>
              <Badge label="Council Member" variant="purple" />
              <p className="text-sm text-slate-300 mt-2">Category: <span className="text-white">{councilMember}</span></p>
            </div>
          ) : (
            <p className="text-slate-500 text-sm">Not a council member</p>
          )}
        </div>
      </div>
    </div>
  );
}
