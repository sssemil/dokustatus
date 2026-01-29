use async_trait::async_trait;
use sqlx::Row;
use uuid::Uuid;

use crate::{
    adapters::persistence::PostgresPersistence,
    app_error::{AppError, AppResult},
    application::use_cases::webhook::{WebhookEndpointProfile, WebhookEndpointRepoTrait},
};

fn row_to_profile(row: sqlx::postgres::PgRow) -> WebhookEndpointProfile {
    WebhookEndpointProfile {
        id: row.get("id"),
        domain_id: row.get("domain_id"),
        url: row.get("url"),
        description: row.get("description"),
        secret_encrypted: row.get("secret_encrypted"),
        event_types: row.get("event_types"),
        is_active: row.get("is_active"),
        consecutive_failures: row.get("consecutive_failures"),
        last_success_at: row.get("last_success_at"),
        last_failure_at: row.get("last_failure_at"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

const SELECT_COLS: &str = r#"
    id, domain_id, url, description, secret_encrypted, event_types,
    is_active, consecutive_failures, last_success_at, last_failure_at,
    created_at, updated_at
"#;

#[async_trait]
impl WebhookEndpointRepoTrait for PostgresPersistence {
    async fn create(
        &self,
        domain_id: Uuid,
        url: &str,
        description: Option<&str>,
        secret_encrypted: &str,
        event_types: &serde_json::Value,
    ) -> AppResult<WebhookEndpointProfile> {
        let row = sqlx::query(&format!(
            r#"
            INSERT INTO webhook_endpoints (domain_id, url, description, secret_encrypted, event_types)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING {}
            "#,
            SELECT_COLS
        ))
        .bind(domain_id)
        .bind(url)
        .bind(description)
        .bind(secret_encrypted)
        .bind(event_types)
        .fetch_one(self.pool())
        .await
        .map_err(AppError::from)?;

        Ok(row_to_profile(row))
    }

    async fn get_by_id(&self, id: Uuid) -> AppResult<Option<WebhookEndpointProfile>> {
        let row = sqlx::query(&format!(
            "SELECT {} FROM webhook_endpoints WHERE id = $1",
            SELECT_COLS
        ))
        .bind(id)
        .fetch_optional(self.pool())
        .await
        .map_err(AppError::from)?;

        Ok(row.map(row_to_profile))
    }

    async fn list_by_domain(&self, domain_id: Uuid) -> AppResult<Vec<WebhookEndpointProfile>> {
        let rows = sqlx::query(&format!(
            "SELECT {} FROM webhook_endpoints WHERE domain_id = $1 ORDER BY created_at ASC",
            SELECT_COLS
        ))
        .bind(domain_id)
        .fetch_all(self.pool())
        .await
        .map_err(AppError::from)?;

        Ok(rows.into_iter().map(row_to_profile).collect())
    }

    async fn list_active_for_event(
        &self,
        domain_id: Uuid,
        event_type: &str,
    ) -> AppResult<Vec<WebhookEndpointProfile>> {
        let rows = sqlx::query(&format!(
            r#"
            SELECT {} FROM webhook_endpoints
            WHERE domain_id = $1 AND is_active = true
              AND (event_types @> '["*"]'::jsonb OR event_types @> to_jsonb(ARRAY[$2::text]))
            "#,
            SELECT_COLS
        ))
        .bind(domain_id)
        .bind(event_type)
        .fetch_all(self.pool())
        .await
        .map_err(AppError::from)?;

        Ok(rows.into_iter().map(row_to_profile).collect())
    }

    async fn update(
        &self,
        id: Uuid,
        url: Option<&str>,
        description: Option<Option<&str>>,
        event_types: Option<&serde_json::Value>,
        is_active: Option<bool>,
    ) -> AppResult<WebhookEndpointProfile> {
        let mut set_clauses = Vec::new();
        let mut param_idx = 2u32; // $1 is id

        if url.is_some() {
            set_clauses.push(format!("url = ${}", param_idx));
            param_idx += 1;
        }
        if description.is_some() {
            set_clauses.push(format!("description = ${}", param_idx));
            param_idx += 1;
        }
        if event_types.is_some() {
            set_clauses.push(format!("event_types = ${}", param_idx));
            param_idx += 1;
        }
        if is_active.is_some() {
            set_clauses.push(format!("is_active = ${}", param_idx));
            // param_idx += 1; // not needed after last
        }

        if set_clauses.is_empty() {
            return self
                .get_by_id(id)
                .await?
                .ok_or(AppError::NotFound);
        }

        let sql = format!(
            "UPDATE webhook_endpoints SET {} WHERE id = $1 RETURNING {}",
            set_clauses.join(", "),
            SELECT_COLS
        );

        let mut query = sqlx::query(&sql).bind(id);
        if let Some(u) = url {
            query = query.bind(u);
        }
        if let Some(d) = description {
            query = query.bind(d);
        }
        if let Some(et) = event_types {
            query = query.bind(et);
        }
        if let Some(a) = is_active {
            query = query.bind(a);
        }

        let row = query
            .fetch_one(self.pool())
            .await
            .map_err(AppError::from)?;

        Ok(row_to_profile(row))
    }

    async fn update_secret(&self, id: Uuid, secret_encrypted: &str) -> AppResult<()> {
        sqlx::query("UPDATE webhook_endpoints SET secret_encrypted = $2 WHERE id = $1")
            .bind(id)
            .bind(secret_encrypted)
            .execute(self.pool())
            .await
            .map_err(AppError::from)?;
        Ok(())
    }

    async fn record_success(&self, id: Uuid) -> AppResult<()> {
        sqlx::query(
            r#"
            UPDATE webhook_endpoints
            SET consecutive_failures = 0, last_success_at = CURRENT_TIMESTAMP
            WHERE id = $1
            "#,
        )
        .bind(id)
        .execute(self.pool())
        .await
        .map_err(AppError::from)?;
        Ok(())
    }

    async fn record_failure(&self, id: Uuid) -> AppResult<()> {
        sqlx::query(
            r#"
            UPDATE webhook_endpoints
            SET consecutive_failures = consecutive_failures + 1,
                last_failure_at = CURRENT_TIMESTAMP
            WHERE id = $1
            "#,
        )
        .bind(id)
        .execute(self.pool())
        .await
        .map_err(AppError::from)?;
        Ok(())
    }

    async fn delete(&self, id: Uuid) -> AppResult<()> {
        sqlx::query("DELETE FROM webhook_endpoints WHERE id = $1")
            .bind(id)
            .execute(self.pool())
            .await
            .map_err(AppError::from)?;
        Ok(())
    }

    async fn count_by_domain(&self, domain_id: Uuid) -> AppResult<i64> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM webhook_endpoints WHERE domain_id = $1",
        )
        .bind(domain_id)
        .fetch_one(self.pool())
        .await
        .map_err(AppError::from)?;
        Ok(count)
    }
}
