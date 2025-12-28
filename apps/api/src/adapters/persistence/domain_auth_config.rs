use async_trait::async_trait;
use sqlx::Row;
use uuid::Uuid;

use crate::{
    adapters::persistence::PostgresPersistence,
    app_error::{AppError, AppResult},
    application::use_cases::domain_auth::{DomainAuthConfigProfile, DomainAuthConfigRepo},
};

fn row_to_profile(row: sqlx::postgres::PgRow) -> DomainAuthConfigProfile {
    DomainAuthConfigProfile {
        id: row.get("id"),
        domain_id: row.get("domain_id"),
        magic_link_enabled: row.get("magic_link_enabled"),
        google_oauth_enabled: row.get("google_oauth_enabled"),
        redirect_url: row.get("redirect_url"),
        whitelist_enabled: row.get("whitelist_enabled"),
        access_token_ttl_secs: row.get("access_token_ttl_secs"),
        refresh_token_ttl_days: row.get("refresh_token_ttl_days"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

#[async_trait]
impl DomainAuthConfigRepo for PostgresPersistence {
    async fn get_by_domain_id(
        &self,
        domain_id: Uuid,
    ) -> AppResult<Option<DomainAuthConfigProfile>> {
        let row = sqlx::query(
            "SELECT id, domain_id, magic_link_enabled, google_oauth_enabled, redirect_url, whitelist_enabled, access_token_ttl_secs, refresh_token_ttl_days, created_at, updated_at FROM domain_auth_config WHERE domain_id = $1",
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
        magic_link_enabled: bool,
        google_oauth_enabled: bool,
        redirect_url: Option<&str>,
        whitelist_enabled: bool,
    ) -> AppResult<DomainAuthConfigProfile> {
        let id = Uuid::new_v4();
        let row = sqlx::query(
            r#"
            INSERT INTO domain_auth_config (id, domain_id, magic_link_enabled, google_oauth_enabled, redirect_url, whitelist_enabled, access_token_ttl_secs, refresh_token_ttl_days)
            VALUES ($1, $2, $3, $4, $5, $6, 86400, 30)
            ON CONFLICT (domain_id) DO UPDATE SET
                magic_link_enabled = EXCLUDED.magic_link_enabled,
                google_oauth_enabled = EXCLUDED.google_oauth_enabled,
                redirect_url = EXCLUDED.redirect_url,
                whitelist_enabled = EXCLUDED.whitelist_enabled,
                updated_at = CURRENT_TIMESTAMP
            RETURNING id, domain_id, magic_link_enabled, google_oauth_enabled, redirect_url, whitelist_enabled, access_token_ttl_secs, refresh_token_ttl_days, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(domain_id)
        .bind(magic_link_enabled)
        .bind(google_oauth_enabled)
        .bind(redirect_url)
        .bind(whitelist_enabled)
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::from)?;
        Ok(row_to_profile(row))
    }

    async fn delete(&self, domain_id: Uuid) -> AppResult<()> {
        sqlx::query("DELETE FROM domain_auth_config WHERE domain_id = $1")
            .bind(domain_id)
            .execute(&self.pool)
            .await
            .map_err(AppError::from)?;
        Ok(())
    }
}
