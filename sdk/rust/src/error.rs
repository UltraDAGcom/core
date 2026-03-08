/// Errors that can occur when interacting with the UltraDAG SDK.
#[derive(Debug, thiserror::Error)]
pub enum UltraDagError {
    /// An HTTP transport error occurred (connection refused, timeout, etc.).
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// The node returned a non-success status code with a message.
    #[error("API error ({status}): {message}")]
    Api {
        /// HTTP status code.
        status: u16,
        /// Error message from the node.
        message: String,
    },

    /// Failed to parse JSON from the node response.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// The provided address string is not valid hex or has the wrong length.
    #[error("Invalid address: {0}")]
    InvalidAddress(String),

    /// The provided secret key is not valid hex or has the wrong length.
    #[error("Invalid key: {0}")]
    InvalidKey(String),
}

/// Convenience alias used throughout the SDK.
pub type Result<T> = std::result::Result<T, UltraDagError>;
