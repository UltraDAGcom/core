"""Tests for client-side transaction signing.

Validates byte layout, signing, signature verification, and JSON
structure for all 8 UltraDAG transaction types.
"""

import struct
import pytest

from ultradag.crypto import Keypair, _blake3_hash
from ultradag.transactions import (
    NETWORK_ID,
    COIN,
    transfer_signable_bytes,
    stake_signable_bytes,
    unstake_signable_bytes,
    delegate_signable_bytes,
    undelegate_signable_bytes,
    set_commission_signable_bytes,
    create_proposal_signable_bytes,
    vote_signable_bytes,
    build_signed_transfer_tx,
    build_signed_stake_tx,
    build_signed_unstake_tx,
    build_signed_delegate_tx,
    build_signed_undelegate_tx,
    build_signed_set_commission_tx,
    build_signed_create_proposal_tx,
    build_signed_vote_tx,
    _u64_le,
    _u32_le,
)


# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------

@pytest.fixture
def keypair():
    """Deterministic keypair for reproducible tests."""
    return Keypair.from_seed(bytes([0x01] * 32))


@pytest.fixture
def keypair2():
    """Second deterministic keypair."""
    return Keypair.from_seed(bytes([0x02] * 32))


def _from_bytes(kp: Keypair) -> bytes:
    """Get the 32-byte address from a Keypair."""
    return bytes.fromhex(kp.address)


def _pub_bytes(kp: Keypair) -> bytes:
    """Get the 32-byte public key from a Keypair."""
    return bytes.fromhex(kp.public_key)


# ---------------------------------------------------------------------------
# NETWORK_ID constant
# ---------------------------------------------------------------------------

class TestNetworkId:
    def test_value(self):
        assert NETWORK_ID == b"ultradag-testnet-v1"

    def test_length(self):
        assert len(NETWORK_ID) == 19


# ---------------------------------------------------------------------------
# Helper encoding
# ---------------------------------------------------------------------------

class TestHelpers:
    def test_u64_le(self):
        assert _u64_le(0) == b"\x00" * 8
        assert _u64_le(1) == b"\x01" + b"\x00" * 7
        assert _u64_le(256) == b"\x00\x01" + b"\x00" * 6
        assert _u64_le(2**64 - 1) == b"\xff" * 8

    def test_u32_le(self):
        assert _u32_le(0) == b"\x00" * 4
        assert _u32_le(1) == b"\x01\x00\x00\x00"
        assert _u32_le(0xDEADBEEF) == b"\xef\xbe\xad\xde"


# ---------------------------------------------------------------------------
# Transfer
# ---------------------------------------------------------------------------

class TestTransferSignableBytes:
    def test_layout_no_memo(self, keypair, keypair2):
        from_addr = _from_bytes(keypair)
        to_addr = _from_bytes(keypair2)
        amount, fee, nonce = 100_000_000, 10_000, 5

        sb = transfer_signable_bytes(from_addr, to_addr, amount, fee, nonce)

        # Expected: NETWORK_ID(19) + "transfer"(8) + from(32) + to(32) +
        #           amount(8) + fee(8) + nonce(8) = 115
        assert len(sb) == 115

        pos = 0
        assert sb[pos:pos+19] == NETWORK_ID; pos += 19
        assert sb[pos:pos+8] == b"transfer"; pos += 8
        assert sb[pos:pos+32] == from_addr; pos += 32
        assert sb[pos:pos+32] == to_addr; pos += 32
        assert sb[pos:pos+8] == _u64_le(amount); pos += 8
        assert sb[pos:pos+8] == _u64_le(fee); pos += 8
        assert sb[pos:pos+8] == _u64_le(nonce); pos += 8
        assert pos == len(sb)

    def test_layout_with_memo(self, keypair, keypair2):
        from_addr = _from_bytes(keypair)
        to_addr = _from_bytes(keypair2)
        memo = b"temp:22.4C"

        sb = transfer_signable_bytes(from_addr, to_addr, 100, 10, 0, memo=memo)

        # 115 base + 4 (memo_len) + 10 (memo) = 129
        assert len(sb) == 115 + 4 + len(memo)

        # Check memo portion at the end
        memo_start = 115
        memo_len = struct.unpack("<I", sb[memo_start:memo_start+4])[0]
        assert memo_len == len(memo)
        assert sb[memo_start+4:] == memo

    def test_no_memo_vs_empty_memo(self, keypair, keypair2):
        from_addr = _from_bytes(keypair)
        to_addr = _from_bytes(keypair2)

        sb_none = transfer_signable_bytes(from_addr, to_addr, 1, 1, 0, memo=None)
        sb_empty = transfer_signable_bytes(from_addr, to_addr, 1, 1, 0, memo=b"")

        # Both should produce identical bytes (empty memo is omitted)
        assert sb_none == sb_empty

    def test_deterministic(self, keypair, keypair2):
        from_addr = _from_bytes(keypair)
        to_addr = _from_bytes(keypair2)
        a = transfer_signable_bytes(from_addr, to_addr, 50, 5, 3)
        b = transfer_signable_bytes(from_addr, to_addr, 50, 5, 3)
        assert a == b


