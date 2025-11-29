use sqlx::PgPool;

use crate::app_error::AppError;
use crate::infra::crypto::ProcessCipher;
use std::sync::Arc;

pub mod pass_status;
pub mod user;

#[derive(Clone)]
pub struct PostgresPersistence {
    pool: PgPool,
    cipher: Arc<ProcessCipher>,
}

impl PostgresPersistence {
    pub fn new(pool: PgPool, cipher: Arc<ProcessCipher>) -> Self {
        PostgresPersistence { pool, cipher }
    }
}

impl From<sqlx::Error> for AppError {
    fn from(value: sqlx::Error) -> Self {
        AppError::Database(value.to_string())
    }
}
