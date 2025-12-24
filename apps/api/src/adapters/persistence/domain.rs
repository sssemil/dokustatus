use async_trait::async_trait;
use sqlx::Row;
use uuid::Uuid;

use crate::{
    adapters::persistence::PostgresPersistence,
    app_error::{AppError, AppResult},
    domain::entities::domain::DomainStatus,
    use_cases::domain::{DomainProfile, DomainRepo},
};

fn row_to_profile(row: sqlx::postgres::PgRow) -> DomainProfile {
    DomainProfile {
        id: row.get("id"),
        user_id: row.get("user_id"),
        domain: row.get("domain"),
        status: DomainStatus::from_str(row.get("status")),
        verification_started_at: row.get("verification_started_at"),
        verified_at: row.get("verified_at"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

#[async_trait]
impl DomainRepo for PostgresPersistence {
    async fn create(&self, user_id: Uuid, domain: &str) -> AppResult<DomainProfile> {
        let id = Uuid::new_v4();
        let row = sqlx::query(
            r#"
                INSERT INTO domains (id, user_id, domain, status)
                VALUES ($1, $2, $3, 'pending_dns')
                RETURNING id, user_id, domain, status, verification_started_at, verified_at, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(user_id)
        .bind(domain)
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::from)?;
        Ok(row_to_profile(row))
    }

    async fn get_by_id(&self, domain_id: Uuid) -> AppResult<Option<DomainProfile>> {
        let row = sqlx::query(
            "SELECT id, user_id, domain, status, verification_started_at, verified_at, created_at, updated_at FROM domains WHERE id = $1",
        )
        .bind(domain_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::from)?;
        Ok(row.map(row_to_profile))
    }

    async fn get_by_domain(&self, domain: &str) -> AppResult<Option<DomainProfile>> {
        let row = sqlx::query(
            "SELECT id, user_id, domain, status, verification_started_at, verified_at, created_at, updated_at FROM domains WHERE domain = $1",
        )
        .bind(domain)
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::from)?;
        Ok(row.map(row_to_profile))
    }

    async fn list_by_user(&self, user_id: Uuid) -> AppResult<Vec<DomainProfile>> {
        let rows = sqlx::query(
            "SELECT id, user_id, domain, status, verification_started_at, verified_at, created_at, updated_at FROM domains WHERE user_id = $1 ORDER BY created_at DESC",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::from)?;
        Ok(rows.into_iter().map(row_to_profile).collect())
    }

    async fn update_status(&self, domain_id: Uuid, status: &str) -> AppResult<DomainProfile> {
        let row = sqlx::query(
            r#"
                UPDATE domains
                SET status = $2
                WHERE id = $1
                RETURNING id, user_id, domain, status, verification_started_at, verified_at, created_at, updated_at
            "#,
        )
        .bind(domain_id)
        .bind(status)
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::from)?;
        Ok(row_to_profile(row))
    }

    async fn set_verifying(&self, domain_id: Uuid) -> AppResult<DomainProfile> {
        let row = sqlx::query(
            r#"
                UPDATE domains
                SET status = 'verifying', verification_started_at = CURRENT_TIMESTAMP
                WHERE id = $1
                RETURNING id, user_id, domain, status, verification_started_at, verified_at, created_at, updated_at
            "#,
        )
        .bind(domain_id)
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::from)?;
        Ok(row_to_profile(row))
    }

    async fn set_verified(&self, domain_id: Uuid) -> AppResult<DomainProfile> {
        let row = sqlx::query(
            r#"
                UPDATE domains
                SET status = 'verified', verified_at = CURRENT_TIMESTAMP
                WHERE id = $1
                RETURNING id, user_id, domain, status, verification_started_at, verified_at, created_at, updated_at
            "#,
        )
        .bind(domain_id)
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::from)?;
        Ok(row_to_profile(row))
    }

    async fn set_failed(&self, domain_id: Uuid) -> AppResult<DomainProfile> {
        let row = sqlx::query(
            r#"
                UPDATE domains
                SET status = 'failed'
                WHERE id = $1
                RETURNING id, user_id, domain, status, verification_started_at, verified_at, created_at, updated_at
            "#,
        )
        .bind(domain_id)
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::from)?;
        Ok(row_to_profile(row))
    }

    async fn delete(&self, domain_id: Uuid) -> AppResult<()> {
        sqlx::query("DELETE FROM domains WHERE id = $1")
            .bind(domain_id)
            .execute(&self.pool)
            .await
            .map_err(AppError::from)?;
        Ok(())
    }

    async fn get_verifying_domains(&self) -> AppResult<Vec<DomainProfile>> {
        let rows = sqlx::query(
            r#"
                SELECT id, user_id, domain, status, verification_started_at, verified_at, created_at, updated_at
                FROM domains
                WHERE status = 'verifying'
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::from)?;
        Ok(rows.into_iter().map(row_to_profile).collect())
    }
}
