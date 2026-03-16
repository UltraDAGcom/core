"""Client-side transaction signing for UltraDAG.

Each function constructs the exact same byte sequence as the Rust
``signable_bytes()`` method for the corresponding transaction type.
These bytes are then signed with Ed25519 and the result is submitted
to the node via ``POST /tx/submit``.

IMPORTANT: The byte layouts here MUST match the Rust implementations
exactly, or signature verification will fail on the server.

Byte format reference (all integers are little-endian):
    - NETWORK_ID: b"ultradag-testnet-v1" (19 bytes)
    - u64: 8 bytes LE  (struct.pack('<Q', v))
    - u32: 4 bytes LE  (struct.pack('<I', v))
    - u8:  1 byte
    - Address: 32 raw bytes
    - Strings: UTF-8 encoded bytes
"""

import struct
from typing import Any, Dict, List, Optional, Union

from .crypto import Keypair, _blake3_hash


# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------

NETWORK_ID: bytes = b"ultradag-testnet-v1"

# Sats / UDAG conversion
COIN: int = 100_000_000
SATS_PER_UDAG: int = COIN


# ---------------------------------------------------------------------------
# Low-level helpers
# ---------------------------------------------------------------------------

def _u64_le(value: int) -> bytes:
    """Pack a u64 as 8 bytes little-endian."""
    return struct.pack("<Q", value)


def _u32_le(value: int) -> bytes:
    """Pack a u32 as 4 bytes little-endian."""
    return struct.pack("<I", value)


def _addr_bytes(hex_or_bytes: Union[str, bytes]) -> bytes:
    """Normalise an address to 32 raw bytes."""
    if isinstance(hex_or_bytes, str):
        b = bytes.fromhex(hex_or_bytes)
    else:
        b = hex_or_bytes
    if len(b) != 32:
        raise ValueError(f"Address must be 32 bytes, got {len(b)}")
    return b


# ---------------------------------------------------------------------------
# Signable-bytes builders  (one per tx type)
# ---------------------------------------------------------------------------

def transfer_signable_bytes(
    from_addr: bytes,
    to: bytes,
    amount: int,
    fee: int,
    nonce: int,
    memo: Optional[bytes] = None,
) -> bytes:
    """Build signable bytes for a Transfer transaction.

    Layout: NETWORK_ID | "transfer" | from(32) | to(32) | amount(u64 LE) |
            fee(u64 LE) | nonce(u64 LE) | [memo_len(u32 LE) | memo]
    """
    buf = bytearray()
    buf.extend(NETWORK_ID)
    buf.extend(b"transfer")
    buf.extend(from_addr)
    buf.extend(to)
    buf.extend(_u64_le(amount))
    buf.extend(_u64_le(fee))
    buf.extend(_u64_le(nonce))
    if memo is not None and len(memo) > 0:
        buf.extend(_u32_le(len(memo)))
        buf.extend(memo)
    return bytes(buf)


def stake_signable_bytes(
    from_addr: bytes,
    amount: int,
    nonce: int,
) -> bytes:
    """Build signable bytes for a Stake transaction.

    Layout: NETWORK_ID | "stake" | from(32) | amount(u64 LE) | nonce(u64 LE)
    """
    buf = bytearray()
    buf.extend(NETWORK_ID)
    buf.extend(b"stake")
    buf.extend(from_addr)
    buf.extend(_u64_le(amount))
    buf.extend(_u64_le(nonce))
    return bytes(buf)


def unstake_signable_bytes(
    from_addr: bytes,
    nonce: int,
) -> bytes:
    """Build signable bytes for an Unstake transaction.

    Layout: NETWORK_ID | "unstake" | from(32) | nonce(u64 LE)
    """
    buf = bytearray()
    buf.extend(NETWORK_ID)
    buf.extend(b"unstake")
    buf.extend(from_addr)
    buf.extend(_u64_le(nonce))
    return bytes(buf)


