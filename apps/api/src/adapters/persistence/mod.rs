use sqlx::PgPool;

use crate::app_error::AppError;

pub mod api_key;
pub mod billing_stripe_config;
pub mod domain;
pub mod domain_auth_config;
pub mod domain_auth_google_oauth;
pub mod domain_auth_magic_link;
pub mod domain_end_user;
pub mod domain_role;
pub mod subscription_event;
pub mod subscription_plan;
pub mod user_subscription;

#[derive(Clone)]
pub struct PostgresPersistence {
    pool: PgPool,
}

impl PostgresPersistence {
    pub fn new(pool: PgPool) -> Self {
        PostgresPersistence { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}

impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        match &err {
            sqlx::Error::RowNotFound => AppError::NotFound,
            sqlx::Error::Database(db_err) => {
                let msg = db_err.message();
                // PostgreSQL unique violation
                if msg.contains("duplicate key") || msg.contains("unique constraint") {
                    AppError::InvalidInput("A record with this value already exists".into())
                }
                // PostgreSQL foreign key violation
                else if msg.contains("foreign key") || msg.contains("violates foreign key") {
                    AppError::InvalidInput("Referenced record not found".into())
                }
                // PostgreSQL not-null violation
                else if msg.contains("null value") && msg.contains("violates not-null") {
                    AppError::InvalidInput("Required field is missing".into())
                } else {
                    // Log the actual error for debugging, but don't expose details
                    tracing::error!(error = ?err, "Database error");
                    AppError::Database("Database operation failed".into())
                }
            }
            _ => {
                tracing::error!(error = ?err, "Database error");
                AppError::Database("Database operation failed".into())
            }
        }
    }
}