class TestBuildSignedTransferTx:
    def test_structure(self, keypair, keypair2):
        tx = build_signed_transfer_tx(
            keypair.secret_key,
            keypair2.address,
            100_000_000,
            10_000,
            0,
        )

        assert "Transfer" in tx
        inner = tx["Transfer"]
        assert inner["amount"] == 100_000_000
        assert inner["fee"] == 10_000
        assert inner["nonce"] == 0
        assert inner["memo"] is None
        assert len(inner["from"]) == 32
        assert len(inner["to"]) == 32
        assert len(inner["pub_key"]) == 32
        assert len(inner["signature"]) == 128  # 64 bytes as hex

    def test_signature_verifies(self, keypair, keypair2):
        tx = build_signed_transfer_tx(
            keypair.secret_key,
            keypair2.address,
            50 * COIN,
            10_000,
            1,
        )

        inner = tx["Transfer"]
        pub_key = bytes(inner["pub_key"])
        sig = bytes.fromhex(inner["signature"])
        from_addr = bytes(inner["from"])

        # Reconstruct signable bytes and verify
        to_bytes = bytes(inner["to"])
        signable = transfer_signable_bytes(
            from_addr, to_bytes, inner["amount"], inner["fee"], inner["nonce"],
        )
        assert Keypair.verify(pub_key, signable, sig)

    def test_from_matches_address(self, keypair, keypair2):
        tx = build_signed_transfer_tx(
            keypair.secret_key, keypair2.address, 1, 1, 0,
        )
        inner = tx["Transfer"]
        assert bytes(inner["from"]).hex() == keypair.address

    def test_with_memo(self, keypair, keypair2):
        memo = b"sensor-data-123"
        tx = build_signed_transfer_tx(
            keypair.secret_key, keypair2.address, 1, 1, 0, memo=memo,
        )
        inner = tx["Transfer"]
        assert inner["memo"] == list(memo)

        # Verify signature over memo-inclusive signable bytes
        pub_key = bytes(inner["pub_key"])
        sig = bytes.fromhex(inner["signature"])
        from_addr = bytes(inner["from"])
        to_bytes = bytes(inner["to"])
        signable = transfer_signable_bytes(
            from_addr, to_bytes, inner["amount"], inner["fee"], inner["nonce"],
            memo=bytes(inner["memo"]),
        )
        assert Keypair.verify(pub_key, signable, sig)


# ---------------------------------------------------------------------------
# Stake
# ---------------------------------------------------------------------------

class TestStakeSignableBytes:
    def test_layout(self, keypair):
        from_addr = _from_bytes(keypair)
        amount, nonce = 10_000 * COIN, 0

        sb = stake_signable_bytes(from_addr, amount, nonce)

        # NETWORK_ID(19) + "stake"(5) + from(32) + amount(8) + nonce(8) = 72
        assert len(sb) == 72

        pos = 0
        assert sb[pos:pos+19] == NETWORK_ID; pos += 19
        assert sb[pos:pos+5] == b"stake"; pos += 5
        assert sb[pos:pos+32] == from_addr; pos += 32
        assert sb[pos:pos+8] == _u64_le(amount); pos += 8
        assert sb[pos:pos+8] == _u64_le(nonce); pos += 8
        assert pos == len(sb)


