package ultradag

import (
	"encoding/binary"
	"encoding/hex"
	"encoding/json"
	"fmt"

	"lukechampine.com/blake3"
)

// NetworkID is the network identifier prepended to all signable bytes.
// Prevents cross-network signature replay (testnet vs mainnet).
var NetworkID = []byte("ultradag-testnet-v1")

// ---------------------------------------------------------------------------
// Signable bytes — one function per transaction type.
//
// These functions produce the EXACT same byte sequences as the Rust
// signable_bytes() methods. Any divergence will cause signature verification
// to fail on the server.
// ---------------------------------------------------------------------------

// TransferSignableBytes builds the signable bytes for a Transfer transaction.
//
// Layout: NETWORK_ID | "transfer" | from(32) | to(32) | amount(u64 LE) |
//
//	fee(u64 LE) | nonce(u64 LE) | [memo_len(u32 LE) | memo bytes]
func TransferSignableBytes(from, to [32]byte, amount, fee, nonce uint64, memo []byte) []byte {
	buf := make([]byte, 0, 128)
	buf = append(buf, NetworkID...)
	buf = append(buf, []byte("transfer")...)
	buf = append(buf, from[:]...)
	buf = append(buf, to[:]...)
	buf = appendU64LE(buf, amount)
	buf = appendU64LE(buf, fee)
	buf = appendU64LE(buf, nonce)
	if len(memo) > 0 {
		buf = appendU32LE(buf, uint32(len(memo)))
		buf = append(buf, memo...)
	}
	return buf
}

// StakeSignableBytes builds the signable bytes for a Stake transaction.
//
// Layout: NETWORK_ID | "stake" | from(32) | amount(u64 LE) | nonce(u64 LE)
func StakeSignableBytes(from [32]byte, amount, nonce uint64) []byte {
	buf := make([]byte, 0, 72)
	buf = append(buf, NetworkID...)
	buf = append(buf, []byte("stake")...)
	buf = append(buf, from[:]...)
	buf = appendU64LE(buf, amount)
	buf = appendU64LE(buf, nonce)
	return buf
}

// UnstakeSignableBytes builds the signable bytes for an Unstake transaction.
//
// Layout: NETWORK_ID | "unstake" | from(32) | nonce(u64 LE)
func UnstakeSignableBytes(from [32]byte, nonce uint64) []byte {
	buf := make([]byte, 0, 66)
	buf = append(buf, NetworkID...)
	buf = append(buf, []byte("unstake")...)
	buf = append(buf, from[:]...)
	buf = appendU64LE(buf, nonce)
	return buf
}

// DelegateSignableBytes builds the signable bytes for a Delegate transaction.
//
// Layout: NETWORK_ID | "delegate" | from(32) | validator(32) | amount(u64 LE) | nonce(u64 LE)
func DelegateSignableBytes(from, validator [32]byte, amount, nonce uint64) []byte {
	buf := make([]byte, 0, 108)
	buf = append(buf, NetworkID...)
	buf = append(buf, []byte("delegate")...)
	buf = append(buf, from[:]...)
	buf = append(buf, validator[:]...)
	buf = appendU64LE(buf, amount)
	buf = appendU64LE(buf, nonce)
	return buf
}

// UndelegateSignableBytes builds the signable bytes for an Undelegate transaction.
//
// Layout: NETWORK_ID | "undelegate" | from(32) | nonce(u64 LE)
func UndelegateSignableBytes(from [32]byte, nonce uint64) []byte {
	buf := make([]byte, 0, 69)
	buf = append(buf, NetworkID...)
	buf = append(buf, []byte("undelegate")...)
	buf = append(buf, from[:]...)
	buf = appendU64LE(buf, nonce)
	return buf
}

