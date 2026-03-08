// Package ultradag provides a Go client for the UltraDAG node HTTP RPC API.
//
// UltraDAG is a DAG-BFT cryptocurrency designed for machine-to-machine
// micropayments. This SDK wraps all RPC endpoints and provides local
// Ed25519 key generation with Blake3 address derivation.
package ultradag

// SatsPerUdag is the number of satoshis in one UDAG.
const SatsPerUdag = 100_000_000

// SatsToUdag converts a satoshi amount to UDAG as a floating point value.
func SatsToUdag(sats uint64) float64 {
	return float64(sats) / float64(SatsPerUdag)
}

// UdagToSats converts a UDAG amount to satoshis.
func UdagToSats(udag float64) uint64 {
	return uint64(udag * float64(SatsPerUdag))
}

// HealthResponse represents the response from GET /health.
type HealthResponse struct {
	Status string `json:"status"`
}

// StatusResponse represents the response from GET /status.
type StatusResponse struct {
	LastFinalizedRound *uint64 `json:"last_finalized_round"`
	PeerCount          int     `json:"peer_count"`
	MempoolSize        int     `json:"mempool_size"`
	TotalSupply        uint64  `json:"total_supply"`
	AccountCount       int     `json:"account_count"`
	DagVertices        int     `json:"dag_vertices"`
	DagRound           uint64  `json:"dag_round"`
	DagTips            int     `json:"dag_tips"`
	FinalizedCount     int     `json:"finalized_count"`
	ValidatorCount     int     `json:"validator_count"`
	TotalStaked        uint64  `json:"total_staked"`
	ActiveStakers      int     `json:"active_stakers"`
	BootstrapConnected bool    `json:"bootstrap_connected"`
}

// BalanceResponse represents the response from GET /balance/{address}.
type BalanceResponse struct {
	Address     string  `json:"address"`
	Balance     uint64  `json:"balance"`
	Nonce       uint64  `json:"nonce"`
	BalanceUdag float64 `json:"balance_udag"`
}

// VertexResponse represents a single vertex in the response from GET /round/{round}.
type VertexResponse struct {
	Round       uint64 `json:"round"`
	Hash        string `json:"hash"`
	Validator   string `json:"validator"`
	Reward      uint64 `json:"reward"`
	TxCount     int    `json:"tx_count"`
	ParentCount int    `json:"parent_count"`
}

// PeersResponse represents the response from GET /peers.
type PeersResponse struct {
	Connected      int             `json:"connected"`
	Peers          []string        `json:"peers"`
	BootstrapNodes []BootstrapNode `json:"bootstrap_nodes"`
}

// BootstrapNode represents a bootstrap node entry in the peers response.
type BootstrapNode struct {
	Address   string `json:"address"`
	Connected bool   `json:"connected"`
}

// KeygenResponse represents the response from GET /keygen.
type KeygenResponse struct {
	SecretKey string `json:"secret_key"`
	Address   string `json:"address"`
}

// ValidatorsResponse represents the response from GET /validators.
type ValidatorsResponse struct {
	Count       int             `json:"count"`
	TotalStaked uint64          `json:"total_staked"`
	Validators  []ValidatorInfo `json:"validators"`
}

// ValidatorInfo represents a single validator in the validators response.
type ValidatorInfo struct {
	Address string `json:"address"`
	Stake   uint64 `json:"stake"`
}

// StakeInfoResponse represents the response from GET /stake/{address}.
type StakeInfoResponse struct {
	Address           string  `json:"address"`
	Staked            uint64  `json:"staked"`
	StakedUdag        float64 `json:"staked_udag"`
	UnlockAtRound     *uint64 `json:"unlock_at_round"`
	IsActiveValidator bool    `json:"is_active_validator"`
}

// ProposalsResponse represents the response from GET /proposals.
type ProposalsResponse struct {
	Count     int                      `json:"count"`
	Proposals []map[string]interface{} `json:"proposals"`
}

// TxRequest represents the request body for POST /tx.
type TxRequest struct {
	SecretKey string `json:"secret_key"`
	To        string `json:"to"`
	Amount    uint64 `json:"amount"`
	Fee       uint64 `json:"fee"`
}

// TxResponse represents the response from POST /tx.
type TxResponse struct {
	Hash   string `json:"hash"`
	From   string `json:"from"`
	To     string `json:"to"`
	Amount uint64 `json:"amount"`
	Fee    uint64 `json:"fee"`
	Nonce  uint64 `json:"nonce"`
}

// FaucetRequest represents the request body for POST /faucet.
type FaucetRequest struct {
	Address string `json:"address"`
	Amount  uint64 `json:"amount"`
}

// StakeRequest represents the request body for POST /stake.
type StakeRequest struct {
	SecretKey string `json:"secret_key"`
	Amount    uint64 `json:"amount"`
}

// UnstakeRequest represents the request body for POST /unstake.
type UnstakeRequest struct {
	SecretKey string `json:"secret_key"`
}

// ProposalRequest represents the request body for POST /proposal.
type ProposalRequest struct {
	SecretKey   string                 `json:"secret_key"`
	Title       string                 `json:"title"`
	Description string                 `json:"description"`
	Params      map[string]interface{} `json:"params,omitempty"`
}

// VoteRequest represents the request body for POST /vote.
type VoteRequest struct {
	SecretKey  string `json:"secret_key"`
	ProposalID string `json:"proposal_id"`
	Vote       string `json:"vote"`
}
