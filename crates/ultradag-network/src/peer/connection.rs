use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::sync::Mutex;
use std::sync::Arc;
use std::time::Duration;

use crate::protocol::{Message, MAX_MESSAGE_SIZE};
use super::noise::NOISE_MAX_PLAINTEXT;

/// Timeout for reading a complete message from a peer (prevents slowloris).
/// Must be significantly longer than the heartbeat interval (30s) to avoid
/// cascading disconnections during temporary stalls. At 120s, the heartbeat
/// has 4 opportunities to deliver a Ping before the connection is killed.
const READ_TIMEOUT: Duration = Duration::from_secs(120);

/// AEAD tag overhead per Noise transport message (Poly1305).
const NOISE_TAG_LEN: usize = 16;

/// Read half of a peer connection, optionally encrypted via Noise protocol.
pub struct PeerReader {
    reader: OwnedReadHalf,
    pub addr: String,
    noise: Option<Arc<Mutex<snow::TransportState>>>,
}

/// Write half of a peer connection, wrapped in a Mutex for shared access.
/// Optionally encrypted via Noise protocol.
#[derive(Clone)]
pub struct PeerWriter {
    writer: Arc<Mutex<OwnedWriteHalf>>,
    pub addr: String,
    noise: Option<Arc<Mutex<snow::TransportState>>>,
}

impl PeerReader {
    pub fn new(reader: OwnedReadHalf, addr: String, noise: Option<Arc<Mutex<snow::TransportState>>>) -> Self {
        Self { reader, addr, noise }
    }

    /// Receive a message from this peer.
    /// Applies a read timeout to prevent slowloris-style attacks.
    pub async fn recv(&mut self) -> std::io::Result<Message> {
        tokio::time::timeout(READ_TIMEOUT, self.recv_inner())
            .await
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::TimedOut, "read timeout"))?
    }

    async fn recv_inner(&mut self) -> std::io::Result<Message> {
        match &self.noise {
            Some(noise) => self.recv_encrypted(noise.clone()).await,
            None => self.recv_plaintext().await,
        }
    }

    async fn recv_plaintext(&mut self) -> std::io::Result<Message> {
        // Read 4-byte length prefix
        let mut len_buf = [0u8; 4];
        self.reader.read_exact(&mut len_buf).await?;
        let len = u32::from_be_bytes(len_buf) as usize;

        if len > MAX_MESSAGE_SIZE {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "message too large",
            ));
        }

        // Read message body
        let mut body = vec![0u8; len];
        self.reader.read_exact(&mut body).await?;

        Message::decode(&body).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, format!("{}", e)))
    }

    async fn recv_encrypted(&mut self, noise: Arc<Mutex<snow::TransportState>>) -> std::io::Result<Message> {
        // Read 4-byte total plaintext length
        let mut len_buf = [0u8; 4];
        self.reader.read_exact(&mut len_buf).await?;
        let total_len = u32::from_be_bytes(len_buf) as usize;

        if total_len > MAX_MESSAGE_SIZE {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "encrypted message too large",
            ));
        }

        let mut plaintext = Vec::with_capacity(total_len);

        while plaintext.len() < total_len {
            // Read 2-byte encrypted chunk length
            let mut chunk_len_buf = [0u8; 2];
            self.reader.read_exact(&mut chunk_len_buf).await?;
            let chunk_len = u16::from_be_bytes(chunk_len_buf) as usize;

            if chunk_len == 0 || chunk_len > 65535 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "invalid encrypted chunk length",
                ));
            }

            // Read encrypted chunk
            let mut encrypted = vec![0u8; chunk_len];
            self.reader.read_exact(&mut encrypted).await?;

            // Decrypt — hold noise lock only for decryption
            let mut decrypted = vec![0u8; chunk_len];
            let dec_len = {
                let mut transport = noise.lock().await;
                transport.read_message(&encrypted, &mut decrypted)
                    .map_err(|e| std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!("noise decrypt: {}", e),
                    ))?
            };
            plaintext.extend_from_slice(&decrypted[..dec_len]);
        }

        Message::decode(&plaintext)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, format!("{}", e)))
    }
}

