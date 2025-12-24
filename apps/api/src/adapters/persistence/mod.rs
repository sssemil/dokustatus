use sqlx::PgPool;

use crate::app_error::AppError;

pub mod domain;
pub mod domain_auth_config;
pub mod domain_auth_magic_link;
pub mod domain_end_user;
pub mod user;

#[derive(Clone)]
pub struct PostgresPersistence {
    pool: PgPool,
}

impl PostgresPersistence {
    pub fn new(pool: PgPool) -> Self {
        PostgresPersistence { pool }
    }
}

impl From<sqlx::Error> for AppError {
    fn from(value: sqlx::Error) -> Self {
        AppError::Database(value.to_string())
    }
}
