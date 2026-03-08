"""Local Ed25519 cryptography for UltraDAG.

Provides offline keypair generation, signing, and address derivation
without requiring an RPC connection to a node.

Address derivation: address = blake3(ed25519_public_key) as 64-char hex string.
"""

import os
import hashlib
from typing import Optional

try:
    import blake3 as _blake3

    def _blake3_hash(data: bytes) -> bytes:
        return _blake3.blake3(data).digest()

except ImportError:
    _blake3 = None

    def _blake3_hash(data: bytes) -> bytes:
        raise ImportError(
            "blake3 package is required for address derivation. "
            "Install it with: pip install blake3"
        )

try:
    from nacl.signing import SigningKey, VerifyKey
    from nacl.exceptions import BadSignatureError as _NaclBadSig

    _BACKEND = "nacl"
except ImportError:
    try:
        from cryptography.hazmat.primitives.asymmetric.ed25519 import (
            Ed25519PrivateKey,
            Ed25519PublicKey,
        )
        from cryptography.hazmat.primitives import serialization
        from cryptography.exceptions import InvalidSignature as _CryptoInvalidSig

        _BACKEND = "cryptography"
    except ImportError:
        _BACKEND = None


class Keypair:
    """An Ed25519 keypair with UltraDAG address derivation.

    The secret key is a 32-byte Ed25519 seed. The address is the blake3
    hash of the Ed25519 public key, represented as a 64-character hex string.

    Attributes:
        secret_key: 64-character hex string of the 32-byte secret key seed.
        public_key: 64-character hex string of the 32-byte Ed25519 public key.
        address: 64-character hex string of the blake3 hash of the public key.
    """

    def __init__(self, secret_key_bytes: bytes):
        """Initialize a keypair from a 32-byte secret key seed.

        Args:
            secret_key_bytes: 32-byte Ed25519 secret key seed.

        Raises:
            ValueError: If the secret key is not exactly 32 bytes.
            ImportError: If neither PyNaCl nor cryptography is installed.
        """
        if len(secret_key_bytes) != 32:
            raise ValueError(f"Secret key must be 32 bytes, got {len(secret_key_bytes)}")

        self._sk_bytes = secret_key_bytes

        if _BACKEND == "nacl":
            self._signing_key = SigningKey(secret_key_bytes)
            self._pk_bytes = bytes(self._signing_key.verify_key)
        elif _BACKEND == "cryptography":
            self._private_key = Ed25519PrivateKey.from_private_bytes(secret_key_bytes)
            self._pk_bytes = self._private_key.public_key().public_bytes(
                serialization.Encoding.Raw,
                serialization.PublicFormat.Raw,
            )
        else:
            raise ImportError(
                "Either PyNaCl or cryptography package is required. "
                "Install with: pip install pynacl  or  pip install cryptography"
            )

        self.secret_key: str = secret_key_bytes.hex()
        self.public_key: str = self._pk_bytes.hex()
        self.address: str = _blake3_hash(self._pk_bytes).hex()

    @classmethod
    def generate(cls) -> "Keypair":
        """Generate a new random keypair.

        Returns:
            A new Keypair with a cryptographically random secret key.
        """
        return cls(os.urandom(32))

    @classmethod
    def from_hex(cls, secret_key_hex: str) -> "Keypair":
        """Create a keypair from a hex-encoded secret key.

        Args:
            secret_key_hex: 64-character hex string of the 32-byte secret key.

        Returns:
            A Keypair derived from the given secret key.

        Raises:
            ValueError: If the hex string is invalid or wrong length.
        """
        try:
            sk_bytes = bytes.fromhex(secret_key_hex)
        except ValueError:
            raise ValueError("Invalid hex string for secret key")
        return cls(sk_bytes)

    @classmethod
    def from_seed(cls, seed: bytes) -> "Keypair":
        """Create a keypair from a deterministic 32-byte seed.

        This matches UltraDAG's SecretKey::from_bytes() behavior for
        deterministic keypairs (e.g., faucet key = [0xFA; 32]).

        Args:
            seed: 32-byte seed value.

        Returns:
            A Keypair derived from the seed.
        """
        return cls(seed)

    def sign(self, message: bytes) -> bytes:
        """Sign a message with this keypair's secret key.

        Args:
            message: Arbitrary bytes to sign.

        Returns:
            64-byte Ed25519 signature.
        """
        if _BACKEND == "nacl":
            signed = self._signing_key.sign(message)
            return signed.signature
        elif _BACKEND == "cryptography":
            return self._private_key.sign(message)
        else:
            raise ImportError("No Ed25519 backend available")

    def sign_hex(self, message: bytes) -> str:
        """Sign a message and return the signature as a hex string.

        Args:
            message: Arbitrary bytes to sign.

        Returns:
            128-character hex string of the 64-byte signature.
        """
        return self.sign(message).hex()

    @staticmethod
    def verify(public_key_bytes: bytes, message: bytes, signature: bytes) -> bool:
        """Verify an Ed25519 signature against a public key.

        Args:
            public_key_bytes: 32-byte Ed25519 public key.
            message: The original message that was signed.
            signature: 64-byte Ed25519 signature.

        Returns:
            True if the signature is valid, False otherwise.
        """
        if _BACKEND == "nacl":
            try:
                verify_key = VerifyKey(public_key_bytes)
                verify_key.verify(message, signature)
                return True
            except _NaclBadSig:
                return False
        elif _BACKEND == "cryptography":
            try:
                pub = Ed25519PublicKey.from_public_bytes(public_key_bytes)
                pub.verify(signature, message)
                return True
            except _CryptoInvalidSig:
                return False
        else:
            raise ImportError("No Ed25519 backend available")

    def __repr__(self) -> str:
        return f"Keypair(address={self.address[:16]}...)"

    def __eq__(self, other: object) -> bool:
        if not isinstance(other, Keypair):
            return NotImplemented
        return self._sk_bytes == other._sk_bytes


def derive_address(public_key_bytes: bytes) -> str:
    """Derive an UltraDAG address from a raw Ed25519 public key.

    Args:
        public_key_bytes: 32-byte Ed25519 public key.

    Returns:
        64-character hex string of the blake3 hash.

    Raises:
        ValueError: If the public key is not 32 bytes.
    """
    if len(public_key_bytes) != 32:
        raise ValueError(f"Public key must be 32 bytes, got {len(public_key_bytes)}")
    return _blake3_hash(public_key_bytes).hex()
