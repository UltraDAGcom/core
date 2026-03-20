use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tracing::{debug, warn};
use ultradag_coin::SecretKey;

/// Noise protocol pattern: XX for mutual authentication without prior key
/// knowledge, 25519 for X25519 DH, ChaChaPoly for AEAD, BLAKE2s for hashing.
const NOISE_PATTERN: &str = "Noise_XX_25519_ChaChaPoly_BLAKE2s";

/// Maximum Noise message size (Noise spec limit).
const NOISE_MAX_MSG: usize = 65535;

/// AEAD tag overhead per Noise message (Poly1305).
pub const NOISE_TAG_LEN: usize = 16;

/// Maximum plaintext bytes per Noise transport message.
pub const NOISE_MAX_PLAINTEXT: usize = NOISE_MAX_MSG - NOISE_TAG_LEN;

/// Handshake message size limit.
const MAX_HANDSHAKE_MSG: usize = 4096;

/// Handshake timeout in seconds.
pub const HANDSHAKE_TIMEOUT_SECS: u64 = 10;

/// Errors during Noise handshake.
#[derive(Debug)]
pub enum NoiseError {
    Snow(snow::Error),
    Io(std::io::Error),
    InvalidIdentity,
    SignatureVerificationFailed,
    MissingRemoteStatic,
    HandshakeMessageTooLarge,
}

impl std::fmt::Display for NoiseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Snow(e) => write!(f, "noise: {}", e),
            Self::Io(e) => write!(f, "io: {}", e),
            Self::InvalidIdentity => write!(f, "invalid identity payload"),
            Self::SignatureVerificationFailed => write!(f, "identity signature verification failed"),
            Self::MissingRemoteStatic => write!(f, "missing remote static key"),
            Self::HandshakeMessageTooLarge => write!(f, "handshake message too large"),
        }
    }
}

impl From<std::io::Error> for NoiseError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

/// Peer's authenticated identity after Noise handshake.
#[derive(Debug, Clone)]
pub struct PeerIdentity {
    /// Ed25519 public key of the peer (validator identity).
    pub ed25519_pubkey: [u8; 32],
    /// UltraDAG address derived from the public key.
    pub address: ultradag_coin::Address,
}

/// Result of a successful Noise handshake.
pub struct HandshakeResult {
    /// Noise transport state for encrypting/decrypting messages.
    pub transport: Arc<Mutex<snow::TransportState>>,
    /// Peer's validator identity (None if peer is an observer).
    pub peer_identity: Option<PeerIdentity>,
}

/// Build identity payload for Noise handshake.
///
/// Format: `[1 byte: has_identity] [32 bytes: ed25519_pubkey] [64 bytes: signature]`
///
/// The signature covers `NETWORK_ID || b"noise-identity" || noise_static_pubkey`,
/// binding the validator's Ed25519 identity to this Noise session.
fn build_identity_payload(identity: Option<&SecretKey>, noise_static_pubkey: &[u8]) -> Vec<u8> {
    match identity {
        Some(sk) => {
            let mut signable = Vec::with_capacity(64);
            signable.extend_from_slice(ultradag_coin::constants::NETWORK_ID);
            signable.extend_from_slice(b"noise-identity");
            signable.extend_from_slice(noise_static_pubkey);
            let sig = sk.sign(&signable);
            let pubkey = sk.verifying_key().to_bytes();

            let mut payload = Vec::with_capacity(1 + 32 + 64);
            payload.push(0x01);
            payload.extend_from_slice(&pubkey);
            payload.extend_from_slice(&sig.0);
            payload
        }
        None => vec![0x00],
    }
}

/// Parse identity payload from a Noise handshake message.
fn parse_identity_payload(payload: &[u8]) -> Result<Option<PeerIdentity>, NoiseError> {
    if payload.is_empty() {
        return Err(NoiseError::InvalidIdentity);
    }
    match payload[0] {
        0x00 => Ok(None),
        0x01 => {
            if payload.len() != 1 + 32 + 64 {
                return Err(NoiseError::InvalidIdentity);
            }
            let mut pubkey = [0u8; 32];
            pubkey.copy_from_slice(&payload[1..33]);
            let address = ultradag_coin::Address::from_pubkey(&pubkey);
            Ok(Some(PeerIdentity {
                ed25519_pubkey: pubkey,
                address,
            }))
        }
        _ => Err(NoiseError::InvalidIdentity),
    }
}

