use thiserror::Error;

use crate::app_error::AppError;

/// Infrastructure errors that can occur during application startup.
///
/// SECURITY: Display messages are sanitized and safe for logs/console output.
/// Debug output includes the full #[source] error chain which may contain
/// secrets (e.g., connection strings) - use Display (%e) not Debug (?e) in logs.
#[derive(Error, Debug)]
pub enum InfraError {
    #[error("Database connection failed. Check DATABASE_URL and ensure the database is running.")]
    DatabaseConnection(#[source] sqlx::Error),

    #[error("Redis connection failed. Check REDIS_URL and credentials.")]
    RedisConnection(#[source] redis::RedisError),

    #[error("Configuration error: environment variable {var} not set")]
    ConfigMissing { var: &'static str },

    #[error("Cipher initialization failed")]
    CipherInit(#[source] AppError),

    #[error("TCP bind failed")]
    TcpBind(#[source] std::io::Error),

    #[error("Server error")]
    Server(#[source] std::io::Error),
}

impl From<sqlx::Error> for InfraError {
    fn from(e: sqlx::Error) -> Self {
        InfraError::DatabaseConnection(e)
    }
}
