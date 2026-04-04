import { useEffect, useState } from 'react';
import { useParams } from 'react-router-dom';
import { Wallet, Shield, Users, Crown } from 'lucide-react';
import { getBalance, getStake, getDelegation, getCouncil, connectToNode, isConnected, formatUdag } from '../lib/api.ts';
import { Badge } from '../components/shared/Badge.tsx';
import { DisplayIdentity } from '../components/shared/DisplayIdentity.tsx';
import { PageHeader } from '../components/shared/PageHeader.tsx';
import { useIsMobile } from '../hooks/useIsMobile';

function AddressHeader({ address, registeredName, isSmartAccount }: { address: string; registeredName: string | null; isSmartAccount: boolean }) {
  return (
    <div style={{ background: 'var(--dag-card)', border: '1px solid var(--dag-border)', borderRadius: 10, padding: 12, display: 'flex', flexDirection: 'column', gap: 8 }}>
      {registeredName && (
        <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 4 }}>
          <span style={{ color: '#00E0C4', fontWeight: 700, fontSize: 18 }}>@{registeredName}</span>
          <span style={{ fontSize: 10, color: 'var(--dag-text-faint)', textTransform: 'uppercase', letterSpacing: '0.05em' }}>ULTRA ID</span>
          {isSmartAccount && <span style={{ fontSize: 10, background: 'rgba(168,85,247,0.12)', color: '#A855F7', padding: '2px 8px', borderRadius: 9999 }}>SmartAccount</span>}
        </div>
      )}

      <DisplayIdentity address={address} advanced copyable knownName={registeredName} size="sm" />
    </div>
  );
}

