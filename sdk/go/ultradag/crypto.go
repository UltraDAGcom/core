package ultradag

import (
	"crypto/ed25519"
	"crypto/rand"
	"encoding/hex"

	"lukechampine.com/blake3"
)

// Keypair holds an Ed25519 signing key and the derived UltraDAG address.
// The address is the Blake3 hash of the Ed25519 public key.
type Keypair struct {
	// SecretKey is the 32-byte Ed25519 seed.
	SecretKey [32]byte
	// PublicKey is the 32-byte Ed25519 public key.
	PublicKey [32]byte
	// Address is the 32-byte Blake3 hash of the public key.
	Address [32]byte
}

// GenerateKeypair creates a new random Ed25519 keypair with a derived
// UltraDAG address. It uses crypto/rand for key generation.
func GenerateKeypair() (*Keypair, error) {
	var seed [32]byte
	if _, err := rand.Read(seed[:]); err != nil {
		return nil, err
	}
	return KeypairFromSecret(seed), nil
}

// KeypairFromSecret derives a keypair from a 32-byte Ed25519 seed.
// This is deterministic: the same seed always produces the same keypair.
func KeypairFromSecret(seed [32]byte) *Keypair {
	privKey := ed25519.NewKeyFromSeed(seed[:])
	pubKey := privKey.Public().(ed25519.PublicKey)

	kp := &Keypair{}
	copy(kp.SecretKey[:], seed[:])
	copy(kp.PublicKey[:], pubKey)
	kp.Address = blake3.Sum256(pubKey)
	return kp
}

// SecretKeyHex returns the secret key as a lowercase hex string.
func (k *Keypair) SecretKeyHex() string {
	return hex.EncodeToString(k.SecretKey[:])
}

// AddressHex returns the address as a lowercase hex string.
func (k *Keypair) AddressHex() string {
	return hex.EncodeToString(k.Address[:])
}

// PublicKeyHex returns the public key as a lowercase hex string.
func (k *Keypair) PublicKeyHex() string {
	return hex.EncodeToString(k.PublicKey[:])
}

// Sign signs a message using the keypair's Ed25519 private key.
// Returns a 64-byte Ed25519 signature.
func (k *Keypair) Sign(msg []byte) []byte {
	privKey := ed25519.NewKeyFromSeed(k.SecretKey[:])
	return ed25519.Sign(privKey, msg)
}

// Verify checks an Ed25519 signature against the keypair's public key.
func (k *Keypair) Verify(msg, sig []byte) bool {
	return ed25519.Verify(k.PublicKey[:], msg, sig)
}

// DeriveAddress computes the UltraDAG address (Blake3 hash) from a raw
// Ed25519 public key.
func DeriveAddress(publicKey []byte) [32]byte {
	return blake3.Sum256(publicKey)
}
