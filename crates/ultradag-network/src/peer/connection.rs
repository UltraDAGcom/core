use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::sync::Mutex;
use std::sync::Arc;

use crate::protocol::{Message, MAX_MESSAGE_SIZE};

/// Read half of a peer connection.
pub struct PeerReader {
    reader: OwnedReadHalf,
    pub addr: String,
}

/// Write half of a peer connection, wrapped in a Mutex for shared access.
#[derive(Clone)]
pub struct PeerWriter {
    writer: Arc<Mutex<OwnedWriteHalf>>,
    pub addr: String,
}

impl PeerReader {
    pub fn new(reader: OwnedReadHalf, addr: String) -> Self {
        Self { reader, addr }
    }

    /// Receive a message from this peer.
    pub async fn recv(&mut self) -> std::io::Result<Message> {
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

        Message::decode(&body).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }
}

impl PeerWriter {
    pub fn new(writer: OwnedWriteHalf, addr: String) -> Self {
        Self {
            writer: Arc::new(Mutex::new(writer)),
            addr,
        }
    }

    /// Send raw bytes with a 4-byte length prefix (for testing malformed/oversized messages).
    pub async fn send_raw(&self, data: &[u8]) -> std::io::Result<()> {
        let len = (data.len() as u32).to_be_bytes();
        let mut writer = self.writer.lock().await;
        writer.write_all(&len).await?;
        writer.write_all(data).await?;
        writer.flush().await?;
        Ok(())
    }

    /// Send a length prefix only (for testing oversized message rejection).
    pub async fn send_raw_len(&self, len: u32) -> std::io::Result<()> {
        let len_bytes = len.to_be_bytes();
        let mut writer = self.writer.lock().await;
        writer.write_all(&len_bytes).await?;
        writer.flush().await?;
        Ok(())
    }

    /// Send a message to this peer.
    pub async fn send(&self, msg: &Message) -> std::io::Result<()> {
        let data = msg.encode().map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        let mut writer = self.writer.lock().await;
        writer.write_all(&data).await?;
        writer.flush().await?;
        Ok(())
    }
}

/// Split a TcpStream into reader and writer halves.
pub fn split_connection(
    stream: tokio::net::TcpStream,
    addr: String,
) -> (PeerReader, PeerWriter) {
    let (read_half, write_half) = stream.into_split();
    (
        PeerReader::new(read_half, addr.clone()),
        PeerWriter::new(write_half, addr),
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
        let (reader, writer) = split_connection(server, "test".into());
        assert_eq!(reader.addr, "test");
        assert_eq!(writer.addr, "test");
    }

    #[tokio::test]
    async fn writer_send_reader_recv_roundtrip() {
        let (server, client) = tcp_pair().await;
        // Server side: writer sends from server to client
        let (_server_reader, server_writer) = split_connection(server, "server".into());
        // Client side: reader receives
        let (mut client_reader, _client_writer) = split_connection(client, "client".into());

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
        let (_, server_writer) = split_connection(server, "s".into());
        let (mut client_reader, _) = split_connection(client, "c".into());

        // Send several messages in sequence
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
        let (mut server_reader, _) = split_connection(server, "s".into());
        // Drop the client immediately so server sees EOF
        drop(client);

        let result = server_reader.recv().await;
        assert!(result.is_err());
    }
}