def delegate_signable_bytes(
    from_addr: bytes,
    validator: bytes,
    amount: int,
    nonce: int,
) -> bytes:
    """Build signable bytes for a Delegate transaction.

    Layout: NETWORK_ID | "delegate" | from(32) | validator(32) |
            amount(u64 LE) | nonce(u64 LE)
    """
    buf = bytearray()
    buf.extend(NETWORK_ID)
    buf.extend(b"delegate")
    buf.extend(from_addr)
    buf.extend(validator)
    buf.extend(_u64_le(amount))
    buf.extend(_u64_le(nonce))
    return bytes(buf)


def undelegate_signable_bytes(
    from_addr: bytes,
    nonce: int,
) -> bytes:
    """Build signable bytes for an Undelegate transaction.

    Layout: NETWORK_ID | "undelegate" | from(32) | nonce(u64 LE)
    """
    buf = bytearray()
    buf.extend(NETWORK_ID)
    buf.extend(b"undelegate")
    buf.extend(from_addr)
    buf.extend(_u64_le(nonce))
    return bytes(buf)


def set_commission_signable_bytes(
    from_addr: bytes,
    commission_percent: int,
    nonce: int,
) -> bytes:
    """Build signable bytes for a SetCommission transaction.

    Layout: NETWORK_ID | "set_commission" | from(32) |
            commission_percent(u8) | nonce(u64 LE)
    """
    buf = bytearray()
    buf.extend(NETWORK_ID)
    buf.extend(b"set_commission")
    buf.extend(from_addr)
    buf.append(commission_percent & 0xFF)
    buf.extend(_u64_le(nonce))
    return bytes(buf)


def create_proposal_signable_bytes(
    from_addr: bytes,
    proposal_id: int,
    title: str,
    description: str,
    proposal_type: Dict[str, Any],
    fee: int,
    nonce: int,
) -> bytes:
    """Build signable bytes for a CreateProposal transaction.

    Layout: NETWORK_ID | "proposal" | from(32) | proposal_id(u64 LE) |
            title_len(u32 LE) | title | desc_len(u32 LE) | desc |
            proposal_type_bytes | fee(u64 LE) | nonce(u64 LE)

    ``proposal_type`` must be one of:
        - ``{"type": "TextProposal"}``
        - ``{"type": "ParameterChange", "param": str, "new_value": str}``
        - ``{"type": "CouncilMembership", "action": "Add"|"Remove",
             "address": bytes|str, "category": str}``
        - ``{"type": "TreasurySpend", "recipient": bytes|str, "amount": int}``
    """
    title_bytes = title.encode("utf-8")
    desc_bytes = description.encode("utf-8")

    buf = bytearray()
    buf.extend(NETWORK_ID)
    buf.extend(b"proposal")
    buf.extend(from_addr)
    buf.extend(_u64_le(proposal_id))
    buf.extend(_u32_le(len(title_bytes)))
    buf.extend(title_bytes)
    buf.extend(_u32_le(len(desc_bytes)))
    buf.extend(desc_bytes)

    pt = proposal_type["type"]
    if pt == "TextProposal":
        buf.append(0)
    elif pt == "ParameterChange":
        buf.append(1)
        param_bytes = proposal_type["param"].encode("utf-8")
        value_bytes = proposal_type["new_value"].encode("utf-8")
        buf.extend(_u32_le(len(param_bytes)))
        buf.extend(param_bytes)
        buf.extend(_u32_le(len(value_bytes)))
        buf.extend(value_bytes)
    elif pt == "CouncilMembership":
        buf.append(2)
        action = proposal_type["action"]
        buf.append(0 if action == "Add" else 1)
        buf.extend(_addr_bytes(proposal_type["address"]))
        buf.extend(proposal_type["category"].encode("utf-8"))
    elif pt == "TreasurySpend":
        buf.append(3)
        buf.extend(_addr_bytes(proposal_type["recipient"]))
        buf.extend(_u64_le(proposal_type["amount"]))
    else:
        raise ValueError(f"Unknown proposal type: {pt}")

    buf.extend(_u64_le(fee))
    buf.extend(_u64_le(nonce))
    return bytes(buf)


