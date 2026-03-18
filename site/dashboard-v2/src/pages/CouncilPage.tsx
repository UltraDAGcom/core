import { useState, useEffect, useCallback } from 'react';
import { getCouncil, getGovernanceConfig, shortAddr } from '../lib/api.ts';
import { Card } from '../components/shared/Card.tsx';
import { CouncilSeatGrid } from '../components/governance/CouncilSeatGrid.tsx';
import { CopyButton } from '../components/shared/CopyButton.tsx';

interface CouncilMember {
  address: string;
  category: string;
}

interface CouncilData {
  members: CouncilMember[];
  total_seats: number;
  filled_seats: number;
  emission_percent: number;
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

  return (
    <div className="space-y-6">
      <h1 className="text-2xl font-bold text-white">Council of 21</h1>

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        <Card title="Seat Categories">
          <CouncilSeatGrid members={members} />
          <div className="mt-4 pt-4 border-t border-dag-border grid grid-cols-3 gap-3 text-sm">
            <div>
              <span className="text-dag-muted block text-xs">Total Seats</span>
              <span className="text-white font-bold">{council?.total_seats ?? 21}</span>
            </div>
            <div>
              <span className="text-dag-muted block text-xs">Filled</span>
              <span className="text-white font-bold">{council?.filled_seats ?? members.length}</span>
            </div>
            <div>
              <span className="text-dag-muted block text-xs">Emission</span>
              <span className="text-white font-bold">{council?.emission_percent ?? 10}%</span>
            </div>
          </div>
        </Card>

        {govConfig && (
          <Card title="Governance Parameters">
            <div className="space-y-2">
              {Object.entries(govConfig).map(([key, value]) => (
                <div key={key} className="flex items-center justify-between text-sm">
                  <span className="text-dag-muted">{key.replace(/_/g, ' ')}</span>
                  <span className="text-white font-mono text-xs">
                    {typeof value === 'number' ? value.toLocaleString() : String(value)}
                  </span>
                </div>
              ))}
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
