"""Tests for UltraDagClient with mocked HTTP responses."""

import json
import unittest
from unittest.mock import patch, MagicMock

import requests

from ultradag.client import UltraDagClient
from ultradag.types import (
    ApiError,
    ConnectionError,
    UltraDagError,
    StatusResponse,
    BalanceResponse,
    TransactionResponse,
)


def _mock_response(status_code=200, json_data=None, text=""):
    """Create a mock requests.Response."""
    resp = MagicMock(spec=requests.Response)
    resp.status_code = status_code
    resp.text = text or json.dumps(json_data or {})
    resp.json.return_value = json_data or {}
    return resp


class TestHealth(unittest.TestCase):
    def setUp(self):
        self.client = UltraDagClient("http://localhost:10333")

    @patch.object(requests.Session, "get")
    def test_health_ok(self, mock_get):
        mock_get.return_value = _mock_response(json_data={"status": "ok"})
        result = self.client.health()
        self.assertEqual(result.status, "ok")
        mock_get.assert_called_once_with("http://localhost:10333/health", timeout=30.0)


class TestGetStatus(unittest.TestCase):
    def setUp(self):
        self.client = UltraDagClient("http://testnode:10333")

    @patch.object(requests.Session, "get")
    def test_status_full(self, mock_get):
        data = {
            "last_finalized_round": 150,
            "peer_count": 3,
            "mempool_size": 42,
            "total_supply": 210000000000000,
            "account_count": 100,
            "dag_vertices": 600,
            "dag_round": 155,
            "dag_tips": 4,
            "finalized_count": 580,
            "validator_count": 4,
            "total_staked": 40000000000000,
            "active_stakers": 4,
            "bootstrap_connected": True,
        }
        mock_get.return_value = _mock_response(json_data=data)
        result = self.client.get_status()
        self.assertIsInstance(result, StatusResponse)
        self.assertEqual(result.last_finalized_round, 150)
        self.assertEqual(result.peer_count, 3)
        self.assertEqual(result.validator_count, 4)
        self.assertEqual(result.total_supply, 210000000000000)
        self.assertTrue(result.bootstrap_connected)

    @patch.object(requests.Session, "get")
    def test_status_missing_fields_default(self, mock_get):
        mock_get.return_value = _mock_response(json_data={"dag_round": 10})
        result = self.client.get_status()
        self.assertEqual(result.dag_round, 10)
        self.assertEqual(result.peer_count, 0)
        self.assertIsNone(result.bootstrap_connected)


class TestGetBalance(unittest.TestCase):
    def setUp(self):
        self.client = UltraDagClient()

    @patch.object(requests.Session, "get")
    def test_balance(self, mock_get):
        addr = "ab" * 32
        mock_get.return_value = _mock_response(json_data={
            "address": addr,
            "balance": 500000000,
            "nonce": 5,
            "balance_tdag": 5.0,
        })
        result = self.client.get_balance(addr)
        self.assertIsInstance(result, BalanceResponse)
        self.assertEqual(result.address, addr)
        self.assertEqual(result.balance, 500000000)
        self.assertEqual(result.nonce, 5)
        self.assertAlmostEqual(result.balance_udag, 5.0)

    @patch.object(requests.Session, "get")
    def test_balance_zero(self, mock_get):
        addr = "00" * 32
        mock_get.return_value = _mock_response(json_data={
            "address": addr,
            "balance": 0,
            "nonce": 0,
        })
        result = self.client.get_balance(addr)
        self.assertEqual(result.balance, 0)
        self.assertEqual(result.nonce, 0)
        self.assertAlmostEqual(result.balance_udag, 0.0)


