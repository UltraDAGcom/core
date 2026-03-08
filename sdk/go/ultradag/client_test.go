package ultradag

import (
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"testing"
)

// newTestServer creates an httptest.Server that handles all RPC endpoints
// with canned responses. Returns the server and a Client pointing at it.
func newTestServer(t *testing.T) (*httptest.Server, *Client) {
	t.Helper()

	mux := http.NewServeMux()

	mux.HandleFunc("/health", func(w http.ResponseWriter, r *http.Request) {
		json.NewEncoder(w).Encode(HealthResponse{Status: "ok"})
	})

	mux.HandleFunc("/status", func(w http.ResponseWriter, r *http.Request) {
		round := uint64(42)
		json.NewEncoder(w).Encode(StatusResponse{
			LastFinalizedRound: &round,
			PeerCount:          3,
			MempoolSize:        5,
			TotalSupply:        2100000000000000,
			AccountCount:       10,
			DagVertices:        200,
			DagRound:           45,
			DagTips:            4,
			FinalizedCount:     180,
			ValidatorCount:     4,
			TotalStaked:        100000000000000,
			ActiveStakers:      4,
			BootstrapConnected: true,
		})
	})

	mux.HandleFunc("/balance/", func(w http.ResponseWriter, r *http.Request) {
		json.NewEncoder(w).Encode(BalanceResponse{
			Address:     "abc123",
			Balance:     5000000000,
			Nonce:       3,
			BalanceUdag: 50.0,
		})
	})

	mux.HandleFunc("/round/", func(w http.ResponseWriter, r *http.Request) {
		json.NewEncoder(w).Encode([]VertexResponse{
			{
				Round:       10,
				Hash:        "deadbeef",
				Validator:   "val1",
				Reward:      5000000000,
				TxCount:     2,
				ParentCount: 3,
			},
		})
	})

	mux.HandleFunc("/mempool", func(w http.ResponseWriter, r *http.Request) {
		json.NewEncoder(w).Encode([]map[string]interface{}{
			{"hash": "tx1", "fee": float64(100000)},
		})
	})

	mux.HandleFunc("/peers", func(w http.ResponseWriter, r *http.Request) {
		json.NewEncoder(w).Encode(PeersResponse{
			Connected: 3,
			Peers:     []string{"1.2.3.4:9333", "5.6.7.8:9333"},
			BootstrapNodes: []BootstrapNode{
				{Address: "206.51.242.223:9333", Connected: true},
			},
		})
	})

	mux.HandleFunc("/keygen", func(w http.ResponseWriter, r *http.Request) {
		json.NewEncoder(w).Encode(KeygenResponse{
			SecretKey: "aabbccdd",
			Address:   "11223344",
		})
	})

	mux.HandleFunc("/validators", func(w http.ResponseWriter, r *http.Request) {
		json.NewEncoder(w).Encode(ValidatorsResponse{
			Count:       2,
			TotalStaked: 200000000000000,
			Validators: []ValidatorInfo{
				{Address: "val1addr", Stake: 100000000000000},
				{Address: "val2addr", Stake: 100000000000000},
			},
		})
	})

	// Stake info GET handler - use a more specific pattern to avoid conflict
	// with POST /stake. We handle both under /stake/ and check method.
	mux.HandleFunc("/stake/", func(w http.ResponseWriter, r *http.Request) {
		json.NewEncoder(w).Encode(StakeInfoResponse{
			Address:           "stakeaddr",
			Staked:            50000000000000,
			StakedUdag:        500000.0,
			UnlockAtRound:     nil,
			IsActiveValidator: true,
		})
	})

	mux.HandleFunc("/stake", func(w http.ResponseWriter, r *http.Request) {
		if r.Method != http.MethodPost {
			http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
			return
		}
		var req StakeRequest
		json.NewDecoder(r.Body).Decode(&req)
		json.NewEncoder(w).Encode(map[string]interface{}{
			"status": "staked",
			"amount": req.Amount,
		})
	})

	mux.HandleFunc("/governance/config", func(w http.ResponseWriter, r *http.Request) {
		json.NewEncoder(w).Encode(map[string]interface{}{
			"min_stake": float64(1000000000000),
		})
	})

	mux.HandleFunc("/proposals", func(w http.ResponseWriter, r *http.Request) {
		json.NewEncoder(w).Encode(ProposalsResponse{
			Count:     1,
			Proposals: []map[string]interface{}{{"id": "prop1"}},
		})
	})

	mux.HandleFunc("/proposal/", func(w http.ResponseWriter, r *http.Request) {
		if r.Method == http.MethodPost {
			json.NewEncoder(w).Encode(map[string]interface{}{"id": "new_prop"})
			return
		}
		json.NewEncoder(w).Encode(map[string]interface{}{
			"id":    "prop1",
			"title": "Test Proposal",
		})
	})

	mux.HandleFunc("/proposal", func(w http.ResponseWriter, r *http.Request) {
		if r.Method != http.MethodPost {
			http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
			return
		}
		json.NewEncoder(w).Encode(map[string]interface{}{"id": "new_prop"})
	})

	mux.HandleFunc("/vote/", func(w http.ResponseWriter, r *http.Request) {
		json.NewEncoder(w).Encode(map[string]interface{}{
			"proposal_id": "prop1",
			"vote":        "yes",
		})
	})

	mux.HandleFunc("/vote", func(w http.ResponseWriter, r *http.Request) {
		if r.Method != http.MethodPost {
			http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
			return
		}
		json.NewEncoder(w).Encode(map[string]interface{}{"status": "voted"})
	})

	mux.HandleFunc("/tx", func(w http.ResponseWriter, r *http.Request) {
		if r.Method != http.MethodPost {
			http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
			return
		}
		var req TxRequest
		json.NewDecoder(r.Body).Decode(&req)
		json.NewEncoder(w).Encode(TxResponse{
			Hash:   "txhash123",
			From:   "fromaddr",
			To:     req.To,
			Amount: req.Amount,
			Fee:    req.Fee,
			Nonce:  1,
		})
	})

	mux.HandleFunc("/faucet", func(w http.ResponseWriter, r *http.Request) {
		if r.Method != http.MethodPost {
			http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
			return
		}
		var req FaucetRequest
		json.NewDecoder(r.Body).Decode(&req)
		json.NewEncoder(w).Encode(map[string]interface{}{
			"status":  "funded",
			"address": req.Address,
			"amount":  float64(req.Amount),
		})
	})

	mux.HandleFunc("/unstake", func(w http.ResponseWriter, r *http.Request) {
		if r.Method != http.MethodPost {
			http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
			return
		}
		json.NewEncoder(w).Encode(map[string]interface{}{
			"status": "unstaking",
		})
	})

	server := httptest.NewServer(mux)
	client := NewClient(server.URL)
	return server, client
}

