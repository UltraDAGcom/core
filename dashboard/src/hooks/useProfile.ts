import { useState, useEffect, useCallback } from 'react';
import { getBalance, getStake, getDelegation, getSmartAccount, getNameInfo, getStreamsSender, getStreamsRecipient, getProposals } from '../lib/api';
import { computeBadges, type Badge, type BadgeInput } from '../lib/badges';

export interface ProfileData {
  address: string;
  name: string | null;
  balance: number;
  staked: number;
  effectiveStake: number;
  delegated: number;
  delegatedTo: string | null;
  isValidator: boolean;
  isCouncil: boolean;
  isSmartAccount: boolean;
  createdAtRound: number | null;
  currentRound: number | null;
  commissionPercent: number | null;
  delegatorCount: number;
  hasRecovery: boolean;
  hasPolicy: boolean;
  keyCount: number;
  streamsSentCount: number;
  streamsReceivedCount: number;
  votedOnProposals: number;
  bio: string | null;
  website: string | null;
  github: string | null;
  twitter: string | null;
  externalAddresses: Array<[string, string]>;
  metadata: Array<[string, string]>;
  expiryRound: number | null;
}

export function useProfile(address: string | undefined) {
  const [profile, setProfile] = useState<ProfileData | null>(null);
  const [badges, setBadges] = useState<Badge[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchProfile = useCallback(async () => {
    if (!address) return;
    setLoading(true);
    setError(null);

    try {
      // Fetch all data sources in parallel
      const [balRes, stakeRes, delegRes, saRes, streamsOutRes, streamsInRes, proposalsRes] = await Promise.allSettled([
        getBalance(address),
        getStake(address),
        getDelegation(address),
        getSmartAccount(address),
        getStreamsSender(address),
        getStreamsRecipient(address),
        getProposals(),
      ]);

      const bal = balRes.status === 'fulfilled' ? balRes.value : null;
      const stake = stakeRes.status === 'fulfilled' ? stakeRes.value : null;
      const deleg = delegRes.status === 'fulfilled' ? delegRes.value : null;
      const sa = saRes.status === 'fulfilled' ? saRes.value : null;
      const streamsOut = streamsOutRes.status === 'fulfilled' ? (Array.isArray(streamsOutRes.value) ? streamsOutRes.value : []) : [];
      const streamsIn = streamsInRes.status === 'fulfilled' ? (Array.isArray(streamsInRes.value) ? streamsInRes.value : []) : [];
      const proposals = proposalsRes.status === 'fulfilled' ? (Array.isArray(proposalsRes.value) ? proposalsRes.value : []) : [];

      if (!bal) { setError('Address not found'); setLoading(false); return; }

      // Resolve name info for profile metadata
      let nameInfo: Record<string, unknown> | null = null;
      if (bal.name) {
        try { nameInfo = await getNameInfo(bal.name); } catch { /* no profile */ }
      }

      const profileMeta = (nameInfo?.profile as { external_addresses?: Array<[string, string]>; metadata?: Array<[string, string]> }) ?? {};
      const metadata = profileMeta.metadata ?? [];
      const getMeta = (key: string) => metadata.find(([k]: [string, string]) => k === key)?.[1] ?? null;

      // Count proposals this address voted on
      let votedCount = 0;
      for (const p of proposals) {
        if (p.voters && Array.isArray(p.voters)) {
          if (p.voters.some((v: Record<string, unknown>) => String(v.address) === address)) {
            votedCount++;
          }
        }
      }

      const data: ProfileData = {
        address,
        name: bal.name ?? null,
        balance: bal.balance ?? 0,
        staked: stake?.staked ?? 0,
        effectiveStake: stake?.effective_stake ?? 0,
        delegated: deleg?.delegated ?? 0,
        delegatedTo: deleg?.validator ?? null,
        isValidator: stake?.is_active_validator === true,
        isCouncil: bal.is_council_member === true,
        isSmartAccount: bal.is_smart_account === true,
        createdAtRound: sa?.created_at_round ?? null,
        currentRound: null, // filled below
        commissionPercent: stake?.commission_percent ?? null,
        delegatorCount: stake?.delegator_count ?? 0,
        hasRecovery: sa?.has_recovery === true,
        hasPolicy: sa?.has_policy === true,
        keyCount: sa?.authorized_keys?.length ?? 0,
        streamsSentCount: streamsOut.length,
        streamsReceivedCount: streamsIn.length,
        votedOnProposals: votedCount,
        bio: getMeta('bio'),
        website: getMeta('website'),
        github: getMeta('github'),
        twitter: getMeta('twitter'),
        externalAddresses: profileMeta.external_addresses ?? [],
        metadata,
        expiryRound: nameInfo?.expiry_round ? Number(nameInfo.expiry_round) : null,
      };

      setProfile(data);

      // Compute badges
      const badgeInput: BadgeInput = {
        balance: { is_council_member: data.isCouncil, staked: data.staked, delegated: data.delegated },
        stake: { is_active_validator: data.isValidator, effective_stake: data.effectiveStake, delegator_count: data.delegatorCount, commission_percent: data.commissionPercent ?? undefined },
        delegation: { delegated: data.delegated, validator: data.delegatedTo ?? undefined },
        smartAccount: { created_at_round: data.createdAtRound ?? undefined, has_recovery: data.hasRecovery, has_policy: data.hasPolicy },
        votedOnProposals: data.votedOnProposals,
        paidBounties: 0, // TODO: check GitHub bounties
        streamsSent: data.streamsSentCount,
        streamsReceived: data.streamsReceivedCount,
      };
      setBadges(computeBadges(badgeInput));
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to load profile');
    } finally {
      setLoading(false);
    }
  }, [address]);

  useEffect(() => { fetchProfile(); }, [fetchProfile]);

  // Refetch on network switch
  useEffect(() => {
    const handler = () => fetchProfile();
    window.addEventListener('ultradag-network-switch', handler);
    return () => window.removeEventListener('ultradag-network-switch', handler);
  }, [fetchProfile]);

  return { profile, badges, loading, error, refresh: fetchProfile };
}
