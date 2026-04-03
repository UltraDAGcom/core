import { useState } from 'react';
import { postProposal } from '../../lib/api';
import type { Wallet } from '../../lib/keystore';
import { isValidAddress, normalizeAddress } from '../../lib/api';
import { X } from 'lucide-react';

const PARAM_OPTIONS = [
  'min_fee_sats',
  'min_stake_to_propose',
  'quorum_numerator',
  'approval_numerator',
  'voting_period_rounds',
  'execution_delay_rounds',
  'max_active_proposals',
  'observer_reward_percent',
  'council_emission_percent',
  'slash_percent',
];

const SEAT_CATEGORIES = [
  'Engineering',
  'Growth',
  'Legal',
  'Research',
  'Community',
  'Operations',
  'Security',
];

interface CreateProposalModalProps {
  wallets: Wallet[];
  onClose: () => void;
  onSuccess: () => void;
}

export function CreateProposalModal({ wallets, onClose, onSuccess }: CreateProposalModalProps) {
  const [walletIdx, setWalletIdx] = useState(0);
  const [proposalType, setProposalType] = useState<'Text' | 'ParameterChange' | 'CouncilMembership' | 'TreasurySpend'>('Text');
  const [title, setTitle] = useState('');
  const [description, setDescription] = useState('');
  const [fee, setFee] = useState('10000');

  // ParameterChange fields
  const [paramName, setParamName] = useState(PARAM_OPTIONS[0]);
  const [paramValue, setParamValue] = useState('');

  // CouncilMembership fields
  const [councilAction, setCouncilAction] = useState<'Add' | 'Remove'>('Add');
  const [councilAddress, setCouncilAddress] = useState('');
  const [councilCategory, setCouncilCategory] = useState(SEAT_CATEGORIES[0]);

  // TreasurySpend fields
  const [treasuryRecipient, setTreasuryRecipient] = useState('');
  const [treasuryAmount, setTreasuryAmount] = useState('');

  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');

  const wallet = wallets[walletIdx];

  const handleSubmit = async () => {
    if (!wallet) return;
    if (!title.trim()) { setError('Title is required'); return; }
    if (!description.trim()) { setError('Description is required'); return; }

    const feeSats = parseInt(fee, 10);
    if (isNaN(feeSats) || feeSats < 10000) { setError('Minimum fee is 10,000 sats (0.0001 UDAG)'); return; }

    const body: Record<string, unknown> = {
      secret_key: wallet.secret_key,
      title: title.trim(),
      description: description.trim(),
      fee: feeSats,
    };

    if (proposalType === 'Text') {
      body.proposal_type = 'text';
    } else if (proposalType === 'ParameterChange') {
      const val = parseInt(paramValue, 10);
      if (isNaN(val)) { setError('Parameter value must be a number'); return; }
      body.proposal_type = 'parameter';
      body.parameter_name = paramName;
      body.parameter_value = String(val);
    } else if (proposalType === 'CouncilMembership') {
      if (!councilAddress.trim()) { setError('Council address is required'); return; }
      if (!isValidAddress(councilAddress.trim())) { setError('Invalid council address (hex or bech32m)'); return; }
      body.proposal_type = 'council_membership';
      body.council_action = councilAction;
      body.council_address = normalizeAddress(councilAddress.trim());
      body.council_category = councilCategory;
    } else {
      const amt = Math.floor(parseFloat(treasuryAmount) * 100_000_000);
      if (isNaN(amt) || amt <= 0) { setError('Treasury amount must be positive'); return; }
      if (!treasuryRecipient.trim()) { setError('Recipient address is required'); return; }
      if (!isValidAddress(treasuryRecipient.trim())) { setError('Invalid recipient address (hex or bech32m)'); return; }
      body.proposal_type = 'treasury_spend';
      body.treasury_recipient = normalizeAddress(treasuryRecipient.trim());
      body.treasury_amount = amt;
    }

    setLoading(true);
    setError('');
    try {
      await postProposal(body);
      onSuccess();
      onClose();
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : 'Failed to create proposal');
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50 p-4">
      <div className="bg-dag-card border border-dag-border rounded-lg w-full max-w-lg max-h-[90vh] overflow-y-auto p-6">
        <div className="flex items-center justify-between mb-5">
          <h2 className="text-lg font-semibold text-white">Create Proposal</h2>
          <button onClick={onClose} className="text-dag-muted hover:text-white">
            <X size={20} />
          </button>
        </div>

        <div className="space-y-4">
          {/* Wallet selector */}
          <label className="block">
            <span className="text-sm text-dag-muted">Wallet</span>
            <select
              value={walletIdx}
              onChange={e => setWalletIdx(Number(e.target.value))}
              className="mt-1 block w-full rounded bg-dag-surface border border-dag-border px-3 py-2 text-sm text-white"
            >
              {wallets.map((w, i) => (
                <option key={i} value={i}>{w.name}</option>
              ))}
            </select>
          </label>

          {/* Proposal type */}
          <label className="block">
            <span className="text-sm text-dag-muted">Type</span>
            <select
              value={proposalType}
              onChange={e => setProposalType(e.target.value as typeof proposalType)}
              className="mt-1 block w-full rounded bg-dag-surface border border-dag-border px-3 py-2 text-sm text-white"
            >
              <option value="Text">Text Proposal</option>
              <option value="ParameterChange">Parameter Change</option>
              <option value="CouncilMembership">Council Membership</option>
              <option value="TreasurySpend">Treasury Spend</option>
            </select>
          </label>

          {/* Title */}
          <label className="block">
            <span className="text-sm text-dag-muted">Title (max 128 chars)</span>
            <input
              type="text"
              maxLength={128}
              value={title}
              onChange={e => setTitle(e.target.value)}
              className="mt-1 block w-full rounded bg-dag-surface border border-dag-border px-3 py-2 text-sm text-white"
            />
          </label>

          {/* Description */}
          <label className="block">
            <span className="text-sm text-dag-muted">Description (max 4096 chars)</span>
            <textarea
              maxLength={4096}
              rows={4}
              value={description}
              onChange={e => setDescription(e.target.value)}
              className="mt-1 block w-full rounded bg-dag-surface border border-dag-border px-3 py-2 text-sm text-white resize-y"
            />
          </label>

          {/* Type-specific fields */}
          {proposalType === 'ParameterChange' && (
            <div className="space-y-3 p-3 rounded bg-dag-surface border border-dag-border">
              <label className="block">
                <span className="text-sm text-dag-muted">Parameter</span>
                <select
                  value={paramName}
                  onChange={e => setParamName(e.target.value)}
                  className="mt-1 block w-full rounded bg-dag-card border border-dag-border px-3 py-2 text-sm text-white"
                >
                  {PARAM_OPTIONS.map(p => <option key={p} value={p}>{p}</option>)}
                </select>
              </label>
              <label className="block">
                <span className="text-sm text-dag-muted">New Value</span>
                <input
                  type="number"
                  value={paramValue}
                  onChange={e => setParamValue(e.target.value)}
                  className="mt-1 block w-full rounded bg-dag-card border border-dag-border px-3 py-2 text-sm text-white"
                />
              </label>
            </div>
          )}

          {proposalType === 'CouncilMembership' && (
            <div className="space-y-3 p-3 rounded bg-dag-surface border border-dag-border">
              <label className="block">
                <span className="text-sm text-dag-muted">Action</span>
                <select
                  value={councilAction}
                  onChange={e => setCouncilAction(e.target.value as 'Add' | 'Remove')}
                  className="mt-1 block w-full rounded bg-dag-card border border-dag-border px-3 py-2 text-sm text-white"
                >
                  <option value="Add">Add Member</option>
                  <option value="Remove">Remove Member</option>
                </select>
              </label>
              <label className="block">
                <span className="text-sm text-dag-muted">Address</span>
                <input
                  type="text"
                  value={councilAddress}
                  onChange={e => setCouncilAddress(e.target.value)}
                  placeholder="hex or bech32m address (tudg1...)"
                  className="mt-1 block w-full rounded bg-dag-card border border-dag-border px-3 py-2 text-sm text-white font-mono"
                />
              </label>
              <label className="block">
                <span className="text-sm text-dag-muted">Category</span>
                <select
                  value={councilCategory}
                  onChange={e => setCouncilCategory(e.target.value)}
                  className="mt-1 block w-full rounded bg-dag-card border border-dag-border px-3 py-2 text-sm text-white"
                >
                  {SEAT_CATEGORIES.map(c => <option key={c} value={c}>{c}</option>)}
                </select>
              </label>
            </div>
          )}

          {proposalType === 'TreasurySpend' && (
            <div className="space-y-3 p-3 rounded bg-dag-surface border border-dag-border">
              <label className="block">
                <span className="text-sm text-dag-muted">Recipient Address</span>
                <input
                  type="text"
                  value={treasuryRecipient}
                  onChange={e => setTreasuryRecipient(e.target.value)}
                  placeholder="hex or bech32m address (tudg1...)"
                  className="mt-1 block w-full rounded bg-dag-card border border-dag-border px-3 py-2 text-sm text-white font-mono"
                />
              </label>
              <label className="block">
                <span className="text-sm text-dag-muted">Amount (UDAG)</span>
                <input
                  type="number"
                  min="0"
                  step="0.01"
                  value={treasuryAmount}
                  onChange={e => setTreasuryAmount(e.target.value)}
                  className="mt-1 block w-full rounded bg-dag-card border border-dag-border px-3 py-2 text-sm text-white"
                />
              </label>
            </div>
          )}

          {/* Fee */}
          <label className="block">
            <span className="text-sm text-dag-muted">Fee (sats, min 10,000)</span>
            <input
              type="number"
              min="10000"
              value={fee}
              onChange={e => setFee(e.target.value)}
              className="mt-1 block w-full rounded bg-dag-surface border border-dag-border px-3 py-2 text-sm text-white"
            />
          </label>

          {error && <p className="text-sm text-dag-red">{error}</p>}

          <button
            onClick={handleSubmit}
            disabled={loading}
            className="w-full py-2.5 rounded bg-dag-blue text-white font-medium text-sm hover:bg-dag-blue/90 disabled:opacity-50"
          >
            {loading ? 'Creating...' : 'Create Proposal'}
          </button>
        </div>
      </div>
    </div>
  );
}