func TestHealth(t *testing.T) {
	server, client := newTestServer(t)
	defer server.Close()

	resp, err := client.Health()
	if err != nil {
		t.Fatalf("Health() error: %v", err)
	}
	if resp.Status != "ok" {
		t.Errorf("expected status 'ok', got %q", resp.Status)
	}
}

func TestStatus(t *testing.T) {
	server, client := newTestServer(t)
	defer server.Close()

	resp, err := client.Status()
	if err != nil {
		t.Fatalf("Status() error: %v", err)
	}
	if resp.LastFinalizedRound == nil || *resp.LastFinalizedRound != 42 {
		t.Errorf("expected LastFinalizedRound=42, got %v", resp.LastFinalizedRound)
	}
	if resp.PeerCount != 3 {
		t.Errorf("expected PeerCount=3, got %d", resp.PeerCount)
	}
	if resp.TotalSupply != 2100000000000000 {
		t.Errorf("expected TotalSupply=2100000000000000, got %d", resp.TotalSupply)
	}
	if !resp.BootstrapConnected {
		t.Error("expected BootstrapConnected=true")
	}
}

func TestBalance(t *testing.T) {
	server, client := newTestServer(t)
	defer server.Close()

	resp, err := client.Balance("abc123")
	if err != nil {
		t.Fatalf("Balance() error: %v", err)
	}
	if resp.Balance != 5000000000 {
		t.Errorf("expected Balance=5000000000, got %d", resp.Balance)
	}
	if resp.Nonce != 3 {
		t.Errorf("expected Nonce=3, got %d", resp.Nonce)
	}
}

func TestRound(t *testing.T) {
	server, client := newTestServer(t)
	defer server.Close()

	resp, err := client.Round(10)
	if err != nil {
		t.Fatalf("Round() error: %v", err)
	}
	if len(resp) != 1 {
		t.Fatalf("expected 1 vertex, got %d", len(resp))
	}
	if resp[0].Hash != "deadbeef" {
		t.Errorf("expected Hash='deadbeef', got %q", resp[0].Hash)
	}
	if resp[0].TxCount != 2 {
		t.Errorf("expected TxCount=2, got %d", resp[0].TxCount)
	}
}

func TestMempool(t *testing.T) {
	server, client := newTestServer(t)
	defer server.Close()

	resp, err := client.Mempool()
	if err != nil {
		t.Fatalf("Mempool() error: %v", err)
	}
	if len(resp) != 1 {
		t.Fatalf("expected 1 tx, got %d", len(resp))
	}
	if resp[0]["hash"] != "tx1" {
		t.Errorf("expected hash='tx1', got %v", resp[0]["hash"])
	}
}

