use serde::{Deserialize, Serialize};
use thiserror::Error;

/// API error codes returned by Reauth endpoints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    InvalidCredentials,
    InvalidApiKey,
    InvalidInput,
    NotFound,
    Forbidden,
    AccountSuspended,
    RateLimited,
    InternalError,
}

impl std::fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::InvalidCredentials => "INVALID_CREDENTIALS",
            Self::InvalidApiKey => "INVALID_API_KEY",
            Self::InvalidInput => "INVALID_INPUT",
            Self::NotFound => "NOT_FOUND",
            Self::Forbidden => "FORBIDDEN",
            Self::AccountSuspended => "ACCOUNT_SUSPENDED",
            Self::RateLimited => "RATE_LIMITED",
            Self::InternalError => "INTERNAL_ERROR",
        };
        write!(f, "{}", s)
    }
}

/// JWT verification errors.
#[derive(Debug, Error)]
pub enum JwtError {
    #[error("Invalid token format: {0}")]
    InvalidFormat(String),

    #[error("Token has expired")]
    Expired,

    #[error("Invalid signature")]
    InvalidSignature,

    #[error("Invalid claims: {0}")]
    InvalidClaims(String),

    #[error("Missing required claim: {0}")]
    MissingClaim(String),

    #[error("JWT library error: {0}")]
    Library(#[from] jsonwebtoken::errors::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_code_serde() {
        let code = ErrorCode::InvalidCredentials;
        let json = serde_json::to_string(&code).unwrap();
        assert_eq!(json, r#""INVALID_CREDENTIALS""#);

        let parsed: ErrorCode = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, code);
    }
}
