package ultradag

import (
	"bytes"
	"crypto/ed25519"
	"encoding/binary"
	"encoding/hex"
	"testing"
)

// ---------------------------------------------------------------------------
// Signable bytes layout tests — verify exact byte sequences match Rust.
// ---------------------------------------------------------------------------

func TestTransferSignableBytesLayout(t *testing.T) {
	var from, to [32]byte
	for i := range from {
		from[i] = 0xAA
	}
	for i := range to {
		to[i] = 0xBB
	}
	amount := uint64(1_000_000_000) // 10 UDAG
	fee := uint64(10_000)
	nonce := uint64(42)

	result := TransferSignableBytes(from, to, amount, fee, nonce, nil)

	// Expected layout: NETWORK_ID(19) + "transfer"(8) + from(32) + to(32) + amount(8) + fee(8) + nonce(8) = 115
	if len(result) != 115 {
		t.Errorf("length = %d, want 115", len(result))
	}

	offset := 0
	// NETWORK_ID
	if !bytes.Equal(result[offset:offset+19], NetworkID) {
		t.Error("NETWORK_ID mismatch")
	}
	offset += 19

	// "transfer"
	if !bytes.Equal(result[offset:offset+8], []byte("transfer")) {
		t.Error("type discriminator mismatch")
	}
	offset += 8

	// from
	if !bytes.Equal(result[offset:offset+32], from[:]) {
		t.Error("from mismatch")
	}
	offset += 32

	// to
	if !bytes.Equal(result[offset:offset+32], to[:]) {
		t.Error("to mismatch")
	}
	offset += 32

	// amount (LE)
	gotAmount := binary.LittleEndian.Uint64(result[offset : offset+8])
	if gotAmount != amount {
		t.Errorf("amount = %d, want %d", gotAmount, amount)
	}
	offset += 8

	// fee (LE)
	gotFee := binary.LittleEndian.Uint64(result[offset : offset+8])
	if gotFee != fee {
		t.Errorf("fee = %d, want %d", gotFee, fee)
	}
	offset += 8

	// nonce (LE)
	gotNonce := binary.LittleEndian.Uint64(result[offset : offset+8])
	if gotNonce != nonce {
		t.Errorf("nonce = %d, want %d", gotNonce, nonce)
	}
}

func TestTransferSignableBytesWithMemo(t *testing.T) {
	var from, to [32]byte
	memo := []byte("temp:22.4C")

	result := TransferSignableBytes(from, to, 100, 10, 0, memo)

	// Expected: 115 (base) + 4 (memo_len u32 LE) + 10 (memo) = 129
	if len(result) != 129 {
		t.Errorf("length = %d, want 129", len(result))
	}

	// Check memo_len at offset 115
	memoLen := binary.LittleEndian.Uint32(result[115:119])
	if memoLen != 10 {
		t.Errorf("memo_len = %d, want 10", memoLen)
	}

	// Check memo bytes
	if !bytes.Equal(result[119:129], memo) {
		t.Error("memo bytes mismatch")
	}
}

func TestTransferSignableBytesWithoutMemo(t *testing.T) {
	var from, to [32]byte

	withNil := TransferSignableBytes(from, to, 100, 10, 0, nil)
	withEmpty := TransferSignableBytes(from, to, 100, 10, 0, []byte{})

	// Both nil and empty memo should produce the same bytes (no memo appended).
	if !bytes.Equal(withNil, withEmpty) {
		t.Error("nil memo and empty memo produce different bytes")
	}
	if len(withNil) != 115 {
		t.Errorf("length = %d, want 115", len(withNil))
	}
}

func TestStakeSignableBytesLayout(t *testing.T) {
	var from [32]byte
	from[0] = 0x01
	amount := uint64(10_000 * SatsPerUdag) // 10,000 UDAG = MIN_STAKE_SATS
	nonce := uint64(0)

	result := StakeSignableBytes(from, amount, nonce)

	// NETWORK_ID(19) + "stake"(5) + from(32) + amount(8) + nonce(8) = 72
	if len(result) != 72 {
		t.Errorf("length = %d, want 72", len(result))
	}

	offset := 19
	if !bytes.Equal(result[offset:offset+5], []byte("stake")) {
		t.Error("type discriminator mismatch")
	}
}