// SetCommissionSignableBytes builds the signable bytes for a SetCommission transaction.
//
// Layout: NETWORK_ID | "set_commission" | from(32) | commission_percent(u8) | nonce(u64 LE)
func SetCommissionSignableBytes(from [32]byte, commissionPercent uint8, nonce uint64) []byte {
	buf := make([]byte, 0, 74)
	buf = append(buf, NetworkID...)
	buf = append(buf, []byte("set_commission")...)
	buf = append(buf, from[:]...)
	buf = append(buf, commissionPercent)
	buf = appendU64LE(buf, nonce)
	return buf
}

// ProposalType represents the type of governance proposal for signable bytes construction.
type ProposalType interface {
	// appendSignableBytes appends proposal-type-specific bytes to buf.
	appendSignableBytes(buf []byte) []byte
	// toJSON returns the serde-compatible JSON representation.
	toJSON() interface{}
}

// TextProposal represents a text-only governance proposal.
type TextProposal struct{}

func (p TextProposal) appendSignableBytes(buf []byte) []byte {
	return append(buf, 0)
}

func (p TextProposal) toJSON() interface{} {
	return "TextProposal"
}

// ParameterChangeProposal represents a proposal to change a governance parameter.
type ParameterChangeProposal struct {
	Param    string
	NewValue string
}

func (p ParameterChangeProposal) appendSignableBytes(buf []byte) []byte {
	buf = append(buf, 1)
	buf = appendU32LE(buf, uint32(len(p.Param)))
	buf = append(buf, []byte(p.Param)...)
	buf = appendU32LE(buf, uint32(len(p.NewValue)))
	buf = append(buf, []byte(p.NewValue)...)
	return buf
}

func (p ParameterChangeProposal) toJSON() interface{} {
	return map[string]interface{}{
		"ParameterChange": map[string]interface{}{
			"param":     p.Param,
			"new_value": p.NewValue,
		},
	}
}

// CouncilMembershipProposal represents a proposal to add or remove a council member.
type CouncilMembershipProposal struct {
	Action   string   // "Add" or "Remove"
	Address  [32]byte // Council member address
	Category string   // "Technical", "Business", "Legal", "Academic", "Community", "Foundation"
}

func (p CouncilMembershipProposal) appendSignableBytes(buf []byte) []byte {
	buf = append(buf, 2)
	if p.Action == "Add" {
		buf = append(buf, 0)
	} else {
		buf = append(buf, 1)
	}
	buf = append(buf, p.Address[:]...)
	buf = append(buf, []byte(p.Category)...)
	return buf
}

func (p CouncilMembershipProposal) toJSON() interface{} {
	return map[string]interface{}{
		"CouncilMembership": map[string]interface{}{
			"action":   p.Action,
			"address":  p.Address,
			"category": p.Category,
		},
	}
}

// TreasurySpendProposal represents a proposal to spend from the DAO treasury.
type TreasurySpendProposal struct {
	Recipient [32]byte
	Amount    uint64
}

func (p TreasurySpendProposal) appendSignableBytes(buf []byte) []byte {
	buf = append(buf, 3)
	buf = append(buf, p.Recipient[:]...)
	buf = appendU64LE(buf, p.Amount)
	return buf
}

func (p TreasurySpendProposal) toJSON() interface{} {
	return map[string]interface{}{
		"TreasurySpend": map[string]interface{}{
			"recipient": p.Recipient,
			"amount":    p.Amount,
		},
	}
}

// CreateProposalSignableBytes builds the signable bytes for a CreateProposal transaction.
//
// Layout: NETWORK_ID | "proposal" | from(32) | proposal_id(u64 LE) |
//
//	title_len(u32 LE) | title | desc_len(u32 LE) | desc |
//	proposal_type_bytes | fee(u64 LE) | nonce(u64 LE)
func CreateProposalSignableBytes(from [32]byte, proposalID uint64, title, description string, proposalType ProposalType, fee, nonce uint64) []byte {
	buf := make([]byte, 0, 256)
	buf = append(buf, NetworkID...)
	buf = append(buf, []byte("proposal")...)
	buf = append(buf, from[:]...)
	buf = appendU64LE(buf, proposalID)
	buf = appendU32LE(buf, uint32(len(title)))
	buf = append(buf, []byte(title)...)
	buf = appendU32LE(buf, uint32(len(description)))
	buf = append(buf, []byte(description)...)
	buf = proposalType.appendSignableBytes(buf)
	buf = appendU64LE(buf, fee)
	buf = appendU64LE(buf, nonce)
	return buf
}

