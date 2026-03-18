import { useState, useEffect } from 'react';
import { Link } from 'react-router-dom';
import {
  Activity,
  Layers,
  Users,
  Coins,
  Database,
  Clock,
  Box,
  Cpu,
  HardDrive,
  Wifi,
  Shield,
  TrendingUp,
  CheckCircle,
  AlertTriangle,
  XCircle,
} from 'lucide-react';
import { getHealthDetailed, getRound, formatUdag } from '../lib/api';
import { Sparkline } from '../components/shared/Sparkline';
import { ActivityBar } from '../components/shared/ActivityBar';
import { AnimatedNumber } from '../components/shared/AnimatedNumber';
import type { NodeStatus } from '../hooks/useNode';

interface DashboardPageProps {
  status: NodeStatus | null;
  loading: boolean;
}

interface HealthData {
  status: string;
  components: {
    dag: { available: boolean; current_round: number; pruning_floor: number; tips_count: number; vertex_count: number };
    finality: { available: boolean; finality_lag: number; last_finalized_round: number; validator_count: number };
    mempool: { available: boolean; transaction_count: number };
    network: { peer_count: number; sync_complete: boolean };
    state: { account_count: number; active_validators: number; available: boolean; total_supply: number };
    checkpoints: { checkpoint_age_seconds: number; disk_count: number; last_checkpoint_round: number; pending_checkpoints: number };
  };
  warnings: string[];
}

interface RoundData {
  round: number;
  vertices: { hash: string; validator: string; tx_count: number }[];
}

const MAX_SUPPLY_SATS = 2_100_000_000_000_000;