class TestGetRound(unittest.TestCase):
    def setUp(self):
        self.client = UltraDagClient()

    @patch.object(requests.Session, "get")
    def test_round_vertices(self, mock_get):
        vertices = [
            {"round": 10, "hash": "aa" * 32, "validator": "bb" * 32, "reward": 5000000000, "tx_count": 3, "parent_count": 2},
            {"round": 10, "hash": "cc" * 32, "validator": "dd" * 32, "reward": 5000000000, "tx_count": 0, "parent_count": 2},
        ]
        mock_get.return_value = _mock_response(json_data=vertices)
        result = self.client.get_round(10)
        self.assertEqual(len(result), 2)
        self.assertEqual(result[0].round, 10)
        self.assertEqual(result[0].tx_count, 3)
        self.assertEqual(result[1].validator, "dd" * 32)

    @patch.object(requests.Session, "get")
    def test_round_empty(self, mock_get):
        mock_get.return_value = _mock_response(json_data=[])
        result = self.client.get_round(999)
        self.assertEqual(result, [])


class TestGetMempool(unittest.TestCase):
    def setUp(self):
        self.client = UltraDagClient()

    @patch.object(requests.Session, "get")
    def test_mempool(self, mock_get):
        txs = [
            {"hash": "ff" * 32, "from": "aa" * 32, "to": "bb" * 32, "amount": 1000, "fee": 100, "nonce": 1},
        ]
        mock_get.return_value = _mock_response(json_data=txs)
        result = self.client.get_mempool()
        self.assertEqual(len(result), 1)
        self.assertEqual(result[0].amount, 1000)
        self.assertEqual(result[0].from_address, "aa" * 32)

    @patch.object(requests.Session, "get")
    def test_mempool_empty(self, mock_get):
        mock_get.return_value = _mock_response(json_data=[])
        result = self.client.get_mempool()
        self.assertEqual(result, [])


class TestGetPeers(unittest.TestCase):
    def setUp(self):
        self.client = UltraDagClient()

    @patch.object(requests.Session, "get")
    def test_peers(self, mock_get):
        mock_get.return_value = _mock_response(json_data={
            "connected": 3,
            "peers": [{"address": "1.2.3.4:9333"}],
            "bootstrap_nodes": [{"address": "206.51.242.223:9333", "connected": True}],
        })
        result = self.client.get_peers()
        self.assertEqual(result.connected, 3)
        self.assertEqual(len(result.peers), 1)
        self.assertEqual(len(result.bootstrap_nodes), 1)


class TestKeygen(unittest.TestCase):
    def setUp(self):
        self.client = UltraDagClient()

    @patch.object(requests.Session, "get")
    def test_keygen(self, mock_get):
        mock_get.return_value = _mock_response(json_data={
            "secret_key": "ab" * 32,
            "address": "cd" * 32,
        })
        result = self.client.keygen()
        self.assertEqual(result.secret_key, "ab" * 32)
        self.assertEqual(result.address, "cd" * 32)


class TestGetValidators(unittest.TestCase):
    def setUp(self):
        self.client = UltraDagClient()

    @patch.object(requests.Session, "get")
    def test_validators(self, mock_get):
        mock_get.return_value = _mock_response(json_data={
            "count": 2,
            "total_staked": 200000000000000,
            "validators": [
                {"address": "aa" * 32, "staked": 100000000000000, "staked_udag": 1000000.0},
                {"address": "bb" * 32, "staked": 100000000000000, "staked_udag": 1000000.0},
            ],
        })
        result = self.client.get_validators()
        self.assertEqual(result.count, 2)
        self.assertEqual(len(result.validators), 2)
        self.assertEqual(result.validators[0].staked, 100000000000000)

    @patch.object(requests.Session, "get")
    def test_validators_empty(self, mock_get):
        mock_get.return_value = _mock_response(json_data={
            "count": 0,
            "total_staked": 0,
            "validators": [],
        })
        result = self.client.get_validators()
        self.assertEqual(result.count, 0)
        self.assertEqual(result.validators, [])


