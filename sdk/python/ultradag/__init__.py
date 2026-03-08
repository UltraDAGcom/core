"""UltraDAG Python SDK.

Provides a client for the UltraDAG node HTTP RPC API and local
Ed25519 cryptography for offline key generation and signing.

Example::

    from ultradag import UltraDagClient, Keypair

    # Connect to a node
    client = UltraDagClient("http://localhost:10333")
    status = client.get_status()

    # Local key generation (no network required)
    kp = Keypair.generate()
    print(f"Address: {kp.address}")
"""

from .client import UltraDagClient
from .crypto import Keypair, derive_address
from .types import (
    UltraDagError,
    ApiError,
    ConnectionError,
    HealthResponse,
    StatusResponse,
    BalanceResponse,
    RoundVertex,
    PeersResponse,
    KeygenResponse,
    ValidatorInfo,
    ValidatorsResponse,
    StakeInfoResponse,
    TransactionResponse,
    FaucetResponse,
    StakeResponse,
    UnstakeResponse,
    ProposalsResponse,
    ProposalSubmitResponse,
    VoteInfo,
    VoteResponse,
    MempoolTransaction,
    sats_to_udag,
    udag_to_sats,
)

__version__ = "0.1.0"

__all__ = [
    "UltraDagClient",
    "Keypair",
    "derive_address",
    "UltraDagError",
    "ApiError",
    "ConnectionError",
    "HealthResponse",
    "StatusResponse",
    "BalanceResponse",
    "RoundVertex",
    "PeersResponse",
    "KeygenResponse",
    "ValidatorInfo",
    "ValidatorsResponse",
    "StakeInfoResponse",
    "TransactionResponse",
    "FaucetResponse",
    "StakeResponse",
    "UnstakeResponse",
    "ProposalsResponse",
    "ProposalSubmitResponse",
    "VoteInfo",
    "VoteResponse",
    "MempoolTransaction",
    "sats_to_udag",
    "udag_to_sats",
]
