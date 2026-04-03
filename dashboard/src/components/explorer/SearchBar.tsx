import { useState, type FormEvent } from 'react';
import { Search } from 'lucide-react';
import { useNavigate } from 'react-router-dom';
import { bech32ToHex, getNodeUrl } from '../../lib/api.ts';

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

  // ULTRA ID (alphanumeric + hyphens, 3-20 chars)
  if (/^[a-z0-9-]{3,20}$/.test(trimmed)) {
    return { type: 'ultra_id', value: trimmed };
  }

  return null;
}

export function SearchBar() {
  const [query, setQuery] = useState('');
  const [error, setError] = useState('');
  const navigate = useNavigate();

  const handleSearch = async (e: FormEvent) => {
    e.preventDefault();
    setError('');
    const result = detectSearchType(query);

    if (!result) {
      setError('Enter an ULTRA ID, round number, tx hash, or address');
      return;
    }

    if (result.type === 'round') {
      navigate(`/round/${result.value}`);
    } else if (result.type === 'bech32_address' || result.type === 'hex_address') {
      navigate(`/address/${result.value}`);
    } else if (result.type === 'ultra_id') {
      try {
        const res = await fetch(`${getNodeUrl()}/balance/${encodeURIComponent(result.value)}`, { signal: AbortSignal.timeout(4000) });
        if (res.ok) {
          const data = await res.json();
          if (data.address) { navigate(`/address/${data.address}`); return; }
        }
        setError(`ULTRA ID "${result.value}" not found`);
      } catch {
        setError('Network error resolving ULTRA ID');
      }
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
          placeholder="Search by ULTRA ID, address, tx hash, or round..."
          className="w-full pl-10 pr-4 py-3 bg-slate-800 border border-slate-700 rounded-lg text-slate-200 placeholder-slate-500 focus:outline-none focus:border-blue-500 focus:ring-1 focus:ring-blue-500 font-mono text-sm"
        />
      </div>
      {error && <p className="mt-2 text-sm text-red-400">{error}</p>}
    </form>
  );
}