class TestGetStake(unittest.TestCase):
    def setUp(self):
        self.client = UltraDagClient()

    @patch.object(requests.Session, "get")
    def test_stake_info(self, mock_get):
        addr = "ab" * 32
        mock_get.return_value = _mock_response(json_data={
            "address": addr,
            "staked": 1000000000000,
            "staked_udag": 10000.0,
            "unlock_at_round": None,
            "is_active_validator": True,
        })
        result = self.client.get_stake(addr)
        self.assertEqual(result.staked, 1000000000000)
        self.assertTrue(result.is_active_validator)
        self.assertIsNone(result.unlock_at_round)


class TestSendTransaction(unittest.TestCase):
    def setUp(self):
        self.client = UltraDagClient()

    @patch.object(requests.Session, "post")
    def test_send_tx(self, mock_post):
        mock_post.return_value = _mock_response(json_data={
            "hash": "ff" * 32,
            "from": "aa" * 32,
            "to": "bb" * 32,
            "amount": 100000000,
            "fee": 100000,
            "nonce": 1,
        })
        result = self.client.send_transaction(
            secret_key="aa" * 32,
            to="bb" * 32,
            amount=100000000,
            fee=100000,
        )
        self.assertIsInstance(result, TransactionResponse)
        self.assertEqual(result.hash, "ff" * 32)
        self.assertEqual(result.amount, 100000000)
        self.assertEqual(result.nonce, 1)

        call_args = mock_post.call_args
        body = call_args[1]["json"] if "json" in call_args[1] else call_args[0][1] if len(call_args[0]) > 1 else None
        # Verify the post was called with correct URL
        self.assertIn("/tx", call_args[0][0])


class TestFaucet(unittest.TestCase):
    def setUp(self):
        self.client = UltraDagClient()

    @patch.object(requests.Session, "post")
    def test_faucet(self, mock_post):
        mock_post.return_value = _mock_response(json_data={
            "tx_hash": "ff" * 32,
            "from": "fa" * 32,
            "to": "bb" * 32,
            "amount": 10000000000,
            "amount_udag": 100.0,
            "nonce": 42,
        })
        result = self.client.faucet("bb" * 32, 10000000000)
        self.assertEqual(result.tx_hash, "ff" * 32)
        self.assertEqual(result.amount, 10000000000)
        self.assertEqual(result.nonce, 42)


class TestStake(unittest.TestCase):
    def setUp(self):
        self.client = UltraDagClient()

    @patch.object(requests.Session, "post")
    def test_stake(self, mock_post):
        mock_post.return_value = _mock_response(json_data={
            "status": "staked",
            "tx_hash": "ff" * 32,
            "address": "aa" * 32,
            "amount": 1000000000000,
            "amount_udag": 10000.0,
            "nonce": 1,
            "note": "Stake registered",
        })
        result = self.client.stake("aa" * 32, 1000000000000)
        self.assertEqual(result.status, "staked")
        self.assertEqual(result.amount, 1000000000000)
        self.assertEqual(result.note, "Stake registered")


class TestUnstake(unittest.TestCase):
    def setUp(self):
        self.client = UltraDagClient()

    @patch.object(requests.Session, "post")
    def test_unstake(self, mock_post):
        mock_post.return_value = _mock_response(json_data={
            "status": "unstaking",
            "tx_hash": "ff" * 32,
            "address": "aa" * 32,
            "unlock_at_round": 2166,
            "nonce": 2,
            "note": "Cooldown started",
        })
        result = self.client.unstake("aa" * 32)
        self.assertEqual(result.status, "unstaking")
        self.assertEqual(result.unlock_at_round, 2166)