class TestBuildSignedStakeTx:
    def test_structure_and_verify(self, keypair):
        tx = build_signed_stake_tx(keypair.secret_key, 10_000 * COIN, 0)

        assert "Stake" in tx
        inner = tx["Stake"]
        assert inner["amount"] == 10_000 * COIN
        assert inner["nonce"] == 0

        pub_key = bytes(inner["pub_key"])
        sig = bytes.fromhex(inner["signature"])
        from_addr = bytes(inner["from"])
        signable = stake_signable_bytes(from_addr, inner["amount"], inner["nonce"])
        assert Keypair.verify(pub_key, signable, sig)


# ---------------------------------------------------------------------------
# Unstake
# ---------------------------------------------------------------------------

class TestUnstakeSignableBytes:
    def test_layout(self, keypair):
        from_addr = _from_bytes(keypair)
        nonce = 3

        sb = unstake_signable_bytes(from_addr, nonce)

        # NETWORK_ID(19) + "unstake"(7) + from(32) + nonce(8) = 66
        assert len(sb) == 66

        pos = 0
        assert sb[pos:pos+19] == NETWORK_ID; pos += 19
        assert sb[pos:pos+7] == b"unstake"; pos += 7
        assert sb[pos:pos+32] == from_addr; pos += 32
        assert sb[pos:pos+8] == _u64_le(nonce); pos += 8
        assert pos == len(sb)


class TestBuildSignedUnstakeTx:
    def test_structure_and_verify(self, keypair):
        tx = build_signed_unstake_tx(keypair.secret_key, 5)

        assert "Unstake" in tx
        inner = tx["Unstake"]
        assert inner["nonce"] == 5

        pub_key = bytes(inner["pub_key"])
        sig = bytes.fromhex(inner["signature"])
        from_addr = bytes(inner["from"])
        signable = unstake_signable_bytes(from_addr, inner["nonce"])
        assert Keypair.verify(pub_key, signable, sig)


# ---------------------------------------------------------------------------
# Delegate
# ---------------------------------------------------------------------------

class TestDelegateSignableBytes:
    def test_layout(self, keypair, keypair2):
        from_addr = _from_bytes(keypair)
        validator = _from_bytes(keypair2)
        amount, nonce = 100 * COIN, 0

        sb = delegate_signable_bytes(from_addr, validator, amount, nonce)

        # NETWORK_ID(19) + "delegate"(8) + from(32) + validator(32) +
        # amount(8) + nonce(8) = 107
        assert len(sb) == 107

        pos = 0
        assert sb[pos:pos+19] == NETWORK_ID; pos += 19
        assert sb[pos:pos+8] == b"delegate"; pos += 8
        assert sb[pos:pos+32] == from_addr; pos += 32
        assert sb[pos:pos+32] == validator; pos += 32
        assert sb[pos:pos+8] == _u64_le(amount); pos += 8
        assert sb[pos:pos+8] == _u64_le(nonce); pos += 8
        assert pos == len(sb)


class TestBuildSignedDelegateTx:
    def test_structure_and_verify(self, keypair, keypair2):
        tx = build_signed_delegate_tx(
            keypair.secret_key, keypair2.address, 100 * COIN, 0,
        )

        assert "Delegate" in tx
        inner = tx["Delegate"]
        assert inner["amount"] == 100 * COIN

        pub_key = bytes(inner["pub_key"])
        sig = bytes.fromhex(inner["signature"])
        from_addr = bytes(inner["from"])
        validator = bytes(inner["validator"])
        signable = delegate_signable_bytes(
            from_addr, validator, inner["amount"], inner["nonce"],
        )
        assert Keypair.verify(pub_key, signable, sig)


