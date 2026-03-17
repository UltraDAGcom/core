use tokio::net::TcpStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use std::time::Duration;

/// A raw TCP client for attacking nodes.
/// No Noise encryption — tests pre-handshake and raw protocol behavior.
pub struct RawAttacker {
    stream: TcpStream,
}

impl RawAttacker {
    /// Connect to a node via raw TCP.
    pub async fn connect(addr: &str) -> std::io::Result<Self> {
        let stream = tokio::time::timeout(
            Duration::from_secs(5),
            TcpStream::connect(addr),
        ).await.map_err(|_| std::io::Error::new(std::io::ErrorKind::TimedOut, "connect timeout"))??;
        Ok(Self { stream })
    }

    /// Send raw bytes.
    pub async fn send_bytes(&mut self, data: &[u8]) -> std::io::Result<()> {
        self.stream.write_all(data).await
    }

    /// Send a length-prefixed message (4-byte BE length + data).
    pub async fn send_length_prefixed(&mut self, data: &[u8]) -> std::io::Result<()> {
        let len = (data.len() as u32).to_be_bytes();
        self.stream.write_all(&len).await?;
        self.stream.write_all(data).await
    }

    /// Try to read some bytes with timeout. Returns empty vec on timeout/close.
    pub async fn try_read(&mut self, timeout_ms: u64) -> Vec<u8> {
        let mut buf = vec![0u8; 4096];
        match tokio::time::timeout(
            Duration::from_millis(timeout_ms),
            self.stream.read(&mut buf),
        ).await {
            Ok(Ok(n)) if n > 0 => buf[..n].to_vec(),
            _ => vec![],
        }
    }

    /// Check if connection is still alive by attempting a zero-byte write.
    pub async fn is_connected(&mut self) -> bool {
        // Try to read with a very short timeout
        let mut buf = [0u8; 1];
        match tokio::time::timeout(
            Duration::from_millis(100),
            self.stream.peek(&mut buf),
        ).await {
            Ok(Ok(0)) => false, // EOF = closed
            Ok(Err(_)) => false, // Error = closed
            Err(_) => true, // Timeout = still open (no data but alive)
            Ok(Ok(_)) => true, // Data available = alive
        }
    }
}