def vote_signable_bytes(
    from_addr: bytes,
    proposal_id: int,
    approve: bool,
    fee: int,
    nonce: int,
) -> bytes:
    """Build signable bytes for a Vote transaction.

    Layout: NETWORK_ID | "vote" | from(32) | proposal_id(u64 LE) |
            vote(1 byte: 1=yes, 0=no) | fee(u64 LE) | nonce(u64 LE)
    """
    buf = bytearray()
    buf.extend(NETWORK_ID)
    buf.extend(b"vote")
    buf.extend(from_addr)
    buf.extend(_u64_le(proposal_id))
    buf.append(1 if approve else 0)
    buf.extend(_u64_le(fee))
    buf.extend(_u64_le(nonce))
    return bytes(buf)


# ---------------------------------------------------------------------------
# High-level builders — return dicts ready for POST /tx/submit
# ---------------------------------------------------------------------------

def build_signed_transfer_tx(
    secret_key_hex: str,
    to_hex: str,
    amount: int,
    fee: int,
    nonce: int,
    memo: Optional[bytes] = None,
) -> Dict[str, Any]:
    """Build a signed Transfer transaction ready for ``POST /tx/submit``.

    Args:
        secret_key_hex: 64-char hex Ed25519 secret key.
        to_hex: 64-char hex recipient address.
        amount: Amount in sats.
        fee: Fee in sats.
        nonce: Sender nonce.
        memo: Optional memo bytes (max 256 bytes).

    Returns:
        JSON-serializable dict matching Rust ``Transaction::Transfer`` serde format.
    """
    kp = Keypair.from_hex(secret_key_hex)
    from_bytes = bytes.fromhex(kp.address)
    to_bytes = _addr_bytes(to_hex)

    signable = transfer_signable_bytes(from_bytes, to_bytes, amount, fee, nonce, memo)
    signature = kp.sign(signable)

    return {
        "Transfer": {
            "from": list(from_bytes),
            "to": list(to_bytes),
            "amount": amount,
            "fee": fee,
            "nonce": nonce,
            "pub_key": list(bytes.fromhex(kp.public_key)),
            "signature": signature.hex(),
            "memo": list(memo) if memo else None,
        }
    }


def build_signed_stake_tx(
    secret_key_hex: str,
    amount: int,
    nonce: int,
) -> Dict[str, Any]:
    """Build a signed Stake transaction ready for ``POST /tx/submit``."""
    kp = Keypair.from_hex(secret_key_hex)
    from_bytes = bytes.fromhex(kp.address)

    signable = stake_signable_bytes(from_bytes, amount, nonce)
    signature = kp.sign(signable)

    return {
        "Stake": {
            "from": list(from_bytes),
            "amount": amount,
            "nonce": nonce,
            "pub_key": list(bytes.fromhex(kp.public_key)),
            "signature": signature.hex(),
        }
    }


def build_signed_unstake_tx(
    secret_key_hex: str,
    nonce: int,
) -> Dict[str, Any]:
    """Build a signed Unstake transaction ready for ``POST /tx/submit``."""
    kp = Keypair.from_hex(secret_key_hex)
    from_bytes = bytes.fromhex(kp.address)

    signable = unstake_signable_bytes(from_bytes, nonce)
    signature = kp.sign(signable)

    return {
        "Unstake": {
            "from": list(from_bytes),
            "nonce": nonce,
            "pub_key": list(bytes.fromhex(kp.public_key)),
            "signature": signature.hex(),
        }
    }


def build_signed_delegate_tx(
    secret_key_hex: str,
    validator_hex: str,
    amount: int,
    nonce: int,
) -> Dict[str, Any]:
    """Build a signed Delegate transaction ready for ``POST /tx/submit``."""
    kp = Keypair.from_hex(secret_key_hex)
    from_bytes = bytes.fromhex(kp.address)
    validator_bytes = _addr_bytes(validator_hex)

    signable = delegate_signable_bytes(from_bytes, validator_bytes, amount, nonce)
    signature = kp.sign(signable)

    return {
        "Delegate": {
            "from": list(from_bytes),
            "validator": list(validator_bytes),
            "amount": amount,
            "nonce": nonce,
            "pub_key": list(bytes.fromhex(kp.public_key)),
            "signature": signature.hex(),
        }
    }