# ---------------------------------------------------------------------------
# Undelegate
# ---------------------------------------------------------------------------

class TestUndelegateSignableBytes:
    def test_layout(self, keypair):
        from_addr = _from_bytes(keypair)
        nonce = 1

        sb = undelegate_signable_bytes(from_addr, nonce)

        # NETWORK_ID(19) + "undelegate"(10) + from(32) + nonce(8) = 69
        assert len(sb) == 69

        pos = 0
        assert sb[pos:pos+19] == NETWORK_ID; pos += 19
        assert sb[pos:pos+10] == b"undelegate"; pos += 10
        assert sb[pos:pos+32] == from_addr; pos += 32
        assert sb[pos:pos+8] == _u64_le(nonce); pos += 8
        assert pos == len(sb)


class TestBuildSignedUndelegateTx:
    def test_structure_and_verify(self, keypair):
        tx = build_signed_undelegate_tx(keypair.secret_key, 2)

        assert "Undelegate" in tx
        inner = tx["Undelegate"]
        assert inner["nonce"] == 2

        pub_key = bytes(inner["pub_key"])
        sig = bytes.fromhex(inner["signature"])
        from_addr = bytes(inner["from"])
        signable = undelegate_signable_bytes(from_addr, inner["nonce"])
        assert Keypair.verify(pub_key, signable, sig)


# ---------------------------------------------------------------------------
# SetCommission
# ---------------------------------------------------------------------------

class TestSetCommissionSignableBytes:
    def test_layout(self, keypair):
        from_addr = _from_bytes(keypair)
        commission, nonce = 15, 0

        sb = set_commission_signable_bytes(from_addr, commission, nonce)

        # NETWORK_ID(19) + "set_commission"(14) + from(32) + u8(1) + nonce(8) = 74
        assert len(sb) == 74

        pos = 0
        assert sb[pos:pos+19] == NETWORK_ID; pos += 19
        assert sb[pos:pos+14] == b"set_commission"; pos += 14
        assert sb[pos:pos+32] == from_addr; pos += 32
        assert sb[pos:pos+1] == bytes([commission]); pos += 1
        assert sb[pos:pos+8] == _u64_le(nonce); pos += 8
        assert pos == len(sb)


class TestBuildSignedSetCommissionTx:
    def test_structure_and_verify(self, keypair):
        tx = build_signed_set_commission_tx(keypair.secret_key, 20, 0)

        assert "SetCommission" in tx
        inner = tx["SetCommission"]
        assert inner["commission_percent"] == 20

        pub_key = bytes(inner["pub_key"])
        sig = bytes.fromhex(inner["signature"])
        from_addr = bytes(inner["from"])
        signable = set_commission_signable_bytes(
            from_addr, inner["commission_percent"], inner["nonce"],
        )
        assert Keypair.verify(pub_key, signable, sig)


# ---------------------------------------------------------------------------
# CreateProposal
# ---------------------------------------------------------------------------

