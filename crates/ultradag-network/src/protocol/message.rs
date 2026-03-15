use serde::{Deserialize, Serialize};

use ultradag_coin::{Block, DagVertex, Transaction};

/// Maximum message size: 4MB
/// Prevents DoS attacks via oversized messages
pub const MAX_MESSAGE_SIZE: usize = 4 * 1024 * 1024;

/// All messages in the UltraDAG P2P protocol.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
    /// Initial handshake.
    Hello {
        version: u32,
        height: u64,
        listen_port: u16,
    },

    /// Response to Hello.
    HelloAck {
        version: u32,
        height: u64,
    },

    /// Announce a new block.
    NewBlock(Block),

    /// Announce a new transaction.
    NewTx(Transaction),

    /// Request blocks starting from a height.
    GetBlocks {
        from_height: u64,
        max_count: u32,
    },

    /// Response with requested blocks.
    Blocks(Vec<Block>),

    /// Request peer list.
    GetPeers,

    /// Response with known peers.
    Peers(Vec<String>),

    /// DAG-BFT: propose a new vertex (block with DAG metadata).
    DagProposal(DagVertex),

    /// DAG-BFT: request DAG vertices from a round range.
    GetDagVertices {
        from_round: u64,
        max_count: u32,
    },

    /// DAG-BFT: response with DAG vertices.
    DagVertices(Vec<DagVertex>),

    /// Ping to keep connection alive.
    Ping(u64),

    /// Pong response.
    Pong(u64),

    /// Equivocation evidence: two vertices from same validator in same round.
    /// Used to detect and broadcast Byzantine behavior.
    EquivocationEvidence {
        vertex1: DagVertex,
        vertex2: DagVertex,
    },

    /// Request specific vertices by hash (for resolving missing parents).
    /// Sent when a vertex fails insertion due to missing parent hashes.
    GetParents {
        hashes: Vec<[u8; 32]>,
    },

    /// Response to GetParents: the requested vertices that the peer has.
    ParentVertices {
        vertices: Vec<DagVertex>,
    },

    /// A validator proposes a checkpoint and requests co-signatures.
    CheckpointProposal(ultradag_coin::consensus::Checkpoint),

    /// A validator's signature on a checkpoint they have verified.
    CheckpointSignatureMsg {
        round: u64,
        checkpoint_hash: [u8; 32],
        signature: ultradag_coin::consensus::CheckpointSignature,
    },

    /// Request the latest checkpoint for fast-sync.
    GetCheckpoint {
        min_round: u64,
    },

    /// Request vertex hashes for a range of rounds (efficient reconciliation).
    /// Response is compact (~32 bytes per vertex vs ~2-5 KB for full vertex).
    GetRoundHashes {
        from_round: u64,
        to_round: u64,
    },

    /// Response with vertex hashes grouped by round.
    /// Receiver compares against local DAG and requests only missing vertices.
    RoundHashes {
        rounds: Vec<(u64, Vec<[u8; 32]>)>,
    },

    /// Response to GetCheckpoint: the latest accepted checkpoint + suffix DAG + state.
    /// Used for fast-sync by new nodes.
    CheckpointSync {
        checkpoint: ultradag_coin::consensus::Checkpoint,
        /// DAG vertices from checkpoint.round to current round (the suffix).
        suffix_vertices: Vec<DagVertex>,
        /// State snapshot at checkpoint.round.
        state_at_checkpoint: ultradag_coin::state::persistence::StateSnapshot,
    },
}

impl Message {
    /// Serialize to length-prefixed binary bytes (postcard format).
    pub fn encode(&self) -> Result<Vec<u8>, postcard::Error> {
        let bytes = postcard::to_allocvec(self)?;
        let len = (bytes.len() as u32).to_be_bytes();
        let mut buf = Vec::with_capacity(4 + bytes.len());
        buf.extend_from_slice(&len);
        buf.extend_from_slice(&bytes);
        Ok(buf)
    }

