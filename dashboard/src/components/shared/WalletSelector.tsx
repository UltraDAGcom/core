import type { Wallet } from '../../lib/keystore';
import { useName } from '../../contexts/NameCacheContext';

interface WalletSelectorProps {
  wallets: Wallet[];
  selectedIdx: number;
  onChange: (idx: number) => void;
  label?: string;
}

function WalletOption({ wallet, value }: { wallet: { name: string; address: string }; value: number }) {
  const { name: ultraId } = useName(wallet.address);
  const display = ultraId ? `@${ultraId}` : wallet.name;
  return <option value={value}>{display}</option>;
}

export function WalletSelector({ wallets, selectedIdx, onChange, label = 'Wallet' }: WalletSelectorProps) {
  if (wallets.length === 0) {
    return <p className="text-dag-muted text-sm">No wallets. Create one in the wallet page first.</p>;
  }

  return (
    <label className="block">
      <span className="text-sm text-dag-muted">{label}</span>
      <select
        value={selectedIdx}
        onChange={e => onChange(Number(e.target.value))}
        className="mt-1 block w-full rounded bg-dag-surface border border-dag-border px-3 py-2 text-sm text-white"
      >
        {wallets.map((w, i) => (
          <WalletOption key={i} wallet={w} value={i} />
        ))}
      </select>
    </label>
  );
}
