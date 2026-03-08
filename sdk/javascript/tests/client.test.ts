import { describe, it, expect, vi, beforeEach } from "vitest";
import { UltraDagClient } from "../src/client.js";
import { UltraDagError, satsToUdag, udagToSats, SATS_PER_UDAG } from "../src/types.js";

// ---------------------------------------------------------------------------
// Mock fetch helper
// ---------------------------------------------------------------------------

function mockFetch(body: unknown, status = 200): typeof globalThis.fetch {
  return vi.fn().mockResolvedValue({
    ok: status >= 200 && status < 300,
    status,
    text: () => Promise.resolve(JSON.stringify(body)),
  }) as unknown as typeof globalThis.fetch;
}

function mockFetchText(text: string, status = 200): typeof globalThis.fetch {
  return vi.fn().mockResolvedValue({
    ok: status >= 200 && status < 300,
    status,
    text: () => Promise.resolve(text),
  }) as unknown as typeof globalThis.fetch;
}

function failingFetch(): typeof globalThis.fetch {
  return vi.fn().mockRejectedValue(new Error("Connection refused")) as unknown as typeof globalThis.fetch;
}

// ---------------------------------------------------------------------------
// Unit helpers
// ---------------------------------------------------------------------------

describe("satsToUdag / udagToSats", () => {
  it("converts sats to UDAG", () => {
    expect(satsToUdag(100_000_000)).toBe(1);
    expect(satsToUdag(50_000_000)).toBe(0.5);
    expect(satsToUdag(0)).toBe(0);
  });

  it("converts UDAG to sats", () => {
    expect(udagToSats(1)).toBe(100_000_000);
    expect(udagToSats(0.5)).toBe(50_000_000);
    expect(udagToSats(0)).toBe(0);
  });

  it("round-trips correctly", () => {
    expect(udagToSats(satsToUdag(123_456_789))).toBe(123_456_789);
  });

  it("SATS_PER_UDAG is 100 million", () => {
    expect(SATS_PER_UDAG).toBe(100_000_000);
  });
});

// ---------------------------------------------------------------------------
// GET endpoints
// ---------------------------------------------------------------------------

