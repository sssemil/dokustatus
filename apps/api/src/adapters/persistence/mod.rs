use sqlx::PgPool;

use crate::app_error::AppError;

const MAX_JSON_LOG_LEN: usize = 200;

/// Parse JSON value to target type, logging warning on failure.
///
/// Handles NULL gracefully (returns default without logging).
/// Only logs warnings for actual parse failures (type mismatches, corruption).
///
/// # Arguments
/// * `json` - The JSON value to parse (may be Value::Null for SQL NULL)
/// * `field_name` - Name of the field being parsed (for logging)
/// * `entity_type` - Type of entity (e.g., "domain_end_user", "subscription_plan")
/// * `entity_id` - ID of the entity (for log filtering)
pub fn parse_json_with_fallback<T: serde::de::DeserializeOwned + Default>(
    json: &serde_json::Value,
    field_name: &str,
    entity_type: &str,
    entity_id: &str,
) -> T {
    // SQL NULL becomes Value::Null - treat as valid empty state, no warning
    if json.is_null() {
        return T::default();
    }

    serde_json::from_value(json.clone()).unwrap_or_else(|err| {
        // Truncate raw JSON to prevent log bloat from large arrays
        let raw_str = json.to_string();
        let truncated = if raw_str.len() > MAX_JSON_LOG_LEN {
            format!("{}...", &raw_str[..MAX_JSON_LOG_LEN])
        } else {
            raw_str
        };

        tracing::warn!(
            field = field_name,
            entity_type = entity_type,
            entity_id = entity_id,
            raw_json = %truncated,
            error = %err,
            "Failed to parse JSON field, using default value"
        );
        T::default()
    })
}

pub mod api_key;
pub mod billing_payment;
pub mod billing_stripe_config;
pub mod domain;
pub mod domain_auth_config;
pub mod domain_auth_google_oauth;
pub mod domain_auth_magic_link;
pub mod domain_end_user;
pub mod domain_role;
pub mod enabled_payment_providers;
pub mod subscription_event;
pub mod subscription_plan;
pub mod user_subscription;
pub mod webhook_delivery;
pub mod webhook_endpoint;
pub mod webhook_event;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_json_valid_array() {
        let json = serde_json::json!(["a", "b", "c"]);
        let result: Vec<String> = parse_json_with_fallback(&json, "test", "entity", "123");
        assert_eq!(result, vec!["a", "b", "c"]);
    }

    #[test]
    fn parse_json_empty_array() {
        let json = serde_json::json!([]);
        let result: Vec<String> = parse_json_with_fallback(&json, "test", "entity", "123");
        assert!(result.is_empty());
    }

    #[test]
    fn parse_json_sql_null_returns_empty_no_warning() {
        // SQL NULL becomes Value::Null - should return default without logging
        let json = serde_json::Value::Null;
        let result: Vec<String> = parse_json_with_fallback(&json, "test", "entity", "123");
        assert!(result.is_empty());
        // Note: Cannot easily verify no log without tracing-test; documented behavior
    }

    #[test]
    fn parse_json_invalid_type_returns_empty() {
        // Type mismatch: numbers instead of strings
        let json = serde_json::json!([1, 2, 3]);
        let result: Vec<String> = parse_json_with_fallback(&json, "test", "entity", "123");
        assert!(result.is_empty());
    }

    #[test]
    fn parse_json_wrong_structure_returns_empty() {
        // Object instead of array
        let json = serde_json::json!({"key": "value"});
        let result: Vec<String> = parse_json_with_fallback(&json, "test", "entity", "123");
        assert!(result.is_empty());
    }

    #[test]
    fn parse_json_nested_objects_returns_empty() {
        let json = serde_json::json!([{"key": "value"}]);
        let result: Vec<String> = parse_json_with_fallback(&json, "test", "entity", "123");
        assert!(result.is_empty());
    }
}