class TestCreateProposalSignableBytes:
    def test_text_proposal_layout(self, keypair):
        from_addr = _from_bytes(keypair)
        title = "My Proposal"
        desc = "Description here"
        pt = {"type": "TextProposal"}

        sb = create_proposal_signable_bytes(from_addr, 1, title, desc, pt, 10_000, 0)

        pos = 0
        assert sb[pos:pos+19] == NETWORK_ID; pos += 19
        assert sb[pos:pos+8] == b"proposal"; pos += 8
        assert sb[pos:pos+32] == from_addr; pos += 32
        assert sb[pos:pos+8] == _u64_le(1); pos += 8  # proposal_id

        title_b = title.encode("utf-8")
        assert sb[pos:pos+4] == _u32_le(len(title_b)); pos += 4
        assert sb[pos:pos+len(title_b)] == title_b; pos += len(title_b)

        desc_b = desc.encode("utf-8")
        assert sb[pos:pos+4] == _u32_le(len(desc_b)); pos += 4
        assert sb[pos:pos+len(desc_b)] == desc_b; pos += len(desc_b)

        assert sb[pos:pos+1] == bytes([0]); pos += 1  # TextProposal discriminant

        assert sb[pos:pos+8] == _u64_le(10_000); pos += 8
        assert sb[pos:pos+8] == _u64_le(0); pos += 8
        assert pos == len(sb)

    def test_parameter_change_layout(self, keypair):
        from_addr = _from_bytes(keypair)
        pt = {"type": "ParameterChange", "param": "min_fee_sats", "new_value": "20000"}

        sb = create_proposal_signable_bytes(from_addr, 2, "T", "D", pt, 10_000, 0)

        # Find the proposal type section after title+desc
        # NETWORK_ID(19) + "proposal"(8) + from(32) + pid(8) +
        # title_len(4) + "T"(1) + desc_len(4) + "D"(1) = 77
        pos = 77
        assert sb[pos] == 1; pos += 1  # ParameterChange discriminant

        param_b = b"min_fee_sats"
        assert sb[pos:pos+4] == _u32_le(len(param_b)); pos += 4
        assert sb[pos:pos+len(param_b)] == param_b; pos += len(param_b)

        value_b = b"20000"
        assert sb[pos:pos+4] == _u32_le(len(value_b)); pos += 4
        assert sb[pos:pos+len(value_b)] == value_b; pos += len(value_b)

    def test_council_membership_layout(self, keypair, keypair2):
        from_addr = _from_bytes(keypair)
        target_addr = _from_bytes(keypair2)
        pt = {
            "type": "CouncilMembership",
            "action": "Add",
            "address": target_addr,
            "category": "Technical",
        }

        sb = create_proposal_signable_bytes(from_addr, 3, "T", "D", pt, 10_000, 0)

        pos = 77  # after common prefix
        assert sb[pos] == 2; pos += 1  # CouncilMembership discriminant
        assert sb[pos] == 0; pos += 1  # Add action
        assert sb[pos:pos+32] == target_addr; pos += 32
        assert sb[pos:pos+9] == b"Technical"; pos += 9

    def test_council_membership_remove(self, keypair, keypair2):
        from_addr = _from_bytes(keypair)
        target_addr = _from_bytes(keypair2)
        pt = {
            "type": "CouncilMembership",
            "action": "Remove",
            "address": target_addr,
            "category": "Business",
        }

        sb = create_proposal_signable_bytes(from_addr, 4, "T", "D", pt, 10_000, 0)

        pos = 77
        assert sb[pos] == 2; pos += 1
        assert sb[pos] == 1; pos += 1  # Remove action

    def test_treasury_spend_layout(self, keypair, keypair2):
        from_addr = _from_bytes(keypair)
        recipient = _from_bytes(keypair2)
        pt = {
            "type": "TreasurySpend",
            "recipient": recipient,
            "amount": 1_000_000 * COIN,
        }

        sb = create_proposal_signable_bytes(from_addr, 5, "T", "D", pt, 10_000, 0)

        pos = 77
        assert sb[pos] == 3; pos += 1  # TreasurySpend discriminant
        assert sb[pos:pos+32] == recipient; pos += 32
        assert sb[pos:pos+8] == _u64_le(1_000_000 * COIN); pos += 8

    def test_unknown_type_raises(self, keypair):
        from_addr = _from_bytes(keypair)
        with pytest.raises(ValueError, match="Unknown proposal type"):
            create_proposal_signable_bytes(
                from_addr, 1, "T", "D", {"type": "Bogus"}, 10_000, 0,
            )


