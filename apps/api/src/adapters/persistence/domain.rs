use async_trait::async_trait;
use sqlx::Row;
use uuid::Uuid;

use crate::{
    adapters::persistence::PostgresPersistence,
    app_error::{AppError, AppResult},
    domain::entities::domain::DomainStatus,
    domain::entities::payment_mode::PaymentMode,
    use_cases::domain::{DomainProfile, DomainRepo},
};

const SELECT_COLS: &str = "id, owner_end_user_id, domain, status, active_payment_mode, verification_started_at, verified_at, created_at, updated_at";

fn row_to_profile(row: sqlx::postgres::PgRow) -> DomainProfile {
    DomainProfile {
        id: row.get("id"),
        owner_end_user_id: row.get("owner_end_user_id"),
        domain: row.get("domain"),
        status: DomainStatus::from_str(row.get("status")),
        billing_stripe_mode: row.get("active_payment_mode"),
        verification_started_at: row.get("verification_started_at"),
        verified_at: row.get("verified_at"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

#[async_trait]
impl DomainRepo for PostgresPersistence {
    async fn create(&self, owner_end_user_id: Uuid, domain: &str) -> AppResult<DomainProfile> {
        let id = Uuid::new_v4();
        let row = sqlx::query(&format!(
            r#"
                INSERT INTO domains (id, owner_end_user_id, domain, status)
                VALUES ($1, $2, $3, 'pending_dns')
                RETURNING {}
            "#,
            SELECT_COLS
        ))
        .bind(id)
        .bind(owner_end_user_id)
        .bind(domain)
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::from)?;
        Ok(row_to_profile(row))
    }

    async fn get_by_id(&self, domain_id: Uuid) -> AppResult<Option<DomainProfile>> {
        let row = sqlx::query(&format!(
            "SELECT {} FROM domains WHERE id = $1",
            SELECT_COLS
        ))
        .bind(domain_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::from)?;
        Ok(row.map(row_to_profile))
    }

    async fn get_by_domain(&self, domain: &str) -> AppResult<Option<DomainProfile>> {
        let row = sqlx::query(&format!(
            "SELECT {} FROM domains WHERE domain = $1",
            SELECT_COLS
        ))
        .bind(domain)
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::from)?;
        Ok(row.map(row_to_profile))
    }

    async fn list_by_owner(&self, owner_end_user_id: Uuid) -> AppResult<Vec<DomainProfile>> {
        let rows = sqlx::query(&format!(
            "SELECT {} FROM domains WHERE owner_end_user_id = $1 ORDER BY created_at DESC",
            SELECT_COLS
        ))
        .bind(owner_end_user_id)
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::from)?;
        Ok(rows.into_iter().map(row_to_profile).collect())
    }

    async fn update_status(&self, domain_id: Uuid, status: &str) -> AppResult<DomainProfile> {
        let row = sqlx::query(&format!(
            r#"
                UPDATE domains
                SET status = $2
                WHERE id = $1
                RETURNING {}
            "#,
            SELECT_COLS
        ))
        .bind(domain_id)
        .bind(status)
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::from)?;
        Ok(row_to_profile(row))
    }

    async fn set_verifying(&self, domain_id: Uuid) -> AppResult<DomainProfile> {
        let row = sqlx::query(&format!(
            r#"
                UPDATE domains
                SET status = 'verifying', verification_started_at = CURRENT_TIMESTAMP
                WHERE id = $1
                RETURNING {}
            "#,
            SELECT_COLS
        ))
        .bind(domain_id)
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::from)?;
        Ok(row_to_profile(row))
    }

    async fn set_verified(&self, domain_id: Uuid) -> AppResult<DomainProfile> {
        let row = sqlx::query(&format!(
            r#"
                UPDATE domains
                SET status = 'verified', verified_at = CURRENT_TIMESTAMP
                WHERE id = $1
                RETURNING {}
            "#,
            SELECT_COLS
        ))
        .bind(domain_id)
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::from)?;
        Ok(row_to_profile(row))
    }

    async fn set_failed(&self, domain_id: Uuid) -> AppResult<DomainProfile> {
        let row = sqlx::query(&format!(
            r#"
                UPDATE domains
                SET status = 'failed'
                WHERE id = $1
                RETURNING {}
            "#,
            SELECT_COLS
        ))
        .bind(domain_id)
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::from)?;
        Ok(row_to_profile(row))
    }

    async fn delete(&self, domain_id: Uuid) -> AppResult<()> {
        let mut tx = self.pool.begin().await.map_err(AppError::from)?;

        sqlx::query("DELETE FROM domains WHERE id = $1")
            .bind(domain_id)
            .execute(&mut *tx)
            .await
            .map_err(AppError::from)?;

        tx.commit().await.map_err(AppError::from)?;
        Ok(())
    }

    async fn get_verifying_domains(&self) -> AppResult<Vec<DomainProfile>> {
        let rows = sqlx::query(&format!(
            r#"
                SELECT {}
                FROM domains
                WHERE status = 'verifying'
            "#,
            SELECT_COLS
        ))
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::from)?;
        Ok(rows.into_iter().map(row_to_profile).collect())
    }

    async fn set_billing_stripe_mode(
        &self,
        domain_id: Uuid,
        mode: PaymentMode,
    ) -> AppResult<DomainProfile> {
        // Dual-write: update both billing_stripe_mode (legacy) and active_payment_mode (new)
        let mode_str = mode.as_str();
        let row = sqlx::query(&format!(
            r#"
                UPDATE domains
                SET billing_stripe_mode = $2::stripe_mode,
                    active_payment_mode = $2::payment_mode,
                    updated_at = CURRENT_TIMESTAMP
                WHERE id = $1
                RETURNING {}
            "#,
            SELECT_COLS
        ))
        .bind(domain_id)
        .bind(mode_str)
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::from)?;
        Ok(row_to_profile(row))
    }
}
