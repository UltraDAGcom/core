import { useState, useEffect, useCallback } from 'react';
import { getBalance, getStake, getDelegation, getSmartAccount, getNameInfo, getStreamsSender, getStreamsRecipient, getProposals, getProposal } from '../lib/api';
import { computeBadges, type Badge, type BadgeInput } from '../lib/badges';
import { fetchBountyIssues, parseBounty } from '../lib/github';

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
  isPerpetualName: boolean;
}

export function useProfile(address: string | undefined) {
  const [profile, setProfile] = useState<ProfileData | null>(null);
  const [badges, setBadges] = useState<Badge[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchProfile = useCallback(async () => {
    if (!address) { setLoading(false); return; }
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
      const streamsOutData = streamsOutRes.status === 'fulfilled' ? streamsOutRes.value : null;
      const streamsInData = streamsInRes.status === 'fulfilled' ? streamsInRes.value : null;
      const streamsOut = Array.isArray(streamsOutData?.streams) ? streamsOutData.streams : (Array.isArray(streamsOutData) ? streamsOutData : []);
      const streamsIn = Array.isArray(streamsInData?.streams) ? streamsInData.streams : (Array.isArray(streamsInData) ? streamsInData : []);
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

      // Count paid bounties whose creator_address matches this profile (cached 2min in github.ts)
      let paidBountiesCount = 0;
      try {
        const { issues } = await fetchBountyIssues();
        const addrLower = address.toLowerCase();
        for (const issue of issues) {
          const parsed = parseBounty(issue);
          if (parsed.status === 'paid' && parsed.creatorAddress && parsed.creatorAddress.toLowerCase() === addrLower) {
            paidBountiesCount++;
          }
        }
      } catch { /* GitHub rate-limited or offline — leave at 0 */ }

      // Count proposals this address voted on (check up to 10 recent proposals)
      let votedCount = 0;
      const recentProposals = proposals.slice(0, 10);
      for (const p of recentProposals) {
        try {
          const detail = await getProposal(p.id);
          if (detail?.voters && Array.isArray(detail.voters)) {
            if (detail.voters.some((v: Record<string, unknown>) => String(v.address) === address)) {
              votedCount++;
            }
          }
        } catch { /* skip */ }
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
        isPerpetualName: nameInfo?.is_perpetual === true,
      };

      setProfile(data);

      // Compute badges
      const badgeInput: BadgeInput = {
        balance: { is_council_member: data.isCouncil, staked: data.staked, delegated: data.delegated },
        stake: { is_active_validator: data.isValidator, effective_stake: data.effectiveStake, delegator_count: data.delegatorCount, commission_percent: data.commissionPercent ?? undefined },
        delegation: { delegated: data.delegated, validator: data.delegatedTo ?? undefined },
        smartAccount: { created_at_round: data.createdAtRound ?? undefined, has_recovery: data.hasRecovery, has_policy: data.hasPolicy },
        votedOnProposals: data.votedOnProposals,
        paidBounties: paidBountiesCount,
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