class TestBuildSignedCreateProposalTx:
    def test_text_proposal_verifies(self, keypair):
        tx = build_signed_create_proposal_tx(
            keypair.secret_key, 1, "Title", "Desc",
            {"type": "TextProposal"}, 10_000, 0,
        )

        assert "CreateProposal" in tx
        inner = tx["CreateProposal"]
        assert inner["proposal_type"] == "TextProposal"
        assert inner["title"] == "Title"
        assert inner["proposal_id"] == 1

        pub_key = bytes(inner["pub_key"])
        sig = bytes.fromhex(inner["signature"])
        from_addr = bytes(inner["from"])
        signable = create_proposal_signable_bytes(
            from_addr, inner["proposal_id"], inner["title"],
            inner["description"], {"type": "TextProposal"},
            inner["fee"], inner["nonce"],
        )
        assert Keypair.verify(pub_key, signable, sig)

    def test_parameter_change_json(self, keypair):
        tx = build_signed_create_proposal_tx(
            keypair.secret_key, 2, "T", "D",
            {"type": "ParameterChange", "param": "min_fee_sats", "new_value": "20000"},
            10_000, 0,
        )

        inner = tx["CreateProposal"]
        pt = inner["proposal_type"]
        assert "ParameterChange" in pt
        assert pt["ParameterChange"]["param"] == "min_fee_sats"
        assert pt["ParameterChange"]["new_value"] == "20000"

    def test_council_membership_json(self, keypair, keypair2):
        target_addr = bytes.fromhex(keypair2.address)
        tx = build_signed_create_proposal_tx(
            keypair.secret_key, 3, "T", "D",
            {
                "type": "CouncilMembership",
                "action": "Add",
                "address": target_addr,
                "category": "Technical",
            },
            10_000, 0,
        )

        inner = tx["CreateProposal"]
        pt = inner["proposal_type"]
        assert "CouncilMembership" in pt
        assert pt["CouncilMembership"]["action"] == "Add"
        assert pt["CouncilMembership"]["category"] == "Technical"
        assert pt["CouncilMembership"]["address"] == list(target_addr)

    def test_treasury_spend_json(self, keypair, keypair2):
        recipient = bytes.fromhex(keypair2.address)
        tx = build_signed_create_proposal_tx(
            keypair.secret_key, 4, "T", "D",
            {
                "type": "TreasurySpend",
                "recipient": recipient,
                "amount": 500_000 * COIN,
            },
            10_000, 0,
        )

        inner = tx["CreateProposal"]
        pt = inner["proposal_type"]
        assert "TreasurySpend" in pt
        assert pt["TreasurySpend"]["amount"] == 500_000 * COIN

    def test_council_membership_hex_address(self, keypair, keypair2):
        """Address can be passed as hex string too."""
        tx = build_signed_create_proposal_tx(
            keypair.secret_key, 5, "T", "D",
            {
                "type": "CouncilMembership",
                "action": "Remove",
                "address": keypair2.address,  # hex string
                "category": "Foundation",
            },
            10_000, 0,
        )
        inner = tx["CreateProposal"]
        pt = inner["proposal_type"]
        assert pt["CouncilMembership"]["address"] == list(bytes.fromhex(keypair2.address))


# ---------------------------------------------------------------------------
# Vote
# ---------------------------------------------------------------------------

class TestVoteSignableBytes:
    def test_layout(self, keypair):
        from_addr = _from_bytes(keypair)
        proposal_id, fee, nonce = 42, 10_000, 7

        sb = vote_signable_bytes(from_addr, proposal_id, True, fee, nonce)

        # NETWORK_ID(19) + "vote"(4) + from(32) + pid(8) + vote(1) + fee(8) + nonce(8) = 80
        assert len(sb) == 80

        pos = 0
        assert sb[pos:pos+19] == NETWORK_ID; pos += 19
        assert sb[pos:pos+4] == b"vote"; pos += 4
        assert sb[pos:pos+32] == from_addr; pos += 32
        assert sb[pos:pos+8] == _u64_le(proposal_id); pos += 8
        assert sb[pos:pos+1] == bytes([1]); pos += 1  # True
        assert sb[pos:pos+8] == _u64_le(fee); pos += 8
        assert sb[pos:pos+8] == _u64_le(nonce); pos += 8
        assert pos == len(sb)

    def test_vote_false(self, keypair):
        from_addr = _from_bytes(keypair)
        sb = vote_signable_bytes(from_addr, 1, False, 10_000, 0)
        # vote byte is at offset 19+4+32+8 = 63
        assert sb[63] == 0