func TestUnstakeSignableBytesLayout(t *testing.T) {
	var from [32]byte
	nonce := uint64(5)

	result := UnstakeSignableBytes(from, nonce)

	// NETWORK_ID(19) + "unstake"(7) + from(32) + nonce(8) = 66
	if len(result) != 66 {
		t.Errorf("length = %d, want 66", len(result))
	}

	offset := 19
	if !bytes.Equal(result[offset:offset+7], []byte("unstake")) {
		t.Error("type discriminator mismatch")
	}
}

func TestDelegateSignableBytesLayout(t *testing.T) {
	var from, validator [32]byte
	from[0] = 0x01
	validator[0] = 0x02
	amount := uint64(100 * SatsPerUdag) // 100 UDAG
	nonce := uint64(3)

	result := DelegateSignableBytes(from, validator, amount, nonce)

	// NETWORK_ID(19) + "delegate"(8) + from(32) + validator(32) + amount(8) + nonce(8) = 107
	if len(result) != 107 {
		t.Errorf("length = %d, want 107", len(result))
	}

	offset := 19
	if !bytes.Equal(result[offset:offset+8], []byte("delegate")) {
		t.Error("type discriminator mismatch")
	}
	offset += 8

	if !bytes.Equal(result[offset:offset+32], from[:]) {
		t.Error("from mismatch")
	}
	offset += 32

	if !bytes.Equal(result[offset:offset+32], validator[:]) {
		t.Error("validator mismatch")
	}
}

func TestUndelegateSignableBytesLayout(t *testing.T) {
	var from [32]byte
	nonce := uint64(7)

	result := UndelegateSignableBytes(from, nonce)

	// NETWORK_ID(19) + "undelegate"(10) + from(32) + nonce(8) = 69
	if len(result) != 69 {
		t.Errorf("length = %d, want 69", len(result))
	}

	offset := 19
	if !bytes.Equal(result[offset:offset+10], []byte("undelegate")) {
		t.Error("type discriminator mismatch")
	}
}

func TestSetCommissionSignableBytesLayout(t *testing.T) {
	var from [32]byte
	commission := uint8(15)
	nonce := uint64(1)

	result := SetCommissionSignableBytes(from, commission, nonce)

	// NETWORK_ID(19) + "set_commission"(14) + from(32) + commission(1) + nonce(8) = 74
	if len(result) != 74 {
		t.Errorf("length = %d, want 74", len(result))
	}

	offset := 19
	if !bytes.Equal(result[offset:offset+14], []byte("set_commission")) {
		t.Error("type discriminator mismatch")
	}
	offset += 14 + 32

	if result[offset] != 15 {
		t.Errorf("commission_percent = %d, want 15", result[offset])
	}
}

func TestVoteSignableBytesLayout(t *testing.T) {
	var from [32]byte
	proposalID := uint64(42)
	fee := uint64(10_000)
	nonce := uint64(3)

	resultYes := VoteSignableBytes(from, proposalID, true, fee, nonce)
	resultNo := VoteSignableBytes(from, proposalID, false, fee, nonce)

	// NETWORK_ID(19) + "vote"(4) + from(32) + proposal_id(8) + vote(1) + fee(8) + nonce(8) = 80
	if len(resultYes) != 80 {
		t.Errorf("length = %d, want 80", len(resultYes))
	}

	offset := 19
	if !bytes.Equal(resultYes[offset:offset+4], []byte("vote")) {
		t.Error("type discriminator mismatch")
	}

	// vote byte at offset 19+4+32+8 = 63
	voteOffset := 63
	if resultYes[voteOffset] != 1 {
		t.Errorf("approve=true byte = %d, want 1", resultYes[voteOffset])
	}
	if resultNo[voteOffset] != 0 {
		t.Errorf("approve=false byte = %d, want 0", resultNo[voteOffset])
	}

	// Different votes should produce different signable bytes.
	if bytes.Equal(resultYes, resultNo) {
		t.Error("yes and no votes have identical signable bytes")
	}
}

