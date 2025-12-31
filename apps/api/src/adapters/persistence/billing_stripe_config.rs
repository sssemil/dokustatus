use async_trait::async_trait;
use sqlx::Row;
use uuid::Uuid;

use crate::{
    adapters::persistence::PostgresPersistence,
    app_error::{AppError, AppResult},
    application::use_cases::domain_billing::{
        BillingStripeConfigProfile, BillingStripeConfigRepo,
    },
    domain::entities::stripe_mode::StripeMode,
};

fn row_to_profile(row: sqlx::postgres::PgRow) -> BillingStripeConfigProfile {
    BillingStripeConfigProfile {
        id: row.get("id"),
        domain_id: row.get("domain_id"),
        stripe_mode: row.get("stripe_mode"),
        stripe_secret_key_encrypted: row.get("stripe_secret_key_encrypted"),
        stripe_publishable_key: row.get("stripe_publishable_key"),
        stripe_webhook_secret_encrypted: row.get("stripe_webhook_secret_encrypted"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

#[async_trait]
impl BillingStripeConfigRepo for PostgresPersistence {
    async fn get_by_domain_and_mode(
        &self,
        domain_id: Uuid,
        mode: StripeMode,
    ) -> AppResult<Option<BillingStripeConfigProfile>> {
        let row = sqlx::query(
            r#"
            SELECT id, domain_id, stripe_mode, stripe_secret_key_encrypted, stripe_publishable_key,
                   stripe_webhook_secret_encrypted, created_at, updated_at
            FROM domain_billing_stripe_config
            WHERE domain_id = $1 AND stripe_mode = $2
            "#,
        )
        .bind(domain_id)
        .bind(mode)
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::from)?;
        Ok(row.map(row_to_profile))
    }

    async fn list_by_domain(
        &self,
        domain_id: Uuid,
    ) -> AppResult<Vec<BillingStripeConfigProfile>> {
        let rows = sqlx::query(
            r#"
            SELECT id, domain_id, stripe_mode, stripe_secret_key_encrypted, stripe_publishable_key,
                   stripe_webhook_secret_encrypted, created_at, updated_at
            FROM domain_billing_stripe_config
            WHERE domain_id = $1
            ORDER BY stripe_mode
            "#,
        )
        .bind(domain_id)
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::from)?;
        Ok(rows.into_iter().map(row_to_profile).collect())
    }

    async fn upsert(
        &self,
        domain_id: Uuid,
        mode: StripeMode,
        stripe_secret_key_encrypted: &str,
        stripe_publishable_key: &str,
        stripe_webhook_secret_encrypted: &str,
    ) -> AppResult<BillingStripeConfigProfile> {
        let id = Uuid::new_v4();
        let row = sqlx::query(
            r#"
            INSERT INTO domain_billing_stripe_config
                (id, domain_id, stripe_mode, stripe_secret_key_encrypted, stripe_publishable_key, stripe_webhook_secret_encrypted)
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (domain_id, stripe_mode) DO UPDATE SET
                stripe_secret_key_encrypted = EXCLUDED.stripe_secret_key_encrypted,
                stripe_publishable_key = EXCLUDED.stripe_publishable_key,
                stripe_webhook_secret_encrypted = EXCLUDED.stripe_webhook_secret_encrypted,
                updated_at = CURRENT_TIMESTAMP
            RETURNING id, domain_id, stripe_mode, stripe_secret_key_encrypted, stripe_publishable_key,
                      stripe_webhook_secret_encrypted, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(domain_id)
        .bind(mode)
        .bind(stripe_secret_key_encrypted)
        .bind(stripe_publishable_key)
        .bind(stripe_webhook_secret_encrypted)
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::from)?;
        Ok(row_to_profile(row))
    }

    async fn delete(&self, domain_id: Uuid, mode: StripeMode) -> AppResult<()> {
        sqlx::query("DELETE FROM domain_billing_stripe_config WHERE domain_id = $1 AND stripe_mode = $2")
            .bind(domain_id)
            .bind(mode)
            .execute(&self.pool)
            .await
            .map_err(AppError::from)?;
        Ok(())
    }

    async fn has_any_config(&self, domain_id: Uuid) -> AppResult<bool> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM domain_billing_stripe_config WHERE domain_id = $1"
        )
        .bind(domain_id)
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::from)?;
        Ok(count > 0)
    }
}