class TestSubmitProposal(unittest.TestCase):
    def setUp(self):
        self.client = UltraDagClient()

    @patch.object(requests.Session, "post")
    def test_submit_proposal(self, mock_post):
        mock_post.return_value = _mock_response(json_data={
            "status": "proposed",
            "tx_hash": "ff" * 32,
            "proposal_id": 1,
            "proposer": "aa" * 32,
            "title": "Increase block reward",
            "note": "Proposal submitted",
        })
        result = self.client.submit_proposal(
            proposer_secret="aa" * 32,
            title="Increase block reward",
            description="Proposal to increase block reward to 100 UDAG",
            proposal_type="parameter_change",
            parameter_name="block_reward",
            parameter_value="10000000000",
        )
        self.assertEqual(result.proposal_id, 1)
        self.assertEqual(result.title, "Increase block reward")

    @patch.object(requests.Session, "post")
    def test_submit_proposal_minimal(self, mock_post):
        mock_post.return_value = _mock_response(json_data={
            "status": "proposed",
            "tx_hash": "ff" * 32,
            "proposal_id": 2,
            "proposer": "aa" * 32,
            "title": "General proposal",
        })
        result = self.client.submit_proposal(
            proposer_secret="aa" * 32,
            title="General proposal",
            description="A general proposal",
            proposal_type="general",
        )
        self.assertEqual(result.proposal_id, 2)


class TestVote(unittest.TestCase):
    def setUp(self):
        self.client = UltraDagClient()

    @patch.object(requests.Session, "post")
    def test_vote_yes(self, mock_post):
        mock_post.return_value = _mock_response(json_data={
            "status": "voted",
            "tx_hash": "ff" * 32,
            "proposal_id": 1,
            "voter": "aa" * 32,
            "vote": "yes",
            "note": "Vote recorded",
        })
        result = self.client.vote("aa" * 32, proposal_id=1, vote="yes")
        self.assertEqual(result.vote, "yes")
        self.assertEqual(result.proposal_id, 1)


class TestGetVote(unittest.TestCase):
    def setUp(self):
        self.client = UltraDagClient()

    @patch.object(requests.Session, "get")
    def test_get_vote(self, mock_get):
        mock_get.return_value = _mock_response(json_data={
            "proposal_id": 1,
            "address": "aa" * 32,
            "vote": "yes",
        })
        result = self.client.get_vote(1, "aa" * 32)
        self.assertEqual(result.vote, "yes")
        self.assertEqual(result.proposal_id, 1)


class TestGetProposals(unittest.TestCase):
    def setUp(self):
        self.client = UltraDagClient()

    @patch.object(requests.Session, "get")
    def test_proposals(self, mock_get):
        mock_get.return_value = _mock_response(json_data={
            "count": 1,
            "proposals": [{"id": 1, "title": "Test"}],
        })
        result = self.client.get_proposals()
        self.assertEqual(result.count, 1)
        self.assertEqual(len(result.proposals), 1)


class TestGetProposal(unittest.TestCase):
    def setUp(self):
        self.client = UltraDagClient()

    @patch.object(requests.Session, "get")
    def test_get_proposal(self, mock_get):
        mock_get.return_value = _mock_response(json_data={
            "id": 1,
            "title": "Test Proposal",
            "proposer": "aa" * 32,
        })
        result = self.client.get_proposal(1)
        self.assertEqual(result["id"], 1)
        self.assertEqual(result["title"], "Test Proposal")


class TestGovernanceConfig(unittest.TestCase):
    def setUp(self):
        self.client = UltraDagClient()

    @patch.object(requests.Session, "get")
    def test_governance_config(self, mock_get):
        mock_get.return_value = _mock_response(json_data={
            "min_proposal_fee": 100000,
            "voting_period_rounds": 1000,
        })
        result = self.client.get_governance_config()
        self.assertEqual(result["min_proposal_fee"], 100000)