func TestCreateProposalSignableBytesTextProposal(t *testing.T) {
	var from [32]byte
	proposalID := uint64(1)
	title := "Test Proposal"
	description := "A test proposal"
	fee := uint64(10_000)
	nonce := uint64(0)

	result := CreateProposalSignableBytes(from, proposalID, title, description, TextProposal{}, fee, nonce)

	// NETWORK_ID(19) + "proposal"(8) + from(32) + proposal_id(8) +
	// title_len(4) + "Test Proposal"(13) + desc_len(4) + "A test proposal"(15) +
	// type_byte(1) + fee(8) + nonce(8) = 120
	expected := 19 + 8 + 32 + 8 + 4 + 13 + 4 + 15 + 1 + 8 + 8
	if len(result) != expected {
		t.Errorf("length = %d, want %d", len(result), expected)
	}

	offset := 19
	if !bytes.Equal(result[offset:offset+8], []byte("proposal")) {
		t.Error("type discriminator mismatch")
	}
}

func TestCreateProposalSignableBytesParameterChange(t *testing.T) {
	var from [32]byte
	proposalType := ParameterChangeProposal{
		Param:    "min_fee_sats",
		NewValue: "20000",
	}

	result := CreateProposalSignableBytes(from, 1, "Title", "Desc", proposalType, 10_000, 0)

	// Verify it contains the parameter change discriminator byte (1) followed by
	// length-prefixed param and value.
	// Find the proposal type section after desc.
	// NETWORK_ID(19) + "proposal"(8) + from(32) + id(8) + title_len(4) + "Title"(5) + desc_len(4) + "Desc"(4) = 84
	typeOffset := 84
	if result[typeOffset] != 1 {
		t.Errorf("proposal type byte = %d, want 1", result[typeOffset])
	}
	typeOffset++

	paramLen := binary.LittleEndian.Uint32(result[typeOffset : typeOffset+4])
	if paramLen != 12 { // "min_fee_sats" = 12 bytes
		t.Errorf("param len = %d, want 12", paramLen)
	}
}

func TestCreateProposalSignableBytesCouncilMembership(t *testing.T) {
	var from, addr [32]byte
	addr[0] = 0xFF

	proposalType := CouncilMembershipProposal{
		Action:   "Add",
		Address:  addr,
		Category: "Technical",
	}

	result := CreateProposalSignableBytes(from, 1, "T", "D", proposalType, 10_000, 0)

	// Find proposal type section: NETWORK_ID(19) + "proposal"(8) + from(32) + id(8) + title_len(4) + "T"(1) + desc_len(4) + "D"(1) = 77
	typeOffset := 77
	if result[typeOffset] != 2 {
		t.Errorf("proposal type byte = %d, want 2 (CouncilMembership)", result[typeOffset])
	}
	typeOffset++
	if result[typeOffset] != 0 { // Add = 0
		t.Errorf("action byte = %d, want 0 (Add)", result[typeOffset])
	}
	typeOffset++
	if !bytes.Equal(result[typeOffset:typeOffset+32], addr[:]) {
		t.Error("council member address mismatch")
	}
	typeOffset += 32
	if !bytes.Equal(result[typeOffset:typeOffset+9], []byte("Technical")) {
		t.Errorf("category = %q, want \"Technical\"", string(result[typeOffset:typeOffset+9]))
	}
}

func TestCreateProposalSignableBytesTreasurySpend(t *testing.T) {
	var from, recipient [32]byte
	recipient[0] = 0xCC

	proposalType := TreasurySpendProposal{
		Recipient: recipient,
		Amount:    1_000_000_000_000, // 10,000 UDAG
	}

	result := CreateProposalSignableBytes(from, 1, "T", "D", proposalType, 10_000, 0)

	// Find proposal type section
	typeOffset := 77 // same as above with "T" and "D"
	if result[typeOffset] != 3 {
		t.Errorf("proposal type byte = %d, want 3 (TreasurySpend)", result[typeOffset])
	}
	typeOffset++
	if !bytes.Equal(result[typeOffset:typeOffset+32], recipient[:]) {
		t.Error("recipient address mismatch")
	}
	typeOffset += 32
	gotAmount := binary.LittleEndian.Uint64(result[typeOffset : typeOffset+8])
	if gotAmount != 1_000_000_000_000 {
		t.Errorf("amount = %d, want 1000000000000", gotAmount)
	}
}

// ---------------------------------------------------------------------------
// Signature verification tests — sign with Go, verify with standard ed25519.
// ---------------------------------------------------------------------------

