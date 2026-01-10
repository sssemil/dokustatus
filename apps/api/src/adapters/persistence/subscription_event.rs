use async_trait::async_trait;
use sqlx::Row;
use uuid::Uuid;

use crate::{
    adapters::persistence::PostgresPersistence,
    app_error::{AppError, AppResult},
    application::use_cases::domain_billing::{
        CreateSubscriptionEventInput, SubscriptionEventProfile, SubscriptionEventRepo,
    },
};

fn row_to_profile(row: sqlx::postgres::PgRow) -> SubscriptionEventProfile {
    SubscriptionEventProfile {
        id: row.get("id"),
        subscription_id: row.get("subscription_id"),
        event_type: row.get("event_type"),
        previous_status: row.get("previous_status"),
        new_status: row.get("new_status"),
        stripe_event_id: row.get("stripe_event_id"),
        metadata: row.get("metadata"),
        created_by: row.get("created_by"),
        created_at: row.get("created_at"),
    }
}

const SELECT_COLS: &str = r#"
    id, subscription_id, event_type, previous_status, new_status,
    stripe_event_id, metadata, created_by, created_at
"#;

#[async_trait]
impl SubscriptionEventRepo for PostgresPersistence {
    async fn create(&self, input: &CreateSubscriptionEventInput) -> AppResult<()> {
        let id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO subscription_events
                (id, subscription_id, event_type, previous_status, new_status, stripe_event_id, metadata, created_by)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#
        )
        .bind(id)
        .bind(input.subscription_id)
        .bind(&input.event_type)
        .bind(input.previous_status)
        .bind(input.new_status)
        .bind(&input.stripe_event_id)
        .bind(&input.metadata)
        .bind(input.created_by)
        .execute(&self.pool)
        .await
        .map_err(AppError::from)?;
        Ok(())
    }

    async fn list_by_subscription(
        &self,
        subscription_id: Uuid,
    ) -> AppResult<Vec<SubscriptionEventProfile>> {
        let rows = sqlx::query(&format!(
            "SELECT {} FROM subscription_events WHERE subscription_id = $1 ORDER BY created_at DESC",
            SELECT_COLS
        ))
        .bind(subscription_id)
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::from)?;
        Ok(rows.into_iter().map(row_to_profile).collect())
    }

    async fn exists_by_stripe_event_id(&self, stripe_event_id: &str) -> AppResult<bool> {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM subscription_events WHERE stripe_event_id = $1)",
        )
        .bind(stripe_event_id)
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::from)?;
        Ok(exists)
    }
}
