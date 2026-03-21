import { useState, type FormEvent } from 'react';
import { Search } from 'lucide-react';
import { useNavigate } from 'react-router-dom';
import { bech32ToHex } from '../../lib/api.ts';

function detectSearchType(query: string): { type: string; value: string } | null {
  const trimmed = query.trim();
  if (!trimmed) return null;

  // Round number
  if (/^\d+$/.test(trimmed)) {
    return { type: 'round', value: trimmed };
  }

  // 40-char hex = address
  if (/^[0-9a-fA-F]{40}$/.test(trimmed)) {
    return { type: 'hex_address', value: trimmed.toLowerCase() };
  }

  // 64-char hex = hash (tx or vertex)
  if (/^[0-9a-fA-F]{64}$/.test(trimmed)) {
    return { type: 'hex64', value: trimmed.toLowerCase() };
  }

  // Bech32m address (udag1... or tudg1...)
  if (/^(?:udag1|tudg1)/i.test(trimmed)) {
    const hex = bech32ToHex(trimmed);
    if (hex) return { type: 'bech32_address', value: hex };
  }

  return null;
}

export function SearchBar() {
  const [query, setQuery] = useState('');
  const [error, setError] = useState('');
  const navigate = useNavigate();

  const handleSearch = (e: FormEvent) => {
    e.preventDefault();
    setError('');
    const result = detectSearchType(query);

    if (!result) {
      setError('Enter a round number, hex hash, or bech32m address (tudg1.../udag1...)');
      return;
    }

    if (result.type === 'round') {
      navigate(`/round/${result.value}`);
    } else if (result.type === 'bech32_address' || result.type === 'hex_address') {
      navigate(`/address/${result.value}`);
    } else {
      // hex64 - navigate to a smart search page that tries tx -> vertex
      navigate(`/search/${result.value}`);
    }
  };

  return (
    <form onSubmit={handleSearch} className="w-full">
      <div className="relative">
        <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-5 h-5 text-slate-400" />
        <input
          type="text"
          value={query}
          onChange={(e) => { setQuery(e.target.value); setError(''); }}
          placeholder="Search by round, tx hash, vertex hash, or address (hex/bech32m)..."
          className="w-full pl-10 pr-4 py-3 bg-slate-800 border border-slate-700 rounded-lg text-slate-200 placeholder-slate-500 focus:outline-none focus:border-blue-500 focus:ring-1 focus:ring-blue-500 font-mono text-sm"
        />
      </div>
      {error && <p className="mt-2 text-sm text-red-400">{error}</p>}
    </form>
  );
}
