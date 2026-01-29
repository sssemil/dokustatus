use async_trait::async_trait;
use sqlx::Row;
use uuid::Uuid;

use crate::{
    adapters::persistence::PostgresPersistence,
    app_error::{AppError, AppResult},
    application::use_cases::webhook::{WebhookEventProfile, WebhookEventRepoTrait},
};

fn row_to_profile(row: sqlx::postgres::PgRow) -> WebhookEventProfile {
    WebhookEventProfile {
        id: row.get("id"),
        domain_id: row.get("domain_id"),
        event_type: row.get("event_type"),
        payload: row.get("payload"),
        payload_raw: row.get("payload_raw"),
        created_at: row.get("created_at"),
    }
}

const SELECT_COLS: &str = "id, domain_id, event_type, payload, payload_raw, created_at";

#[async_trait]
impl WebhookEventRepoTrait for PostgresPersistence {
    async fn create(
        &self,
        domain_id: Uuid,
        event_type: &str,
        payload: &serde_json::Value,
        payload_raw: &str,
    ) -> AppResult<WebhookEventProfile> {
        let row = sqlx::query(&format!(
            r#"
            INSERT INTO webhook_events (domain_id, event_type, payload, payload_raw)
            VALUES ($1, $2, $3, $4)
            RETURNING {}
            "#,
            SELECT_COLS
        ))
        .bind(domain_id)
        .bind(event_type)
        .bind(payload)
        .bind(payload_raw)
        .fetch_one(self.pool())
        .await
        .map_err(AppError::from)?;

        Ok(row_to_profile(row))
    }

    async fn get_by_id(&self, id: Uuid) -> AppResult<Option<WebhookEventProfile>> {
        let row = sqlx::query(&format!(
            "SELECT {} FROM webhook_events WHERE id = $1",
            SELECT_COLS
        ))
        .bind(id)
        .fetch_optional(self.pool())
        .await
        .map_err(AppError::from)?;

        Ok(row.map(row_to_profile))
    }

    async fn list_by_domain(
        &self,
        domain_id: Uuid,
        event_type_filter: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> AppResult<Vec<WebhookEventProfile>> {
        let rows = match event_type_filter {
            Some(filter) => {
                sqlx::query(&format!(
                    r#"
                    SELECT {} FROM webhook_events
                    WHERE domain_id = $1 AND event_type = $2
                    ORDER BY created_at DESC
                    LIMIT $3 OFFSET $4
                    "#,
                    SELECT_COLS
                ))
                .bind(domain_id)
                .bind(filter)
                .bind(limit)
                .bind(offset)
                .fetch_all(self.pool())
                .await
                .map_err(AppError::from)?
            }
            None => {
                sqlx::query(&format!(
                    r#"
                    SELECT {} FROM webhook_events
                    WHERE domain_id = $1
                    ORDER BY created_at DESC
                    LIMIT $2 OFFSET $3
                    "#,
                    SELECT_COLS
                ))
                .bind(domain_id)
                .bind(limit)
                .bind(offset)
                .fetch_all(self.pool())
                .await
                .map_err(AppError::from)?
            }
        };

        Ok(rows.into_iter().map(row_to_profile).collect())
    }
}
