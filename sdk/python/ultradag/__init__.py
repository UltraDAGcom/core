"""UltraDAG Python SDK.

Provides a client for the UltraDAG node HTTP RPC API, local
Ed25519 cryptography for offline key generation and signing,
and client-side transaction construction for mainnet compatibility.

Example::

    from ultradag import UltraDagClient, Keypair
    from ultradag.transactions import build_signed_transfer_tx

    # Connect to a node
    client = UltraDagClient("http://localhost:10333")
    status = client.get_status()

    # Local key generation (no network required)
    kp = Keypair.generate()
    print(f"Address: {kp.address}")

    # Client-side signed transaction (mainnet-compatible)
    tx = build_signed_transfer_tx(kp.secret_key, recipient, 100_000_000, 10_000, 0)
    client.submit_signed_transaction(tx)
"""

from .client import UltraDagClient
from .crypto import Keypair, derive_address
from .transactions import (
    NETWORK_ID,
    COIN,
    SATS_PER_UDAG,
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
)
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
    # Client
    "UltraDagClient",
    # Crypto
    "Keypair",
    "derive_address",
    # Transaction constants
    "NETWORK_ID",
    "COIN",
    "SATS_PER_UDAG",
    # Signable bytes builders
    "transfer_signable_bytes",
    "stake_signable_bytes",
    "unstake_signable_bytes",
    "delegate_signable_bytes",
    "undelegate_signable_bytes",
    "set_commission_signable_bytes",
    "create_proposal_signable_bytes",
    "vote_signable_bytes",
    # Signed transaction builders
    "build_signed_transfer_tx",
    "build_signed_stake_tx",
    "build_signed_unstake_tx",
    "build_signed_delegate_tx",
    "build_signed_undelegate_tx",
    "build_signed_set_commission_tx",
    "build_signed_create_proposal_tx",
    "build_signed_vote_tx",
    # Error types
    "UltraDagError",
    "ApiError",
    "ConnectionError",
    # Response types
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