/// Verify that the peer's Ed25519 signature binds their identity to their Noise static key.
fn verify_identity_signature(
    identity: &PeerIdentity,
    noise_static_pubkey: &[u8],
    signature_bytes: &[u8],
) -> Result<(), NoiseError> {
    let mut signable = Vec::with_capacity(64);
    signable.extend_from_slice(ultradag_coin::constants::NETWORK_ID);
    signable.extend_from_slice(b"noise-identity");
    signable.extend_from_slice(noise_static_pubkey);

    let mut sig_arr = [0u8; 64];
    if signature_bytes.len() != 64 {
        return Err(NoiseError::SignatureVerificationFailed);
    }
    sig_arr.copy_from_slice(signature_bytes);

    let sig = ultradag_coin::Signature(sig_arr);
    if !sig.verify_with_pubkey_bytes(&identity.ed25519_pubkey, &signable) {
        warn!("Noise identity signature verification failed for peer {}", identity.address.short());
        return Err(NoiseError::SignatureVerificationFailed);
    }

    Ok(())
}

/// Send a handshake message with 2-byte length prefix.
async fn send_handshake_msg(stream: &mut TcpStream, data: &[u8]) -> Result<(), NoiseError> {
    if data.len() > MAX_HANDSHAKE_MSG {
        return Err(NoiseError::HandshakeMessageTooLarge);
    }
    let len = (data.len() as u16).to_be_bytes();
    stream.write_all(&len).await?;
    stream.write_all(data).await?;
    stream.flush().await?;
    Ok(())
}

/// Receive a handshake message with 2-byte length prefix.
async fn recv_handshake_msg(stream: &mut TcpStream) -> Result<Vec<u8>, NoiseError> {
    let mut len_buf = [0u8; 2];
    stream.read_exact(&mut len_buf).await?;
    let len = u16::from_be_bytes(len_buf) as usize;
    if len > MAX_HANDSHAKE_MSG {
        return Err(NoiseError::HandshakeMessageTooLarge);
    }
    let mut buf = vec![0u8; len];
    stream.read_exact(&mut buf).await?;
    Ok(buf)
}

/// Perform Noise_XX handshake as the initiator (outgoing connection).
///
/// Generates an ephemeral X25519 keypair for this connection (forward secrecy).
/// If `identity` is provided, signs the Noise static pubkey with the validator's
/// Ed25519 key, proving validator identity to the peer.
pub async fn handshake_initiator(
    stream: &mut TcpStream,
    identity: Option<&SecretKey>,
) -> Result<HandshakeResult, NoiseError> {
    let params = NOISE_PATTERN.parse().map_err(NoiseError::Snow)?;
    let builder = snow::Builder::new(params);
    let noise_keypair = builder.generate_keypair().map_err(NoiseError::Snow)?;
    let mut noise = builder
        .local_private_key(&noise_keypair.private)
        .build_initiator()
        .map_err(NoiseError::Snow)?;

    // Message 1: -> e (empty payload)
    let mut buf = vec![0u8; NOISE_MAX_MSG];
    let len = noise.write_message(&[], &mut buf).map_err(NoiseError::Snow)?;
    send_handshake_msg(stream, &buf[..len]).await?;

    // Message 2: <- e, ee, s, es + responder identity payload
    let msg2 = recv_handshake_msg(stream).await?;
    let mut payload_buf = vec![0u8; MAX_HANDSHAKE_MSG];
    let payload_len = noise
        .read_message(&msg2, &mut payload_buf)
        .map_err(NoiseError::Snow)?;
    let peer_identity = parse_identity_payload(&payload_buf[..payload_len])?;

    // Verify responder's identity signature against their Noise static key
    if let Some(ref id) = peer_identity {
        let remote_static = noise
            .get_remote_static()
            .ok_or(NoiseError::MissingRemoteStatic)?;
        let sig_bytes = &payload_buf[33..payload_len]; // skip 0x01 + 32-byte pubkey
        verify_identity_signature(id, remote_static, sig_bytes)?;
    }

    // Message 3: -> s, se + our identity payload
    let our_payload = build_identity_payload(identity, &noise_keypair.public);
    let len = noise
        .write_message(&our_payload, &mut buf)
        .map_err(NoiseError::Snow)?;
    send_handshake_msg(stream, &buf[..len]).await?;

    let transport = noise.into_transport_mode().map_err(NoiseError::Snow)?;

    debug!("Noise handshake complete (initiator)");

    Ok(HandshakeResult {
        transport: Arc::new(Mutex::new(transport)),
        peer_identity,
    })
}

