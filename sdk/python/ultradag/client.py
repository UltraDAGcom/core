"""UltraDAG RPC client.

Wraps the HTTP JSON-RPC interface exposed by ultradag-node.
Default base URL: http://localhost:10333
"""

from typing import List, Optional, Dict, Any

import requests

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


class UltraDagClient:
    """Client for the UltraDAG node HTTP RPC API.

    Args:
        base_url: Base URL of the node RPC server (default: http://localhost:10333).
        timeout: Request timeout in seconds (default: 30).
        session: Optional requests.Session for connection pooling / custom config.

    Example::

        client = UltraDagClient("http://localhost:10333")
        status = client.get_status()
        print(f"Round: {status.dag_round}, Validators: {status.validator_count}")
    """

    def __init__(
        self,
        base_url: str = "http://localhost:10333",
        timeout: float = 30.0,
        session: Optional[requests.Session] = None,
    ):
        self.base_url = base_url.rstrip("/")
        self.timeout = timeout
        self._session = session or requests.Session()

    # ------------------------------------------------------------------
    # Internal helpers
    # ------------------------------------------------------------------

    def _get(self, path: str) -> Any:
        """Perform a GET request and return parsed JSON."""
        url = f"{self.base_url}{path}"
        try:
            resp = self._session.get(url, timeout=self.timeout)
        except requests.exceptions.ConnectionError as exc:
            raise ConnectionError(
                f"Failed to connect to {url}: {exc}"
            ) from exc
        except requests.exceptions.Timeout as exc:
            raise ConnectionError(
                f"Request to {url} timed out after {self.timeout}s"
            ) from exc
        except requests.exceptions.RequestException as exc:
            raise UltraDagError(f"Request failed: {exc}") from exc

        if resp.status_code >= 400:
            raise ApiError(
                f"API error {resp.status_code}: {resp.text}",
                status_code=resp.status_code,
                response_body=resp.text,
            )
        return resp.json()

    def _post(self, path: str, body: Dict[str, Any]) -> Any:
        """Perform a POST request with JSON body and return parsed JSON."""
        url = f"{self.base_url}{path}"
        try:
            resp = self._session.post(url, json=body, timeout=self.timeout)
        except requests.exceptions.ConnectionError as exc:
            raise ConnectionError(
                f"Failed to connect to {url}: {exc}"
            ) from exc
        except requests.exceptions.Timeout as exc:
            raise ConnectionError(
                f"Request to {url} timed out after {self.timeout}s"
            ) from exc
        except requests.exceptions.RequestException as exc:
            raise UltraDagError(f"Request failed: {exc}") from exc

        if resp.status_code >= 400:
            raise ApiError(
                f"API error {resp.status_code}: {resp.text}",
                status_code=resp.status_code,
                response_body=resp.text,
            )
        return resp.json()

    # ------------------------------------------------------------------
    # GET endpoints
    # ------------------------------------------------------------------

    def health(self) -> HealthResponse:
        """Check node health.

        Returns:
            HealthResponse with status field.
        """
        data = self._get("/health")
        return HealthResponse(status=data.get("status", "ok"))

    def get_status(self) -> StatusResponse:
        """Get node status including DAG state, supply, and validator info.

        Returns:
            StatusResponse with all status fields.
        """
        data = self._get("/status")
        return StatusResponse(
            last_finalized_round=data.get("last_finalized_round", 0),
            peer_count=data.get("peer_count", 0),
            mempool_size=data.get("mempool_size", 0),
            total_supply=data.get("total_supply", 0),
            account_count=data.get("account_count", 0),
            dag_vertices=data.get("dag_vertices", 0),
            dag_round=data.get("dag_round", 0),
            dag_tips=data.get("dag_tips", 0),
            finalized_count=data.get("finalized_count", 0),
            validator_count=data.get("validator_count", 0),
            total_staked=data.get("total_staked", 0),
            active_stakers=data.get("active_stakers", 0),
            bootstrap_connected=data.get("bootstrap_connected"),
        )

    def get_balance(self, address: str) -> BalanceResponse:
        """Get balance and nonce for an address.

        Args:
            address: 64-character hex address string.

        Returns:
            BalanceResponse with balance in sats, nonce, and optional UDAG display.
        """
        data = self._get(f"/balance/{address}")
        return BalanceResponse(
            address=data.get("address", address),
            balance=data.get("balance", 0),
            nonce=data.get("nonce", 0),
            balance_tdag=data.get("balance_tdag"),
        )

    def get_round(self, round_number: int) -> List[RoundVertex]:
        """Get all vertices in a specific DAG round.

        Args:
            round_number: The round number to query.

        Returns:
            List of RoundVertex objects for the round.
        """
        data = self._get(f"/round/{round_number}")
        if not isinstance(data, list):
            data = data.get("vertices", []) if isinstance(data, dict) else []
        return [
            RoundVertex(
                round=v.get("round", round_number),
                hash=v.get("hash", ""),
                validator=v.get("validator", ""),
                reward=v.get("reward", 0),
                tx_count=v.get("tx_count", 0),
                parent_count=v.get("parent_count", 0),
            )
            for v in data
        ]

    def get_mempool(self) -> List[MempoolTransaction]:
        """Get pending transactions in the mempool (top 100 by fee).

        Returns:
            List of MempoolTransaction objects.
        """
        data = self._get("/mempool")
        if not isinstance(data, list):
            data = data.get("transactions", []) if isinstance(data, dict) else []
        return [
            MempoolTransaction(
                hash=tx.get("hash"),
                from_address=tx.get("from"),
                to=tx.get("to"),
                amount=tx.get("amount"),
                fee=tx.get("fee"),
                nonce=tx.get("nonce"),
                extra={k: v for k, v in tx.items() if k not in ("hash", "from", "to", "amount", "fee", "nonce")},
            )
            for tx in data
        ]

    def get_peers(self) -> PeersResponse:
        """Get connected peers and bootstrap node info.

        Returns:
            PeersResponse with peer list and bootstrap status.
        """
        data = self._get("/peers")
        return PeersResponse(
            connected=data.get("connected", 0),
            peers=data.get("peers", []),
            bootstrap_nodes=data.get("bootstrap_nodes", []),
        )

    def keygen(self) -> KeygenResponse:
        """Generate a new keypair on the server.

        Note: For local (offline) key generation, use ultradag.crypto.Keypair.generate()
        instead. Server-side keygen transmits the secret key over the network.

        Returns:
            KeygenResponse with secret_key and address.
        """
        data = self._get("/keygen")
        return KeygenResponse(
            secret_key=data["secret_key"],
            address=data["address"],
        )

    def get_validators(self) -> ValidatorsResponse:
        """Get the list of active validators and their stakes.

        Returns:
            ValidatorsResponse with count, total_staked, and validator list.
        """
        data = self._get("/validators")
        validators = [
            ValidatorInfo(
                address=v.get("address", ""),
                staked=v.get("staked", 0),
                staked_udag=v.get("staked_udag"),
            )
            for v in data.get("validators", [])
        ]
        return ValidatorsResponse(
            count=data.get("count", 0),
            total_staked=data.get("total_staked", 0),
            validators=validators,
        )

    def get_stake(self, address: str) -> StakeInfoResponse:
        """Get staking information for an address.

        Args:
            address: 64-character hex address string.

        Returns:
            StakeInfoResponse with stake amount and validator status.
        """
        data = self._get(f"/stake/{address}")
        return StakeInfoResponse(
            address=data.get("address", address),
            staked=data.get("staked", 0),
            staked_udag=data.get("staked_udag"),
            unlock_at_round=data.get("unlock_at_round"),
            is_active_validator=data.get("is_active_validator", False),
        )

    def get_governance_config(self) -> Dict[str, Any]:
        """Get the current governance configuration.

        Returns:
            Dictionary with governance parameters.
        """
        return self._get("/governance/config")

    def get_proposals(self) -> ProposalsResponse:
        """Get all governance proposals.

        Returns:
            ProposalsResponse with count and proposal list.
        """
        data = self._get("/proposals")
        return ProposalsResponse(
            count=data.get("count", 0),
            proposals=data.get("proposals", []),
        )

    def get_proposal(self, proposal_id: int) -> Dict[str, Any]:
        """Get details of a specific governance proposal.

        Args:
            proposal_id: The proposal ID.

        Returns:
            Dictionary with proposal details.
        """
        return self._get(f"/proposal/{proposal_id}")

    def get_vote(self, proposal_id: int, address: str) -> VoteInfo:
        """Get vote information for a specific proposal and address.

        Args:
            proposal_id: The proposal ID.
            address: 64-character hex address of the voter.

        Returns:
            VoteInfo with vote details.
        """
        data = self._get(f"/vote/{proposal_id}/{address}")
        return VoteInfo(
            proposal_id=data.get("proposal_id", proposal_id),
            address=data.get("address", address),
            vote=data.get("vote"),
            extra={k: v for k, v in data.items() if k not in ("proposal_id", "address", "vote")},
        )

    # ------------------------------------------------------------------
    # POST endpoints
    # ------------------------------------------------------------------

    def send_transaction(
        self, secret_key: str, to: str, amount: int, fee: int
    ) -> TransactionResponse:
        """Submit a signed transaction.

        Args:
            secret_key: 64-character hex secret key of the sender.
            to: 64-character hex address of the recipient.
            amount: Amount to send in sats.
            fee: Transaction fee in sats.

        Returns:
            TransactionResponse with hash, addresses, amount, fee, and nonce.
        """
        data = self._post("/tx", {
            "secret_key": secret_key,
            "to": to,
            "amount": amount,
            "fee": fee,
        })
        return TransactionResponse(
            hash=data.get("hash", ""),
            from_address=data.get("from", ""),
            to=data.get("to", to),
            amount=data.get("amount", amount),
            fee=data.get("fee", fee),
            nonce=data.get("nonce", 0),
        )

    def faucet(self, address: str, amount: int) -> FaucetResponse:
        """Request testnet tokens from the faucet.

        Args:
            address: 64-character hex address to receive tokens.
            amount: Amount in sats to request.

        Returns:
            FaucetResponse with transaction details.
        """
        data = self._post("/faucet", {
            "address": address,
            "amount": amount,
        })
        return FaucetResponse(
            tx_hash=data.get("tx_hash", ""),
            from_address=data.get("from", ""),
            to=data.get("to", address),
            amount=data.get("amount", amount),
            amount_udag=data.get("amount_udag"),
            nonce=data.get("nonce", 0),
        )

    def stake(self, secret_key: str, amount: int) -> StakeResponse:
        """Stake UDAG tokens as a validator.

        Args:
            secret_key: 64-character hex secret key of the staker.
            amount: Amount to stake in sats.

        Returns:
            StakeResponse with transaction details and stake info.
        """
        data = self._post("/stake", {
            "secret_key": secret_key,
            "amount": amount,
        })
        return StakeResponse(
            status=data.get("status", ""),
            tx_hash=data.get("tx_hash", ""),
            address=data.get("address", ""),
            amount=data.get("amount", amount),
            amount_udag=data.get("amount_udag"),
            nonce=data.get("nonce", 0),
            note=data.get("note"),
        )

    def unstake(self, secret_key: str) -> UnstakeResponse:
        """Begin unstaking (starts cooldown period).

        Args:
            secret_key: 64-character hex secret key of the staker.

        Returns:
            UnstakeResponse with unlock round and transaction details.
        """
        data = self._post("/unstake", {
            "secret_key": secret_key,
        })
        return UnstakeResponse(
            status=data.get("status", ""),
            tx_hash=data.get("tx_hash", ""),
            address=data.get("address", ""),
            unlock_at_round=data.get("unlock_at_round", 0),
            nonce=data.get("nonce", 0),
            note=data.get("note"),
        )

    def submit_proposal(
        self,
        proposer_secret: str,
        title: str,
        description: str,
        proposal_type: str,
        parameter_name: Optional[str] = None,
        parameter_value: Optional[str] = None,
        fee: Optional[int] = None,
    ) -> ProposalSubmitResponse:
        """Submit a governance proposal.

        Args:
            proposer_secret: 64-character hex secret key of the proposer.
            title: Proposal title.
            description: Proposal description.
            proposal_type: Type of proposal (e.g., "parameter_change").
            parameter_name: Name of parameter to change (for parameter proposals).
            parameter_value: New value for the parameter.
            fee: Optional proposal fee in sats.

        Returns:
            ProposalSubmitResponse with proposal ID and transaction details.
        """
        body: Dict[str, Any] = {
            "proposer_secret": proposer_secret,
            "title": title,
            "description": description,
            "proposal_type": proposal_type,
        }
        if parameter_name is not None:
            body["parameter_name"] = parameter_name
        if parameter_value is not None:
            body["parameter_value"] = parameter_value
        if fee is not None:
            body["fee"] = fee

        data = self._post("/proposal", body)
        return ProposalSubmitResponse(
            status=data.get("status", ""),
            tx_hash=data.get("tx_hash", ""),
            proposal_id=data.get("proposal_id", 0),
            proposer=data.get("proposer", ""),
            title=data.get("title", title),
            note=data.get("note"),
        )

    def vote(
        self,
        voter_secret: str,
        proposal_id: int,
        vote: str,
        fee: Optional[int] = None,
    ) -> VoteResponse:
        """Vote on a governance proposal.

        Args:
            voter_secret: 64-character hex secret key of the voter.
            proposal_id: ID of the proposal to vote on.
            vote: Vote value (e.g., "yes", "no", "abstain").
            fee: Optional vote fee in sats.

        Returns:
            VoteResponse with vote confirmation.
        """
        body: Dict[str, Any] = {
            "voter_secret": voter_secret,
            "proposal_id": proposal_id,
            "vote": vote,
        }
        if fee is not None:
            body["fee"] = fee

        data = self._post("/vote", body)
        return VoteResponse(
            status=data.get("status", ""),
            tx_hash=data.get("tx_hash", ""),
            proposal_id=data.get("proposal_id", proposal_id),
            voter=data.get("voter", ""),
            vote=data.get("vote", vote),
            note=data.get("note"),
        )

    def submit_signed_transaction(self, tx: Dict[str, Any]) -> Dict[str, Any]:
        """Submit a pre-signed transaction to the node via POST /tx/submit.

        This is the mainnet-compatible transaction path. The transaction
        must be fully signed client-side (e.g., via the builder functions
        in ``ultradag.transactions``).

        Args:
            tx: A JSON-serializable dict matching the Rust ``Transaction``
                serde format (e.g., ``{"Transfer": {...}}``).

        Returns:
            Server response as a dictionary.
        """
        return self._post("/tx/submit", tx)

    # ------------------------------------------------------------------
    # Convenience helpers
    # ------------------------------------------------------------------

    def wait_for_finality(
        self, target_round: int, poll_interval: float = 1.0, max_wait: float = 120.0
    ) -> StatusResponse:
        """Poll until a specific round is finalized.

        Args:
            target_round: The round number to wait for finalization.
            poll_interval: Seconds between polls (default: 1.0).
            max_wait: Maximum seconds to wait before raising (default: 120.0).

        Returns:
            StatusResponse once the target round is finalized.

        Raises:
            UltraDagError: If max_wait is exceeded.
        """
        import time

        start = time.monotonic()
        while True:
            status = self.get_status()
            if status.last_finalized_round >= target_round:
                return status
            elapsed = time.monotonic() - start
            if elapsed >= max_wait:
                raise UltraDagError(
                    f"Timed out waiting for round {target_round} finality "
                    f"(current: {status.last_finalized_round}, waited {elapsed:.1f}s)"
                )
            time.sleep(poll_interval)
