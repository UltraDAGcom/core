"""Data types for UltraDAG RPC responses."""

from dataclasses import dataclass, field
from typing import List, Optional, Any, Dict


SATS_PER_UDAG = 100_000_000


def sats_to_udag(sats: int) -> float:
    """Convert satoshi amount to UDAG."""
    return sats / SATS_PER_UDAG


def udag_to_sats(udag: float) -> int:
    """Convert UDAG amount to satoshi."""
    return round(udag * SATS_PER_UDAG)


class UltraDagError(Exception):
    """Base exception for UltraDAG SDK errors."""

    def __init__(self, message: str, status_code: Optional[int] = None, response_body: Optional[str] = None):
        super().__init__(message)
        self.status_code = status_code
        self.response_body = response_body


class ApiError(UltraDagError):
    """Raised when the RPC API returns an error response."""
    pass


class ConnectionError(UltraDagError):
    """Raised when the node is unreachable."""
    pass


@dataclass
class HealthResponse:
    """Response from GET /health."""
    status: str


@dataclass
class StatusResponse:
    """Response from GET /status."""
    last_finalized_round: int
    peer_count: int
    mempool_size: int
    total_supply: int
    account_count: int
    dag_vertices: int
    dag_round: int
    dag_tips: int
    finalized_count: int
    validator_count: int
    total_staked: int
    active_stakers: int
    bootstrap_connected: Optional[bool] = None


@dataclass
class BalanceResponse:
    """Response from GET /balance/{address}."""
    address: str
    balance: int
    nonce: int
    balance_tdag: Optional[float] = None

    @property
    def balance_udag(self) -> float:
        """Balance in UDAG."""
        return sats_to_udag(self.balance)


@dataclass
class RoundVertex:
    """A single vertex in a round."""
    round: int
    hash: str
    validator: str
    reward: int
    tx_count: int
    parent_count: int


@dataclass
class PeerInfo:
    """Information about a connected peer."""
    address: str
    extra: Dict[str, Any] = field(default_factory=dict)


@dataclass
class PeersResponse:
    """Response from GET /peers."""
    connected: int
    peers: List[Dict[str, Any]]
    bootstrap_nodes: List[Dict[str, Any]]


@dataclass
class KeygenResponse:
    """Response from GET /keygen (server-side key generation)."""
    secret_key: str
    address: str


@dataclass
class ValidatorInfo:
    """Information about a single validator."""
    address: str
    staked: int
    staked_udag: Optional[float] = None

    @property
    def staked_in_udag(self) -> float:
        """Staked amount in UDAG."""
        return sats_to_udag(self.staked)


@dataclass
class ValidatorsResponse:
    """Response from GET /validators."""
    count: int
    total_staked: int
    validators: List[ValidatorInfo]


@dataclass
class StakeInfoResponse:
    """Response from GET /stake/{address}."""
    address: str
    staked: int
    staked_udag: Optional[float] = None
    unlock_at_round: Optional[int] = None
    is_active_validator: bool = False


@dataclass
class TransactionResponse:
    """Response from POST /tx."""
    hash: str
    from_address: str
    to: str
    amount: int
    fee: int
    nonce: int


@dataclass
class FaucetResponse:
    """Response from POST /faucet."""
    tx_hash: str
    from_address: str
    to: str
    amount: int
    nonce: int
    amount_udag: Optional[float] = None


@dataclass
class StakeResponse:
    """Response from POST /stake."""
    status: str
    tx_hash: str
    address: str
    amount: int
    nonce: int
    amount_udag: Optional[float] = None
    note: Optional[str] = None


@dataclass
class UnstakeResponse:
    """Response from POST /unstake."""
    status: str
    tx_hash: str
    address: str
    unlock_at_round: int
    nonce: int
    note: Optional[str] = None


@dataclass
class ProposalInfo:
    """Information about a governance proposal."""
    id: int
    proposer: str
    title: str
    description: str
    proposal_type: str
    parameter_name: Optional[str] = None
    parameter_value: Optional[str] = None
    status: Optional[str] = None
    extra: Dict[str, Any] = field(default_factory=dict)


@dataclass
class ProposalsResponse:
    """Response from GET /proposals."""
    count: int
    proposals: List[Dict[str, Any]]


@dataclass
class ProposalSubmitResponse:
    """Response from POST /proposal."""
    status: str
    tx_hash: str
    proposal_id: int
    proposer: str
    title: str
    note: Optional[str] = None


@dataclass
class VoteInfo:
    """Response from GET /vote/{proposal_id}/{address}."""
    proposal_id: int
    address: str
    vote: Optional[str] = None
    extra: Dict[str, Any] = field(default_factory=dict)


@dataclass
class VoteResponse:
    """Response from POST /vote."""
    status: str
    tx_hash: str
    proposal_id: int
    voter: str
    vote: str
    note: Optional[str] = None


@dataclass
class MempoolTransaction:
    """A transaction in the mempool."""
    hash: Optional[str] = None
    from_address: Optional[str] = None
    to: Optional[str] = None
    amount: Optional[int] = None
    fee: Optional[int] = None
    nonce: Optional[int] = None
    extra: Dict[str, Any] = field(default_factory=dict)
