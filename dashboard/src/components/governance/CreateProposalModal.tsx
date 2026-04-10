import { useState } from 'react';
import { postProposal } from '../../lib/api';
import type { Wallet } from '../../lib/keystore';
import { isValidAddress, normalizeAddress } from '../../lib/api';
import { primaryButtonStyle, inputStyle as themeInputStyle } from '../../lib/theme';

const PARAM_OPTIONS = [
  'min_fee_sats',
  'min_stake_to_propose',
  'quorum_numerator',
  'approval_numerator',
  'voting_period_rounds',
  'execution_delay_rounds',
  'max_active_proposals',
  'observer_reward_percent',
  'validator_emission_percent',
  'council_emission_percent',
  'treasury_emission_percent',
  'founder_emission_percent',
  'ecosystem_emission_percent',
  'reserve_emission_percent',
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

const modalInputStyle: React.CSSProperties = {
  ...themeInputStyle,
  marginTop: 4,
};

const labelStyle: React.CSSProperties = {
  fontSize: 10.5, color: 'var(--dag-text-muted)', fontWeight: 600,
  letterSpacing: 1, display: 'block',
};

const sectionBoxStyle: React.CSSProperties = {
  padding: 12, borderRadius: 10,
  background: 'var(--dag-input-bg)', border: '1px solid var(--dag-border)',
  display: 'flex', flexDirection: 'column', gap: 12,
};

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
    <div style={{
      position: 'fixed', inset: 0, background: 'var(--dag-overlay)', display: 'flex',
      alignItems: 'center', justifyContent: 'center', zIndex: 50, padding: 16,
    }}>
      <div style={{
        background: 'var(--dag-card)', border: '1px solid var(--dag-border)', borderRadius: 14,
        width: '100%', maxWidth: 520, maxHeight: '90vh', overflowY: 'auto', padding: '20px 22px',
      }}>
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 20 }}>
          <h2 style={{ fontSize: 16, fontWeight: 700, color: 'var(--dag-text)' }}>Create Proposal</h2>
          <button onClick={onClose} style={{ background: 'none', border: 'none', color: 'var(--dag-text-faint)', fontSize: 18, cursor: 'pointer' }}>✕</button>
        </div>

        <div style={{ display: 'flex', flexDirection: 'column', gap: 14 }}>
          {/* Wallet selector */}
          <div>
            <span style={labelStyle}>Wallet</span>
            <select value={walletIdx} onChange={e => setWalletIdx(Number(e.target.value))} style={modalInputStyle}>
              {wallets.map((w, i) => <option key={i} value={i} style={{ background: 'var(--dag-bg)' }}>{w.name}</option>)}
            </select>
          </div>

          {/* Proposal type */}
          <div>
            <span style={labelStyle}>Type</span>
            <select value={proposalType} onChange={e => setProposalType(e.target.value as typeof proposalType)} style={modalInputStyle}>
              <option value="Text">Text Proposal</option>
              <option value="ParameterChange">Parameter Change</option>
              <option value="CouncilMembership">Council Membership</option>
              <option value="TreasurySpend">Treasury Spend</option>
            </select>
          </div>

          {/* Title */}
          <div>
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'baseline' }}>
              <span style={labelStyle}>Title</span>
              <span style={{ fontSize: 9.5, color: title.length > 120 ? '#FFB800' : 'var(--dag-text-faint)', fontFamily: "'DM Mono',monospace" }}>{title.length}/128</span>
            </div>
            <input type="text" maxLength={128} value={title} onChange={e => setTitle(e.target.value)} style={modalInputStyle} />
          </div>

          {/* Description */}
          <div>
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'baseline' }}>
              <span style={labelStyle}>Description</span>
              <span style={{ fontSize: 9.5, color: description.length > 3900 ? '#FFB800' : 'var(--dag-text-faint)', fontFamily: "'DM Mono',monospace" }}>{description.length}/4096</span>
            </div>
            <textarea maxLength={4096} rows={4} value={description} onChange={e => setDescription(e.target.value)}
              style={{ ...modalInputStyle, resize: 'vertical' } as React.CSSProperties} />
          </div>

          {/* Type-specific fields */}
          {proposalType === 'ParameterChange' && (
            <div style={sectionBoxStyle}>
              <div>
                <span style={labelStyle}>Parameter</span>
                <select value={paramName} onChange={e => setParamName(e.target.value)} style={modalInputStyle}>
                  {PARAM_OPTIONS.map(p => <option key={p} value={p}>{p}</option>)}
                </select>
              </div>
              <div>
                <span style={labelStyle}>New Value</span>
                <input type="number" value={paramValue} onChange={e => setParamValue(e.target.value)} style={modalInputStyle} />
              </div>
            </div>
          )}

          {proposalType === 'CouncilMembership' && (
            <div style={sectionBoxStyle}>
              <div>
                <span style={labelStyle}>Action</span>
                <select value={councilAction} onChange={e => setCouncilAction(e.target.value as 'Add' | 'Remove')} style={modalInputStyle}>
                  <option value="Add">Add Member</option>
                  <option value="Remove">Remove Member</option>
                </select>
              </div>
              <div>
                <span style={labelStyle}>Address</span>
                <input type="text" value={councilAddress} onChange={e => setCouncilAddress(e.target.value)}
                  placeholder="hex or bech32m address (tudg1...)" style={{ ...modalInputStyle, fontFamily: "'DM Mono',monospace" }} />
              </div>
              <div>
                <span style={labelStyle}>Category</span>
                <select value={councilCategory} onChange={e => setCouncilCategory(e.target.value)} style={modalInputStyle}>
                  {SEAT_CATEGORIES.map(c => <option key={c} value={c}>{c}</option>)}
                </select>
              </div>
            </div>
          )}

          {proposalType === 'TreasurySpend' && (
            <div style={sectionBoxStyle}>
              <div>
                <span style={labelStyle}>Recipient Address</span>
                <input type="text" value={treasuryRecipient} onChange={e => setTreasuryRecipient(e.target.value)}
                  placeholder="hex or bech32m address (tudg1...)" style={{ ...modalInputStyle, fontFamily: "'DM Mono',monospace" }} />
              </div>
              <div>
                <span style={labelStyle}>Amount (UDAG)</span>
                <input type="number" min="0" step="0.01" value={treasuryAmount} onChange={e => setTreasuryAmount(e.target.value)} style={modalInputStyle} />
              </div>
            </div>
          )}

          {/* Fee */}
          <div>
            <span style={labelStyle}>Fee (sats, min 10,000)</span>
            <input type="number" min="10000" value={fee} onChange={e => setFee(e.target.value)} style={modalInputStyle} />
          </div>

          {error && <p style={{ fontSize: 11, color: '#EF4444' }}>{error}</p>}

          <button onClick={handleSubmit} disabled={loading} style={{
            ...primaryButtonStyle, width: '100%', padding: '12px 0',
            opacity: loading ? 0.5 : 1,
          }}>
            {loading ? 'Creating...' : 'Create Proposal'}
          </button>
        </div>
      </div>
    </div>
  );
}
