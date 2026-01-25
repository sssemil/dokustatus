use async_trait::async_trait;
use sqlx::Row;
use uuid::Uuid;

use crate::{
    adapters::persistence::PostgresPersistence,
    app_error::{AppError, AppResult},
    application::use_cases::api_key::{ApiKeyProfile, ApiKeyRepoTrait, ApiKeyWithDomain},
};

fn row_to_profile(row: sqlx::postgres::PgRow) -> ApiKeyProfile {
    ApiKeyProfile {
        id: row.get("id"),
        domain_id: row.get("domain_id"),
        key_prefix: row.get("key_prefix"),
        name: row.get("name"),
        last_used_at: row.get("last_used_at"),
        revoked_at: row.get("revoked_at"),
        created_at: row.get("created_at"),
    }
}

fn row_to_key_with_domain(row: sqlx::postgres::PgRow) -> ApiKeyWithDomain {
    ApiKeyWithDomain {
        id: row.get("id"),
        domain_id: row.get("domain_id"),
        domain_name: row.get("domain_name"),
        revoked_at: row.get("revoked_at"),
    }
}

#[async_trait]
impl ApiKeyRepoTrait for PostgresPersistence {
    async fn create(
        &self,
        domain_id: Uuid,
        key_prefix: &str,
        key_hash: &str,
        name: &str,
        created_by_end_user_id: Uuid,
    ) -> AppResult<ApiKeyProfile> {
        let row = sqlx::query(
            r#"
            INSERT INTO domain_api_keys (domain_id, key_prefix, key_hash, name, created_by_end_user_id)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id, domain_id, key_prefix, name, last_used_at, revoked_at, created_at
            "#,
        )
        .bind(domain_id)
        .bind(key_prefix)
        .bind(key_hash)
        .bind(name)
        .bind(created_by_end_user_id)
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::from)?;

        Ok(row_to_profile(row))
    }

    async fn get_by_hash(&self, key_hash: &str) -> AppResult<Option<ApiKeyWithDomain>> {
        let row = sqlx::query(
            r#"
            SELECT k.id, k.domain_id, d.domain as domain_name, k.revoked_at
            FROM domain_api_keys k
            JOIN domains d ON d.id = k.domain_id
            WHERE k.key_hash = $1
            "#,
        )
        .bind(key_hash)
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::from)?;

        Ok(row.map(row_to_key_with_domain))
    }

    async fn list_by_domain(&self, domain_id: Uuid) -> AppResult<Vec<ApiKeyProfile>> {
        let rows = sqlx::query(
            r#"
            SELECT id, domain_id, key_prefix, name, last_used_at, revoked_at, created_at
            FROM domain_api_keys
            WHERE domain_id = $1
            ORDER BY created_at DESC
            "#,
        )
        .bind(domain_id)
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::from)?;

        Ok(rows.into_iter().map(row_to_profile).collect())
    }

    async fn revoke(&self, id: Uuid) -> AppResult<()> {
        sqlx::query("UPDATE domain_api_keys SET revoked_at = CURRENT_TIMESTAMP WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(AppError::from)?;

        Ok(())
    }

    async fn update_last_used(&self, id: Uuid) -> AppResult<()> {
        sqlx::query("UPDATE domain_api_keys SET last_used_at = CURRENT_TIMESTAMP WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(AppError::from)?;

        Ok(())
    }
}
