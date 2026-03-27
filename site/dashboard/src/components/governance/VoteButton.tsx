import { useState } from 'react';
import { postVote, getNodeUrl } from '../../lib/api';
import { hasPasskeyWallet, getPasskeyWallet } from '../../lib/passkey-wallet';
import { signAndSubmitSmartOp } from '../../lib/webauthn-sign';

interface VoteButtonProps {
  proposalId: number;
  secretKey: string;
  approve: boolean;
  fee: number;
  onSuccess: () => void;
}

export function VoteButton({ proposalId, secretKey, approve, fee, onSuccess }: VoteButtonProps) {
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');

  const handleVote = async () => {
    setLoading(true);
    setError('');
    try {
      if (hasPasskeyWallet() && !secretKey) {
        const pw = getPasskeyWallet();
        if (!pw) throw new Error('No passkey wallet');
        const balRes = await fetch(`${getNodeUrl()}/balance/${pw.address}`, { signal: AbortSignal.timeout(5000) });
        const balData = await balRes.json();
        await signAndSubmitSmartOp(
          { Vote: { proposal_id: proposalId, approve } },
          fee, balData.nonce ?? 0,
        );
        onSuccess();
        return;
      }
      await postVote({
        secret_key: secretKey,
        proposal_id: proposalId,
        vote: approve,
        fee,
      });
      onSuccess();
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : 'Vote failed');
    } finally {
      setLoading(false);
    }
  };

  return (
    <div>
      <button
        onClick={handleVote}
        disabled={loading}
        className={`px-4 py-2 rounded text-sm font-medium transition-colors disabled:opacity-50 ${
          approve
            ? 'bg-dag-green/20 text-dag-green border border-dag-green/40 hover:bg-dag-green/30'
            : 'bg-dag-red/20 text-dag-red border border-dag-red/40 hover:bg-dag-red/30'
        }`}
      >
        {loading ? 'Voting...' : approve ? 'Vote YES' : 'Vote NO'}
      </button>
      {error && <p className="text-xs text-dag-red mt-1">{error}</p>}
    </div>
  );
}