// VoteSignableBytes builds the signable bytes for a Vote transaction.
//
// Layout: NETWORK_ID | "vote" | from(32) | proposal_id(u64 LE) | vote(1 byte) |
//
//	fee(u64 LE) | nonce(u64 LE)
func VoteSignableBytes(from [32]byte, proposalID uint64, approve bool, fee, nonce uint64) []byte {
	buf := make([]byte, 0, 76)
	buf = append(buf, NetworkID...)
	buf = append(buf, []byte("vote")...)
	buf = append(buf, from[:]...)
	buf = appendU64LE(buf, proposalID)
	if approve {
		buf = append(buf, 1)
	} else {
		buf = append(buf, 0)
	}
	buf = appendU64LE(buf, fee)
	buf = appendU64LE(buf, nonce)
	return buf
}

// ---------------------------------------------------------------------------
// Signed transaction builders — sign and return JSON-compatible structs.
// ---------------------------------------------------------------------------

// BuildSignedTransferTx creates a signed Transfer transaction ready for POST /tx/submit.
func BuildSignedTransferTx(kp *Keypair, to [32]byte, amount, fee, nonce uint64, memo []byte) map[string]interface{} {
	signable := TransferSignableBytes(kp.Address, to, amount, fee, nonce, memo)
	sig := kp.Sign(signable)

	tx := map[string]interface{}{
		"from":      kp.Address,
		"to":        to,
		"amount":    amount,
		"fee":       fee,
		"nonce":     nonce,
		"pub_key":   kp.PublicKey,
		"signature": hex.EncodeToString(sig),
	}
	if len(memo) > 0 {
		tx["memo"] = memo
	} else {
		tx["memo"] = nil
	}

	return map[string]interface{}{
		"Transfer": tx,
	}
}

// BuildSignedStakeTx creates a signed Stake transaction ready for POST /tx/submit.
func BuildSignedStakeTx(kp *Keypair, amount, nonce uint64) map[string]interface{} {
	signable := StakeSignableBytes(kp.Address, amount, nonce)
	sig := kp.Sign(signable)

	return map[string]interface{}{
		"Stake": map[string]interface{}{
			"from":      kp.Address,
			"amount":    amount,
			"nonce":     nonce,
			"pub_key":   kp.PublicKey,
			"signature": hex.EncodeToString(sig),
		},
	}
}

// BuildSignedUnstakeTx creates a signed Unstake transaction ready for POST /tx/submit.
func BuildSignedUnstakeTx(kp *Keypair, nonce uint64) map[string]interface{} {
	signable := UnstakeSignableBytes(kp.Address, nonce)
	sig := kp.Sign(signable)

	return map[string]interface{}{
		"Unstake": map[string]interface{}{
			"from":      kp.Address,
			"nonce":     nonce,
			"pub_key":   kp.PublicKey,
			"signature": hex.EncodeToString(sig),
		},
	}
}

// BuildSignedDelegateTx creates a signed Delegate transaction ready for POST /tx/submit.
func BuildSignedDelegateTx(kp *Keypair, validator [32]byte, amount, nonce uint64) map[string]interface{} {
	signable := DelegateSignableBytes(kp.Address, validator, amount, nonce)
	sig := kp.Sign(signable)

	return map[string]interface{}{
		"Delegate": map[string]interface{}{
			"from":      kp.Address,
			"validator": validator,
			"amount":    amount,
			"nonce":     nonce,
			"pub_key":   kp.PublicKey,
			"signature": hex.EncodeToString(sig),
		},
	}
}

