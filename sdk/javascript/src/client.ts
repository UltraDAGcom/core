import {
  UltraDagError,
  type HealthResponse,
  type StatusResponse,
  type BalanceResponse,
  type RoundVertex,
  type MempoolTransaction,
  type PeerInfo,
  type KeygenResponse,
  type ValidatorsResponse,
  type StakeInfoResponse,
  type GovernanceConfig,
  type ProposalsResponse,
  type ProposalDetail,
  type VoteInfo,
  type SendTxRequest,
  type SendTxResponse,
  type FaucetRequest,
  type FaucetResponse,
  type StakeRequest,
  type StakeResponse,
  type UnstakeRequest,
  type UnstakeResponse,
  type CreateProposalRequest,
  type CreateProposalResponse,
  type CastVoteRequest,
  type CastVoteResponse,
} from "./types.js";

// ---------------------------------------------------------------------------
// Client options
// ---------------------------------------------------------------------------

export interface UltraDagClientOptions {
  /**
   * Base URL of the UltraDAG node RPC server.
   * @default "http://localhost:10333"
   */
  baseUrl?: string;

  /**
   * Optional custom `fetch` implementation (useful for testing or
   * environments without a global `fetch`).
   */
  fetch?: (url: string, init?: RequestInit) => Promise<Response>;
}

// ---------------------------------------------------------------------------
// Client
// ---------------------------------------------------------------------------

/**
 * HTTP RPC client for an UltraDAG node.
 *
 * All amounts are expressed in **sats** (the smallest unit) unless a field
 * name explicitly ends with `_udag` or `_tdag`.
 *
 * ```ts
 * import { UltraDagClient } from "ultradag";
 *
 * const client = new UltraDagClient({ baseUrl: "http://localhost:10333" });
 * const status = await client.getStatus();
 * console.log(status.last_finalized_round);
 * ```
 */
export class UltraDagClient {
  private readonly baseUrl: string;
  private readonly _fetch: (url: string, init?: RequestInit) => Promise<Response>;

  constructor(options: UltraDagClientOptions = {}) {
    this.baseUrl = (options.baseUrl ?? "http://localhost:10333").replace(
      /\/$/,
      "",
    );
    this._fetch = options.fetch ?? globalThis.fetch;
  }

  // -----------------------------------------------------------------------
  // Internal helpers
  // -----------------------------------------------------------------------

  private async request<T>(
    method: "GET" | "POST",
    path: string,
    body?: unknown,
  ): Promise<T> {
    const url = `${this.baseUrl}${path}`;
    const init: RequestInit = {
      method,
      headers: { "Content-Type": "application/json" },
    };
    if (body !== undefined) {
      init.body = JSON.stringify(body);
    }

    let res: Response;
    try {
      res = await this._fetch(url, init);
    } catch (err: unknown) {
      const message =
        err instanceof Error ? err.message : "Network request failed";
      throw new UltraDagError(message);
    }

    const text = await res.text();

    if (!res.ok) {
      throw new UltraDagError(
        `HTTP ${res.status}: ${text}`,
        res.status,
        text,
      );
    }

    try {
      return JSON.parse(text) as T;
    } catch {
      throw new UltraDagError("Invalid JSON response from node", res.status, text);
    }
  }

  private get<T>(path: string): Promise<T> {
    return this.request<T>("GET", path);
  }

  private post<T>(path: string, body: unknown): Promise<T> {
    return this.request<T>("POST", path, body);
  }

  // -----------------------------------------------------------------------
  // GET endpoints
  // -----------------------------------------------------------------------

  /**
   * Check node health.
   * @returns `{ status: "ok" }` when the node is reachable.
   */
  async getHealth(): Promise<HealthResponse> {
    return this.get<HealthResponse>("/health");
  }

  /**
   * Retrieve high-level node status including DAG metrics, supply, and peer
   * information.
   */
  async getStatus(): Promise<StatusResponse> {
    return this.get<StatusResponse>("/status");
  }

  /**
   * Query the balance and nonce for an address.
   * @param address - 64-character hex address.
   */
  async getBalance(address: string): Promise<BalanceResponse> {
    return this.get<BalanceResponse>(`/balance/${address}`);
  }

  /**
   * List all DAG vertices produced in a given round.
   * @param round - Round number (non-negative integer).
   */
  async getRound(round: number): Promise<RoundVertex[]> {
    return this.get<RoundVertex[]>(`/round/${round}`);
  }

  /**
   * List pending mempool transactions (up to 100, ordered by fee
   * descending).
   */
  async getMempool(): Promise<MempoolTransaction[]> {
    return this.get<MempoolTransaction[]>("/mempool");
  }

  /**
   * Retrieve connected peer information and bootstrap node status.
   */
  async getPeers(): Promise<PeerInfo> {
    return this.get<PeerInfo>("/peers");
  }

  /**
   * Ask the node to generate a fresh Ed25519 keypair (server-side).
   *
   * For local (offline) key generation see `Keypair.generate()` from the
   * crypto module.
   */
  async keygen(): Promise<KeygenResponse> {
    return this.get<KeygenResponse>("/keygen");
  }

  /**
   * List active validators with their stake amounts.
   */
  async getValidators(): Promise<ValidatorsResponse> {
    return this.get<ValidatorsResponse>("/validators");
  }

  /**
   * Query staking information for an address.
   * @param address - 64-character hex address.
   */
  async getStakeInfo(address: string): Promise<StakeInfoResponse> {
    return this.get<StakeInfoResponse>(`/stake/${address}`);
  }