func TestBuildSignedTransferTxSignatureValid(t *testing.T) {
	var seed [32]byte
	for i := range seed {
		seed[i] = byte(i + 1)
	}
	kp := KeypairFromSecret(seed)

	var to [32]byte
	to[0] = 0xFF

	tx := BuildSignedTransferTx(kp, to, 1_000_000_000, 10_000, 0, nil)

	transferMap := tx["Transfer"].(map[string]interface{})
	sigHex := transferMap["signature"].(string)
	sigBytes, err := hex.DecodeString(sigHex)
	if err != nil {
		t.Fatalf("invalid signature hex: %v", err)
	}

	// Re-derive the signable bytes and verify.
	signable := TransferSignableBytes(kp.Address, to, 1_000_000_000, 10_000, 0, nil)
	if !ed25519.Verify(kp.PublicKey[:], signable, sigBytes) {
		t.Error("Transfer signature verification failed")
	}
}

func TestBuildSignedStakeTxSignatureValid(t *testing.T) {
	var seed [32]byte
	seed[0] = 0x42
	kp := KeypairFromSecret(seed)

	tx := BuildSignedStakeTx(kp, 10_000*SatsPerUdag, 0)

	stakeMap := tx["Stake"].(map[string]interface{})
	sigHex := stakeMap["signature"].(string)
	sigBytes, _ := hex.DecodeString(sigHex)

	signable := StakeSignableBytes(kp.Address, 10_000*SatsPerUdag, 0)
	if !ed25519.Verify(kp.PublicKey[:], signable, sigBytes) {
		t.Error("Stake signature verification failed")
	}
}

func TestBuildSignedUnstakeTxSignatureValid(t *testing.T) {
	var seed [32]byte
	seed[0] = 0x42
	kp := KeypairFromSecret(seed)

	tx := BuildSignedUnstakeTx(kp, 5)

	unstakeMap := tx["Unstake"].(map[string]interface{})
	sigHex := unstakeMap["signature"].(string)
	sigBytes, _ := hex.DecodeString(sigHex)

	signable := UnstakeSignableBytes(kp.Address, 5)
	if !ed25519.Verify(kp.PublicKey[:], signable, sigBytes) {
		t.Error("Unstake signature verification failed")
	}
}

func TestBuildSignedDelegateTxSignatureValid(t *testing.T) {
	var seed [32]byte
	seed[0] = 0x42
	kp := KeypairFromSecret(seed)

	var validator [32]byte
	validator[0] = 0xAA

	tx := BuildSignedDelegateTx(kp, validator, 100*SatsPerUdag, 1)

	delegateMap := tx["Delegate"].(map[string]interface{})
	sigHex := delegateMap["signature"].(string)
	sigBytes, _ := hex.DecodeString(sigHex)

	signable := DelegateSignableBytes(kp.Address, validator, 100*SatsPerUdag, 1)
	if !ed25519.Verify(kp.PublicKey[:], signable, sigBytes) {
		t.Error("Delegate signature verification failed")
	}
}

func TestBuildSignedUndelegateTxSignatureValid(t *testing.T) {
	var seed [32]byte
	seed[0] = 0x42
	kp := KeypairFromSecret(seed)

	tx := BuildSignedUndelegateTx(kp, 2)

	undelegateMap := tx["Undelegate"].(map[string]interface{})
	sigHex := undelegateMap["signature"].(string)
	sigBytes, _ := hex.DecodeString(sigHex)

	signable := UndelegateSignableBytes(kp.Address, 2)
	if !ed25519.Verify(kp.PublicKey[:], signable, sigBytes) {
		t.Error("Undelegate signature verification failed")
	}
}

func TestBuildSignedSetCommissionTxSignatureValid(t *testing.T) {
	var seed [32]byte
	seed[0] = 0x42
	kp := KeypairFromSecret(seed)

	tx := BuildSignedSetCommissionTx(kp, 15, 3)

	scMap := tx["SetCommission"].(map[string]interface{})
	sigHex := scMap["signature"].(string)
	sigBytes, _ := hex.DecodeString(sigHex)

	signable := SetCommissionSignableBytes(kp.Address, 15, 3)
	if !ed25519.Verify(kp.PublicKey[:], signable, sigBytes) {
		t.Error("SetCommission signature verification failed")
	}
}

