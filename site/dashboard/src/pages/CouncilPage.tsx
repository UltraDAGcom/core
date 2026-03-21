import { useState, useEffect, useCallback } from 'react';
import { getCouncil, getGovernanceConfig, shortAddr } from '../lib/api.ts';
import { Card } from '../components/shared/Card.tsx';
import { CouncilSeatGrid } from '../components/governance/CouncilSeatGrid.tsx';
import { CopyButton } from '../components/shared/CopyButton.tsx';

interface CouncilMember {
  address: string;
  category: string;
}

interface SeatInfo {
  available: number;
  filled: number;
  max: number;
}

interface CouncilData {
  members: CouncilMember[];
  total_seats: number;
  filled_seats: number;
  member_count: number;
  max_members: number;
  emission_percent: number;
  per_member_reward_sats: number;
  per_member_reward_udag: number;
  seats: Record<string, SeatInfo>;
}

export function CouncilPage() {
  const [council, setCouncil] = useState<CouncilData | null>(null);
  const [govConfig, setGovConfig] = useState<Record<string, unknown> | null>(null);
  const [loading, setLoading] = useState(true);

  const refresh = useCallback(async () => {
    try {
      const [councilData, config] = await Promise.all([
        getCouncil().catch(() => null),
        getGovernanceConfig().catch(() => null),
      ]);
      if (councilData) setCouncil(councilData);
      if (config) setGovConfig(config);
    } catch {
      // ignore
    }
    setLoading(false);
  }, []);

  useEffect(() => {
    refresh();
    const interval = setInterval(refresh, 30000);
    return () => clearInterval(interval);
  }, [refresh]);

  if (loading) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="animate-pulse text-dag-muted">Loading council data...</div>
      </div>
    );
  }

  const members = council?.members ?? [];
  const memberCount = council?.member_count ?? members.length;
  const maxMembers = council?.max_members ?? 21;
  const openSeats = maxMembers - memberCount;
  const perMemberReward = council?.per_member_reward_udag ?? 0;
  const emissionPercent = council?.emission_percent ?? 10;

  return (
    <div className="space-y-6 animate-page-enter">
      <h1 className="text-2xl font-bold text-white">Council of 21</h1>

      {/* Stats row */}
      <div className="grid grid-cols-2 sm:grid-cols-4 gap-4">
        <div className="rounded-lg bg-dag-card border border-dag-border p-4">
          <span className="text-dag-muted text-xs block mb-1">Members</span>
          <span className="text-white font-bold text-xl font-mono">{memberCount}<span className="text-dag-muted text-sm font-normal">/{maxMembers}</span></span>
        </div>
        <div className="rounded-lg bg-dag-card border border-dag-border p-4">
          <span className="text-dag-muted text-xs block mb-1">Open Seats</span>
          <span className={`font-bold text-xl font-mono ${openSeats > 0 ? 'text-dag-green' : 'text-dag-muted'}`}>{openSeats}</span>
        </div>
        <div className="rounded-lg bg-dag-card border border-dag-border p-4">
          <span className="text-dag-muted text-xs block mb-1">Per-member Reward</span>
          <span className="text-white font-bold text-xl font-mono">{perMemberReward}<span className="text-dag-muted text-sm font-normal"> UDAG/round</span></span>
        </div>
        <div className="rounded-lg bg-dag-card border border-dag-border p-4">
          <span className="text-dag-muted text-xs block mb-1">Emission</span>
          <span className="text-dag-accent font-bold text-xl font-mono">{emissionPercent}%</span>
        </div>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        <Card title="Seat Categories">
          <CouncilSeatGrid members={members} seats={council?.seats} />
        </Card>

        {govConfig && (
          <Card title="Governance Parameters">
            <div className="space-y-4">
              {/* Derived percentages */}
              {(() => {
                const pctKeys = ['quorum_percent', 'approval_percent'];
                const pctEntries = Object.entries(govConfig).filter(([k]) => pctKeys.includes(k));
                return pctEntries.length > 0 ? (
                  <div className="grid grid-cols-2 gap-3">
                    {pctEntries.map(([key, value]) => (
                      <div key={key} className="bg-dag-bg/50 rounded-lg p-3 text-center">
                        <span className="text-dag-muted text-xs block mb-1">{key.replace(/_/g, ' ')}</span>
                        <span className="text-white font-bold text-lg">
                          {typeof value === 'number' ? `${value}%` : `${value}%`}
                        </span>
                      </div>
                    ))}
                  </div>
                ) : null;
              })()}

              {/* Governable params tag list */}
              {Array.isArray(govConfig.governable_params) && (
                <div>
                  <span className="text-dag-muted text-xs block mb-2">Governable parameters</span>
                  <div className="flex flex-wrap gap-1.5">
                    {(govConfig.governable_params as string[]).map((param) => (
                      <span
                        key={param}
                        className="inline-block px-2 py-0.5 rounded-full bg-dag-accent/15 text-dag-accent text-xs font-medium"
                      >
                        {param.replace(/_/g, ' ')}
                      </span>
                    ))}
                  </div>
                </div>
              )}

              {/* Raw numeric params */}
              <div className="space-y-2">
                {Object.entries(govConfig)
                  .filter(([key]) => !['quorum_percent', 'approval_percent', 'governable_params'].includes(key))
                  .map(([key, value]) => (
                    <div key={key} className="flex items-center justify-between text-sm">
                      <span className="text-dag-muted">{key.replace(/_/g, ' ')}</span>
                      <span className="text-white font-mono text-xs">
                        {typeof value === 'number' ? value.toLocaleString() : String(value)}
                      </span>
                    </div>
                  ))}
              </div>
            </div>
          </Card>
        )}
      </div>

      <Card title={`Members (${members.length})`}>
        {members.length === 0 ? (
          <p className="text-dag-muted text-sm">No council members.</p>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="text-left text-dag-muted border-b border-dag-border">
                  <th className="py-2 px-3 font-medium">#</th>
                  <th className="py-2 px-3 font-medium">Address</th>
                  <th className="py-2 px-3 font-medium">Category</th>
                </tr>
              </thead>
              <tbody>
                {members.map((m, i) => (
                  <tr key={m.address} className="border-b border-dag-border/50 hover:bg-dag-card-hover transition-colors">
                    <td className="py-2.5 px-3 text-dag-muted">{i + 1}</td>
                    <td className="py-2.5 px-3">
                      <div className="flex items-center gap-1">
                        <span className="font-mono text-xs text-white">{shortAddr(m.address)}</span>
                        <CopyButton text={m.address} />
                      </div>
                    </td>
                    <td className="py-2.5 px-3 text-dag-muted">{m.category}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </Card>
    </div>
  );
}
