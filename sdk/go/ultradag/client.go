package ultradag

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"time"
)

const (
	// DefaultBaseURL is the default UltraDAG node RPC address.
	DefaultBaseURL = "http://localhost:10333"

	// DefaultTimeout is the default HTTP client timeout.
	DefaultTimeout = 30 * time.Second
)

// Client is an HTTP client for the UltraDAG node RPC API.
type Client struct {
	baseURL    string
	httpClient *http.Client
}

// NewClient creates a new Client with the given base URL.
// The base URL should not have a trailing slash.
func NewClient(baseURL string) *Client {
	return &Client{
		baseURL: baseURL,
		httpClient: &http.Client{
			Timeout: DefaultTimeout,
		},
	}
}

// NewDefaultClient creates a new Client pointing at localhost:10333.
func NewDefaultClient() *Client {
	return NewClient(DefaultBaseURL)
}

// NewClientWithHTTP creates a new Client with a custom http.Client,
// useful for testing or custom transport configuration.
func NewClientWithHTTP(baseURL string, httpClient *http.Client) *Client {
	return &Client{
		baseURL:    baseURL,
		httpClient: httpClient,
	}
}

// doGet performs a GET request and decodes the JSON response into dest.
func (c *Client) doGet(path string, dest interface{}) error {
	resp, err := c.httpClient.Get(c.baseURL + path)
	if err != nil {
		return err
	}
	defer resp.Body.Close()

	body, err := io.ReadAll(resp.Body)
	if err != nil {
		return err
	}

	if resp.StatusCode != http.StatusOK {
		return &APIError{
			StatusCode: resp.StatusCode,
			Status:     resp.Status,
			Body:       string(body),
		}
	}

	return json.Unmarshal(body, dest)
}

// doPost performs a POST request with a JSON body and decodes the response into dest.
func (c *Client) doPost(path string, reqBody interface{}, dest interface{}) error {
	jsonBytes, err := json.Marshal(reqBody)
	if err != nil {
		return err
	}

	resp, err := c.httpClient.Post(
		c.baseURL+path,
		"application/json",
		bytes.NewReader(jsonBytes),
	)
	if err != nil {
		return err
	}
	defer resp.Body.Close()

	body, err := io.ReadAll(resp.Body)
	if err != nil {
		return err
	}

	if resp.StatusCode != http.StatusOK {
		return &APIError{
			StatusCode: resp.StatusCode,
			Status:     resp.Status,
			Body:       string(body),
		}
	}

	return json.Unmarshal(body, dest)
}

// Health checks whether the node is healthy.
func (c *Client) Health() (*HealthResponse, error) {
	var resp HealthResponse
	if err := c.doGet("/health", &resp); err != nil {
		return nil, err
	}
	return &resp, nil
}

// Status returns the current node status including DAG state, peers,
// mempool, and supply information.
func (c *Client) Status() (*StatusResponse, error) {
	var resp StatusResponse
	if err := c.doGet("/status", &resp); err != nil {
		return nil, err
	}
	return &resp, nil
}

// Balance returns the balance and nonce for the given address.
// The address should be a 64-character hex string.
func (c *Client) Balance(address string) (*BalanceResponse, error) {
	var resp BalanceResponse
	if err := c.doGet(fmt.Sprintf("/balance/%s", address), &resp); err != nil {
		return nil, err
	}
	return &resp, nil
}

// Round returns all vertices produced in the given round number.
func (c *Client) Round(round uint64) ([]VertexResponse, error) {
	var resp []VertexResponse
	if err := c.doGet(fmt.Sprintf("/round/%d", round), &resp); err != nil {
		return nil, err
	}
	return resp, nil
}

// Mempool returns the pending transactions in the mempool (top 100 by fee).
func (c *Client) Mempool() ([]map[string]interface{}, error) {
	var resp []map[string]interface{}
	if err := c.doGet("/mempool", &resp); err != nil {
		return nil, err
	}
	return resp, nil
}

// Peers returns the connected peers and bootstrap node status.
func (c *Client) Peers() (*PeersResponse, error) {
	var resp PeersResponse
	if err := c.doGet("/peers", &resp); err != nil {
		return nil, err
	}
	return &resp, nil
}

