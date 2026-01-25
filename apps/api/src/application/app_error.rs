use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Database error: {0}")]
    Database(String),

    #[error("Too many requests. Please slow down.")]
    RateLimited,

    #[error("Invalid credentials")]
    InvalidCredentials,

    #[error("Invalid API key")]
    InvalidApiKey,

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("OAuth retry window expired")]
    OAuthRetryExpired,

    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("Account suspended")]
    AccountSuspended,

    #[error("Too many documents")]
    TooManyDocuments,

    #[error("Too many API keys for this domain (max 5)")]
    TooManyApiKeys,

    #[error("No API key configured for this domain")]
    NoApiKeyConfigured,

    #[error("Not found")]
    NotFound,

    #[error("Forbidden")]
    Forbidden,

    #[error("Session mismatch - please use the same browser/device where you requested the link")]
    SessionMismatch,

    #[error("Payment declined: {0}")]
    PaymentDeclined(String),

    #[error("Payment provider not configured")]
    ProviderNotConfigured,

    #[error("Payment provider not supported")]
    ProviderNotSupported,

    #[error("Internal error: {0}")]
    Internal(String),
}

#[derive(Clone, Copy, Debug)]
pub enum ErrorCode {
    DatabaseError,
    InvalidCredentials,
    InvalidApiKey,
    RateLimited,
    InvalidInput,
    OAuthRetryExpired,
    ValidationError,
    AccountSuspended,
    TooManyDocuments,
    TooManyApiKeys,
    NoApiKeyConfigured,
    NotFound,
    Forbidden,
    SessionMismatch,
    PaymentDeclined,
    ProviderNotConfigured,
    ProviderNotSupported,
    InternalError,
}

impl ErrorCode {
    pub fn as_str(&self) -> &'static str {
        match self {
            ErrorCode::DatabaseError => "DATABASE_ERROR",
            ErrorCode::InvalidCredentials => "INVALID_CREDENTIALS",
            ErrorCode::InvalidApiKey => "INVALID_API_KEY",
            ErrorCode::RateLimited => "RATE_LIMITED",
            ErrorCode::InvalidInput => "INVALID_INPUT",
            ErrorCode::OAuthRetryExpired => "OAUTH_RETRY_EXPIRED",
            ErrorCode::ValidationError => "VALIDATION_ERROR",
            ErrorCode::AccountSuspended => "ACCOUNT_SUSPENDED",
            ErrorCode::TooManyDocuments => "TOO_MANY_DOCUMENTS",
            ErrorCode::TooManyApiKeys => "TOO_MANY_API_KEYS",
            ErrorCode::NoApiKeyConfigured => "NO_API_KEY_CONFIGURED",
            ErrorCode::NotFound => "NOT_FOUND",
            ErrorCode::Forbidden => "FORBIDDEN",
            ErrorCode::SessionMismatch => "SESSION_MISMATCH",
            ErrorCode::PaymentDeclined => "PAYMENT_DECLINED",
            ErrorCode::ProviderNotConfigured => "PROVIDER_NOT_CONFIGURED",
            ErrorCode::ProviderNotSupported => "PROVIDER_NOT_SUPPORTED",
            ErrorCode::InternalError => "INTERNAL_ERROR",
        }
    }
}

pub type AppResult<T> = Result<T, AppError>;
