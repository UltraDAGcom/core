/**
 * GitHub API client for bug bounty integration.
 * Reads bounty issues from the UltraDAG repo (public, no auth needed).
 */

const REPO = 'UltraDAGcom/core';
const API_BASE = 'https://api.github.com';
const CACHE_TTL_MS = 120_000; // 2 minutes

// ── Types ────────────────────────────────────────────────────────────────

export interface GitHubIssue {
  id: number;
  number: number;
  title: string;
  body: string | null;
  state: 'open' | 'closed';
  html_url: string;
  created_at: string;
  updated_at: string;
  user: { login: string; avatar_url: string } | null;
  labels: Array<{ name: string; color: string }>;
  assignee: { login: string } | null;
  comments: number;
}

export type BountyCategory = 'security-critical' | 'security-high' | 'security-medium' | 'security-low' | 'bug' | 'feature';
export type BountyStatus = 'open' | 'claimed' | 'in-review' | 'approved' | 'paid' | 'cancelled';

export interface ParsedBounty {
  issue: GitHubIssue;
  category: BountyCategory;
  reward: number;         // UDAG (float)
  rewardSats: number;     // sats (integer)
  creatorAddress: string | null;
  status: BountyStatus;
  severity: string;       // Human-readable
  severityColor: string;
}

// ── Severity info ────────────────────────────────────────────────────────

const SEVERITY_MAP: Record<BountyCategory, { label: string; color: string }> = {
  'security-critical': { label: 'Critical', color: '#EF4444' },
  'security-high':     { label: 'High',     color: '#F97316' },
  'security-medium':   { label: 'Medium',   color: '#FFB800' },
  'security-low':      { label: 'Low',      color: '#00E0C4' },
  'bug':               { label: 'Bug',      color: '#0066FF' },
  'feature':           { label: 'Feature',  color: '#A855F7' },
};

export function severityInfo(cat: BountyCategory) { return SEVERITY_MAP[cat]; }

const STATUS_COLORS: Record<BountyStatus, string> = {
  'open':      '#00E0C4',
  'claimed':   '#0066FF',
  'in-review': '#FFB800',
  'approved':  '#A855F7',
  'paid':      '#34d399',
  'cancelled': '#6b7280',
};

export function statusColor(status: BountyStatus) { return STATUS_COLORS[status]; }

// ── Parsing ──────────────────────────────────────────────────────────────

const SATS = 100_000_000;

export function parseFrontmatter(body: string | null): { reward?: number; creator_address?: string } {
  if (!body) return {};
  const match = body.match(/^---\s*\n([\s\S]*?)\n---/);
  if (!match) {
    // Fallback: try to extract reward from title-like pattern [500 UDAG]
    const rewardMatch = body.match(/\[(\d+(?:\.\d+)?)\s*UDAG\]/i);
    return rewardMatch ? { reward: parseFloat(rewardMatch[1]) } : {};
  }
  const lines = match[1].split('\n');
  const result: Record<string, string> = {};
  for (const line of lines) {
    const kv = line.match(/^(\w+)\s*:\s*(.+)$/);
    if (kv) result[kv[1].toLowerCase()] = kv[2].trim();
  }
  return {
    reward: result.reward ? parseFloat(result.reward) : undefined,
    creator_address: result.creator_address || result.creator || undefined,
  };
}

export function categoryFromLabels(labels: Array<{ name: string }>): BountyCategory {
  const names = labels.map(l => l.name.toLowerCase());
  if (names.includes('bounty:security-critical')) return 'security-critical';
  if (names.includes('bounty:security-high'))     return 'security-high';
  if (names.includes('bounty:security-medium'))   return 'security-medium';
  if (names.includes('bounty:security-low'))      return 'security-low';
  if (names.includes('bounty:bug'))               return 'bug';
  if (names.includes('bounty:feature'))           return 'feature';
  // Default based on partial matches
  if (names.some(n => n.includes('security')))    return 'security-medium';
  if (names.some(n => n.includes('bug')))         return 'bug';
  if (names.some(n => n.includes('feature')))     return 'feature';
  return 'bug';
}

export function deriveBountyStatus(issue: GitHubIssue): BountyStatus {
  const labels = issue.labels.map(l => l.name.toLowerCase());
  if (labels.includes('paid'))      return 'paid';
  if (labels.includes('approved'))  return 'approved';
  if (labels.includes('in-review')) return 'in-review';
  if (labels.includes('claimed'))   return 'claimed';
  if (issue.state === 'closed' && !labels.includes('paid')) return 'cancelled';
  return 'open';
}

export function parseBounty(issue: GitHubIssue): ParsedBounty {
  const fm = parseFrontmatter(issue.body);
  const category = categoryFromLabels(issue.labels);
  const info = severityInfo(category);
  const reward = fm.reward ?? 0;

  // Also check title for reward pattern: "Fix X [500 UDAG]"
  let finalReward = reward;
  if (!finalReward) {
    const titleMatch = issue.title.match(/\[(\d+(?:\.\d+)?)\s*UDAG\]/i);
    if (titleMatch) finalReward = parseFloat(titleMatch[1]);
  }

  return {
    issue,
    category,
    reward: finalReward,
    rewardSats: Math.round(finalReward * SATS),
    creatorAddress: fm.creator_address ?? null,
    status: deriveBountyStatus(issue),
    severity: info.label,
    severityColor: info.color,
  };
}

// ── Description extraction (strip frontmatter) ──────────────────────────

export function bountyDescription(body: string | null): string {
  if (!body) return '';
  return body.replace(/^---\s*\n[\s\S]*?\n---\s*\n?/, '').trim();
}

// ── API fetch with caching ───────────────────────────────────────────────

let cachedIssues: GitHubIssue[] = [];
let cacheTime = 0;
let lastRateLimit: number | null = null;

export async function fetchBountyIssues(): Promise<{ issues: GitHubIssue[]; rateLimitRemaining: number | null }> {
  if (Date.now() - cacheTime < CACHE_TTL_MS && cachedIssues.length > 0) {
    return { issues: cachedIssues, rateLimitRemaining: lastRateLimit };
  }

  const url = `${API_BASE}/repos/${REPO}/issues?labels=bounty&state=all&per_page=100&sort=updated&direction=desc`;
  const res = await fetch(url, { signal: AbortSignal.timeout(10000) });

  lastRateLimit = res.headers.get('X-RateLimit-Remaining') ? parseInt(res.headers.get('X-RateLimit-Remaining')!) : null;

  if (res.status === 403) {
    // Rate limited — return cached data
    return { issues: cachedIssues, rateLimitRemaining: 0 };
  }

  if (!res.ok) {
    throw new Error(`GitHub API error: ${res.status}`);
  }

  const issues: GitHubIssue[] = await res.json();
  cachedIssues = issues;
  cacheTime = Date.now();

  return { issues, rateLimitRemaining: lastRateLimit };
}

// ── Time formatting ──────────────────────────────────────────────────────

export function timeAgo(dateStr: string): string {
  const diff = Date.now() - new Date(dateStr).getTime();
  const mins = Math.floor(diff / 60000);
  if (mins < 1) return 'just now';
  if (mins < 60) return `${mins}m ago`;
  const hours = Math.floor(mins / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  if (days < 30) return `${days}d ago`;
  const months = Math.floor(days / 30);
  return `${months}mo ago`;
}
