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

    #[error("Too many documents")]
    TooManyDocuments,

    #[error("Not found")]
    NotFound,

    #[error("Internal error: {0}")]
    Internal(String),
}

#[derive(Clone, Copy, Debug)]
pub enum ErrorCode {
    DatabaseError,
    InvalidCredentials,
    RateLimited,
    InvalidInput,
    TooManyDocuments,
    NotFound,
    InternalError,
}

impl ErrorCode {
    pub fn as_str(&self) -> &'static str {
        match self {
            ErrorCode::DatabaseError => "DATABASE_ERROR",
            ErrorCode::InvalidCredentials => "INVALID_CREDENTIALS",
            ErrorCode::RateLimited => "RATE_LIMITED",
            ErrorCode::InvalidInput => "INVALID_INPUT",
            ErrorCode::TooManyDocuments => "TOO_MANY_DOCUMENTS",
            ErrorCode::NotFound => "NOT_FOUND",
            ErrorCode::InternalError => "INTERNAL_ERROR",
        }
    }
}

pub type AppResult<T> = Result<T, AppError>;
