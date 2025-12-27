use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Database error: {0}")]
    Database(String),

    #[error("Too many requests. Please slow down.")]
    RateLimited,

    #[error("Invalid credentials")]
    InvalidCredentials,

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Account suspended")]
    AccountSuspended,

    #[error("Too many documents")]
    TooManyDocuments,

    #[error("Not found")]
    NotFound,

    #[error("Session mismatch - please use the same browser/device where you requested the link")]
    SessionMismatch,

    #[error("Internal error: {0}")]
    Internal(String),
}

#[derive(Clone, Copy, Debug)]
pub enum ErrorCode {
    DatabaseError,
    InvalidCredentials,
    RateLimited,
    InvalidInput,
    AccountSuspended,
    TooManyDocuments,
    NotFound,
    SessionMismatch,
    InternalError,
}

impl ErrorCode {
    pub fn as_str(&self) -> &'static str {
        match self {
            ErrorCode::DatabaseError => "DATABASE_ERROR",
            ErrorCode::InvalidCredentials => "INVALID_CREDENTIALS",
            ErrorCode::RateLimited => "RATE_LIMITED",
            ErrorCode::InvalidInput => "INVALID_INPUT",
            ErrorCode::AccountSuspended => "ACCOUNT_SUSPENDED",
            ErrorCode::TooManyDocuments => "TOO_MANY_DOCUMENTS",
            ErrorCode::NotFound => "NOT_FOUND",
            ErrorCode::SessionMismatch => "SESSION_MISMATCH",
            ErrorCode::InternalError => "INTERNAL_ERROR",
        }
    }
}

pub type AppResult<T> = Result<T, AppError>;
