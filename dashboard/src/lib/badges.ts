/**
 * Badge computation for ULTRA ID profiles.
 * All badges are derived from on-chain data — no self-declaration.
 */

export interface Badge {
  id: string;
  label: string;
  icon: string;
  color: string;
  description: string;
  earned: boolean;
}

export interface BadgeInput {
  balance?: { is_council_member?: boolean; staked?: number; delegated?: number };
  stake?: { is_active_validator?: boolean; effective_stake?: number; delegator_count?: number; commission_percent?: number };
  delegation?: { delegated?: number; validator?: string };
  smartAccount?: { created_at_round?: number; has_recovery?: boolean; has_policy?: boolean };
  votedOnProposals?: number;
  paidBounties?: number;
  streamsSent?: number;
  streamsReceived?: number;
}

const PIONEER_ROUND_THRESHOLD = 50_000; // ~2.9 days at 5s rounds

export function computeBadges(data: BadgeInput): Badge[] {
  const staked = data.stake?.effective_stake ?? data.balance?.staked ?? 0;
  const isValidator = data.stake?.is_active_validator === true;
  const isCouncil = data.balance?.is_council_member === true;
  const hasDelegated = (data.delegation?.delegated ?? 0) > 0;
  const hasRecovery = data.smartAccount?.has_recovery === true;
  const hasPolicy = data.smartAccount?.has_policy === true;
  const createdAt = data.smartAccount?.created_at_round;
  const isPioneer = createdAt != null && createdAt < PIONEER_ROUND_THRESHOLD;

  return [
    {
      id: 'validator',
      label: 'Validator',
      icon: '⬡',
      color: '#00E0C4',
      description: 'Active network validator',
      earned: isValidator,
    },
    {
      id: 'council',
      label: 'Council',
      icon: '♛',
      color: '#A855F7',
      description: 'Council of 21 member',
      earned: isCouncil,
    },
    {
      id: 'staker',
      label: 'Staker',
      icon: '◆',
      color: '#0066FF',
      description: 'Has UDAG staked',
      earned: staked > 0,
    },
    {
      id: 'delegator',
      label: 'Delegator',
      icon: '↗',
      color: '#60a5fa',
      description: 'Delegated to a validator',
      earned: hasDelegated,
    },
    {
      id: 'voter',
      label: 'Voter',
      icon: '⚙',
      color: '#FFB800',
      description: 'Participated in governance voting',
      earned: (data.votedOnProposals ?? 0) > 0,
    },
    {
      id: 'builder',
      label: 'Builder',
      icon: '⚡',
      color: '#fb923c',
      description: 'Completed bug bounties',
      earned: (data.paidBounties ?? 0) > 0,
    },
    {
      id: 'secured',
      label: 'Secured',
      icon: '🛡',
      color: '#34d399',
      description: 'Has recovery guardians or spending policy',
      earned: hasRecovery || hasPolicy,
    },
    {
      id: 'pioneer',
      label: 'Pioneer',
      icon: '★',
      color: '#fbbf24',
      description: 'Early network participant',
      earned: isPioneer,
    },
  ];
}

/** Generate a deterministic avatar gradient from an address */
export function avatarGradient(address: string): string {
  let h1 = 0, h2 = 0, h3 = 0;
  for (let i = 0; i < address.length; i++) {
    const c = address.charCodeAt(i);
    h1 = ((h1 << 5) - h1 + c) | 0;
    h2 = ((h2 << 7) - h2 + c) | 0;
    h3 = ((h3 << 3) - h3 + c) | 0;
  }
  const hue1 = Math.abs(h1) % 360;
  const hue2 = (hue1 + 40 + Math.abs(h2) % 80) % 360;
  const hue3 = (hue2 + 40 + Math.abs(h3) % 80) % 360;
  return `linear-gradient(135deg, hsl(${hue1},70%,55%), hsl(${hue2},65%,50%), hsl(${hue3},60%,45%))`;
}

/** Format a round number to an approximate date string */
export function roundToDate(round: number, currentRound: number): string {
  const roundsAgo = currentRound - round;
  const secondsAgo = roundsAgo * 5;
  const date = new Date(Date.now() - secondsAgo * 1000);
  return date.toLocaleDateString(undefined, { month: 'short', year: 'numeric' });
}
