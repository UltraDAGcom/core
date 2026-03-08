// ---------------------------------------------------------------------------
// Shared constants
// ---------------------------------------------------------------------------

/** 1 UDAG = 100,000,000 sats */
export const SATS_PER_UDAG = 100_000_000;

// ---------------------------------------------------------------------------
// Unit conversion helpers
// ---------------------------------------------------------------------------

/**
 * Convert an amount in sats to UDAG.
 * @param sats - Amount in sats (smallest unit).
 * @returns Amount in UDAG as a floating-point number.
 */
export function satsToUdag(sats: number | bigint): number {
  return Number(sats) / SATS_PER_UDAG;
}

/**
 * Convert an amount in UDAG to sats.
 * @param udag - Amount in UDAG.
 * @returns Amount in sats as an integer.
 */
export function udagToSats(udag: number): number {
  return Math.round(udag * SATS_PER_UDAG);
}

// ---------------------------------------------------------------------------
// GET /health
// ---------------------------------------------------------------------------

export interface HealthResponse {
  status: string;
}

// ---------------------------------------------------------------------------
// GET /status
// ---------------------------------------------------------------------------

export interface StatusResponse {
  last_finalized_round: number;
  peer_count: number;
  mempool_size: number;
  total_supply: number;
  account_count: number;
  dag_vertices: number;
  dag_round: number;
  dag_tips: number;
  finalized_count: number;
  validator_count: number;
  total_staked: number;
  active_stakers: number;
  bootstrap_connected: boolean;
}

// ---------------------------------------------------------------------------
// GET /balance/:address
// ---------------------------------------------------------------------------

export interface BalanceResponse {
  address: string;
  balance: number;
  nonce: number;
  balance_tdag: number;
}

// ---------------------------------------------------------------------------
// GET /round/:round
// ---------------------------------------------------------------------------

export interface RoundVertex {
  round: number;
  hash: string;
  validator: string;
  reward: number;
  tx_count: number;
  parent_count: number;
}

// ---------------------------------------------------------------------------
// GET /mempool
// ---------------------------------------------------------------------------

export interface MempoolTransaction {
  from: string;
  to: string;
  amount: number;
  fee: number;
  nonce: number;
  hash?: string;
  [key: string]: unknown;
}

// ---------------------------------------------------------------------------
// GET /peers
// ---------------------------------------------------------------------------

export interface PeerInfo {
  connected: number;
  peers: string[];
  bootstrap_nodes: string[];
}

// ---------------------------------------------------------------------------
// GET /keygen
// ---------------------------------------------------------------------------

export interface KeygenResponse {
  secret_key: string;
  address: string;
}

// ---------------------------------------------------------------------------
// GET /validators
// ---------------------------------------------------------------------------

export interface ValidatorInfo {
  address: string;
  staked: number;
  staked_udag: number;
}

export interface ValidatorsResponse {
  count: number;
  total_staked: number;
  validators: ValidatorInfo[];
}

// ---------------------------------------------------------------------------
// GET /stake/:address
// ---------------------------------------------------------------------------

export interface StakeInfoResponse {
  address: string;
  staked: number;
  staked_udag: number;
  unlock_at_round: number | null;
  is_active_validator: boolean;
}

// ---------------------------------------------------------------------------
// GET /governance/config
// ---------------------------------------------------------------------------

export interface GovernanceConfig {
  [key: string]: unknown;
}

// ---------------------------------------------------------------------------
// GET /proposals
// ---------------------------------------------------------------------------

export interface Proposal {
  id: number;
  [key: string]: unknown;
}

export interface ProposalsResponse {
  count: number;
  proposals: Proposal[];
}

// ---------------------------------------------------------------------------
// GET /proposal/:id
// ---------------------------------------------------------------------------

export interface ProposalDetail {
  id: number;
  [key: string]: unknown;
}

// ---------------------------------------------------------------------------
// GET /vote/:proposal_id/:address
// ---------------------------------------------------------------------------

export interface VoteInfo {
  [key: string]: unknown;
}

// ---------------------------------------------------------------------------
// POST /tx
// ---------------------------------------------------------------------------

export interface SendTxRequest {
  secret_key: string;
  to: string;
  amount: number;
  fee: number;
}

export interface SendTxResponse {
  hash: string;
  from: string;
  to: string;
  amount: number;
  fee: number;
  nonce: number;
}

// ---------------------------------------------------------------------------
// POST /faucet
// ---------------------------------------------------------------------------

export interface FaucetRequest {
  address: string;
  amount: number;
}

export interface FaucetResponse {
  tx_hash: string;
  from: string;
  to: string;
  amount: number;
  amount_udag: number;
  nonce: number;
}

// ---------------------------------------------------------------------------
// POST /stake
// ---------------------------------------------------------------------------

export interface StakeRequest {
  secret_key: string;
  amount: number;
}

export interface StakeResponse {
  status: string;
  tx_hash: string;
  address: string;
  amount: number;
  amount_udag: number;
  nonce: number;
  note: string;
}

// ---------------------------------------------------------------------------
// POST /unstake
// ---------------------------------------------------------------------------

export interface UnstakeRequest {
  secret_key: string;
}

export interface UnstakeResponse {
  status: string;
  tx_hash: string;
  address: string;
  unlock_at_round: number;
  nonce: number;
  note: string;
}

// ---------------------------------------------------------------------------
// POST /proposal
// ---------------------------------------------------------------------------

export interface CreateProposalRequest {
  proposer_secret: string;
  title: string;
  description: string;
  proposal_type: string;
  parameter_name?: string;
  parameter_value?: string;
  fee?: number;
}

export interface CreateProposalResponse {
  [key: string]: unknown;
}

// ---------------------------------------------------------------------------
// POST /vote
// ---------------------------------------------------------------------------

export interface CastVoteRequest {
  voter_secret: string;
  proposal_id: number;
  vote: string;
  fee?: number;
}

export interface CastVoteResponse {
  [key: string]: unknown;
}

// ---------------------------------------------------------------------------
// Error
// ---------------------------------------------------------------------------

/**
 * Error thrown by UltraDagClient when an API call fails.
 */
export class UltraDagError extends Error {
  /** HTTP status code, if available. */
  public readonly status: number | undefined;
  /** Raw response body, if available. */
  public readonly body: string | undefined;

  constructor(message: string, status?: number, body?: string) {
    super(message);
    this.name = "UltraDagError";
    this.status = status;
    this.body = body;
  }
}
