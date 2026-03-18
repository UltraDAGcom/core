import { useState, type FormEvent } from 'react';
import { Search } from 'lucide-react';
import { useNavigate } from 'react-router-dom';

function detectSearchType(query: string): { type: string; value: string } | null {
  const trimmed = query.trim();
  if (!trimmed) return null;

  // Round number
  if (/^\d+$/.test(trimmed)) {
    return { type: 'round', value: trimmed };
  }

  // 64-char hex = hash (tx, vertex, or address)
  if (/^[0-9a-fA-F]{64}$/.test(trimmed)) {
    return { type: 'hex64', value: trimmed.toLowerCase() };
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
      setError('Enter a round number or 64-char hex hash (tx, vertex, or address)');
      return;
    }

    if (result.type === 'round') {
      navigate(`/round/${result.value}`);
    } else {
      // hex64 - navigate to a smart search page that tries tx -> vertex -> address
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
          placeholder="Search by round, tx hash, vertex hash, or address..."
          className="w-full pl-10 pr-4 py-3 bg-slate-800 border border-slate-700 rounded-lg text-slate-200 placeholder-slate-500 focus:outline-none focus:border-blue-500 focus:ring-1 focus:ring-blue-500 font-mono text-sm"
        />
      </div>
      {error && <p className="mt-2 text-sm text-red-400">{error}</p>}
    </form>
  );
}
