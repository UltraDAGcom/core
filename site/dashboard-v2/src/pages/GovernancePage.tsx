import { useState, useEffect, useCallback } from 'react';
import { Plus } from 'lucide-react';
import { getProposals, getProposal, formatUdag, shortAddr } from '../lib/api.ts';
import { useKeystore } from '../hooks/useKeystore.ts';
import { Card } from '../components/shared/Card.tsx';
import { ProposalCard } from '../components/governance/ProposalCard.tsx';
import { CreateProposalModal } from '../components/governance/CreateProposalModal.tsx';
import { VoteButton } from '../components/governance/VoteButton.tsx';
import { Pagination } from '../components/shared/Pagination.tsx';
import { StatusBadge } from '../components/shared/StatusBadge.tsx';

const PER_PAGE = 10;

interface Proposal {
  id: number;
  title: string;
  description: string;
  status: string;
  proposal_type: string;
  proposer: string;
  votes_for: number;
  votes_against: number;
  snapshot_total_stake: number;
  voting_ends: number;
  execute_at_round: number | null;
  voters?: Array<{ address: string; vote: string; weight: number }>;
}

export function GovernancePage() {
  const { wallets, unlocked } = useKeystore();
  const [proposals, setProposals] = useState<Proposal[]>([]);
  const [selected, setSelected] = useState<Proposal | null>(null);
  const [showCreate, setShowCreate] = useState(false);
  const [page, setPage] = useState(1);
  const [loading, setLoading] = useState(true);

  const refresh = useCallback(async () => {
    try {
      const data = await getProposals();
      const list: Proposal[] = Array.isArray(data) ? data : (data?.proposals ?? []);
      setProposals(list.sort((a, b) => b.id - a.id));
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

  const handleSelectProposal = async (id: number) => {
    try {
      const detail = await getProposal(id);
      setSelected(detail);
    } catch {
      const p = proposals.find(x => x.id === id);
      if (p) setSelected(p);
    }
  };

  const totalPages = Math.max(1, Math.ceil(proposals.length / PER_PAGE));
  const pageProposals = proposals.slice((page - 1) * PER_PAGE, page * PER_PAGE);

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold text-white">Governance</h1>
        {unlocked && wallets.length > 0 && (
          <button
            onClick={() => setShowCreate(true)}
            className="flex items-center gap-2 px-4 py-2 rounded-lg bg-dag-blue text-white text-sm font-medium hover:bg-dag-blue/90 transition-colors"
          >
            <Plus className="w-4 h-4" />
            New Proposal
          </button>
        )}
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        <div className="lg:col-span-2 space-y-3">
          <Card title={`Proposals (${proposals.length})`}>
            {loading ? (
              <p className="text-dag-muted text-sm">Loading proposals...</p>
            ) : proposals.length === 0 ? (
              <p className="text-dag-muted text-sm">No proposals found.</p>
            ) : (
              <>
                <div className="space-y-3">
                  {pageProposals.map(p => (
                    <ProposalCard
                      key={p.id}
                      id={p.id}
                      title={p.title}
                      status={p.status}
                      proposal_type={p.proposal_type}
                      votes_for={p.votes_for}
                      votes_against={p.votes_against}
                      snapshot_total_stake={p.snapshot_total_stake}
                      onClick={() => handleSelectProposal(p.id)}
                    />
                  ))}
                </div>
                <Pagination currentPage={page} totalPages={totalPages} onPageChange={setPage} />
              </>
            )}
          </Card>
        </div>

        <div>
          {selected ? (
            <Card title={`Proposal #${selected.id}`}>
              <div className="space-y-3">
                <div className="flex items-center gap-2">
                  <StatusBadge status={selected.status} />
                  <span className="text-xs text-dag-muted px-1.5 py-0.5 rounded bg-dag-card border border-dag-border">
                    {selected.proposal_type}
                  </span>
                </div>
                <h3 className="text-white font-medium">{selected.title}</h3>
                <p className="text-sm text-dag-muted whitespace-pre-wrap">{selected.description}</p>
                <div className="grid grid-cols-2 gap-2 text-sm">
                  <div>
                    <span className="text-dag-muted block text-xs">Proposer</span>
                    <span className="text-white font-mono text-xs">{shortAddr(selected.proposer)}</span>
                  </div>
                  <div>
                    <span className="text-dag-muted block text-xs">Voting Ends</span>
                    <span className="text-white">Round {selected.voting_ends?.toLocaleString()}</span>
                  </div>
                  <div>
                    <span className="text-dag-muted block text-xs">Votes For</span>
                    <span className="text-dag-green">{formatUdag(selected.votes_for)}</span>
                  </div>
                  <div>
                    <span className="text-dag-muted block text-xs">Votes Against</span>
                    <span className="text-dag-red">{formatUdag(selected.votes_against)}</span>
                  </div>
                </div>

                {unlocked && wallets.length > 0 && selected.status === 'Active' && (
                  <div className="pt-3 border-t border-dag-border flex gap-2">
                    <VoteButton proposalId={selected.id} secretKey={wallets[0].secret_key} approve={true} fee={10000} onSuccess={refresh} />
                    <VoteButton proposalId={selected.id} secretKey={wallets[0].secret_key} approve={false} fee={10000} onSuccess={refresh} />
                  </div>
                )}

                {selected.voters && selected.voters.length > 0 && (
                  <div className="pt-3 border-t border-dag-border">
                    <h4 className="text-sm font-medium text-white mb-2">Voters ({selected.voters.length})</h4>
                    <div className="space-y-1">
                      {selected.voters.map(v => (
                        <div key={v.address} className="flex items-center justify-between text-xs">
                          <span className="font-mono text-dag-muted">{shortAddr(v.address)}</span>
                          <span className={v.vote === 'yes' ? 'text-dag-green' : 'text-dag-red'}>
                            {v.vote === 'yes' ? 'YES' : 'NO'} ({formatUdag(v.weight)})
                          </span>
                        </div>
                      ))}
                    </div>
                  </div>
                )}
              </div>
            </Card>
          ) : (
            <Card>
              <div className="flex flex-col items-center justify-center py-8 text-center">
                <p className="text-sm text-dag-muted">Select a proposal to view details</p>
              </div>
            </Card>
          )}
        </div>
      </div>

      {showCreate && (
        <CreateProposalModal
          wallets={wallets}
          onClose={() => setShowCreate(false)}
          onSuccess={refresh}
        />
      )}
    </div>
  );
}