function formatUptime(seconds: number): string {
  const d = Math.floor(seconds / 86400);
  const h = Math.floor((seconds % 86400) / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  if (d > 0) return `${d}d ${h}h ${m}m`;
  if (h > 0) return `${h}h ${m}m`;
  return `${m}m`;
}

function formatMemory(bytes: number): string {
  return (bytes / 1048576).toFixed(1) + ' MB';
}

function FinalityBadge({ lag }: { lag: number }) {
  const color = lag <= 3 ? 'bg-dag-green/20 text-dag-green' : lag <= 10 ? 'bg-dag-yellow/20 text-dag-yellow' : 'bg-dag-red/20 text-dag-red';
  return (
    <span className={`inline-flex items-center px-2 py-0.5 rounded-full text-xs font-medium ${color}`}>
      lag {lag}
    </span>
  );
}

export function DashboardPage({ status, loading }: DashboardPageProps) {
  const [health, setHealth] = useState<HealthData | null>(null);
  const [recentRounds, setRecentRounds] = useState<RoundData[]>([]);
  const [vertexHistory, setVertexHistory] = useState<number[]>([]);
  const [roundsLoading, setRoundsLoading] = useState(false);

  // Fetch health data
  useEffect(() => {
    let cancelled = false;
    async function fetchHealth() {
      try {
        const h = await getHealthDetailed();
        if (!cancelled) setHealth(h);
      } catch { /* ignore */ }
    }
    fetchHealth();
    const iv = setInterval(fetchHealth, 10_000);
    return () => { cancelled = true; clearInterval(iv); };
  }, []);

  // Fetch recent rounds (20 in parallel for sparkline + activity bar)
  useEffect(() => {
    if (!status || status.last_finalized_round < 1) return;
    let cancelled = false;
    async function fetchRounds() {
      setRoundsLoading(true);
      const base = status!.last_finalized_round;
      const count = Math.min(20, base);
      const roundNumbers = Array.from({ length: count }, (_, i) => base - i).filter(r => r >= 1);

      const results = await Promise.all(
        roundNumbers.map(async (r): Promise<RoundData | null> => {
          try {
            const data = await getRound(r);
            const vertices = Array.isArray(data) ? data : data?.vertices ?? [];
            return {
              round: r,
              vertices: vertices.map((v: any) => ({
                hash: v.hash ?? '',
                validator: v.validator ?? '',
                tx_count: v.tx_count ?? v.transactions ?? 0,
              })),
            };
          } catch {
            return null;
          }
        }),
      );

      if (!cancelled) {
        const rounds = results.filter((r): r is RoundData => r !== null);
        rounds.sort((a, b) => b.round - a.round);
        setRecentRounds(rounds);
        const ascending = [...rounds].sort((a, b) => a.round - b.round);
        setVertexHistory(ascending.map(r => r.vertices.length));
        setRoundsLoading(false);
      }
    }
    fetchRounds();
    return () => { cancelled = true; };
  }, [status?.last_finalized_round]);

  if (loading) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="flex items-center gap-3">
          <div className="w-5 h-5 border-2 border-dag-accent border-t-transparent rounded-full animate-spin" />
          <span className="text-dag-muted">Connecting to node...</span>
        </div>
      </div>
    );
  }

  if (!status) {
    return (
      <div className="flex flex-col items-center justify-center h-64 gap-3">
        <XCircle className="w-10 h-10 text-dag-red" />
        <p className="text-dag-muted">Unable to connect to any node.</p>
      </div>
    );
  }

  const supplyPercent = (status.total_supply / MAX_SUPPLY_SATS) * 100;
  const emitted = status.total_supply - status.treasury_balance;
  const remaining = MAX_SUPPLY_SATS - status.total_supply;

  return (
    <div className="space-y-6">
      {/* Header */}
      <div>
        <h1 className="text-2xl font-bold text-white">Dashboard</h1>
        <p className="text-sm text-dag-muted mt-1">UltraDAG testnet overview</p>
      </div>

      {/* Health Banner */}
      <HealthBanner health={health} />

      {/* Key Metrics - 4 big cards */}
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4 stagger-enter">
        <MetricCard
          icon={Layers}
          iconColor="text-dag-blue"
          label="DAG Round"
          value={<AnimatedNumber value={status.dag_round} />}
          badge={<FinalityBadge lag={status.finality_lag} />}
          sub={
            <div className="flex items-center gap-2 mt-1">
              <span>Finalized: <AnimatedNumber value={status.last_finalized_round} /></span>
              <Sparkline data={vertexHistory} height={24} width={100} />
            </div>
          }
        />
        <MetricCard
          icon={Coins}
          iconColor="text-dag-accent"
          label="Total Supply"
          value={`${formatUdag(status.total_supply)} UDAG`}
          sub={
            <div className="mt-2">
              <div className="w-full bg-dag-surface rounded-full h-1.5 overflow-hidden">
                <div
                  className="h-full bg-gradient-to-r from-dag-accent to-dag-blue rounded-full transition-all duration-500"
                  style={{ width: `${Math.min(supplyPercent, 100)}%` }}
                />
              </div>
              <span className="text-[10px] text-dag-muted mt-1">{supplyPercent.toFixed(4)}% of 21M</span>
            </div>
          }
        />
        <MetricCard
          icon={Users}
          iconColor="text-dag-purple"
          label="Network"
          value={`${status.validators} validators`}
          sub={`${status.peer_count} peers connected`}
        />
        <MetricCard
          icon={Shield}
          iconColor="text-dag-green"
          label="Treasury"
          value={`${formatUdag(status.treasury_balance)} UDAG`}
          sub="Protocol reserve"
        />
      </div>

      {/* Supply Progress */}
      <div className="bg-dag-card border border-dag-border rounded-xl p-5">
        <div className="flex items-center gap-2 mb-4">
          <TrendingUp className="w-4 h-4 text-dag-accent" />
          <h2 className="text-sm font-semibold text-white">Emission Progress</h2>
        </div>
        <div className="w-full bg-dag-surface rounded-full h-3 overflow-hidden mb-4">
          <div
            className="h-full rounded-full transition-all duration-700 relative overflow-hidden"
            style={{
              width: `${Math.min(supplyPercent, 100)}%`,
              background: 'linear-gradient(90deg, #6366f1 0%, #8b5cf6 40%, #a78bfa 70%, #c4b5fd 100%)',
            }}
          >
            <div className="absolute inset-0 bg-gradient-to-r from-transparent via-white/10 to-transparent animate-pulse" />
          </div>
        </div>
        <div className="grid grid-cols-3 gap-4 text-center">
          <div>
            <p className="text-xs text-dag-muted mb-0.5">Emitted</p>
            <p className="text-sm font-semibold text-white">{formatUdag(emitted)} UDAG</p>
          </div>
          <div>
            <p className="text-xs text-dag-muted mb-0.5">Remaining</p>
            <p className="text-sm font-semibold text-white">{formatUdag(remaining)} UDAG</p>
          </div>
          <div>
            <p className="text-xs text-dag-muted mb-0.5">Treasury</p>
            <p className="text-sm font-semibold text-dag-accent">{formatUdag(status.treasury_balance)} UDAG</p>
          </div>
        </div>
      </div>

      {/* Network Vitals Grid */}
      <div>
        <h2 className="text-sm font-semibold text-dag-muted uppercase tracking-wider mb-3">Network Vitals</h2>
        <div className="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-6 gap-3 stagger-enter">
          <VitalCard icon={Database} label="Accounts" value={<AnimatedNumber value={status.active_accounts} />} />
          <VitalCard
            icon={Box}
            label="Mempool"
            value={<AnimatedNumber value={status.mempool_size} />}
            accent={status.mempool_size > 0 ? 'text-dag-yellow' : undefined}
          />
          <VitalCard icon={Activity} label="DAG Vertices" value={<AnimatedNumber value={status.total_vertices} />} />
          <VitalCard icon={CheckCircle} label="Finalized" value={<AnimatedNumber value={status.finalized_count} />} />
          <VitalCard icon={Cpu} label="Memory" value={formatMemory(status.memory_usage_bytes)} />
          <VitalCard icon={Clock} label="Uptime" value={formatUptime(status.uptime_seconds)} />
        </div>
      </div>

      {/* Checkpoint + Staking info row */}
      {health && (
        <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
          <div className="bg-dag-card border border-dag-border rounded-xl p-4">
            <div className="flex items-center gap-2 mb-3">
              <HardDrive className="w-4 h-4 text-dag-blue" />
              <h3 className="text-sm font-semibold text-white">Checkpoints</h3>
            </div>
            <div className="space-y-2 text-sm">
              <div className="flex justify-between">
                <span className="text-dag-muted">Last checkpoint</span>
                <span className="text-white font-mono">Round {health.components.checkpoints.last_checkpoint_round.toLocaleString()}</span>
              </div>
              <div className="flex justify-between">
                <span className="text-dag-muted">Age</span>
                <span className="text-white">{formatUptime(health.components.checkpoints.checkpoint_age_seconds)}</span>
              </div>
              <div className="flex justify-between">
                <span className="text-dag-muted">On disk</span>
                <span className="text-white">{health.components.checkpoints.disk_count}</span>
              </div>
              <div className="flex justify-between">
                <span className="text-dag-muted">Pending</span>
                <span className="text-white">{health.components.checkpoints.pending_checkpoints}</span>
              </div>
            </div>
          </div>
          <div className="bg-dag-card border border-dag-border rounded-xl p-4">
            <div className="flex items-center gap-2 mb-3">
              <Wifi className="w-4 h-4 text-dag-green" />
              <h3 className="text-sm font-semibold text-white">DAG Status</h3>
            </div>
            <div className="space-y-2 text-sm">
              <div className="flex justify-between">
                <span className="text-dag-muted">Pruning floor</span>
                <span className="text-white font-mono">{health.components.dag.pruning_floor.toLocaleString()}</span>
              </div>
              <div className="flex justify-between">
                <span className="text-dag-muted">Tips</span>
                <span className="text-white">{health.components.dag.tips_count}</span>
              </div>
              <div className="flex justify-between">
                <span className="text-dag-muted">Sync</span>
                <span className={health.components.network.sync_complete ? 'text-dag-green' : 'text-dag-yellow'}>
                  {health.components.network.sync_complete ? 'Complete' : 'Syncing...'}
                </span>
              </div>
              <div className="flex justify-between">
                <span className="text-dag-muted">Staked</span>
                <span className="text-white">{formatUdag(status.total_staked)} UDAG</span>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* Recent Finalized Rounds */}
      <div className="bg-dag-card border border-dag-border rounded-xl p-5">
        <div className="flex items-center justify-between mb-4">
          <div className="flex items-center gap-2">
            <Layers className="w-4 h-4 text-dag-blue" />
            <h2 className="text-sm font-semibold text-white">Recent Finalized Rounds</h2>
          </div>
          <Link to="/explorer" className="text-xs text-dag-accent hover:text-dag-accent/80 transition-colors">
            View all
          </Link>
        </div>
        {recentRounds.length > 0 && (
          <ActivityBar
            rounds={recentRounds.map(r => ({
              round: r.round,
              vertexCount: r.vertices.length,
              txCount: r.vertices.reduce((s, v) => s + v.tx_count, 0),
            }))}
          />
        )}
        {roundsLoading && recentRounds.length === 0 ? (
          <div className="flex items-center justify-center py-8">
            <div className="w-4 h-4 border-2 border-dag-accent border-t-transparent rounded-full animate-spin" />
          </div>
        ) : recentRounds.length === 0 ? (
          <p className="text-dag-muted text-sm text-center py-4">No rounds available yet.</p>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="text-dag-muted text-xs uppercase tracking-wider border-b border-dag-border">
                  <th className="text-left pb-2 pr-4">Round</th>
                  <th className="text-center pb-2 px-4">Vertices</th>
                  <th className="text-center pb-2 px-4">Transactions</th>
                  <th className="text-right pb-2 pl-4">Status</th>
                </tr>
              </thead>
              <tbody>
                {recentRounds.map((r) => {
                  const totalTx = r.vertices.reduce((s, v) => s + v.tx_count, 0);
                  return (
                    <tr key={r.round} className="border-b border-dag-border/50 hover:bg-dag-surface/50 transition-colors">
                      <td className="py-2.5 pr-4">
                        <Link
                          to={`/round/${r.round}`}
                          className="text-dag-accent hover:text-dag-accent/80 font-mono font-medium transition-colors"
                        >
                          #{r.round.toLocaleString()}
                        </Link>
                      </td>
                      <td className="py-2.5 px-4 text-center text-white">{r.vertices.length}</td>
                      <td className="py-2.5 px-4 text-center text-white">{totalTx}</td>
                      <td className="py-2.5 pl-4 text-right">
                        <span className="inline-flex items-center px-2 py-0.5 rounded-full text-[10px] font-medium bg-dag-green/20 text-dag-green">
                          Finalized
                        </span>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        )}
      </div>
    </div>
  );
}

/* ─── Sub-components ─── */

function HealthBanner({ health }: { health: HealthData | null }) {
  if (!health) {
    return (
      <div className="flex items-center gap-2 px-4 py-2 rounded-lg bg-dag-surface border border-dag-border text-sm">
        <div className="w-2 h-2 rounded-full bg-dag-muted animate-pulse" />
        <span className="text-dag-muted">Checking node health...</span>
      </div>
    );
  }

  const isHealthy = health.status === 'healthy';
  const isDegraded = health.status === 'degraded' || health.status === 'warning';

  const dotColor = isHealthy ? 'bg-dag-green' : isDegraded ? 'bg-dag-yellow' : 'bg-dag-red';
  const textColor = isHealthy ? 'text-dag-green' : isDegraded ? 'text-dag-yellow' : 'text-dag-red';
  const StatusIcon = isHealthy ? CheckCircle : isDegraded ? AlertTriangle : XCircle;
  const label = health.status.charAt(0).toUpperCase() + health.status.slice(1);

  return (
    <div className="flex flex-wrap items-center gap-x-4 gap-y-1 px-4 py-2 rounded-lg bg-dag-surface border border-dag-border text-sm">
      <div className="flex items-center gap-2">
        <div className={`w-2 h-2 rounded-full ${dotColor} ${isHealthy ? 'animate-pulse' : ''}`} />
        <StatusIcon className={`w-3.5 h-3.5 ${textColor}`} />
        <span className={`font-medium ${textColor}`}>{label}</span>
      </div>
      {health.warnings.length > 0 && (
        <div className="flex items-center gap-1 text-dag-yellow">
          <AlertTriangle className="w-3 h-3" />
          <span className="text-xs">{health.warnings.join(' | ')}</span>
        </div>
      )}
      <div className="ml-auto flex items-center gap-3 text-xs text-dag-muted">
        <span>Checkpoint: {health.components.checkpoints.last_checkpoint_round.toLocaleString()}</span>
        <span>Pruning: {health.components.dag.pruning_floor.toLocaleString()}</span>
      </div>
    </div>
  );
}

function MetricCard({
  icon: Icon,
  iconColor,
  label,
  value,
  badge,
  sub,
}: {
  icon: React.ElementType;
  iconColor: string;
  label: string;
  value: React.ReactNode;
  badge?: React.ReactNode;
  sub?: React.ReactNode;
}) {
  return (
    <div className="bg-dag-card border border-dag-border rounded-xl p-5 card-gradient-border hover:border-slate-500 transition-colors">
      <div className="flex items-center justify-between mb-3">
        <span className="text-xs text-dag-muted uppercase tracking-wider">{label}</span>
        <Icon className={`w-5 h-5 ${iconColor}`} />
      </div>
      <div className="flex items-center gap-2">
        <p className="text-xl font-bold text-white">{value}</p>
        {badge}
      </div>
      {sub && (typeof sub === 'string' ? <p className="text-xs text-dag-muted mt-1">{sub}</p> : <div className="text-xs text-dag-muted mt-1">{sub}</div>)}
    </div>
  );
}

function VitalCard({
  icon: Icon,
  label,
  value,
  accent,
}: {
  icon: React.ElementType;
  label: string;
  value: React.ReactNode;
  accent?: string;
}) {
  return (
    <div className="bg-dag-card border border-dag-border rounded-lg p-3 hover:border-slate-500 transition-colors">
      <div className="flex items-center gap-1.5 mb-1.5">
        <Icon className="w-3.5 h-3.5 text-dag-muted" />
        <span className="text-[10px] text-dag-muted uppercase tracking-wider">{label}</span>
      </div>
      <p className={`text-lg font-bold ${accent || 'text-white'}`}>{value}</p>
    </div>
  );
}