describe("UltraDagClient GET endpoints", () => {
  let client: UltraDagClient;
  let fetchFn: ReturnType<typeof vi.fn>;

  function setup(body: unknown, status = 200) {
    const f = mockFetch(body, status);
    fetchFn = f as unknown as ReturnType<typeof vi.fn>;
    client = new UltraDagClient({ baseUrl: "http://node:10333", fetch: f });
  }

  // -- /health ------------------------------------------------------------

  it("getHealth returns status", async () => {
    setup({ status: "ok" });
    const res = await client.getHealth();
    expect(res.status).toBe("ok");
    expect(fetchFn).toHaveBeenCalledWith(
      "http://node:10333/health",
      expect.objectContaining({ method: "GET" }),
    );
  });

  // -- /status ------------------------------------------------------------

  it("getStatus returns node metrics", async () => {
    const data = {
      last_finalized_round: 42,
      peer_count: 3,
      mempool_size: 10,
      total_supply: 2_100_000_000_000_000,
      account_count: 5,
      dag_vertices: 200,
      dag_round: 45,
      dag_tips: 4,
      finalized_count: 180,
      validator_count: 4,
      total_staked: 100_000_000_000,
      active_stakers: 4,
      bootstrap_connected: true,
    };
    setup(data);
    const res = await client.getStatus();
    expect(res.last_finalized_round).toBe(42);
    expect(res.validator_count).toBe(4);
    expect(res.bootstrap_connected).toBe(true);
  });

  // -- /balance/:address --------------------------------------------------

  it("getBalance returns balance info", async () => {
    const data = {
      address: "aa".repeat(32),
      balance: 500_000_000,
      nonce: 3,
      balance_tdag: 5.0,
    };
    setup(data);
    const res = await client.getBalance("aa".repeat(32));
    expect(res.balance).toBe(500_000_000);
    expect(res.nonce).toBe(3);
    expect(fetchFn).toHaveBeenCalledWith(
      `http://node:10333/balance/${"aa".repeat(32)}`,
      expect.anything(),
    );
  });

  // -- /round/:round ------------------------------------------------------

  it("getRound returns vertex list", async () => {
    const vertices = [
      { round: 10, hash: "ab".repeat(32), validator: "cd".repeat(32), reward: 50_000_000_00, tx_count: 2, parent_count: 3 },
    ];
    setup(vertices);
    const res = await client.getRound(10);
    expect(res).toHaveLength(1);
    expect(res[0].round).toBe(10);
    expect(fetchFn).toHaveBeenCalledWith(
      "http://node:10333/round/10",
      expect.anything(),
    );
  });

  // -- /mempool -----------------------------------------------------------

  it("getMempool returns transaction list", async () => {
    const txs = [{ from: "aa".repeat(32), to: "bb".repeat(32), amount: 1000, fee: 100, nonce: 1 }];
    setup(txs);
    const res = await client.getMempool();
    expect(res).toHaveLength(1);
    expect(res[0].amount).toBe(1000);
  });

  // -- /peers -------------------------------------------------------------

  it("getPeers returns peer info", async () => {
    setup({ connected: 3, peers: ["1.2.3.4:9333"], bootstrap_nodes: ["5.6.7.8:9333"] });
    const res = await client.getPeers();
    expect(res.connected).toBe(3);
    expect(res.peers).toContain("1.2.3.4:9333");
  });

  // -- /keygen ------------------------------------------------------------

  it("keygen returns new keypair", async () => {
    setup({ secret_key: "ab".repeat(32), address: "cd".repeat(32) });
    const res = await client.keygen();
    expect(res.secret_key).toHaveLength(64);
    expect(res.address).toHaveLength(64);
  });

  // -- /validators --------------------------------------------------------

  it("getValidators returns validator list", async () => {
    const data = {
      count: 2,
      total_staked: 200_000_000_000,
      validators: [
        { address: "aa".repeat(32), staked: 100_000_000_000, staked_udag: 1000 },
        { address: "bb".repeat(32), staked: 100_000_000_000, staked_udag: 1000 },
      ],
    };
    setup(data);
    const res = await client.getValidators();
    expect(res.count).toBe(2);
    expect(res.validators).toHaveLength(2);
  });

  // -- /stake/:address ----------------------------------------------------

  it("getStakeInfo returns stake details", async () => {
    const data = {
      address: "aa".repeat(32),
      staked: 100_000_000_000,
      staked_udag: 1000,
      unlock_at_round: null,
      is_active_validator: true,
    };
    setup(data);
    const res = await client.getStakeInfo("aa".repeat(32));
    expect(res.is_active_validator).toBe(true);
    expect(res.staked).toBe(100_000_000_000);
  });

  // -- /governance/config -------------------------------------------------

  it("getGovernanceConfig returns config object", async () => {
    setup({ voting_period: 1000, quorum: 50 });
    const res = await client.getGovernanceConfig();
    expect(res).toHaveProperty("voting_period");
  });

  // -- /proposals ---------------------------------------------------------

  it("getProposals returns proposals list", async () => {
    setup({ count: 1, proposals: [{ id: 1, title: "Test" }] });
    const res = await client.getProposals();
    expect(res.count).toBe(1);
    expect(res.proposals).toHaveLength(1);
  });

  // -- /proposal/:id ------------------------------------------------------

  it("getProposal returns proposal details", async () => {
    setup({ id: 5, title: "Change round time" });
    const res = await client.getProposal(5);
    expect(res.id).toBe(5);
    expect(fetchFn).toHaveBeenCalledWith(
      "http://node:10333/proposal/5",
      expect.anything(),
    );
  });

  // -- /vote/:proposal_id/:address ----------------------------------------

  it("getVote returns vote info", async () => {
    setup({ vote: "yes", weight: 100 });
    const res = await client.getVote(5, "aa".repeat(32));
    expect(res).toHaveProperty("vote");
    expect(fetchFn).toHaveBeenCalledWith(
      `http://node:10333/vote/5/${"aa".repeat(32)}`,
      expect.anything(),
    );
  });
});