export function AddressPage() {
  const { address } = useParams<{ address: string }>();
  const m = useIsMobile();
  const [balance, setBalance] = useState<Record<string, unknown> | null>(null);
  const [stake, setStake] = useState<Record<string, unknown> | null>(null);
  const [delegation, setDelegation] = useState<Record<string, unknown> | null>(null);
  const [councilMember, setCouncilMember] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');
  const [switchCount, setSwitchCount] = useState(0);

  useEffect(() => {
    const handler = () => setSwitchCount(n => n + 1);
    window.addEventListener('ultradag-network-switch', handler);
    return () => window.removeEventListener('ultradag-network-switch', handler);
  }, []);

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
          const member = members.find((mem) => String(mem.address) === address);
          if (member) setCouncilMember(String(member.category ?? 'Member'));
        }
      } catch (e) {
        setError(e instanceof Error ? e.message : 'Failed to fetch address info');
      } finally {
        setLoading(false);
      }
    };

    fetchAll();
  }, [address, switchCount]);

  if (loading) return <div style={{ color: 'var(--dag-text-faint)', padding: '32px 0', textAlign: 'center' }}>Loading address...</div>;

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
    <div style={{ padding: m ? '12px 14px' : '18px 26px', fontFamily: "'DM Sans',sans-serif" }}>
      <PageHeader title={registeredName ? `@${registeredName}` : 'Address'} subtitle="Address details" />
      <div style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>

      <AddressHeader address={address ?? ''} registeredName={registeredName} isSmartAccount={isSmartAccount} />

      {error && <div style={{ background: 'rgba(239,68,68,0.06)', border: '1px solid rgba(239,68,68,0.15)', borderRadius: 10, padding: 12, fontSize: 12, color: '#EF4444' }}>{error}</div>}

      <div style={{ display: 'grid', gridTemplateColumns: m ? '1fr' : '1fr 1fr', gap: 16 }}>
        {/* Balance card */}
        <div style={{ background: 'var(--dag-card)', border: '1px solid var(--dag-border)', borderRadius: 10, padding: 16 }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 12 }}>
            <Wallet style={{ width: 16, height: 16, color: '#00E0C4' }} />
            <h2 style={{ fontSize: 12, fontWeight: 600, color: 'var(--dag-text)' }}>Balance</h2>
          </div>
          <p style={{ fontSize: 22, fontWeight: 700, color: 'var(--dag-text)', fontFamily: "'DM Mono',monospace" }}>
            {formatUdag(balanceSats)} <span style={{ fontSize: 12, color: 'var(--dag-text-muted)' }}>UDAG</span>
          </p>
          <p style={{ fontSize: 10, color: 'var(--dag-text-faint)', marginTop: 4 }}>{balanceSats.toLocaleString()} sats</p>
          <div style={{ marginTop: 12, paddingTop: 12, borderTop: '1px solid var(--dag-border)', display: 'flex', flexDirection: 'column', gap: 8 }}>
            <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: 12 }}>
              <span style={{ color: 'var(--dag-text-faint)' }}>Nonce</span>
              <span style={{ fontFamily: "'DM Mono',monospace", color: 'var(--dag-text-secondary)' }}>{nonce}</span>
            </div>
            {balanceDelegated > 0 && (
              <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: 12 }}>
                <span style={{ color: 'var(--dag-text-faint)' }}>Delegated</span>
                <span style={{ fontFamily: "'DM Mono',monospace", color: 'var(--dag-text-secondary)' }}>{formatUdag(balanceDelegated)} UDAG</span>
              </div>
            )}
          </div>
        </div>

        {/* Staking card */}
        <div style={{ background: 'var(--dag-card)', border: '1px solid var(--dag-border)', borderRadius: 10, padding: 16 }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 12 }}>
            <Shield style={{ width: 16, height: 16, color: '#A855F7' }} />
            <h2 style={{ fontSize: 12, fontWeight: 600, color: 'var(--dag-text)' }}>Staking</h2>
            {isActiveValidator && <Badge label="Active Validator" variant="green" />}
          </div>
          {stakedSats > 0 ? (
            <>
              <p style={{ fontSize: 22, fontWeight: 700, color: 'var(--dag-text)', fontFamily: "'DM Mono',monospace" }}>
                {formatUdag(stakedSats)} <span style={{ fontSize: 12, color: 'var(--dag-text-muted)' }}>UDAG</span>
              </p>
              <p style={{ fontSize: 10, color: 'var(--dag-text-faint)', marginTop: 4 }}>{stakedSats.toLocaleString()} sats staked</p>
              <div style={{ marginTop: 12, paddingTop: 12, borderTop: '1px solid var(--dag-border)', display: 'flex', flexDirection: 'column', gap: 8 }}>
                {effectiveStake > 0 && (
                  <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: 12 }}>
                    <span style={{ color: 'var(--dag-text-faint)' }}>Effective Stake</span>
                    <span style={{ fontFamily: "'DM Mono',monospace", color: 'var(--dag-text-secondary)' }}>{formatUdag(effectiveStake)} UDAG</span>
                  </div>
                )}
                {commission != null && (
                  <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: 12 }}>
                    <span style={{ color: 'var(--dag-text-faint)' }}>Commission</span>
                    <span style={{ fontFamily: "'DM Mono',monospace", color: 'var(--dag-text-secondary)' }}>{String(commission)}%</span>
                  </div>
                )}
                {stake?.unlock_at_round != null && (
                  <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: 12 }}>
                    <span style={{ color: 'var(--dag-text-faint)' }}>Unstaking at</span>
                    <span style={{ fontFamily: "'DM Mono',monospace", color: '#FFB800' }}>Round {Number(stake.unlock_at_round).toLocaleString()}</span>
                  </div>
                )}
              </div>
            </>
          ) : (
            <p style={{ color: 'var(--dag-text-faint)', fontSize: 12 }}>Not staking</p>
          )}
        </div>

        {/* Delegation card */}
        <div style={{ background: 'var(--dag-card)', border: '1px solid var(--dag-border)', borderRadius: 10, padding: 16 }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 12 }}>
            <Users style={{ width: 16, height: 16, color: '#00E0C4' }} />
            <h2 style={{ fontSize: 12, fontWeight: 600, color: 'var(--dag-text)' }}>Delegation</h2>
          </div>
          {delegatedSats > 0 ? (
            <>
              <p style={{ fontSize: 22, fontWeight: 700, color: 'var(--dag-text)', fontFamily: "'DM Mono',monospace" }}>
                {formatUdag(delegatedSats)} <span style={{ fontSize: 12, color: 'var(--dag-text-muted)' }}>UDAG</span>
              </p>
              {delegationValidator && (
                <div style={{ marginTop: 12, paddingTop: 12, borderTop: '1px solid var(--dag-border)' }}>
                  <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', fontSize: 12 }}>
                    <span style={{ color: 'var(--dag-text-faint)' }}>Delegated to</span>
                    <DisplayIdentity address={delegationValidator} link size="xs" />
                  </div>
                  {delegation?.unlock_at_round != null && (
                    <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: 12, marginTop: 4 }}>
                      <span style={{ color: 'var(--dag-text-faint)' }}>Undelegating at</span>
                      <span style={{ fontFamily: "'DM Mono',monospace", color: '#FFB800' }}>Round {Number(delegation.unlock_at_round).toLocaleString()}</span>
                    </div>
                  )}
                </div>
              )}
            </>
          ) : (
            <p style={{ color: 'var(--dag-text-faint)', fontSize: 12 }}>No delegation</p>
          )}
        </div>

        {/* Council card */}
        <div style={{ background: 'var(--dag-card)', border: '1px solid var(--dag-border)', borderRadius: 10, padding: 16 }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 12 }}>
            <Crown style={{ width: 16, height: 16, color: '#FFB800' }} />
            <h2 style={{ fontSize: 12, fontWeight: 600, color: 'var(--dag-text)' }}>Council</h2>
          </div>
          {councilMember ? (
            <div>
              <Badge label="Council Member" variant="purple" />
              <p style={{ fontSize: 12, color: 'var(--dag-text-secondary)', marginTop: 8 }}>Category: <span style={{ color: 'var(--dag-text)' }}>{councilMember}</span></p>
            </div>
          ) : (
            <p style={{ color: 'var(--dag-text-faint)', fontSize: 12 }}>Not a council member</p>
          )}
        </div>
      </div>
      </div>
    </div>
  );
}
