"""Tests for local Ed25519 cryptography module."""

import unittest

from ultradag.crypto import Keypair, derive_address
from ultradag.types import sats_to_udag, udag_to_sats


class TestKeypairGeneration(unittest.TestCase):
    def test_generate_random(self):
        kp = Keypair.generate()
        self.assertEqual(len(kp.secret_key), 64)
        self.assertEqual(len(kp.public_key), 64)
        self.assertEqual(len(kp.address), 64)

    def test_generate_unique(self):
        kp1 = Keypair.generate()
        kp2 = Keypair.generate()
        self.assertNotEqual(kp1.secret_key, kp2.secret_key)
        self.assertNotEqual(kp1.address, kp2.address)

    def test_from_hex(self):
        hex_key = "ab" * 32
        kp = Keypair.from_hex(hex_key)
        self.assertEqual(kp.secret_key, hex_key)
        self.assertEqual(len(kp.address), 64)

    def test_from_hex_invalid(self):
        with self.assertRaises(ValueError):
            Keypair.from_hex("not_hex")

    def test_from_hex_wrong_length(self):
        with self.assertRaises(ValueError):
            Keypair.from_hex("ab" * 16)

    def test_from_seed(self):
        seed = bytes([0xFA] * 32)
        kp = Keypair.from_seed(seed)
        self.assertEqual(len(kp.address), 64)
        # Deterministic: same seed produces same keypair
        kp2 = Keypair.from_seed(seed)
        self.assertEqual(kp.secret_key, kp2.secret_key)
        self.assertEqual(kp.address, kp2.address)

    def test_from_bytes_wrong_length(self):
        with self.assertRaises(ValueError):
            Keypair(b"too_short")

    def test_faucet_keypair_deterministic(self):
        """Verify faucet key derivation matches UltraDAG convention."""
        faucet_seed = bytes([0xFA] * 32)
        kp = Keypair.from_seed(faucet_seed)
        # Same seed always produces the same keypair
        kp2 = Keypair.from_seed(faucet_seed)
        self.assertEqual(kp.address, kp2.address)
        self.assertEqual(kp.public_key, kp2.public_key)

    def test_dev_keypair_deterministic(self):
        """Verify dev allocation key derivation matches UltraDAG convention."""
        dev_seed = bytes([0xDE] * 32)
        kp = Keypair.from_seed(dev_seed)
        kp2 = Keypair.from_seed(dev_seed)
        self.assertEqual(kp.address, kp2.address)

    def test_different_seeds_different_keys(self):
        kp_faucet = Keypair.from_seed(bytes([0xFA] * 32))
        kp_dev = Keypair.from_seed(bytes([0xDE] * 32))
        self.assertNotEqual(kp_faucet.address, kp_dev.address)
        self.assertNotEqual(kp_faucet.public_key, kp_dev.public_key)


class TestKeypairEquality(unittest.TestCase):
    def test_equal_keypairs(self):
        seed = bytes([0x01] * 32)
        kp1 = Keypair.from_seed(seed)
        kp2 = Keypair.from_seed(seed)
        self.assertEqual(kp1, kp2)

    def test_unequal_keypairs(self):
        kp1 = Keypair.from_seed(bytes([0x01] * 32))
        kp2 = Keypair.from_seed(bytes([0x02] * 32))
        self.assertNotEqual(kp1, kp2)

    def test_not_equal_to_other_types(self):
        kp = Keypair.generate()
        self.assertNotEqual(kp, "not a keypair")
        self.assertNotEqual(kp, 42)

    def test_repr(self):
        kp = Keypair.generate()
        r = repr(kp)
        self.assertIn("Keypair(address=", r)
        self.assertIn("...", r)


class TestSigning(unittest.TestCase):
    def test_sign_and_verify(self):
        kp = Keypair.generate()
        message = b"hello ultradag"
        sig = kp.sign(message)
        self.assertEqual(len(sig), 64)
        self.assertTrue(
            Keypair.verify(bytes.fromhex(kp.public_key), message, sig)
        )

    def test_sign_hex(self):
        kp = Keypair.generate()
        message = b"test message"
        sig_hex = kp.sign_hex(message)
        self.assertEqual(len(sig_hex), 128)
        sig_bytes = bytes.fromhex(sig_hex)
        self.assertTrue(
            Keypair.verify(bytes.fromhex(kp.public_key), message, sig_bytes)
        )

    def test_verify_wrong_message(self):
        kp = Keypair.generate()
        sig = kp.sign(b"correct message")
        self.assertFalse(
            Keypair.verify(bytes.fromhex(kp.public_key), b"wrong message", sig)
        )

    def test_verify_wrong_key(self):
        kp1 = Keypair.generate()
        kp2 = Keypair.generate()
        sig = kp1.sign(b"message")
        self.assertFalse(
            Keypair.verify(bytes.fromhex(kp2.public_key), b"message", sig)
        )

    def test_sign_empty_message(self):
        kp = Keypair.generate()
        sig = kp.sign(b"")
        self.assertEqual(len(sig), 64)
        self.assertTrue(Keypair.verify(bytes.fromhex(kp.public_key), b"", sig))

    def test_sign_large_message(self):
        kp = Keypair.generate()
        message = b"x" * 10000
        sig = kp.sign(message)
        self.assertTrue(Keypair.verify(bytes.fromhex(kp.public_key), message, sig))

    def test_deterministic_signatures(self):
        """Ed25519 signatures are deterministic for the same key + message."""
        kp = Keypair.generate()
        message = b"deterministic"
        sig1 = kp.sign(message)
        sig2 = kp.sign(message)
        self.assertEqual(sig1, sig2)


class TestAddressDerivation(unittest.TestCase):
    def test_derive_address_from_pubkey(self):
        kp = Keypair.generate()
        addr = derive_address(bytes.fromhex(kp.public_key))
        self.assertEqual(addr, kp.address)

    def test_derive_address_wrong_length(self):
        with self.assertRaises(ValueError):
            derive_address(b"short")

    def test_address_is_blake3_of_pubkey(self):
        """Verify address = blake3(ed25519_pubkey)."""
        import blake3
        kp = Keypair.generate()
        expected = blake3.blake3(bytes.fromhex(kp.public_key)).hexdigest()
        self.assertEqual(kp.address, expected)

    def test_address_length(self):
        kp = Keypair.generate()
        # 32 bytes = 64 hex chars
        self.assertEqual(len(kp.address), 64)
        # Verify it is valid hex
        bytes.fromhex(kp.address)


class TestConversionHelpers(unittest.TestCase):
    def test_sats_to_udag(self):
        self.assertAlmostEqual(sats_to_udag(100_000_000), 1.0)
        self.assertAlmostEqual(sats_to_udag(50_000_000), 0.5)
        self.assertAlmostEqual(sats_to_udag(0), 0.0)
        self.assertAlmostEqual(sats_to_udag(1), 0.00000001)

    def test_udag_to_sats(self):
        self.assertEqual(udag_to_sats(1.0), 100_000_000)
        self.assertEqual(udag_to_sats(0.5), 50_000_000)
        self.assertEqual(udag_to_sats(0.0), 0)
        self.assertEqual(udag_to_sats(21_000_000), 2_100_000_000_000_000)

    def test_roundtrip(self):
        original_sats = 123_456_789
        udag = sats_to_udag(original_sats)
        back = udag_to_sats(udag)
        self.assertEqual(back, original_sats)


if __name__ == "__main__":
    unittest.main()