def build_signed_undelegate_tx(
    secret_key_hex: str,
    nonce: int,
) -> Dict[str, Any]:
    """Build a signed Undelegate transaction ready for ``POST /tx/submit``."""
    kp = Keypair.from_hex(secret_key_hex)
    from_bytes = bytes.fromhex(kp.address)

    signable = undelegate_signable_bytes(from_bytes, nonce)
    signature = kp.sign(signable)

    return {
        "Undelegate": {
            "from": list(from_bytes),
            "nonce": nonce,
            "pub_key": list(bytes.fromhex(kp.public_key)),
            "signature": signature.hex(),
        }
    }


def build_signed_set_commission_tx(
    secret_key_hex: str,
    commission_percent: int,
    nonce: int,
) -> Dict[str, Any]:
    """Build a signed SetCommission transaction ready for ``POST /tx/submit``."""
    kp = Keypair.from_hex(secret_key_hex)
    from_bytes = bytes.fromhex(kp.address)

    signable = set_commission_signable_bytes(from_bytes, commission_percent, nonce)
    signature = kp.sign(signable)

    return {
        "SetCommission": {
            "from": list(from_bytes),
            "commission_percent": commission_percent,
            "nonce": nonce,
            "pub_key": list(bytes.fromhex(kp.public_key)),
            "signature": signature.hex(),
        }
    }


def build_signed_create_proposal_tx(
    secret_key_hex: str,
    proposal_id: int,
    title: str,
    description: str,
    proposal_type: Dict[str, Any],
    fee: int,
    nonce: int,
) -> Dict[str, Any]:
    """Build a signed CreateProposal transaction ready for ``POST /tx/submit``.

    See :func:`create_proposal_signable_bytes` for ``proposal_type`` format.
    """
    kp = Keypair.from_hex(secret_key_hex)
    from_bytes = bytes.fromhex(kp.address)

    signable = create_proposal_signable_bytes(
        from_bytes, proposal_id, title, description, proposal_type, fee, nonce,
    )
    signature = kp.sign(signable)

    # Build serde-compatible proposal_type JSON
    pt = proposal_type["type"]
    if pt == "TextProposal":
        pt_json: Any = "TextProposal"
    elif pt == "ParameterChange":
        pt_json = {
            "ParameterChange": {
                "param": proposal_type["param"],
                "new_value": proposal_type["new_value"],
            }
        }
    elif pt == "CouncilMembership":
        addr = proposal_type["address"]
        if isinstance(addr, str):
            addr_list = list(bytes.fromhex(addr))
        else:
            addr_list = list(addr)
        pt_json = {
            "CouncilMembership": {
                "action": proposal_type["action"],
                "address": addr_list,
                "category": proposal_type["category"],
            }
        }
    elif pt == "TreasurySpend":
        recip = proposal_type["recipient"]
        if isinstance(recip, str):
            recip_list = list(bytes.fromhex(recip))
        else:
            recip_list = list(recip)
        pt_json = {
            "TreasurySpend": {
                "recipient": recip_list,
                "amount": proposal_type["amount"],
            }
        }
    else:
        raise ValueError(f"Unknown proposal type: {pt}")

    return {
        "CreateProposal": {
            "from": list(from_bytes),
            "proposal_id": proposal_id,
            "title": title,
            "description": description,
            "proposal_type": pt_json,
            "fee": fee,
            "nonce": nonce,
            "pub_key": list(bytes.fromhex(kp.public_key)),
            "signature": signature.hex(),
        }
    }


def build_signed_vote_tx(
    secret_key_hex: str,
    proposal_id: int,
    approve: bool,
    fee: int,
    nonce: int,
) -> Dict[str, Any]:
    """Build a signed Vote transaction ready for ``POST /tx/submit``."""
    kp = Keypair.from_hex(secret_key_hex)
    from_bytes = bytes.fromhex(kp.address)

    signable = vote_signable_bytes(from_bytes, proposal_id, approve, fee, nonce)
    signature = kp.sign(signable)

    return {
        "Vote": {
            "from": list(from_bytes),
            "proposal_id": proposal_id,
            "vote": approve,
            "fee": fee,
            "nonce": nonce,
            "pub_key": list(bytes.fromhex(kp.public_key)),
            "signature": signature.hex(),
        }
    }
