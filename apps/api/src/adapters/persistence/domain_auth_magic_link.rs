use async_trait::async_trait;
use sqlx::Row;
use uuid::Uuid;

use crate::{
    adapters::persistence::PostgresPersistence,
    app_error::{AppError, AppResult},
    application::use_cases::domain_auth::{
        DomainAuthMagicLinkProfile, DomainAuthMagicLinkRepoTrait,
    },
};

fn row_to_profile(row: sqlx::postgres::PgRow) -> DomainAuthMagicLinkProfile {
    DomainAuthMagicLinkProfile {
        id: row.get("id"),
        domain_id: row.get("domain_id"),
        resend_api_key_encrypted: row.get("resend_api_key_encrypted"),
        from_email: row.get("from_email"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

#[async_trait]
impl DomainAuthMagicLinkRepoTrait for PostgresPersistence {
    async fn get_by_domain_id(
        &self,
        domain_id: Uuid,
    ) -> AppResult<Option<DomainAuthMagicLinkProfile>> {
        let row = sqlx::query(
            "SELECT id, domain_id, resend_api_key_encrypted, from_email, created_at, updated_at FROM domain_auth_magic_link WHERE domain_id = $1",
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
        resend_api_key_encrypted: &str,
        from_email: &str,
    ) -> AppResult<DomainAuthMagicLinkProfile> {
        let id = Uuid::new_v4();
        let row = sqlx::query(
            r#"
            INSERT INTO domain_auth_magic_link (id, domain_id, resend_api_key_encrypted, from_email)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (domain_id) DO UPDATE SET
                resend_api_key_encrypted = EXCLUDED.resend_api_key_encrypted,
                from_email = EXCLUDED.from_email,
                updated_at = CURRENT_TIMESTAMP
            RETURNING id, domain_id, resend_api_key_encrypted, from_email, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(domain_id)
        .bind(resend_api_key_encrypted)
        .bind(from_email)
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::from)?;
        Ok(row_to_profile(row))
    }

    async fn update_from_email(&self, domain_id: Uuid, from_email: &str) -> AppResult<()> {
        let result = sqlx::query(
            "UPDATE domain_auth_magic_link SET from_email = $2, updated_at = CURRENT_TIMESTAMP WHERE domain_id = $1",
        )
        .bind(domain_id)
        .bind(from_email)
        .execute(&self.pool)
        .await
        .map_err(AppError::from)?;

        if result.rows_affected() == 0 {
            return Err(AppError::NotFound);
        }
        Ok(())
    }

    async fn delete(&self, domain_id: Uuid) -> AppResult<()> {
        sqlx::query("DELETE FROM domain_auth_magic_link WHERE domain_id = $1")
            .bind(domain_id)
            .execute(&self.pool)
            .await
            .map_err(AppError::from)?;
        Ok(())
    }
}