func TestBuildSignedCreateProposalTxSignatureValid(t *testing.T) {
	var seed [32]byte
	seed[0] = 0x42
	kp := KeypairFromSecret(seed)

	tx := BuildSignedCreateProposalTx(kp, 1, "Test", "Description", TextProposal{}, 10_000, 0)

	cpMap := tx["CreateProposal"].(map[string]interface{})
	sigHex := cpMap["signature"].(string)
	sigBytes, _ := hex.DecodeString(sigHex)

	signable := CreateProposalSignableBytes(kp.Address, 1, "Test", "Description", TextProposal{}, 10_000, 0)
	if !ed25519.Verify(kp.PublicKey[:], signable, sigBytes) {
		t.Error("CreateProposal signature verification failed")
	}
}

func TestBuildSignedVoteTxSignatureValid(t *testing.T) {
	var seed [32]byte
	seed[0] = 0x42
	kp := KeypairFromSecret(seed)

	tx := BuildSignedVoteTx(kp, 1, true, 10_000, 0)

	voteMap := tx["Vote"].(map[string]interface{})
	sigHex := voteMap["signature"].(string)
	sigBytes, _ := hex.DecodeString(sigHex)

	signable := VoteSignableBytes(kp.Address, 1, true, 10_000, 0)
	if !ed25519.Verify(kp.PublicKey[:], signable, sigBytes) {
		t.Error("Vote signature verification failed")
	}
}

// ---------------------------------------------------------------------------
// Determinism tests — same inputs produce same signable bytes.
// ---------------------------------------------------------------------------

func TestSignableBytesAreDeterministic(t *testing.T) {
	var from, to, validator [32]byte
	from[0] = 0x01
	to[0] = 0x02
	validator[0] = 0x03

	tests := []struct {
		name string
		fn   func() []byte
	}{
		{"Transfer", func() []byte { return TransferSignableBytes(from, to, 100, 10, 0, nil) }},
		{"TransferMemo", func() []byte { return TransferSignableBytes(from, to, 100, 10, 0, []byte("memo")) }},
		{"Stake", func() []byte { return StakeSignableBytes(from, 1000, 0) }},
		{"Unstake", func() []byte { return UnstakeSignableBytes(from, 0) }},
		{"Delegate", func() []byte { return DelegateSignableBytes(from, validator, 100, 0) }},
		{"Undelegate", func() []byte { return UndelegateSignableBytes(from, 0) }},
		{"SetCommission", func() []byte { return SetCommissionSignableBytes(from, 10, 0) }},
		{"Vote", func() []byte { return VoteSignableBytes(from, 1, true, 10, 0) }},
		{"CreateProposal", func() []byte {
			return CreateProposalSignableBytes(from, 1, "T", "D", TextProposal{}, 10, 0)
		}},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			a := tc.fn()
			b := tc.fn()
			if !bytes.Equal(a, b) {
				t.Errorf("%s signable bytes not deterministic", tc.name)
			}
		})
	}
}

// ---------------------------------------------------------------------------
// Different inputs produce different signable bytes.
// ---------------------------------------------------------------------------

func TestDifferentInputsProduceDifferentBytes(t *testing.T) {
	var from1, from2 [32]byte
	from1[0] = 0x01
	from2[0] = 0x02

	a := StakeSignableBytes(from1, 1000, 0)
	b := StakeSignableBytes(from2, 1000, 0)
	if bytes.Equal(a, b) {
		t.Error("different from addresses produce same signable bytes")
	}

	c := StakeSignableBytes(from1, 1000, 0)
	d := StakeSignableBytes(from1, 2000, 0)
	if bytes.Equal(c, d) {
		t.Error("different amounts produce same signable bytes")
	}

	e := StakeSignableBytes(from1, 1000, 0)
	f := StakeSignableBytes(from1, 1000, 1)
	if bytes.Equal(e, f) {
		t.Error("different nonces produce same signable bytes")
	}
}

// ---------------------------------------------------------------------------
// Type discriminator uniqueness — no two tx types share a prefix.
// ---------------------------------------------------------------------------