impl PeerWriter {
    pub fn new(writer: OwnedWriteHalf, addr: String, noise: Option<Arc<Mutex<snow::TransportState>>>) -> Self {
        Self {
            writer: Arc::new(Mutex::new(writer)),
            addr,
            noise,
        }
    }

    /// Send raw bytes with a 4-byte length prefix (for testing malformed/oversized messages).
    /// Only available in tests via the `test-helpers` feature.
    #[cfg(any(test, feature = "test-helpers"))]
    pub async fn send_raw(&self, data: &[u8]) -> std::io::Result<()> {
        let len = (data.len() as u32).to_be_bytes();
        let mut writer = self.writer.lock().await;
        writer.write_all(&len).await?;
        writer.write_all(data).await?;
        writer.flush().await?;
        Ok(())
    }

    /// Send a length prefix only (for testing oversized message rejection).
    /// Only available in tests via the `test-helpers` feature.
    #[cfg(any(test, feature = "test-helpers"))]
    pub async fn send_raw_len(&self, len: u32) -> std::io::Result<()> {
        let len_bytes = len.to_be_bytes();
        let mut writer = self.writer.lock().await;
        writer.write_all(&len_bytes).await?;
        writer.flush().await?;
        Ok(())
    }

    /// Send a message to this peer.
    pub async fn send(&self, msg: &Message) -> std::io::Result<()> {
        match &self.noise {
            Some(noise) => self.send_encrypted(noise, msg).await,
            None => self.send_plaintext(msg).await,
        }
    }

    async fn send_plaintext(&self, msg: &Message) -> std::io::Result<()> {
        let data = msg.encode().map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, format!("{}", e)))?;
        let mut writer = self.writer.lock().await;
        writer.write_all(&data).await?;
        writer.flush().await?;
        Ok(())
    }

    async fn send_encrypted(&self, noise: &Arc<Mutex<snow::TransportState>>, msg: &Message) -> std::io::Result<()> {
        let plaintext = postcard::to_allocvec(msg)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, format!("{}", e)))?;

        if plaintext.len() > MAX_MESSAGE_SIZE {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "message too large to encrypt",
            ));
        }

        // Encrypt all chunks under noise lock, then write under writer lock.
        // Never hold both locks simultaneously to prevent deadlock.
        let mut wire_data = Vec::with_capacity(4 + plaintext.len() + plaintext.len() / NOISE_MAX_PLAINTEXT * (2 + NOISE_TAG_LEN) + 64);
        wire_data.extend_from_slice(&(plaintext.len() as u32).to_be_bytes());
        {
            let mut transport = noise.lock().await;
            for chunk in plaintext.chunks(NOISE_MAX_PLAINTEXT) {
                let mut buf = vec![0u8; chunk.len() + NOISE_TAG_LEN + 16];
                let len = transport
                    .write_message(chunk, &mut buf)
                    .map_err(|e| std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!("noise encrypt: {}", e),
                    ))?;
                wire_data.extend_from_slice(&(len as u16).to_be_bytes());
                wire_data.extend_from_slice(&buf[..len]);
            }
        }

        let mut writer = self.writer.lock().await;
        writer.write_all(&wire_data).await?;
        writer.flush().await?;
        Ok(())
    }
}

