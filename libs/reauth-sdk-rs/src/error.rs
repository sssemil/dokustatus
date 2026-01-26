use reauth_types::{ErrorCode, JwtError};
use thiserror::Error;

/// SDK-specific errors.
#[derive(Debug, Error)]
pub enum ReauthError {
    /// Token is invalid (malformed, wrong format, etc.)
    #[error("Invalid token: {0}")]
    InvalidToken(String),

    /// JWT verification failed
    #[error("JWT error: {0}")]
    Jwt(#[from] JwtError),

    /// Domain in token doesn't match configured domain
    #[error("Domain mismatch: expected {expected}, got {actual}")]
    DomainMismatch { expected: String, actual: String },

    /// API returned an error
    #[error("API error: {code} - {message}")]
    ApiError { code: ErrorCode, message: String },

    /// Network error (only with `client` feature)
    #[cfg(feature = "client")]
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    /// Configuration error
    #[error("Configuration error: {0}")]
    Config(String),
}