// BuildSignedUndelegateTx creates a signed Undelegate transaction ready for POST /tx/submit.
func BuildSignedUndelegateTx(kp *Keypair, nonce uint64) map[string]interface{} {
	signable := UndelegateSignableBytes(kp.Address, nonce)
	sig := kp.Sign(signable)

	return map[string]interface{}{
		"Undelegate": map[string]interface{}{
			"from":      kp.Address,
			"nonce":     nonce,
			"pub_key":   kp.PublicKey,
			"signature": hex.EncodeToString(sig),
		},
	}
}

// BuildSignedSetCommissionTx creates a signed SetCommission transaction ready for POST /tx/submit.
func BuildSignedSetCommissionTx(kp *Keypair, commissionPercent uint8, nonce uint64) map[string]interface{} {
	signable := SetCommissionSignableBytes(kp.Address, commissionPercent, nonce)
	sig := kp.Sign(signable)

	return map[string]interface{}{
		"SetCommission": map[string]interface{}{
			"from":               kp.Address,
			"commission_percent": commissionPercent,
			"nonce":              nonce,
			"pub_key":            kp.PublicKey,
			"signature":          hex.EncodeToString(sig),
		},
	}
}

// BuildSignedCreateProposalTx creates a signed CreateProposal transaction ready for POST /tx/submit.
func BuildSignedCreateProposalTx(kp *Keypair, proposalID uint64, title, description string, proposalType ProposalType, fee, nonce uint64) map[string]interface{} {
	signable := CreateProposalSignableBytes(kp.Address, proposalID, title, description, proposalType, fee, nonce)
	sig := kp.Sign(signable)

	return map[string]interface{}{
		"CreateProposal": map[string]interface{}{
			"from":          kp.Address,
			"proposal_id":   proposalID,
			"title":         title,
			"description":   description,
			"proposal_type": proposalType.toJSON(),
			"fee":           fee,
			"nonce":         nonce,
			"pub_key":       kp.PublicKey,
			"signature":     hex.EncodeToString(sig),
		},
	}
}

// BuildSignedVoteTx creates a signed Vote transaction ready for POST /tx/submit.
func BuildSignedVoteTx(kp *Keypair, proposalID uint64, approve bool, fee, nonce uint64) map[string]interface{} {
	signable := VoteSignableBytes(kp.Address, proposalID, approve, fee, nonce)
	sig := kp.Sign(signable)

	return map[string]interface{}{
		"Vote": map[string]interface{}{
			"from":        kp.Address,
			"proposal_id": proposalID,
			"vote":        approve,
			"fee":         fee,
			"nonce":       nonce,
			"pub_key":     kp.PublicKey,
			"signature":   hex.EncodeToString(sig),
		},
	}
}

// ---------------------------------------------------------------------------
// Client method for submitting pre-signed transactions.
// ---------------------------------------------------------------------------

// SubmitSignedTransaction submits a pre-signed transaction to POST /tx/submit.
// The tx parameter should be a map[string]interface{} from one of the
// BuildSigned*Tx functions.
func (c *Client) SubmitSignedTransaction(tx interface{}) (*SubmitTxResponse, error) {
	var resp SubmitTxResponse
	if err := c.doPost("/tx/submit", tx, &resp); err != nil {
		return nil, err
	}
	return &resp, nil
}

// SubmitTxResponse represents the response from POST /tx/submit.
type SubmitTxResponse struct {
	Hash   string `json:"hash"`
	Status string `json:"status"`
}

// SubmitSignedTransactionRaw submits a pre-signed transaction and returns
// the raw JSON response as a map, for cases where the response format is
// not known in advance.
func (c *Client) SubmitSignedTransactionRaw(tx interface{}) (map[string]interface{}, error) {
	jsonBytes, err := json.Marshal(tx)
	if err != nil {
		return nil, fmt.Errorf("failed to marshal transaction: %w", err)
	}
	_ = jsonBytes // used by doPost internally via the tx parameter
	var resp map[string]interface{}
	if err := c.doPost("/tx/submit", tx, &resp); err != nil {
		return nil, err
	}
	return resp, nil
}