func TestPeers(t *testing.T) {
	server, client := newTestServer(t)
	defer server.Close()

	resp, err := client.Peers()
	if err != nil {
		t.Fatalf("Peers() error: %v", err)
	}
	if resp.Connected != 3 {
		t.Errorf("expected Connected=3, got %d", resp.Connected)
	}
	if len(resp.Peers) != 2 {
		t.Errorf("expected 2 peers, got %d", len(resp.Peers))
	}
	if len(resp.BootstrapNodes) != 1 {
		t.Errorf("expected 1 bootstrap node, got %d", len(resp.BootstrapNodes))
	}
}

func TestKeygen(t *testing.T) {
	server, client := newTestServer(t)
	defer server.Close()

	resp, err := client.Keygen()
	if err != nil {
		t.Fatalf("Keygen() error: %v", err)
	}
	if resp.SecretKey != "aabbccdd" {
		t.Errorf("expected SecretKey='aabbccdd', got %q", resp.SecretKey)
	}
	if resp.Address != "11223344" {
		t.Errorf("expected Address='11223344', got %q", resp.Address)
	}
}

func TestValidators(t *testing.T) {
	server, client := newTestServer(t)
	defer server.Close()

	resp, err := client.Validators()
	if err != nil {
		t.Fatalf("Validators() error: %v", err)
	}
	if resp.Count != 2 {
		t.Errorf("expected Count=2, got %d", resp.Count)
	}
	if len(resp.Validators) != 2 {
		t.Fatalf("expected 2 validators, got %d", len(resp.Validators))
	}
	if resp.Validators[0].Address != "val1addr" {
		t.Errorf("expected first validator address='val1addr', got %q", resp.Validators[0].Address)
	}
}

func TestStakeInfo(t *testing.T) {
	server, client := newTestServer(t)
	defer server.Close()

	resp, err := client.StakeInfo("stakeaddr")
	if err != nil {
		t.Fatalf("StakeInfo() error: %v", err)
	}
	if resp.Staked != 50000000000000 {
		t.Errorf("expected Staked=50000000000000, got %d", resp.Staked)
	}
	if !resp.IsActiveValidator {
		t.Error("expected IsActiveValidator=true")
	}
}

func TestGovernanceConfig(t *testing.T) {
	server, client := newTestServer(t)
	defer server.Close()

	resp, err := client.GovernanceConfig()
	if err != nil {
		t.Fatalf("GovernanceConfig() error: %v", err)
	}
	if resp["min_stake"] == nil {
		t.Error("expected min_stake key in response")
	}
}

func TestProposals(t *testing.T) {
	server, client := newTestServer(t)
	defer server.Close()

	resp, err := client.Proposals()
	if err != nil {
		t.Fatalf("Proposals() error: %v", err)
	}
	if resp.Count != 1 {
		t.Errorf("expected Count=1, got %d", resp.Count)
	}
}

func TestProposal(t *testing.T) {
	server, client := newTestServer(t)
	defer server.Close()

	resp, err := client.Proposal("prop1")
	if err != nil {
		t.Fatalf("Proposal() error: %v", err)
	}
	if resp["id"] != "prop1" {
		t.Errorf("expected id='prop1', got %v", resp["id"])
	}
}

func TestVoteGet(t *testing.T) {
	server, client := newTestServer(t)
	defer server.Close()

	resp, err := client.Vote("prop1", "voteraddr")
	if err != nil {
		t.Fatalf("Vote() error: %v", err)
	}
	if resp["vote"] != "yes" {
		t.Errorf("expected vote='yes', got %v", resp["vote"])
	}
}

func TestSendTx(t *testing.T) {
	server, client := newTestServer(t)
	defer server.Close()

	resp, err := client.SendTx("secrethex", "toaddr", 1000000000, 100000)
	if err != nil {
		t.Fatalf("SendTx() error: %v", err)
	}
	if resp.Hash != "txhash123" {
		t.Errorf("expected Hash='txhash123', got %q", resp.Hash)
	}
	if resp.To != "toaddr" {
		t.Errorf("expected To='toaddr', got %q", resp.To)
	}
	if resp.Amount != 1000000000 {
		t.Errorf("expected Amount=1000000000, got %d", resp.Amount)
	}
}

func TestFaucet(t *testing.T) {
	server, client := newTestServer(t)
	defer server.Close()

	resp, err := client.Faucet("myaddr", 5000000000)
	if err != nil {
		t.Fatalf("Faucet() error: %v", err)
	}
	if resp["status"] != "funded" {
		t.Errorf("expected status='funded', got %v", resp["status"])
	}
}

func TestStakePost(t *testing.T) {
	server, client := newTestServer(t)
	defer server.Close()

	resp, err := client.Stake("secrethex", 100000000000000)
	if err != nil {
		t.Fatalf("Stake() error: %v", err)
	}
	if resp["status"] != "staked" {
		t.Errorf("expected status='staked', got %v", resp["status"])
	}
}