/// Perform Noise_XX handshake as the responder (incoming connection).
///
/// Generates an ephemeral X25519 keypair for this connection (forward secrecy).
/// If `identity` is provided, signs the Noise static pubkey with the validator's
/// Ed25519 key, proving validator identity to the peer.
pub async fn handshake_responder(
    stream: &mut TcpStream,
    identity: Option<&SecretKey>,
) -> Result<HandshakeResult, NoiseError> {
    let params = NOISE_PATTERN.parse().map_err(NoiseError::Snow)?;
    let builder = snow::Builder::new(params);
    let noise_keypair = builder.generate_keypair().map_err(NoiseError::Snow)?;
    let mut noise = builder
        .local_private_key(&noise_keypair.private)
        .build_responder()
        .map_err(NoiseError::Snow)?;

    // Message 1: <- e
    let msg1 = recv_handshake_msg(stream).await?;
    let mut payload_buf = vec![0u8; MAX_HANDSHAKE_MSG];
    noise
        .read_message(&msg1, &mut payload_buf)
        .map_err(NoiseError::Snow)?;

    // Message 2: -> e, ee, s, es + our identity payload
    let our_payload = build_identity_payload(identity, &noise_keypair.public);
    let mut buf = vec![0u8; NOISE_MAX_MSG];
    let len = noise
        .write_message(&our_payload, &mut buf)
        .map_err(NoiseError::Snow)?;
    send_handshake_msg(stream, &buf[..len]).await?;

    // Message 3: <- s, se + initiator identity payload
    let msg3 = recv_handshake_msg(stream).await?;
    let payload_len = noise
        .read_message(&msg3, &mut payload_buf)
        .map_err(NoiseError::Snow)?;
    let peer_identity = parse_identity_payload(&payload_buf[..payload_len])?;

    // Verify initiator's identity signature against their Noise static key
    if let Some(ref id) = peer_identity {
        let remote_static = noise
            .get_remote_static()
            .ok_or(NoiseError::MissingRemoteStatic)?;
        let sig_bytes = &payload_buf[33..payload_len]; // skip 0x01 + 32-byte pubkey
        verify_identity_signature(id, remote_static, sig_bytes)?;
    }

    let transport = noise.into_transport_mode().map_err(NoiseError::Snow)?;

    debug!("Noise handshake complete (responder)");

    Ok(HandshakeResult {
        transport: Arc::new(Mutex::new(transport)),
        peer_identity,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    async fn tcp_pair() -> (TcpStream, TcpStream) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let client = tokio::spawn(async move {
            TcpStream::connect(addr).await.unwrap()
        });
        let (server, _) = listener.accept().await.unwrap();
        let client = client.await.unwrap();
        (server, client)
    }

    #[tokio::test]
    async fn handshake_without_identity() {
        let (mut server, mut client) = tcp_pair().await;

        let (server_result, client_result) = tokio::join!(
            handshake_responder(&mut server, None),
            handshake_initiator(&mut client, None),
        );

        let server_hs = server_result.unwrap();
        let client_hs = client_result.unwrap();

        assert!(server_hs.peer_identity.is_none());
        assert!(client_hs.peer_identity.is_none());
    }

    #[tokio::test]
    async fn handshake_with_identity() {
        let (mut server, mut client) = tcp_pair().await;

        let server_key = SecretKey::generate();
        let client_key = SecretKey::generate();

        let (server_result, client_result) = tokio::join!(
            handshake_responder(&mut server, Some(&server_key)),
            handshake_initiator(&mut client, Some(&client_key)),
        );

        let server_hs = server_result.unwrap();
        let client_hs = client_result.unwrap();

        // Server sees client's identity
        let server_peer = server_hs.peer_identity.unwrap();
        assert_eq!(server_peer.address, client_key.address());

        // Client sees server's identity
        let client_peer = client_hs.peer_identity.unwrap();
        assert_eq!(client_peer.address, server_key.address());
    }

    #[tokio::test]
    async fn handshake_mixed_identity() {
        let (mut server, mut client) = tcp_pair().await;

        let server_key = SecretKey::generate();

        let (server_result, client_result) = tokio::join!(
            handshake_responder(&mut server, Some(&server_key)),
            handshake_initiator(&mut client, None), // observer
        );

        let server_hs = server_result.unwrap();
        let client_hs = client_result.unwrap();

        // Server sees no client identity (observer)
        assert!(server_hs.peer_identity.is_none());

        // Client sees server's identity
        let client_peer = client_hs.peer_identity.unwrap();
        assert_eq!(client_peer.address, server_key.address());
    }

    #[tokio::test]
    async fn handshake_fails_gracefully_on_immediate_close() {
        // Initiator side: peer closes connection immediately after connect.
        // Handshake should return an error (Io or Snow), not panic.
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server_handle = tokio::spawn(async move {
            let (server_stream, _) = listener.accept().await.unwrap();
            // Drop the server stream immediately to simulate a connection that closes
            drop(server_stream);
        });

        let mut client = TcpStream::connect(addr).await.unwrap();
        let result = handshake_initiator(&mut client, None).await;

        // Must be an error, not a panic
        assert!(result.is_err(), "Handshake should fail when peer closes connection");

        server_handle.await.unwrap();
    }

    #[tokio::test]
    async fn handshake_fails_gracefully_on_garbage_data() {
        // Responder receives garbage instead of a valid Noise handshake message.
        // Should return an error, not panic.
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let client_handle = tokio::spawn(async move {
            let mut client = TcpStream::connect(addr).await.unwrap();
            // Send garbage: a 2-byte length prefix followed by random bytes
            let garbage = b"\x00\x10GARBAGE_DATA_HERE";
            client.write_all(garbage).await.unwrap();
            client.flush().await.unwrap();
            // Keep connection alive briefly so responder can read
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        });

        let (mut server, _) = listener.accept().await.unwrap();
        let result = handshake_responder(&mut server, None).await;

        // Must be an error (Snow protocol error on invalid handshake), not a panic
        assert!(result.is_err(), "Handshake should fail on garbage data");

        client_handle.await.unwrap();
    }

    #[tokio::test]
    async fn transport_encrypt_decrypt_roundtrip() {
        let (mut server, mut client) = tcp_pair().await;

        let (server_result, client_result) = tokio::join!(
            handshake_responder(&mut server, None),
            handshake_initiator(&mut client, None),
        );

        let server_hs = server_result.unwrap();
        let client_hs = client_result.unwrap();

        // Client encrypts, server decrypts
        let plaintext = b"hello ultradag";
        let mut ciphertext = vec![0u8; plaintext.len() + NOISE_TAG_LEN];
        let len = {
            let mut t = client_hs.transport.lock().await;
            t.write_message(plaintext, &mut ciphertext).unwrap()
        };

        let mut decrypted = vec![0u8; len];
        let dec_len = {
            let mut t = server_hs.transport.lock().await;
            t.read_message(&ciphertext[..len], &mut decrypted).unwrap()
        };

        assert_eq!(&decrypted[..dec_len], plaintext);
    }
}