// ---------------------------------------------------------------------------
// POST endpoints
// ---------------------------------------------------------------------------

describe("UltraDagClient POST endpoints", () => {
  let client: UltraDagClient;
  let fetchFn: ReturnType<typeof vi.fn>;

  function setup(body: unknown, status = 200) {
    const f = mockFetch(body, status);
    fetchFn = f as unknown as ReturnType<typeof vi.fn>;
    client = new UltraDagClient({ baseUrl: "http://node:10333", fetch: f });
  }

  // -- /tx ----------------------------------------------------------------

  it("sendTransaction posts tx and returns result", async () => {
    const response = {
      hash: "ff".repeat(32),
      from: "aa".repeat(32),
      to: "bb".repeat(32),
      amount: 1_000_000,
      fee: 1000,
      nonce: 1,
    };
    setup(response);
    const res = await client.sendTransaction({
      secret_key: "cc".repeat(32),
      to: "bb".repeat(32),
      amount: 1_000_000,
      fee: 1000,
    });
    expect(res.hash).toBe("ff".repeat(32));
    expect(res.amount).toBe(1_000_000);
    // Verify POST body
    const callArgs = fetchFn.mock.calls[0];
    expect(callArgs[1].method).toBe("POST");
    const sentBody = JSON.parse(callArgs[1].body);
    expect(sentBody.to).toBe("bb".repeat(32));
    expect(sentBody.amount).toBe(1_000_000);
  });

  // -- /faucet ------------------------------------------------------------

  it("faucet sends address and amount", async () => {
    const response = {
      tx_hash: "dd".repeat(32),
      from: "fa".repeat(32),
      to: "aa".repeat(32),
      amount: 100_000_000,
      amount_udag: 1.0,
      nonce: 0,
    };
    setup(response);
    const res = await client.faucet("aa".repeat(32), 100_000_000);
    expect(res.tx_hash).toBe("dd".repeat(32));
    expect(res.amount_udag).toBe(1.0);
    const sentBody = JSON.parse(fetchFn.mock.calls[0][1].body);
    expect(sentBody.address).toBe("aa".repeat(32));
    expect(sentBody.amount).toBe(100_000_000);
  });

  // -- /stake -------------------------------------------------------------

  it("stake posts staking request", async () => {
    const response = {
      status: "staked",
      tx_hash: "ee".repeat(32),
      address: "aa".repeat(32),
      amount: 1_000_000_000_000,
      amount_udag: 10000,
      nonce: 2,
      note: "Staked 10000 UDAG",
    };
    setup(response);
    const res = await client.stake({
      secret_key: "cc".repeat(32),
      amount: 1_000_000_000_000,
    });
    expect(res.status).toBe("staked");
    expect(res.amount_udag).toBe(10000);
  });

  // -- /unstake -----------------------------------------------------------

  it("unstake posts unstake request", async () => {
    const response = {
      status: "unstaking",
      tx_hash: "ff".repeat(32),
      address: "aa".repeat(32),
      unlock_at_round: 5000,
      nonce: 3,
      note: "Unstaking, cooldown 2016 rounds",
    };
    setup(response);
    const res = await client.unstake({ secret_key: "cc".repeat(32) });
    expect(res.status).toBe("unstaking");
    expect(res.unlock_at_round).toBe(5000);
  });

  // -- /proposal ----------------------------------------------------------

  it("createProposal posts proposal", async () => {
    setup({ id: 1, status: "created" });
    const res = await client.createProposal({
      proposer_secret: "cc".repeat(32),
      title: "Change round time",
      description: "Reduce to 3 seconds",
      proposal_type: "parameter_change",
      parameter_name: "round_ms",
      parameter_value: "3000",
    });
    expect(res).toHaveProperty("id");
    const sentBody = JSON.parse(fetchFn.mock.calls[0][1].body);
    expect(sentBody.title).toBe("Change round time");
    expect(sentBody.parameter_name).toBe("round_ms");
  });

  // -- /vote --------------------------------------------------------------

  it("castVote posts vote", async () => {
    setup({ status: "voted" });
    const res = await client.castVote({
      voter_secret: "cc".repeat(32),
      proposal_id: 1,
      vote: "yes",
    });
    expect(res).toHaveProperty("status");
    const sentBody = JSON.parse(fetchFn.mock.calls[0][1].body);
    expect(sentBody.proposal_id).toBe(1);
    expect(sentBody.vote).toBe("yes");
  });
});