/// Split a TcpStream into reader and writer halves, optionally with Noise encryption.
pub fn split_connection(
    stream: tokio::net::TcpStream,
    addr: String,
    noise: Option<Arc<Mutex<snow::TransportState>>>,
) -> (PeerReader, PeerWriter) {
    let (read_half, write_half) = stream.into_split();
    (
        PeerReader::new(read_half, addr.clone(), noise.clone()),
        PeerWriter::new(write_half, addr, noise),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    /// Create a connected (server_stream, client_stream) pair on localhost.
    async fn tcp_pair() -> (tokio::net::TcpStream, tokio::net::TcpStream) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let client = tokio::spawn(async move {
            tokio::net::TcpStream::connect(addr).await.unwrap()
        });
        let (server, _) = listener.accept().await.unwrap();
        let client = client.await.unwrap();
        (server, client)
    }

    #[tokio::test]
    async fn split_connection_creates_reader_and_writer() {
        let (server, _client) = tcp_pair().await;
        let (reader, writer) = split_connection(server, "test".into(), None);
        assert_eq!(reader.addr, "test");
        assert_eq!(writer.addr, "test");
    }

    #[tokio::test]
    async fn writer_send_reader_recv_roundtrip() {
        let (server, client) = tcp_pair().await;
        let (_server_reader, server_writer) = split_connection(server, "server".into(), None);
        let (mut client_reader, _client_writer) = split_connection(client, "client".into(), None);

        let msg = Message::Ping(42);
        server_writer.send(&msg).await.unwrap();

        let received = client_reader.recv().await.unwrap();
        match received {
            Message::Ping(n) => assert_eq!(n, 42),
            other => panic!("expected Ping(42), got {:?}", other),
        }
    }

    #[tokio::test]
    async fn multiple_messages_roundtrip() {
        let (server, client) = tcp_pair().await;
        let (_, server_writer) = split_connection(server, "s".into(), None);
        let (mut client_reader, _) = split_connection(client, "c".into(), None);

        server_writer.send(&Message::Ping(1)).await.unwrap();
        server_writer.send(&Message::Pong(2)).await.unwrap();
        server_writer.send(&Message::GetPeers).await.unwrap();

        match client_reader.recv().await.unwrap() {
            Message::Ping(1) => {}
            other => panic!("expected Ping(1), got {:?}", other),
        }
        match client_reader.recv().await.unwrap() {
            Message::Pong(2) => {}
            other => panic!("expected Pong(2), got {:?}", other),
        }
        match client_reader.recv().await.unwrap() {
            Message::GetPeers => {}
            other => panic!("expected GetPeers, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn recv_eof_returns_error() {
        let (server, client) = tcp_pair().await;
        let (mut server_reader, _) = split_connection(server, "s".into(), None);
        drop(client);

        let result = server_reader.recv().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn encrypted_roundtrip() {
        let (mut server_stream, mut client_stream) = tcp_pair().await;

        // Perform Noise handshake
        let (server_hs, client_hs) = tokio::join!(
            super::super::noise::handshake_responder(&mut server_stream, None),
            super::super::noise::handshake_initiator(&mut client_stream, None),
        );
        let server_transport = server_hs.unwrap().transport;
        let client_transport = client_hs.unwrap().transport;

        // Split into encrypted reader/writer
        let (mut server_reader, _server_writer) = split_connection(
            server_stream, "server".into(), Some(server_transport.clone()),
        );
        let (_client_reader, client_writer) = split_connection(
            client_stream, "client".into(), Some(client_transport.clone()),
        );

        // Client sends encrypted, server receives encrypted
        let msg = Message::Ping(99);
        client_writer.send(&msg).await.unwrap();

        let received = server_reader.recv().await.unwrap();
        match received {
            Message::Ping(n) => assert_eq!(n, 99),
            other => panic!("expected Ping(99), got {:?}", other),
        }
    }

    #[tokio::test]
    async fn encrypted_multiple_messages() {
        let (mut server_stream, mut client_stream) = tcp_pair().await;

        let (server_hs, client_hs) = tokio::join!(
            super::super::noise::handshake_responder(&mut server_stream, None),
            super::super::noise::handshake_initiator(&mut client_stream, None),
        );
        let server_transport = server_hs.unwrap().transport;
        let client_transport = client_hs.unwrap().transport;

        let (mut server_reader, _) = split_connection(
            server_stream, "server".into(), Some(server_transport.clone()),
        );
        let (_, client_writer) = split_connection(
            client_stream, "client".into(), Some(client_transport.clone()),
        );

        // Send multiple messages
        client_writer.send(&Message::Ping(1)).await.unwrap();
        client_writer.send(&Message::Pong(2)).await.unwrap();
        client_writer.send(&Message::GetPeers).await.unwrap();

        match server_reader.recv().await.unwrap() {
            Message::Ping(1) => {}
            other => panic!("expected Ping(1), got {:?}", other),
        }
        match server_reader.recv().await.unwrap() {
            Message::Pong(2) => {}
            other => panic!("expected Pong(2), got {:?}", other),
        }
        match server_reader.recv().await.unwrap() {
            Message::GetPeers => {}
            other => panic!("expected GetPeers, got {:?}", other),
        }
    }
}