    /// Deserialize from binary bytes (without length prefix).
    /// Enforces maximum message size to prevent DoS attacks.
    pub fn decode(data: &[u8]) -> Result<Self, postcard::Error> {
        // CRITICAL: Reject oversized messages before deserialization
        if data.len() > MAX_MESSAGE_SIZE {
            return Err(postcard::Error::DeserializeUnexpectedEnd);
        }
        postcard::from_bytes(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ultradag_coin::{Address, Block, BlockHeader, CoinbaseTx, Signature, Transaction, TransferTx};

    fn test_block() -> Block {
        let header = BlockHeader {
            version: 1,
            height: 1,
            timestamp: 0,
            prev_hash: [0u8; 32],
            merkle_root: [0u8; 32],
        };
        let coinbase = CoinbaseTx {
            to: Address::ZERO,
            amount: 5_000_000_000,
            height: 1,
        };
        Block {
            header,
            coinbase,
            transactions: vec![],
        }
    }

    fn test_tx() -> Transaction {
        Transaction::Transfer(TransferTx {
            from: Address::ZERO,
            to: Address::ZERO,
            amount: 100,
            fee: 1,
            nonce: 0,
            pub_key: [0u8; 32],
            signature: Signature([0u8; 64]),
            memo: None,
        })
    }

    /// Helper: encode a message, verify 4-byte length prefix, decode body, return decoded.
    fn roundtrip(msg: &Message) -> Vec<u8> {
        let encoded = msg.encode().unwrap();
        // First 4 bytes are big-endian u32 length prefix
        assert!(encoded.len() >= 4);
        let len = u32::from_be_bytes([encoded[0], encoded[1], encoded[2], encoded[3]]) as usize;
        assert_eq!(len, encoded.len() - 4);
        // Body must be valid postcard binary
        let body = &encoded[4..];
        Message::decode(body).expect("roundtrip decode failed");
        encoded
    }

    #[test]
    fn encode_decode_hello() {
        let msg = Message::Hello {
            version: 1,
            height: 42,
            listen_port: 9000,
        };
        let encoded = roundtrip(&msg);
        let decoded = Message::decode(&encoded[4..]).unwrap();
        match decoded {
            Message::Hello { version, height, listen_port } => {
                assert_eq!(version, 1);
                assert_eq!(height, 42);
                assert_eq!(listen_port, 9000);
            }
            _ => panic!("expected Hello"),
        }
    }

    #[test]
    fn encode_decode_hello_ack() {
        let msg = Message::HelloAck {
            version: 1,
            height: 100,
        };
        let encoded = roundtrip(&msg);
        let decoded = Message::decode(&encoded[4..]).unwrap();
        match decoded {
            Message::HelloAck { version, height } => {
                assert_eq!(version, 1);
                assert_eq!(height, 100);
            }
            _ => panic!("expected HelloAck"),
        }
    }

    #[test]
    fn encode_decode_new_block() {
        let block = test_block();
        let msg = Message::NewBlock(block.clone());
        let encoded = roundtrip(&msg);
        let decoded = Message::decode(&encoded[4..]).unwrap();
        match decoded {
            Message::NewBlock(b) => {
                assert_eq!(b.header.height, 1);
                assert_eq!(b.coinbase.amount, 5_000_000_000);
            }
            _ => panic!("expected NewBlock"),
        }
    }

    #[test]
    fn encode_decode_new_tx() {
        let tx = test_tx();
        let msg = Message::NewTx(tx);
        let encoded = roundtrip(&msg);
        let decoded = Message::decode(&encoded[4..]).unwrap();
        match decoded {
            Message::NewTx(t) => {
                if let ultradag_coin::Transaction::Transfer(ref transfer) = t {
                    assert_eq!(transfer.amount, 100);
                    assert_eq!(transfer.fee, 1);
                } else {
                    panic!("expected Transfer variant");
                }
            }
            _ => panic!("expected NewTx"),
        }
    }

    #[test]
    fn encode_decode_get_blocks() {
        let msg = Message::GetBlocks {
            from_height: 10,
            max_count: 50,
        };
        let encoded = roundtrip(&msg);
        let decoded = Message::decode(&encoded[4..]).unwrap();
        match decoded {
            Message::GetBlocks { from_height, max_count } => {
                assert_eq!(from_height, 10);
                assert_eq!(max_count, 50);
            }
            _ => panic!("expected GetBlocks"),
        }
    }

    #[test]
    fn encode_decode_blocks() {
        let msg = Message::Blocks(vec![test_block(), test_block()]);
        let encoded = roundtrip(&msg);
        let decoded = Message::decode(&encoded[4..]).unwrap();
        match decoded {
            Message::Blocks(blocks) => assert_eq!(blocks.len(), 2),
            _ => panic!("expected Blocks"),
        }
    }

    #[test]
    fn encode_decode_get_peers() {
        let msg = Message::GetPeers;
        let encoded = roundtrip(&msg);
        let decoded = Message::decode(&encoded[4..]).unwrap();
        assert!(matches!(decoded, Message::GetPeers));
    }

    #[test]
    fn encode_decode_peers() {
        let msg = Message::Peers(vec!["127.0.0.1:9000".into(), "10.0.0.1:9001".into()]);
        let encoded = roundtrip(&msg);
        let decoded = Message::decode(&encoded[4..]).unwrap();
        match decoded {
            Message::Peers(addrs) => {
                assert_eq!(addrs.len(), 2);
                assert_eq!(addrs[0], "127.0.0.1:9000");
            }
            _ => panic!("expected Peers"),
        }
    }

    #[test]
    fn encode_decode_ping() {
        let msg = Message::Ping(12345);
        let encoded = roundtrip(&msg);
        let decoded = Message::decode(&encoded[4..]).unwrap();
        match decoded {
            Message::Ping(n) => assert_eq!(n, 12345),
            _ => panic!("expected Ping"),
        }
    }

    #[test]
    fn encode_decode_pong() {
        let msg = Message::Pong(67890);
        let encoded = roundtrip(&msg);
        let decoded = Message::decode(&encoded[4..]).unwrap();
        match decoded {
            Message::Pong(n) => assert_eq!(n, 67890),
            _ => panic!("expected Pong"),
        }
    }

    #[test]
    fn reject_oversized_message() {
        let oversized_data = vec![0u8; MAX_MESSAGE_SIZE + 1];
        let result = Message::decode(&oversized_data);
        assert!(result.is_err(), "Oversized message should be rejected");
    }

    #[test]
    fn accept_max_size_message() {
        let ping_bytes = postcard::to_allocvec(&Message::Ping(12345)).unwrap();
        assert!(ping_bytes.len() <= MAX_MESSAGE_SIZE);
        let result = Message::decode(&ping_bytes);
        assert!(result.is_ok(), "Message at or below max size should be accepted");
    }

    #[test]
    fn encode_decode_checkpoint_proposal() {
        use ultradag_coin::consensus::Checkpoint;
        
        let checkpoint = Checkpoint {
            round: 1000,
            state_root: [1u8; 32],
            dag_tip: [2u8; 32],
            total_supply: 1_000_000_000,
            prev_checkpoint_hash: [0u8; 32],
            signatures: vec![],
        };

        let msg = Message::CheckpointProposal(checkpoint);
        let encoded = roundtrip(&msg);
        let decoded = Message::decode(&encoded[4..]).unwrap();
        
        match decoded {
            Message::CheckpointProposal(cp) => {
                assert_eq!(cp.round, 1000);
                assert_eq!(cp.total_supply, 1_000_000_000);
            }
            _ => panic!("expected CheckpointProposal"),
        }
    }

    #[test]
    fn encode_decode_checkpoint_signature_msg() {
        use ultradag_coin::consensus::CheckpointSignature;
        
        let sig = CheckpointSignature {
            validator: Address::ZERO,
            pub_key: [0u8; 32],
            signature: Signature([0u8; 64]),
        };
        
        let msg = Message::CheckpointSignatureMsg {
            round: 1000,
            checkpoint_hash: [3u8; 32],
            signature: sig,
        };
        
        let encoded = roundtrip(&msg);
        let decoded = Message::decode(&encoded[4..]).unwrap();
        
        match decoded {
            Message::CheckpointSignatureMsg { round, checkpoint_hash, .. } => {
                assert_eq!(round, 1000);
                assert_eq!(checkpoint_hash, [3u8; 32]);
            }
            _ => panic!("expected CheckpointSignatureMsg"),
        }
    }

    #[test]
    fn encode_decode_get_checkpoint() {
        let msg = Message::GetCheckpoint { min_round: 500 };
        let encoded = roundtrip(&msg);
        let decoded = Message::decode(&encoded[4..]).unwrap();
        
        match decoded {
            Message::GetCheckpoint { min_round } => {
                assert_eq!(min_round, 500);
            }
            _ => panic!("expected GetCheckpoint"),
        }
    }

    #[test]
    fn encode_decode_checkpoint_sync() {
        use ultradag_coin::consensus::Checkpoint;
        use ultradag_coin::state::persistence::StateSnapshot;
        
        let checkpoint = Checkpoint {
            round: 1000,
            state_root: [1u8; 32],
            dag_tip: [2u8; 32],
            total_supply: 1_000_000_000,
            prev_checkpoint_hash: [0u8; 32],
            signatures: vec![],
        };

        let state = StateSnapshot {
            accounts: vec![],
            stake_accounts: vec![],
            active_validator_set: vec![],
            current_epoch: 0,
            total_supply: 1_000_000_000,
            last_finalized_round: Some(1000),
            proposals: vec![],
            votes: vec![],
            next_proposal_id: 0,
            governance_params: Default::default(),
            council_members: vec![],
            treasury_balance: 0,
        };

        let msg = Message::CheckpointSync {
            checkpoint,
            suffix_vertices: vec![],
            state_at_checkpoint: state,
        };
        
        let encoded = roundtrip(&msg);
        let decoded = Message::decode(&encoded[4..]).unwrap();
        
        match decoded {
            Message::CheckpointSync { checkpoint, suffix_vertices, state_at_checkpoint } => {
                assert_eq!(checkpoint.round, 1000);
                assert_eq!(suffix_vertices.len(), 0);
                assert_eq!(state_at_checkpoint.total_supply, 1_000_000_000);
            }
            _ => panic!("expected CheckpointSync"),
        }
    }
}
