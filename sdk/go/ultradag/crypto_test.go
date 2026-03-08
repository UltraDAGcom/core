package ultradag

import (
	"crypto/ed25519"
	"encoding/hex"
	"testing"

	"lukechampine.com/blake3"
)

func TestGenerateKeypair(t *testing.T) {
	kp, err := GenerateKeypair()
	if err != nil {
		t.Fatalf("GenerateKeypair() error: %v", err)
	}

	// Secret key should not be all zeros.
	allZero := true
	for _, b := range kp.SecretKey {
		if b != 0 {
			allZero = false
			break
		}
	}
	if allZero {
		t.Error("SecretKey is all zeros")
	}

	// Address should be blake3 of public key.
	expectedAddr := blake3.Sum256(kp.PublicKey[:])
	if kp.Address != expectedAddr {
		t.Errorf("Address mismatch: got %x, want %x", kp.Address, expectedAddr)
	}
}

func TestGenerateKeypairUniqueness(t *testing.T) {
	kp1, _ := GenerateKeypair()
	kp2, _ := GenerateKeypair()
	if kp1.SecretKey == kp2.SecretKey {
		t.Error("two generated keypairs have identical secret keys")
	}
	if kp1.Address == kp2.Address {
		t.Error("two generated keypairs have identical addresses")
	}
}

func TestKeypairFromSecret(t *testing.T) {
	var seed [32]byte
	for i := range seed {
		seed[i] = byte(i)
	}

	kp := KeypairFromSecret(seed)

	// Should be deterministic.
	kp2 := KeypairFromSecret(seed)
	if kp.SecretKey != kp2.SecretKey {
		t.Error("KeypairFromSecret is not deterministic for SecretKey")
	}
	if kp.PublicKey != kp2.PublicKey {
		t.Error("KeypairFromSecret is not deterministic for PublicKey")
	}
	if kp.Address != kp2.Address {
		t.Error("KeypairFromSecret is not deterministic for Address")
	}

	// Verify address = blake3(pubkey).
	expectedAddr := blake3.Sum256(kp.PublicKey[:])
	if kp.Address != expectedAddr {
		t.Errorf("Address = %x, want %x", kp.Address, expectedAddr)
	}

	// Verify public key matches ed25519 derivation.
	privKey := ed25519.NewKeyFromSeed(seed[:])
	pubKey := privKey.Public().(ed25519.PublicKey)
	var expectedPub [32]byte
	copy(expectedPub[:], pubKey)
	if kp.PublicKey != expectedPub {
		t.Error("PublicKey does not match ed25519 derivation")
	}
}

func TestKeypairFromSecretFaucet(t *testing.T) {
	// Reproduce the faucet keypair: seed = [0xFA; 32].
	var seed [32]byte
	for i := range seed {
		seed[i] = 0xFA
	}
	kp := KeypairFromSecret(seed)

	// Just verify it produces a valid keypair with non-zero address.
	if kp.AddressHex() == "0000000000000000000000000000000000000000000000000000000000000000" {
		t.Error("faucet keypair has zero address")
	}
}

func TestSecretKeyHex(t *testing.T) {
	var seed [32]byte
	seed[0] = 0xAB
	seed[31] = 0xCD
	kp := KeypairFromSecret(seed)

	hexStr := kp.SecretKeyHex()
	if len(hexStr) != 64 {
		t.Errorf("SecretKeyHex length = %d, want 64", len(hexStr))
	}

	// Decode and verify.
	decoded, err := hex.DecodeString(hexStr)
	if err != nil {
		t.Fatalf("SecretKeyHex produced invalid hex: %v", err)
	}
	if decoded[0] != 0xAB || decoded[31] != 0xCD {
		t.Error("SecretKeyHex round-trip failed")
	}
}

func TestAddressHex(t *testing.T) {
	var seed [32]byte
	kp := KeypairFromSecret(seed)
	hexStr := kp.AddressHex()
	if len(hexStr) != 64 {
		t.Errorf("AddressHex length = %d, want 64", len(hexStr))
	}
	_, err := hex.DecodeString(hexStr)
	if err != nil {
		t.Fatalf("AddressHex produced invalid hex: %v", err)
	}
}

func TestPublicKeyHex(t *testing.T) {
	var seed [32]byte
	kp := KeypairFromSecret(seed)
	hexStr := kp.PublicKeyHex()
	if len(hexStr) != 64 {
		t.Errorf("PublicKeyHex length = %d, want 64", len(hexStr))
	}
}

func TestSign(t *testing.T) {
	var seed [32]byte
	for i := range seed {
		seed[i] = byte(i + 1)
	}
	kp := KeypairFromSecret(seed)

	msg := []byte("hello ultradag")
	sig := kp.Sign(msg)

	if len(sig) != ed25519.SignatureSize {
		t.Fatalf("signature length = %d, want %d", len(sig), ed25519.SignatureSize)
	}

	// Verify with standard library.
	if !ed25519.Verify(kp.PublicKey[:], msg, sig) {
		t.Error("signature verification failed with ed25519.Verify")
	}
}

func TestVerify(t *testing.T) {
	var seed [32]byte
	for i := range seed {
		seed[i] = 42
	}
	kp := KeypairFromSecret(seed)

	msg := []byte("verify this message")
	sig := kp.Sign(msg)

	if !kp.Verify(msg, sig) {
		t.Error("Verify returned false for valid signature")
	}

	// Tamper with message.
	tampered := []byte("tampered message")
	if kp.Verify(tampered, sig) {
		t.Error("Verify returned true for tampered message")
	}

	// Tamper with signature.
	badSig := make([]byte, len(sig))
	copy(badSig, sig)
	badSig[0] ^= 0xFF
	if kp.Verify(msg, badSig) {
		t.Error("Verify returned true for tampered signature")
	}
}

func TestSignDeterministic(t *testing.T) {
	var seed [32]byte
	seed[0] = 0x99
	kp := KeypairFromSecret(seed)

	msg := []byte("deterministic signature")
	sig1 := kp.Sign(msg)
	sig2 := kp.Sign(msg)

	// Ed25519 signatures are deterministic.
	for i := range sig1 {
		if sig1[i] != sig2[i] {
			t.Fatal("signatures differ for same message")
		}
	}
}

func TestDeriveAddress(t *testing.T) {
	pubKey := make([]byte, 32)
	for i := range pubKey {
		pubKey[i] = byte(i)
	}

	addr := DeriveAddress(pubKey)
	expected := blake3.Sum256(pubKey)
	if addr != expected {
		t.Errorf("DeriveAddress = %x, want %x", addr, expected)
	}
}

func TestDeriveAddressMatchesKeypair(t *testing.T) {
	var seed [32]byte
	for i := range seed {
		seed[i] = 0x55
	}
	kp := KeypairFromSecret(seed)

	addr := DeriveAddress(kp.PublicKey[:])
	if addr != kp.Address {
		t.Error("DeriveAddress does not match keypair Address")
	}
}