  /**
   * Retrieve the current governance configuration.
   */
  async getGovernanceConfig(): Promise<GovernanceConfig> {
    return this.get<GovernanceConfig>("/governance/config");
  }

  /**
   * List all governance proposals.
   */
  async getProposals(): Promise<ProposalsResponse> {
    return this.get<ProposalsResponse>("/proposals");
  }

  /**
   * Retrieve details for a specific governance proposal.
   * @param id - Proposal ID.
   */
  async getProposal(id: number): Promise<ProposalDetail> {
    return this.get<ProposalDetail>(`/proposal/${id}`);
  }

  /**
   * Check how an address voted on a proposal.
   * @param proposalId - Proposal ID.
   * @param address - 64-character hex address.
   */
  async getVote(proposalId: number, address: string): Promise<VoteInfo> {
    return this.get<VoteInfo>(`/vote/${proposalId}/${address}`);
  }

  // -----------------------------------------------------------------------
  // POST endpoints
  // -----------------------------------------------------------------------

  /**
   * Submit a signed transaction to the network.
   *
   * The node signs the transaction server-side using the provided secret key.
   *
   * @param params - Transaction parameters.
   * @param params.secret_key - 64-char hex secret key of the sender.
   * @param params.to - 64-char hex destination address.
   * @param params.amount - Amount in sats.
   * @param params.fee - Fee in sats.
   */
  async sendTransaction(params: SendTxRequest): Promise<SendTxResponse> {
    return this.post<SendTxResponse>("/tx", params);
  }

  /**
   * Request testnet tokens from the faucet.
   *
   * @param address - Recipient address.
   * @param amount - Amount in sats.
   */
  async faucet(address: string, amount: number): Promise<FaucetResponse> {
    const body: FaucetRequest = { address, amount };
    return this.post<FaucetResponse>("/faucet", body);
  }

  /**
   * Stake UDAG to become a validator (or increase existing stake).
   *
   * @param params - Staking parameters.
   * @param params.secret_key - 64-char hex secret key.
   * @param params.amount - Amount in sats to stake.
   */
  async stake(params: StakeRequest): Promise<StakeResponse> {
    return this.post<StakeResponse>("/stake", params);
  }

  /**
   * Begin the unstaking cooldown period.
   *
   * @param params - Unstake parameters.
   * @param params.secret_key - 64-char hex secret key.
   */
  async unstake(params: UnstakeRequest): Promise<UnstakeResponse> {
    return this.post<UnstakeResponse>("/unstake", params);
  }

  /**
   * Create a governance proposal.
   *
   * @param params - Proposal parameters.
   */
  async createProposal(
    params: CreateProposalRequest,
  ): Promise<CreateProposalResponse> {
    return this.post<CreateProposalResponse>("/proposal", params);
  }

  /**
   * Cast a vote on a governance proposal.
   *
   * @param params - Vote parameters.
   */
  async castVote(params: CastVoteRequest): Promise<CastVoteResponse> {
    return this.post<CastVoteResponse>("/vote", params);
  }

  // -----------------------------------------------------------------------
  // Client-side signed transaction submission
  // -----------------------------------------------------------------------

  /**
   * Submit a pre-signed transaction to the network via `/tx/submit`.
   *
   * This is the **mainnet transaction path** — no secret keys are sent to
   * the server.  Build the transaction object using the `buildSigned*Tx`
   * helpers from `transactions.ts`, then pass it here.
   *
   * @param tx - A fully-signed Transaction object matching the Rust
   *   `Transaction` serde JSON format (e.g. `{ Transfer: { ... } }`).
   * @returns `{ status: "pending", tx_hash: string }` on success.
   */
  async submitSignedTransaction(
    tx: object,
  ): Promise<{ status: string; tx_hash: string }> {
    return this.post<{ status: string; tx_hash: string }>("/tx/submit", tx);
  }

  // ---------------------------------------------------------------------------
  // SmartAccount endpoints
  // ---------------------------------------------------------------------------

  /** Get SmartAccount configuration for an address (or name). */
  async getSmartAccount(addressOrName: string): Promise<object | null> {
    try {
      return await this.get<object>(`/smart-account/${encodeURIComponent(addressOrName)}`);
    } catch {
      return null;
    }
  }

  // ---------------------------------------------------------------------------
  // Name Registry endpoints
  // ---------------------------------------------------------------------------

  /** Resolve a name to an address. Returns null if not found. */
  async resolveName(name: string): Promise<{ name: string; address: string; expiry_round: number } | null> {
    try {
      return await this.get(`/name/resolve/${encodeURIComponent(name)}`);
    } catch {
      return null;
    }
  }

  /** Reverse lookup: address to name. */
  async reverseName(address: string): Promise<{ address: string; name: string } | null> {
    try {
      return await this.get(`/name/reverse/${encodeURIComponent(address)}`);
    } catch {
      return null;
    }
  }

  /** Check if a name is available and its price. */
  async checkNameAvailability(name: string): Promise<{ available: boolean; valid: boolean; annual_fee: number }> {
    return this.get(`/name/available/${encodeURIComponent(name)}`);
  }

  /** Get full info for a registered name. */
  async getNameInfo(name: string): Promise<object | null> {
    try {
      return await this.get(`/name/info/${encodeURIComponent(name)}`);
    } catch {
      return null;
    }
  }
}