class TestErrorHandling(unittest.TestCase):
    def setUp(self):
        self.client = UltraDagClient()

    @patch.object(requests.Session, "get")
    def test_api_error_404(self, mock_get):
        mock_get.return_value = _mock_response(
            status_code=404,
            text="Not found",
        )
        with self.assertRaises(ApiError) as ctx:
            self.client.get_balance("00" * 32)
        self.assertEqual(ctx.exception.status_code, 404)

    @patch.object(requests.Session, "get")
    def test_api_error_400(self, mock_get):
        mock_get.return_value = _mock_response(
            status_code=400,
            text="Invalid address",
        )
        with self.assertRaises(ApiError) as ctx:
            self.client.get_balance("invalid")
        self.assertEqual(ctx.exception.status_code, 400)

    @patch.object(requests.Session, "post")
    def test_api_error_insufficient_balance(self, mock_post):
        mock_post.return_value = _mock_response(
            status_code=400,
            text="Insufficient balance",
        )
        with self.assertRaises(ApiError) as ctx:
            self.client.send_transaction("aa" * 32, "bb" * 32, 999999999999999, 100000)
        self.assertIn("Insufficient balance", ctx.exception.response_body)

    @patch.object(requests.Session, "get")
    def test_connection_error(self, mock_get):
        mock_get.side_effect = requests.exceptions.ConnectionError("refused")
        with self.assertRaises(ConnectionError):
            self.client.health()

    @patch.object(requests.Session, "get")
    def test_timeout_error(self, mock_get):
        mock_get.side_effect = requests.exceptions.Timeout("timed out")
        with self.assertRaises(ConnectionError):
            self.client.get_status()

    @patch.object(requests.Session, "post")
    def test_post_connection_error(self, mock_post):
        mock_post.side_effect = requests.exceptions.ConnectionError("refused")
        with self.assertRaises(ConnectionError):
            self.client.send_transaction("aa" * 32, "bb" * 32, 1000, 100)

    @patch.object(requests.Session, "post")
    def test_post_timeout_error(self, mock_post):
        mock_post.side_effect = requests.exceptions.Timeout("timed out")
        with self.assertRaises(ConnectionError):
            self.client.faucet("aa" * 32, 1000)

    @patch.object(requests.Session, "get")
    def test_generic_request_error(self, mock_get):
        mock_get.side_effect = requests.exceptions.RequestException("unknown error")
        with self.assertRaises(UltraDagError):
            self.client.health()


class TestBaseUrlHandling(unittest.TestCase):
    def test_trailing_slash_stripped(self):
        client = UltraDagClient("http://localhost:10333/")
        self.assertEqual(client.base_url, "http://localhost:10333")

    def test_custom_timeout(self):
        client = UltraDagClient(timeout=5.0)
        self.assertEqual(client.timeout, 5.0)

    def test_custom_session(self):
        session = requests.Session()
        client = UltraDagClient(session=session)
        self.assertIs(client._session, session)


class TestWaitForFinality(unittest.TestCase):
    def setUp(self):
        self.client = UltraDagClient()

    @patch.object(requests.Session, "get")
    def test_already_finalized(self, mock_get):
        mock_get.return_value = _mock_response(json_data={
            "last_finalized_round": 100,
            "peer_count": 3,
            "mempool_size": 0,
            "total_supply": 0,
            "account_count": 0,
            "dag_vertices": 0,
            "dag_round": 100,
            "dag_tips": 0,
            "finalized_count": 0,
            "validator_count": 0,
            "total_staked": 0,
            "active_stakers": 0,
        })
        result = self.client.wait_for_finality(50)
        self.assertEqual(result.last_finalized_round, 100)

    @patch("time.sleep")
    @patch.object(requests.Session, "get")
    def test_timeout_waiting(self, mock_get, mock_sleep):
        mock_get.return_value = _mock_response(json_data={
            "last_finalized_round": 5,
            "peer_count": 0,
            "mempool_size": 0,
            "total_supply": 0,
            "account_count": 0,
            "dag_vertices": 0,
            "dag_round": 5,
            "dag_tips": 0,
            "finalized_count": 0,
            "validator_count": 0,
            "total_staked": 0,
            "active_stakers": 0,
        })
        with self.assertRaises(UltraDagError) as ctx:
            self.client.wait_for_finality(100, poll_interval=0.01, max_wait=0.0)
        self.assertIn("Timed out", str(ctx.exception))


if __name__ == "__main__":
    unittest.main()