// ---------------------------------------------------------------------------
// Error handling
// ---------------------------------------------------------------------------

describe("UltraDagClient error handling", () => {
  it("throws UltraDagError on HTTP error status", async () => {
    const f = mockFetch({ error: "Not found" }, 404);
    const client = new UltraDagClient({ baseUrl: "http://node:10333", fetch: f });
    await expect(client.getBalance("aa".repeat(32))).rejects.toThrow(UltraDagError);
    try {
      await client.getBalance("aa".repeat(32));
    } catch (e) {
      expect(e).toBeInstanceOf(UltraDagError);
      const err = e as UltraDagError;
      expect(err.status).toBe(404);
      expect(err.body).toContain("Not found");
    }
  });

  it("throws UltraDagError on network failure", async () => {
    const f = failingFetch();
    const client = new UltraDagClient({ baseUrl: "http://node:10333", fetch: f });
    await expect(client.getHealth()).rejects.toThrow(UltraDagError);
    try {
      await client.getHealth();
    } catch (e) {
      expect(e).toBeInstanceOf(UltraDagError);
      expect((e as UltraDagError).message).toContain("Connection refused");
    }
  });

  it("throws UltraDagError on invalid JSON response", async () => {
    const f = mockFetchText("not json", 200);
    const client = new UltraDagClient({ baseUrl: "http://node:10333", fetch: f });
    await expect(client.getHealth()).rejects.toThrow(UltraDagError);
    try {
      await client.getHealth();
    } catch (e) {
      expect(e).toBeInstanceOf(UltraDagError);
      expect((e as UltraDagError).message).toContain("Invalid JSON");
    }
  });

  it("throws UltraDagError on server 500 error", async () => {
    const f = mockFetchText("Internal Server Error", 500);
    const client = new UltraDagClient({ baseUrl: "http://node:10333", fetch: f });
    await expect(client.getStatus()).rejects.toThrow(UltraDagError);
    try {
      await client.getStatus();
    } catch (e) {
      const err = e as UltraDagError;
      expect(err.status).toBe(500);
    }
  });
});

// ---------------------------------------------------------------------------
// Client configuration
// ---------------------------------------------------------------------------

describe("UltraDagClient configuration", () => {
  it("uses default base URL when none provided", async () => {
    const f = mockFetch({ status: "ok" });
    const fetchFn = f as unknown as ReturnType<typeof vi.fn>;
    const client = new UltraDagClient({ fetch: f });
    await client.getHealth();
    expect(fetchFn.mock.calls[0][0]).toBe("http://localhost:10333/health");
  });

  it("strips trailing slash from base URL", async () => {
    const f = mockFetch({ status: "ok" });
    const fetchFn = f as unknown as ReturnType<typeof vi.fn>;
    const client = new UltraDagClient({ baseUrl: "http://node:10333/", fetch: f });
    await client.getHealth();
    expect(fetchFn.mock.calls[0][0]).toBe("http://node:10333/health");
  });
});