func TestUnstake(t *testing.T) {
	server, client := newTestServer(t)
	defer server.Close()

	resp, err := client.Unstake("secrethex")
	if err != nil {
		t.Fatalf("Unstake() error: %v", err)
	}
	if resp["status"] != "unstaking" {
		t.Errorf("expected status='unstaking', got %v", resp["status"])
	}
}

func TestSubmitProposal(t *testing.T) {
	server, client := newTestServer(t)
	defer server.Close()

	resp, err := client.SubmitProposal(ProposalRequest{
		SecretKey:   "secret",
		Title:       "My Proposal",
		Description: "A proposal",
	})
	if err != nil {
		t.Fatalf("SubmitProposal() error: %v", err)
	}
	if resp["id"] != "new_prop" {
		t.Errorf("expected id='new_prop', got %v", resp["id"])
	}
}

func TestSubmitVote(t *testing.T) {
	server, client := newTestServer(t)
	defer server.Close()

	resp, err := client.SubmitVote(VoteRequest{
		SecretKey:  "secret",
		ProposalID: "prop1",
		Vote:       "yes",
	})
	if err != nil {
		t.Fatalf("SubmitVote() error: %v", err)
	}
	if resp["status"] != "voted" {
		t.Errorf("expected status='voted', got %v", resp["status"])
	}
}

func TestAPIError(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		http.Error(w, `{"error":"not found"}`, http.StatusNotFound)
	}))
	defer server.Close()

	client := NewClient(server.URL)
	_, err := client.Health()
	if err == nil {
		t.Fatal("expected error for 404 response")
	}

	apiErr, ok := err.(*APIError)
	if !ok {
		t.Fatalf("expected *APIError, got %T", err)
	}
	if apiErr.StatusCode != 404 {
		t.Errorf("expected StatusCode=404, got %d", apiErr.StatusCode)
	}
	if !apiErr.IsNotFound() {
		t.Error("expected IsNotFound()=true")
	}
	if apiErr.IsServerError() {
		t.Error("expected IsServerError()=false for 404")
	}
}

func TestAPIErrorServerError(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		http.Error(w, "internal error", http.StatusInternalServerError)
	}))
	defer server.Close()

	client := NewClient(server.URL)
	_, err := client.Status()
	if err == nil {
		t.Fatal("expected error for 500 response")
	}

	apiErr, ok := err.(*APIError)
	if !ok {
		t.Fatalf("expected *APIError, got %T", err)
	}
	if !apiErr.IsServerError() {
		t.Error("expected IsServerError()=true")
	}
	if apiErr.IsNotFound() {
		t.Error("expected IsNotFound()=false for 500")
	}
}

func TestAPIErrorMessage(t *testing.T) {
	err := &APIError{StatusCode: 400, Status: "400 Bad Request", Body: "invalid json"}
	msg := err.Error()
	if msg != "ultradag: API error 400 400 Bad Request: invalid json" {
		t.Errorf("unexpected error message: %s", msg)
	}

	err2 := &APIError{StatusCode: 500, Status: "500 Internal Server Error"}
	msg2 := err2.Error()
	if msg2 != "ultradag: API error 500 500 Internal Server Error" {
		t.Errorf("unexpected error message: %s", msg2)
	}
}

func TestConnectionError(t *testing.T) {
	client := NewClient("http://127.0.0.1:1") // port 1 should refuse connection
	_, err := client.Health()
	if err == nil {
		t.Fatal("expected connection error")
	}
	// Should not be an APIError, but a net error
	if _, ok := err.(*APIError); ok {
		t.Error("expected net error, not APIError")
	}
}

func TestNewDefaultClient(t *testing.T) {
	client := NewDefaultClient()
	if client.baseURL != DefaultBaseURL {
		t.Errorf("expected baseURL=%q, got %q", DefaultBaseURL, client.baseURL)
	}
}

func TestSatsToUdag(t *testing.T) {
	tests := []struct {
		sats uint64
		want float64
	}{
		{100000000, 1.0},
		{50000000, 0.5},
		{0, 0.0},
		{2100000000000000, 21000000.0},
	}
	for _, tt := range tests {
		got := SatsToUdag(tt.sats)
		if got != tt.want {
			t.Errorf("SatsToUdag(%d) = %f, want %f", tt.sats, got, tt.want)
		}
	}
}

func TestUdagToSats(t *testing.T) {
	tests := []struct {
		udag float64
		want uint64
	}{
		{1.0, 100000000},
		{0.5, 50000000},
		{0.0, 0},
		{21000000.0, 2100000000000000},
	}
	for _, tt := range tests {
		got := UdagToSats(tt.udag)
		if got != tt.want {
			t.Errorf("UdagToSats(%f) = %d, want %d", tt.udag, got, tt.want)
		}
	}
}
