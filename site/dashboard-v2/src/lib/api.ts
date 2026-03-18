// UltraDAG Node API client
const NODES = [
  'https://ultradag-node-1.fly.dev',
  'https://ultradag-node-2.fly.dev',
  'https://ultradag-node-3.fly.dev',
  'https://ultradag-node-4.fly.dev',
  'https://ultradag-node-5.fly.dev',
];

let currentNode = NODES[0];
let connected = false;

export function getNodeUrl() { return currentNode; }
export function isConnected() { return connected; }

async function fetchJson<T>(path: string, options?: RequestInit): Promise<T> {
  const res = await fetch(currentNode + path, {
    ...options,
    headers: { 'Content-Type': 'application/json', ...options?.headers },
  });
  if (!res.ok) {
    const body = await res.text();
    throw new Error(body || `HTTP ${res.status}`);
  }
  return res.json();
}

export async function connectToNode(): Promise<boolean> {
  for (const node of NODES) {
    try {
      const res = await fetch(node + '/health', { signal: AbortSignal.timeout(5000) });
      if (res.ok) {
        currentNode = node;
        connected = true;
        return true;
      }
    } catch {}
  }
  connected = false;
  return false;
}

// GET endpoints
export const getStatus = () => fetchJson<any>('/status');
export const getBalance = (addr: string) => fetchJson<any>(`/balance/${addr}`);
export const getMempool = () => fetchJson<any>('/mempool');
export const getPeers = () => fetchJson<any>('/peers');
export const getValidators = () => fetchJson<any>('/validators');
export const getStake = (addr: string) => fetchJson<any>(`/stake/${addr}`);
export const getDelegation = (addr: string) => fetchJson<any>(`/delegation/${addr}`);
export const getProposals = () => fetchJson<any>('/proposals');
export const getProposal = (id: number) => fetchJson<any>(`/proposal/${id}`);
export const getCouncil = () => fetchJson<any>('/council');
export const getGovernanceConfig = () => fetchJson<any>('/governance/config');
export const getRound = (n: number) => fetchJson<any>(`/round/${n}`);
export const getVertex = (hash: string) => fetchJson<any>(`/vertex/${hash}`);
export const getTx = (hash: string) => fetchJson<any>(`/tx/${hash}`);
export const getFeeEstimate = () => fetchJson<any>('/fee-estimate');
export const getMetrics = () => fetchJson<any>('/metrics/json');
export const getHealthDetailed = () => fetchJson<any>('/health/detailed');

// POST endpoints
export const postTx = (body: any) => fetchJson<any>('/tx', { method: 'POST', body: JSON.stringify(body) });
export const postTxSubmit = (body: any) => fetchJson<any>('/tx/submit', { method: 'POST', body: JSON.stringify(body) });
export const postStake = (body: any) => fetchJson<any>('/stake', { method: 'POST', body: JSON.stringify(body) });
export const postUnstake = (body: any) => fetchJson<any>('/unstake', { method: 'POST', body: JSON.stringify(body) });
export const postDelegate = (body: any) => fetchJson<any>('/delegate', { method: 'POST', body: JSON.stringify(body) });
export const postUndelegate = (body: any) => fetchJson<any>('/undelegate', { method: 'POST', body: JSON.stringify(body) });
export const postSetCommission = (body: any) => fetchJson<any>('/set-commission', { method: 'POST', body: JSON.stringify(body) });
export const postFaucet = (body: any) => fetchJson<any>('/faucet', { method: 'POST', body: JSON.stringify(body) });
export const postProposal = (body: any) => fetchJson<any>('/proposal', { method: 'POST', body: JSON.stringify(body) });
export const postVote = (body: any) => fetchJson<any>('/vote', { method: 'POST', body: JSON.stringify(body) });

// Helpers
export function formatUdag(sats: number): string {
  return (sats / 100_000_000).toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 4 });
}

export function bytesToHex(bytes: number[]): string {
  return bytes.map(b => b.toString(16).padStart(2, '0')).join('');
}

export function formatProposalType(pt: unknown): string {
  if (typeof pt === 'string') return pt;
  if (pt && typeof pt === 'object') {
    const key = Object.keys(pt)[0];
    if (key === 'CouncilMembership') {
      const v = (pt as Record<string, Record<string, unknown>>)[key];
      const addr = Array.isArray(v.address) ? shortAddr(bytesToHex(v.address as number[])) : shortAddr(String(v.address));
      return `Council: ${v.action} ${v.category} (${addr})`;
    }
    if (key === 'ParameterChange') {
      const v = (pt as Record<string, Record<string, unknown>>)[key];
      return `Param: ${v.param} → ${v.value}`;
    }
    if (key === 'TreasurySpend') {
      const v = (pt as Record<string, Record<string, unknown>>)[key];
      const addr = Array.isArray(v.recipient) ? shortAddr(bytesToHex(v.recipient as number[])) : shortAddr(String(v.recipient));
      return `Treasury: ${formatUdag(v.amount as number)} to ${addr}`;
    }
    if (key === 'Text') return 'Text';
    return key;
  }
  return String(pt);
}

export function shortHash(hash: string): string {
  if (!hash || hash.length < 12) return hash || '';
  return hash.slice(0, 8) + '...' + hash.slice(-4);
}

export function shortAddr(addr: string): string {
  if (!addr || addr.length < 16) return addr || '';
  return addr.slice(0, 10) + '...' + addr.slice(-6);
}