// ---------------------------------------------------------------------------
// SmartAccount Transaction Types
// ---------------------------------------------------------------------------

// ComputeKeyID computes blake3(keyType || pubkey)[..8].
func ComputeKeyID(keyType byte, pubkey []byte) [8]byte {
	data := append([]byte{keyType}, pubkey...)
	hash := blake3.Sum256(data)
	var id [8]byte
	copy(id[:], hash[:8])
	return id
}

// BuildSmartTransferSignableBytes constructs signable_bytes for SmartTransferTx.
func BuildSmartTransferSignableBytes(from, to [20]byte, amount, fee, nonce uint64, keyID [8]byte, memo []byte) []byte {
	buf := make([]byte, 0, 128)
	buf = append(buf, NetworkID...)
	buf = append(buf, []byte("smart_transfer")...)
	buf = append(buf, from[:]...)
	buf = append(buf, to[:]...)
	buf = appendU64LE(buf, amount)
	buf = appendU64LE(buf, fee)
	buf = appendU64LE(buf, nonce)
	buf = append(buf, keyID[:]...)
	if len(memo) > 0 {
		buf = appendU32LE(buf, uint32(len(memo)))
		buf = append(buf, memo...)
	}
	return buf
}

// BuildRegisterNameSignableBytes constructs signable_bytes for RegisterNameTx.
func BuildRegisterNameSignableBytes(from [20]byte, name string, durationYears uint8, fee, nonce uint64) []byte {
	nameBytes := []byte(name)
	buf := make([]byte, 0, 80+len(nameBytes))
	buf = append(buf, NetworkID...)
	buf = append(buf, []byte("name_register")...)
	buf = append(buf, from[:]...)
	buf = appendU32LE(buf, uint32(len(nameBytes)))
	buf = append(buf, nameBytes...)
	buf = append(buf, durationYears)
	buf = appendU64LE(buf, fee)
	buf = appendU64LE(buf, nonce)
	return buf
}

// ---------------------------------------------------------------------------
// Name Registry & SmartAccount client methods
// ---------------------------------------------------------------------------

// ResolveName looks up a name and returns the address (or empty string).
func (c *Client) ResolveName(name string) (string, error) {
	var resp map[string]interface{}
	if err := c.doGet("/name/resolve/"+name, &resp); err != nil {
		return "", err
	}
	addr, _ := resp["address"].(string)
	return addr, nil
}

// CheckNameAvailability checks if a name is available.
func (c *Client) CheckNameAvailability(name string) (bool, uint64, error) {
	var resp map[string]interface{}
	if err := c.doGet("/name/available/"+name, &resp); err != nil {
		return false, 0, err
	}
	available, _ := resp["available"].(bool)
	fee, _ := resp["annual_fee"].(float64)
	return available, uint64(fee), nil
}

// GetSmartAccount returns SmartAccount info for an address or name.
func (c *Client) GetSmartAccount(addressOrName string) (map[string]interface{}, error) {
	var resp map[string]interface{}
	if err := c.doGet("/smart-account/"+addressOrName, &resp); err != nil {
		return nil, err
	}
	return resp, nil
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

// appendU64LE appends a uint64 in little-endian byte order to buf.
func appendU64LE(buf []byte, v uint64) []byte {
	var b [8]byte
	binary.LittleEndian.PutUint64(b[:], v)
	return append(buf, b[:]...)
}

// appendU32LE appends a uint32 in little-endian byte order to buf.
func appendU32LE(buf []byte, v uint32) []byte {
	var b [4]byte
	binary.LittleEndian.PutUint32(b[:], v)
	return append(buf, b[:]...)
}