// Keygen requests the node to generate a new keypair.
// For local key generation without a network call, use GenerateKeypair instead.
func (c *Client) Keygen() (*KeygenResponse, error) {
	var resp KeygenResponse
	if err := c.doGet("/keygen", &resp); err != nil {
		return nil, err
	}
	return &resp, nil
}

// Validators returns the list of active validators and their stakes.
func (c *Client) Validators() (*ValidatorsResponse, error) {
	var resp ValidatorsResponse
	if err := c.doGet("/validators", &resp); err != nil {
		return nil, err
	}
	return &resp, nil
}

// StakeInfo returns staking information for the given address.
func (c *Client) StakeInfo(address string) (*StakeInfoResponse, error) {
	var resp StakeInfoResponse
	if err := c.doGet(fmt.Sprintf("/stake/%s", address), &resp); err != nil {
		return nil, err
	}
	return &resp, nil
}

// GovernanceConfig returns the current governance configuration.
func (c *Client) GovernanceConfig() (map[string]interface{}, error) {
	var resp map[string]interface{}
	if err := c.doGet("/governance/config", &resp); err != nil {
		return nil, err
	}
	return resp, nil
}

// Proposals returns the list of governance proposals.
func (c *Client) Proposals() (*ProposalsResponse, error) {
	var resp ProposalsResponse
	if err := c.doGet("/proposals", &resp); err != nil {
		return nil, err
	}
	return &resp, nil
}

// Proposal returns details of a specific governance proposal by ID.
func (c *Client) Proposal(id string) (map[string]interface{}, error) {
	var resp map[string]interface{}
	if err := c.doGet(fmt.Sprintf("/proposal/%s", id), &resp); err != nil {
		return nil, err
	}
	return resp, nil
}

// Vote returns the vote of a specific address on a proposal.
func (c *Client) Vote(proposalID, address string) (map[string]interface{}, error) {
	var resp map[string]interface{}
	if err := c.doGet(fmt.Sprintf("/vote/%s/%s", proposalID, address), &resp); err != nil {
		return nil, err
	}
	return resp, nil
}

// SendTx submits a signed transaction to the node.
// The secret key is used server-side to sign the transaction.
// Amount and fee are in sats (1 UDAG = 100,000,000 sats).
func (c *Client) SendTx(secretKey, to string, amount, fee uint64) (*TxResponse, error) {
	req := TxRequest{
		SecretKey: secretKey,
		To:        to,
		Amount:    amount,
		Fee:       fee,
	}
	var resp TxResponse
	if err := c.doPost("/tx", req, &resp); err != nil {
		return nil, err
	}
	return &resp, nil
}

// Faucet requests testnet tokens from the faucet.
// Amount is in sats.
func (c *Client) Faucet(address string, amount uint64) (map[string]interface{}, error) {
	req := FaucetRequest{
		Address: address,
		Amount:  amount,
	}
	var resp map[string]interface{}
	if err := c.doPost("/faucet", req, &resp); err != nil {
		return nil, err
	}
	return resp, nil
}

// Stake locks UDAG as validator stake. Amount is in sats.
func (c *Client) Stake(secretKey string, amount uint64) (map[string]interface{}, error) {
	req := StakeRequest{
		SecretKey: secretKey,
		Amount:    amount,
	}
	var resp map[string]interface{}
	if err := c.doPost("/stake", req, &resp); err != nil {
		return nil, err
	}
	return resp, nil
}

// Unstake begins the unstake cooldown period for the validator.
func (c *Client) Unstake(secretKey string) (map[string]interface{}, error) {
	req := UnstakeRequest{
		SecretKey: secretKey,
	}
	var resp map[string]interface{}
	if err := c.doPost("/unstake", req, &resp); err != nil {
		return nil, err
	}
	return resp, nil
}

// SubmitProposal creates a new governance proposal.
func (c *Client) SubmitProposal(req ProposalRequest) (map[string]interface{}, error) {
	var resp map[string]interface{}
	if err := c.doPost("/proposal", req, &resp); err != nil {
		return nil, err
	}
	return resp, nil
}

// SubmitVote votes on a governance proposal.
func (c *Client) SubmitVote(req VoteRequest) (map[string]interface{}, error) {
	var resp map[string]interface{}
	if err := c.doPost("/vote", req, &resp); err != nil {
		return nil, err
	}
	return resp, nil
}