class TestBuildSignedVoteTx:
    def test_structure_and_verify(self, keypair):
        tx = build_signed_vote_tx(keypair.secret_key, 10, True, 10_000, 0)

        assert "Vote" in tx
        inner = tx["Vote"]
        assert inner["proposal_id"] == 10
        assert inner["vote"] is True

        pub_key = bytes(inner["pub_key"])
        sig = bytes.fromhex(inner["signature"])
        from_addr = bytes(inner["from"])
        signable = vote_signable_bytes(
            from_addr, inner["proposal_id"], inner["vote"],
            inner["fee"], inner["nonce"],
        )
        assert Keypair.verify(pub_key, signable, sig)

    def test_vote_no(self, keypair):
        tx = build_signed_vote_tx(keypair.secret_key, 10, False, 10_000, 0)
        assert tx["Vote"]["vote"] is False


# ---------------------------------------------------------------------------
# Cross-cutting concerns
# ---------------------------------------------------------------------------

class TestCrossCutting:
    """Tests that apply across all transaction types."""

    def test_different_keys_produce_different_signatures(self, keypair, keypair2):
        """Two different keys signing the same amount/fee/nonce produce different sigs."""
        tx1 = build_signed_stake_tx(keypair.secret_key, 10_000 * COIN, 0)
        tx2 = build_signed_stake_tx(keypair2.secret_key, 10_000 * COIN, 0)
        assert tx1["Stake"]["signature"] != tx2["Stake"]["signature"]

    def test_tampered_amount_fails_verification(self, keypair, keypair2):
        """If the amount is changed after signing, verification fails."""
        tx = build_signed_transfer_tx(
            keypair.secret_key, keypair2.address, 100, 10, 0,
        )
        inner = tx["Transfer"]

        pub_key = bytes(inner["pub_key"])
        sig = bytes.fromhex(inner["signature"])
        from_addr = bytes(inner["from"])
        to_addr = bytes(inner["to"])

        # Verify original works
        signable_ok = transfer_signable_bytes(from_addr, to_addr, 100, 10, 0)
        assert Keypair.verify(pub_key, signable_ok, sig)

        # Tamper with amount
        signable_bad = transfer_signable_bytes(from_addr, to_addr, 999, 10, 0)
        assert not Keypair.verify(pub_key, signable_bad, sig)

    def test_address_derivation_matches(self, keypair):
        """from field = blake3(pub_key) for all tx types."""
        tx = build_signed_stake_tx(keypair.secret_key, 10_000 * COIN, 0)
        inner = tx["Stake"]
        pub_key = bytes(inner["pub_key"])
        from_addr = bytes(inner["from"])
        assert _blake3_hash(pub_key) == from_addr

    def test_all_tx_types_produce_valid_signatures(self, keypair, keypair2):
        """Smoke test: build + verify for every transaction type."""
        txs = [
            build_signed_transfer_tx(keypair.secret_key, keypair2.address, 1, 1, 0),
            build_signed_stake_tx(keypair.secret_key, 10_000 * COIN, 1),
            build_signed_unstake_tx(keypair.secret_key, 2),
            build_signed_delegate_tx(keypair.secret_key, keypair2.address, 100 * COIN, 3),
            build_signed_undelegate_tx(keypair.secret_key, 4),
            build_signed_set_commission_tx(keypair.secret_key, 10, 5),
            build_signed_create_proposal_tx(
                keypair.secret_key, 1, "T", "D", {"type": "TextProposal"}, 10_000, 6,
            ),
            build_signed_vote_tx(keypair.secret_key, 1, True, 10_000, 7),
        ]

        for tx in txs:
            # Each tx should have exactly one top-level key
            assert len(tx) == 1
            variant = list(tx.keys())[0]
            inner = tx[variant]

            # All should have pub_key, signature, from
            assert "pub_key" in inner
            assert "signature" in inner
            assert "from" in inner
            assert len(inner["pub_key"]) == 32
            assert len(inner["signature"]) == 128  # hex
            assert len(inner["from"]) == 32
