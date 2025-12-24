use async_trait::async_trait;
use sqlx::Row;
use uuid::Uuid;

use crate::{
    adapters::persistence::PostgresPersistence,
    app_error::{AppError, AppResult},
    application::use_cases::domain_auth::{DomainEndUserProfile, DomainEndUserRepo},
};

fn row_to_profile(row: sqlx::postgres::PgRow) -> DomainEndUserProfile {
    DomainEndUserProfile {
        id: row.get("id"),
        domain_id: row.get("domain_id"),
        email: row.get("email"),
        email_verified_at: row.get("email_verified_at"),
        last_login_at: row.get("last_login_at"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

#[async_trait]
impl DomainEndUserRepo for PostgresPersistence {
    async fn get_by_id(&self, id: Uuid) -> AppResult<Option<DomainEndUserProfile>> {
        let row = sqlx::query(
            "SELECT id, domain_id, email, email_verified_at, last_login_at, created_at, updated_at FROM domain_end_users WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::from)?;
        Ok(row.map(row_to_profile))
    }

    async fn get_by_domain_and_email(&self, domain_id: Uuid, email: &str) -> AppResult<Option<DomainEndUserProfile>> {
        let row = sqlx::query(
            "SELECT id, domain_id, email, email_verified_at, last_login_at, created_at, updated_at FROM domain_end_users WHERE domain_id = $1 AND email = $2",
        )
        .bind(domain_id)
        .bind(email.to_lowercase())
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::from)?;
        Ok(row.map(row_to_profile))
    }

    async fn upsert(&self, domain_id: Uuid, email: &str) -> AppResult<DomainEndUserProfile> {
        let id = Uuid::new_v4();
        let normalized_email = email.to_lowercase();
        let row = sqlx::query(
            r#"
            INSERT INTO domain_end_users (id, domain_id, email)
            VALUES ($1, $2, $3)
            ON CONFLICT (domain_id, email) DO UPDATE SET
                updated_at = CURRENT_TIMESTAMP
            RETURNING id, domain_id, email, email_verified_at, last_login_at, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(domain_id)
        .bind(&normalized_email)
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::from)?;
        Ok(row_to_profile(row))
    }

    async fn mark_verified(&self, id: Uuid) -> AppResult<DomainEndUserProfile> {
        let row = sqlx::query(
            r#"
            UPDATE domain_end_users
            SET email_verified_at = CURRENT_TIMESTAMP, last_login_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP
            WHERE id = $1
            RETURNING id, domain_id, email, email_verified_at, last_login_at, created_at, updated_at
            "#,
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::from)?;
        Ok(row_to_profile(row))
    }

    async fn update_last_login(&self, id: Uuid) -> AppResult<()> {
        sqlx::query("UPDATE domain_end_users SET last_login_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(AppError::from)?;
        Ok(())
    }

    async fn list_by_domain(&self, domain_id: Uuid) -> AppResult<Vec<DomainEndUserProfile>> {
        let rows = sqlx::query(
            "SELECT id, domain_id, email, email_verified_at, last_login_at, created_at, updated_at FROM domain_end_users WHERE domain_id = $1 ORDER BY created_at DESC",
        )
        .bind(domain_id)
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::from)?;
        Ok(rows.into_iter().map(row_to_profile).collect())
    }
}
