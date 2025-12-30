use async_trait::async_trait;
use sqlx::Row;
use uuid::Uuid;

use crate::{
    adapters::persistence::PostgresPersistence,
    app_error::{AppError, AppResult},
    application::use_cases::domain_auth::{
        DomainAuthGoogleOAuthProfile, DomainAuthGoogleOAuthRepo,
    },
};

fn row_to_profile(row: sqlx::postgres::PgRow) -> DomainAuthGoogleOAuthProfile {
    DomainAuthGoogleOAuthProfile {
        id: row.get("id"),
        domain_id: row.get("domain_id"),
        client_id: row.get("client_id"),
        client_secret_encrypted: row.get("client_secret_encrypted"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

#[async_trait]
impl DomainAuthGoogleOAuthRepo for PostgresPersistence {
    async fn get_by_domain_id(
        &self,
        domain_id: Uuid,
    ) -> AppResult<Option<DomainAuthGoogleOAuthProfile>> {
        let row = sqlx::query(
            "SELECT id, domain_id, client_id, client_secret_encrypted, created_at, updated_at FROM domain_auth_google_oauth WHERE domain_id = $1",
        )
        .bind(domain_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::from)?;
        Ok(row.map(row_to_profile))
    }

    async fn upsert(
        &self,
        domain_id: Uuid,
        client_id: &str,
        client_secret_encrypted: &str,
    ) -> AppResult<DomainAuthGoogleOAuthProfile> {
        let id = Uuid::new_v4();
        let row = sqlx::query(
            r#"
            INSERT INTO domain_auth_google_oauth (id, domain_id, client_id, client_secret_encrypted)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (domain_id) DO UPDATE SET
                client_id = EXCLUDED.client_id,
                client_secret_encrypted = EXCLUDED.client_secret_encrypted,
                updated_at = CURRENT_TIMESTAMP
            RETURNING id, domain_id, client_id, client_secret_encrypted, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(domain_id)
        .bind(client_id)
        .bind(client_secret_encrypted)
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::from)?;
        Ok(row_to_profile(row))
    }

    async fn delete(&self, domain_id: Uuid) -> AppResult<()> {
        sqlx::query("DELETE FROM domain_auth_google_oauth WHERE domain_id = $1")
            .bind(domain_id)
            .execute(&self.pool)
            .await
            .map_err(AppError::from)?;
        Ok(())
    }
}