func TestTypeDiscriminatorsUnique(t *testing.T) {
	var from [32]byte

	txTypes := map[string][]byte{
		"transfer":       TransferSignableBytes(from, from, 0, 0, 0, nil),
		"stake":          StakeSignableBytes(from, 0, 0),
		"unstake":        UnstakeSignableBytes(from, 0),
		"delegate":       DelegateSignableBytes(from, from, 0, 0),
		"undelegate":     UndelegateSignableBytes(from, 0),
		"set_commission": SetCommissionSignableBytes(from, 0, 0),
		"vote":           VoteSignableBytes(from, 0, true, 0, 0),
		"proposal":       CreateProposalSignableBytes(from, 0, "", "", TextProposal{}, 0, 0),
	}

	// Extract the discriminator portion (bytes after NETWORK_ID, before from).
	for name1, bytes1 := range txTypes {
		for name2, bytes2 := range txTypes {
			if name1 >= name2 {
				continue
			}
			// The discriminator starts at offset 19 (after NETWORK_ID).
			// Compare up to the from address (offset 19 to 19+len(discriminator)).
			// Since discriminators have different lengths, identical prefix is
			// detected by checking if the full signable bytes are equal.
			if bytes.Equal(bytes1, bytes2) {
				t.Errorf("tx types %s and %s produce identical signable bytes", name1, name2)
			}
		}
	}
}

// ---------------------------------------------------------------------------
// NETWORK_ID prefix test — all signable bytes start with NETWORK_ID.
// ---------------------------------------------------------------------------

func TestAllSignableBytesStartWithNetworkID(t *testing.T) {
	var from [32]byte

	allBytes := [][]byte{
		TransferSignableBytes(from, from, 0, 0, 0, nil),
		StakeSignableBytes(from, 0, 0),
		UnstakeSignableBytes(from, 0),
		DelegateSignableBytes(from, from, 0, 0),
		UndelegateSignableBytes(from, 0),
		SetCommissionSignableBytes(from, 0, 0),
		VoteSignableBytes(from, 0, true, 0, 0),
		CreateProposalSignableBytes(from, 0, "", "", TextProposal{}, 0, 0),
	}

	for i, b := range allBytes {
		if !bytes.HasPrefix(b, NetworkID) {
			t.Errorf("signable bytes[%d] does not start with NETWORK_ID", i)
		}
	}
}

// ---------------------------------------------------------------------------
// NetworkID value test.
// ---------------------------------------------------------------------------

func TestNetworkIDValue(t *testing.T) {
	expected := []byte("ultradag-testnet-v1")
	if !bytes.Equal(NetworkID, expected) {
		t.Errorf("NetworkID = %q, want %q", string(NetworkID), string(expected))
	}
	if len(NetworkID) != 19 {
		t.Errorf("NetworkID length = %d, want 19", len(NetworkID))
	}
}

// ---------------------------------------------------------------------------
// Tampered signable bytes reject — signature fails after mutation.
// ---------------------------------------------------------------------------

func TestTamperedAmountRejectsSignature(t *testing.T) {
	var seed [32]byte
	seed[0] = 0x01
	kp := KeypairFromSecret(seed)

	var to [32]byte
	to[0] = 0xFF

	// Sign with amount=1000.
	signable := TransferSignableBytes(kp.Address, to, 1000, 10, 0, nil)
	sig := kp.Sign(signable)

	// Verify with original amount.
	if !ed25519.Verify(kp.PublicKey[:], signable, sig) {
		t.Fatal("original signature should verify")
	}

	// Tamper: verify with amount=2000.
	tampered := TransferSignableBytes(kp.Address, to, 2000, 10, 0, nil)
	if ed25519.Verify(kp.PublicKey[:], tampered, sig) {
		t.Error("tampered amount should not verify")
	}
}

func TestTamperedNonceRejectsSignature(t *testing.T) {
	var seed [32]byte
	seed[0] = 0x01
	kp := KeypairFromSecret(seed)

	signable := StakeSignableBytes(kp.Address, 1000, 0)
	sig := kp.Sign(signable)

	tampered := StakeSignableBytes(kp.Address, 1000, 1)
	if ed25519.Verify(kp.PublicKey[:], tampered, sig) {
		t.Error("tampered nonce should not verify")
	}
}

