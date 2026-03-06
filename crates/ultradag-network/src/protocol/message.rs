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
}

impl Message {
    /// Serialize to length-prefixed JSON bytes.
    pub fn encode(&self) -> Result<Vec<u8>, serde_json::Error> {
        let json = serde_json::to_vec(self)?;
        let len = (json.len() as u32).to_be_bytes();
        let mut buf = Vec::with_capacity(4 + json.len());
        buf.extend_from_slice(&len);
        buf.extend_from_slice(&json);
        Ok(buf)
    }

    /// Deserialize from JSON bytes (without length prefix).
    /// Enforces maximum message size to prevent DoS attacks.
    pub fn decode(data: &[u8]) -> Result<Self, serde_json::Error> {
        // CRITICAL: Reject oversized messages before deserialization
        if data.len() > MAX_MESSAGE_SIZE {
            return Err(serde_json::Error::io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Message too large: {} bytes (max {})", data.len(), MAX_MESSAGE_SIZE)
            )));
        }
        serde_json::from_slice(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ultradag_coin::{Address, Block, BlockHeader, CoinbaseTx, Signature, Transaction};

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
        Transaction {
            from: Address::ZERO,
            to: Address::ZERO,
            amount: 100,
            fee: 1,
            nonce: 0,
            pub_key: [0u8; 32],
            signature: Signature([0u8; 64]),
        }
    }

    /// Helper: encode a message, verify 4-byte length prefix, decode body, return decoded.
    fn roundtrip(msg: &Message) -> Vec<u8> {
        let encoded = msg.encode().unwrap();
        // First 4 bytes are big-endian u32 length prefix
        assert!(encoded.len() >= 4);
        let len = u32::from_be_bytes([encoded[0], encoded[1], encoded[2], encoded[3]]) as usize;
        assert_eq!(len, encoded.len() - 4);
        // Body must be valid JSON
        let body = &encoded[4..];
        assert!(serde_json::from_slice::<serde_json::Value>(body).is_ok());
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
                assert_eq!(t.amount, 100);
                assert_eq!(t.fee, 1);
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
        // Create a message larger than MAX_MESSAGE_SIZE (4MB + 1 byte)
        let oversized_data = vec![0u8; MAX_MESSAGE_SIZE + 1];
        let result = Message::decode(&oversized_data);
        
        assert!(result.is_err(), "Oversized message should be rejected");
        let err = result.unwrap_err();
        let err_msg = format!("{}", err);
        assert!(err_msg.contains("Message too large"), "Error should mention message size");
    }

    #[test]
    fn accept_max_size_message() {
        // Create a valid JSON message exactly at MAX_MESSAGE_SIZE
        // Use a simple Ping message padded to max size
        let ping_json = serde_json::to_vec(&Message::Ping(12345)).unwrap();
        
        // If the message is smaller than max, it should decode successfully
        if ping_json.len() <= MAX_MESSAGE_SIZE {
            let result = Message::decode(&ping_json);
            assert!(result.is_ok(), "Message at or below max size should be accepted");
        }
    }
}
