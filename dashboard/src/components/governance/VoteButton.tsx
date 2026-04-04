import { useState } from 'react';
import { postVote, getNodeUrl } from '../../lib/api';
import { hasPasskeyWallet, getPasskeyWallet } from '../../lib/passkey-wallet';
import { signAndSubmitSmartOp } from '../../lib/webauthn-sign';
import { primaryButtonStyle, dangerButtonStyle } from '../../lib/theme';

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

  const style = approve
    ? { ...primaryButtonStyle, padding: '8px 16px', fontSize: 12, opacity: loading ? 0.5 : 1 }
    : { ...dangerButtonStyle, padding: '8px 16px', fontSize: 12, opacity: loading ? 0.5 : 1 };

  return (
    <div>
      <button
        onClick={handleVote}
        disabled={loading}
        style={style}
      >
        {loading ? 'Voting...' : approve ? 'Vote YES' : 'Vote NO'}
      </button>
      {error && <p style={{ fontSize: 11, color: '#EF4444', marginTop: 4 }}>{error}</p>}
    </div>
  );
}