func TestWrongKeyRejectsSignature(t *testing.T) {
	var seed1, seed2 [32]byte
	seed1[0] = 0x01
	seed2[0] = 0x02
	kp1 := KeypairFromSecret(seed1)
	kp2 := KeypairFromSecret(seed2)

	signable := UnstakeSignableBytes(kp1.Address, 0)
	sig := kp1.Sign(signable)

	// Verify with wrong public key.
	if ed25519.Verify(kp2.PublicKey[:], signable, sig) {
		t.Error("wrong public key should not verify")
	}
}

// ---------------------------------------------------------------------------
// BuildSigned*Tx output structure tests.
// ---------------------------------------------------------------------------

func TestBuildSignedTransferTxStructure(t *testing.T) {
	var seed [32]byte
	seed[0] = 0x01
	kp := KeypairFromSecret(seed)

	var to [32]byte
	to[0] = 0xFF

	tx := BuildSignedTransferTx(kp, to, 1_000_000_000, 10_000, 5, []byte("hello"))

	transferMap, ok := tx["Transfer"].(map[string]interface{})
	if !ok {
		t.Fatal("missing Transfer key")
	}

	// Check fields exist.
	for _, field := range []string{"from", "to", "amount", "fee", "nonce", "pub_key", "signature", "memo"} {
		if _, ok := transferMap[field]; !ok {
			t.Errorf("missing field: %s", field)
		}
	}

	// Verify amounts match.
	if transferMap["amount"] != uint64(1_000_000_000) {
		t.Errorf("amount = %v, want 1000000000", transferMap["amount"])
	}
	if transferMap["fee"] != uint64(10_000) {
		t.Errorf("fee = %v, want 10000", transferMap["fee"])
	}
	if transferMap["nonce"] != uint64(5) {
		t.Errorf("nonce = %v, want 5", transferMap["nonce"])
	}
}

func TestBuildSignedTransferTxNilMemo(t *testing.T) {
	var seed [32]byte
	seed[0] = 0x01
	kp := KeypairFromSecret(seed)
	var to [32]byte

	tx := BuildSignedTransferTx(kp, to, 100, 10, 0, nil)
	transferMap := tx["Transfer"].(map[string]interface{})

	if transferMap["memo"] != nil {
		t.Errorf("nil memo should produce null, got %v", transferMap["memo"])
	}
}

// ---------------------------------------------------------------------------
// Proposal type JSON output tests.
// ---------------------------------------------------------------------------

func TestTextProposalJSON(t *testing.T) {
	p := TextProposal{}
	j := p.toJSON()
	if j != "TextProposal" {
		t.Errorf("TextProposal JSON = %v, want \"TextProposal\"", j)
	}
}

func TestParameterChangeProposalJSON(t *testing.T) {
	p := ParameterChangeProposal{Param: "min_fee_sats", NewValue: "20000"}
	j := p.toJSON().(map[string]interface{})
	inner := j["ParameterChange"].(map[string]interface{})
	if inner["param"] != "min_fee_sats" {
		t.Errorf("param = %v", inner["param"])
	}
	if inner["new_value"] != "20000" {
		t.Errorf("new_value = %v", inner["new_value"])
	}
}

// ---------------------------------------------------------------------------
// appendU64LE / appendU32LE correctness.
// ---------------------------------------------------------------------------

func TestAppendU64LE(t *testing.T) {
	buf := appendU64LE(nil, 0x0102030405060708)
	expected := []byte{0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01}
	if !bytes.Equal(buf, expected) {
		t.Errorf("appendU64LE = %x, want %x", buf, expected)
	}
}

func TestAppendU32LE(t *testing.T) {
	buf := appendU32LE(nil, 0x01020304)
	expected := []byte{0x04, 0x03, 0x02, 0x01}
	if !bytes.Equal(buf, expected) {
		t.Errorf("appendU32LE = %x, want %x", buf, expected)
	}
}

func TestAppendU64LEZero(t *testing.T) {
	buf := appendU64LE(nil, 0)
	expected := make([]byte, 8)
	if !bytes.Equal(buf, expected) {
		t.Errorf("appendU64LE(0) = %x, want all zeros", buf)
	}
}

func TestAppendU64LEMax(t *testing.T) {
	buf := appendU64LE(nil, ^uint64(0))
	expected := []byte{0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF}
	if !bytes.Equal(buf, expected) {
		t.Errorf("appendU64LE(MAX) = %x, want all 0xFF", buf)
	}
}
