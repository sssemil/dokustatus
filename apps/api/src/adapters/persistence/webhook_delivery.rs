use async_trait::async_trait;
use chrono::NaiveDateTime;
use sqlx::Row;
use uuid::Uuid;

use crate::{
    adapters::persistence::PostgresPersistence,
    app_error::{AppError, AppResult},
    application::use_cases::webhook::{
        WebhookDeliveryProfile, WebhookDeliveryRepoTrait, WebhookDeliveryWithDetails,
    },
};

fn row_to_profile(row: sqlx::postgres::PgRow) -> WebhookDeliveryProfile {
    WebhookDeliveryProfile {
        id: row.get("id"),
        webhook_event_id: row.get("webhook_event_id"),
        webhook_endpoint_id: row.get("webhook_endpoint_id"),
        status: row.get::<String, _>("status"),
        attempt_count: row.get("attempt_count"),
        next_attempt_at: row.get("next_attempt_at"),
        locked_at: row.get("locked_at"),
        last_response_status: row.get("last_response_status"),
        last_response_body: row.get("last_response_body"),
        last_error: row.get("last_error"),
        completed_at: row.get("completed_at"),
        created_at: row.get("created_at"),
    }
}

fn row_to_details(row: sqlx::postgres::PgRow) -> WebhookDeliveryWithDetails {
    WebhookDeliveryWithDetails {
        delivery_id: row.get("delivery_id"),
        event_id: row.get("event_id"),
        endpoint_id: row.get("endpoint_id"),
        attempt_count: row.get("attempt_count"),
        endpoint_url: row.get("endpoint_url"),
        secret_encrypted: row.get("secret_encrypted"),
        payload_raw: row.get("payload_raw"),
        event_type: row.get("event_type"),
        event_created_at: row.get("event_created_at"),
    }
}

const SELECT_COLS: &str = r#"
    id, webhook_event_id, webhook_endpoint_id, status::text as status,
    attempt_count, next_attempt_at, locked_at,
    last_response_status, last_response_body, last_error,
    completed_at, created_at
"#;

#[async_trait]
impl WebhookDeliveryRepoTrait for PostgresPersistence {
    async fn create(&self, event_id: Uuid, endpoint_id: Uuid) -> AppResult<WebhookDeliveryProfile> {
        let row = sqlx::query(&format!(
            r#"
            INSERT INTO webhook_deliveries (webhook_event_id, webhook_endpoint_id)
            VALUES ($1, $2)
            RETURNING {}
            "#,
            SELECT_COLS
        ))
        .bind(event_id)
        .bind(endpoint_id)
        .fetch_one(self.pool())
        .await
        .map_err(AppError::from)?;

        Ok(row_to_profile(row))
    }

    async fn claim_pending_batch(&self, limit: i64) -> AppResult<Vec<WebhookDeliveryWithDetails>> {
        let rows = sqlx::query(
            r#"
            WITH claimed AS (
                UPDATE webhook_deliveries
                SET status = 'in_progress', locked_at = NOW()
                WHERE id IN (
                    SELECT id FROM webhook_deliveries
                    WHERE status = 'pending' AND next_attempt_at <= NOW()
                    ORDER BY next_attempt_at
                    LIMIT $1
                    FOR UPDATE SKIP LOCKED
                )
                RETURNING id, webhook_event_id, webhook_endpoint_id, attempt_count
            )
            SELECT
                c.id AS delivery_id,
                c.webhook_event_id AS event_id,
                c.webhook_endpoint_id AS endpoint_id,
                c.attempt_count,
                ep.url AS endpoint_url,
                ep.secret_encrypted,
                ev.payload_raw,
                ev.event_type,
                ev.created_at AS event_created_at
            FROM claimed c
            JOIN webhook_endpoints ep ON ep.id = c.webhook_endpoint_id
            JOIN webhook_events ev ON ev.id = c.webhook_event_id
            "#,
        )
        .bind(limit)
        .fetch_all(self.pool())
        .await
        .map_err(AppError::from)?;

        Ok(rows.into_iter().map(row_to_details).collect())
    }

    async fn mark_succeeded(&self, id: Uuid, response_status: i32) -> AppResult<()> {
        sqlx::query(
            r#"
            UPDATE webhook_deliveries
            SET status = 'succeeded',
                attempt_count = attempt_count + 1,
                last_response_status = $2,
                completed_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(response_status)
        .execute(self.pool())
        .await
        .map_err(AppError::from)?;
        Ok(())
    }

    async fn mark_failed(
        &self,
        id: Uuid,
        attempt_count: i32,
        next_attempt_at: NaiveDateTime,
        response_status: Option<i32>,
        response_body: Option<&str>,
        error: Option<&str>,
    ) -> AppResult<()> {
        sqlx::query(
            r#"
            UPDATE webhook_deliveries
            SET status = 'pending',
                attempt_count = $2,
                next_attempt_at = $3,
                locked_at = NULL,
                last_response_status = $4,
                last_response_body = $5,
                last_error = $6
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(attempt_count)
        .bind(next_attempt_at)
        .bind(response_status)
        .bind(response_body)
        .bind(error)
        .execute(self.pool())
        .await
        .map_err(AppError::from)?;
        Ok(())
    }

    async fn mark_abandoned(
        &self,
        id: Uuid,
        response_status: Option<i32>,
        response_body: Option<&str>,
        error: Option<&str>,
    ) -> AppResult<()> {
        sqlx::query(
            r#"
            UPDATE webhook_deliveries
            SET status = 'abandoned',
                last_response_status = $2,
                last_response_body = $3,
                last_error = $4,
                completed_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(response_status)
        .bind(response_body)
        .bind(error)
        .execute(self.pool())
        .await
        .map_err(AppError::from)?;
        Ok(())
    }

    async fn release_stale(&self, threshold_secs: i64) -> AppResult<i64> {
        let result = sqlx::query(
            r#"
            UPDATE webhook_deliveries
            SET status = 'pending',
                locked_at = NULL,
                attempt_count = attempt_count + 1
            WHERE status = 'in_progress'
              AND locked_at < NOW() - make_interval(secs => $1::double precision)
            "#,
        )
        .bind(threshold_secs as f64)
        .execute(self.pool())
        .await
        .map_err(AppError::from)?;
        Ok(result.rows_affected() as i64)
    }

    async fn list_by_event(
        &self,
        event_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> AppResult<Vec<WebhookDeliveryProfile>> {
        let rows = sqlx::query(&format!(
            r#"
            SELECT {} FROM webhook_deliveries
            WHERE webhook_event_id = $1
            ORDER BY created_at DESC
            LIMIT $2 OFFSET $3
            "#,
            SELECT_COLS
        ))
        .bind(event_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(self.pool())
        .await
        .map_err(AppError::from)?;

        Ok(rows.into_iter().map(row_to_profile).collect())
    }

    async fn list_by_endpoint(
        &self,
        endpoint_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> AppResult<Vec<WebhookDeliveryProfile>> {
        let rows = sqlx::query(&format!(
            r#"
            SELECT {} FROM webhook_deliveries
            WHERE webhook_endpoint_id = $1
            ORDER BY created_at DESC
            LIMIT $2 OFFSET $3
            "#,
            SELECT_COLS
        ))
        .bind(endpoint_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(self.pool())
        .await
        .map_err(AppError::from)?;

        Ok(rows.into_iter().map(row_to_profile).collect())
    }
}
