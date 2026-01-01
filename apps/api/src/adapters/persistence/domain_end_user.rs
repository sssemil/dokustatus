use async_trait::async_trait;
use sqlx::Row;
use uuid::Uuid;

use crate::{
    adapters::persistence::PostgresPersistence,
    app_error::{AppError, AppResult},
    application::use_cases::domain_auth::{DomainEndUserProfile, DomainEndUserRepo},
};

fn row_to_profile(row: sqlx::postgres::PgRow) -> DomainEndUserProfile {
    let id: uuid::Uuid = row.get("id");
    let roles_json: serde_json::Value = row.get("roles");
    let roles: Vec<String> =
        super::parse_json_with_fallback(&roles_json, "roles", "domain_end_user", &id.to_string());

    DomainEndUserProfile {
        id,
        domain_id: row.get("domain_id"),
        email: row.get("email"),
        roles,
        google_id: row.get("google_id"),
        email_verified_at: row.get("email_verified_at"),
        last_login_at: row.get("last_login_at"),
        is_frozen: row.get("is_frozen"),
        is_whitelisted: row.get("is_whitelisted"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

#[async_trait]
impl DomainEndUserRepo for PostgresPersistence {
    async fn get_by_id(&self, id: Uuid) -> AppResult<Option<DomainEndUserProfile>> {
        let row = sqlx::query(
            "SELECT id, domain_id, email, roles, google_id, email_verified_at, last_login_at, is_frozen, is_whitelisted, created_at, updated_at FROM domain_end_users WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::from)?;
        Ok(row.map(row_to_profile))
    }

    async fn get_by_domain_and_email(
        &self,
        domain_id: Uuid,
        email: &str,
    ) -> AppResult<Option<DomainEndUserProfile>> {
        let row = sqlx::query(
            "SELECT id, domain_id, email, roles, google_id, email_verified_at, last_login_at, is_frozen, is_whitelisted, created_at, updated_at FROM domain_end_users WHERE domain_id = $1 AND email = $2",
        )
        .bind(domain_id)
        .bind(email.to_lowercase())
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::from)?;
        Ok(row.map(row_to_profile))
    }

    async fn get_by_domain_and_google_id(
        &self,
        domain_id: Uuid,
        google_id: &str,
    ) -> AppResult<Option<DomainEndUserProfile>> {
        let row = sqlx::query(
            "SELECT id, domain_id, email, roles, google_id, email_verified_at, last_login_at, is_frozen, is_whitelisted, created_at, updated_at FROM domain_end_users WHERE domain_id = $1 AND google_id = $2",
        )
        .bind(domain_id)
        .bind(google_id)
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
            INSERT INTO domain_end_users (id, domain_id, email, roles)
            VALUES ($1, $2, $3, '[]'::jsonb)
            ON CONFLICT (domain_id, email) DO UPDATE SET
                updated_at = CURRENT_TIMESTAMP
            RETURNING id, domain_id, email, roles, google_id, email_verified_at, last_login_at, is_frozen, is_whitelisted, created_at, updated_at
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

    async fn upsert_with_google_id(
        &self,
        domain_id: Uuid,
        email: &str,
        google_id: &str,
    ) -> AppResult<DomainEndUserProfile> {
        let id = Uuid::new_v4();
        let normalized_email = email.to_lowercase();
        let row = sqlx::query(
            r#"
            INSERT INTO domain_end_users (id, domain_id, email, google_id, roles, email_verified_at)
            VALUES ($1, $2, $3, $4, '[]'::jsonb, CURRENT_TIMESTAMP)
            ON CONFLICT (domain_id, email) DO UPDATE SET
                google_id = EXCLUDED.google_id,
                email_verified_at = COALESCE(domain_end_users.email_verified_at, CURRENT_TIMESTAMP),
                updated_at = CURRENT_TIMESTAMP
            RETURNING id, domain_id, email, roles, google_id, email_verified_at, last_login_at, is_frozen, is_whitelisted, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(domain_id)
        .bind(&normalized_email)
        .bind(google_id)
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
            RETURNING id, domain_id, email, roles, google_id, email_verified_at, last_login_at, is_frozen, is_whitelisted, created_at, updated_at
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

    async fn set_google_id(&self, id: Uuid, google_id: &str) -> AppResult<()> {
        sqlx::query("UPDATE domain_end_users SET google_id = $1, updated_at = CURRENT_TIMESTAMP WHERE id = $2")
            .bind(google_id)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(AppError::from)?;
        Ok(())
    }

    async fn clear_google_id(&self, id: Uuid) -> AppResult<()> {
        sqlx::query("UPDATE domain_end_users SET google_id = NULL, updated_at = CURRENT_TIMESTAMP WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(AppError::from)?;
        Ok(())
    }

    async fn list_by_domain(&self, domain_id: Uuid) -> AppResult<Vec<DomainEndUserProfile>> {
        let rows = sqlx::query(
            "SELECT id, domain_id, email, roles, google_id, email_verified_at, last_login_at, is_frozen, is_whitelisted, created_at, updated_at FROM domain_end_users WHERE domain_id = $1 ORDER BY created_at DESC",
        )
        .bind(domain_id)
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::from)?;
        Ok(rows.into_iter().map(row_to_profile).collect())
    }

    async fn delete(&self, id: Uuid) -> AppResult<()> {
        sqlx::query("DELETE FROM domain_end_users WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(AppError::from)?;
        Ok(())
    }

    async fn set_frozen(&self, id: Uuid, frozen: bool) -> AppResult<()> {
        sqlx::query("UPDATE domain_end_users SET is_frozen = $1, updated_at = CURRENT_TIMESTAMP WHERE id = $2")
            .bind(frozen)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(AppError::from)?;
        Ok(())
    }

    async fn set_whitelisted(&self, id: Uuid, whitelisted: bool) -> AppResult<()> {
        sqlx::query("UPDATE domain_end_users SET is_whitelisted = $1, updated_at = CURRENT_TIMESTAMP WHERE id = $2")
            .bind(whitelisted)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(AppError::from)?;
        Ok(())
    }

    async fn whitelist_all_in_domain(&self, domain_id: Uuid) -> AppResult<()> {
        sqlx::query("UPDATE domain_end_users SET is_whitelisted = true, updated_at = CURRENT_TIMESTAMP WHERE domain_id = $1")
            .bind(domain_id)
            .execute(&self.pool)
            .await
            .map_err(AppError::from)?;
        Ok(())
    }

    async fn count_by_domain_ids(&self, domain_ids: &[Uuid]) -> AppResult<i64> {
        if domain_ids.is_empty() {
            return Ok(0);
        }
        let row: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM domain_end_users WHERE domain_id = ANY($1)")
                .bind(domain_ids)
                .fetch_one(&self.pool)
                .await
                .map_err(AppError::from)?;
        Ok(row.0)
    }

    async fn get_waitlist_position(&self, domain_id: Uuid, user_id: Uuid) -> AppResult<i64> {
        // Get the user's created_at timestamp
        let user_row: Option<(chrono::NaiveDateTime,)> = sqlx::query_as(
            "SELECT created_at FROM domain_end_users WHERE id = $1 AND domain_id = $2",
        )
        .bind(user_id)
        .bind(domain_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::from)?;

        let Some((user_created_at,)) = user_row else {
            return Err(AppError::NotFound);
        };

        // Count non-whitelisted users created before this user (their position is ahead)
        let row: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM domain_end_users WHERE domain_id = $1 AND is_whitelisted = false AND created_at < $2",
        )
        .bind(domain_id)
        .bind(user_created_at)
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::from)?;

        // Position is count + 1 (1-indexed)
        Ok(row.0 + 1)
    }

    async fn set_roles(&self, id: Uuid, roles: &[String]) -> AppResult<()> {
        let roles_json = serde_json::to_value(roles).map_err(|err| {
            tracing::error!(
                error = %err,
                roles_count = roles.len(),
                entity_id = %id,
                "Failed to serialize roles to JSON - this indicates a bug"
            );
            AppError::Internal("Failed to serialize roles".into())
        })?;
        sqlx::query(
            "UPDATE domain_end_users SET roles = $1, updated_at = CURRENT_TIMESTAMP WHERE id = $2",
        )
        .bind(roles_json)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(AppError::from)?;
        Ok(())
    }

    async fn remove_role_from_all_users(&self, domain_id: Uuid, role_name: &str) -> AppResult<()> {
        // Remove role from JSONB array for all users in domain
        sqlx::query(
            r#"
            UPDATE domain_end_users
            SET roles = roles - $1, updated_at = CURRENT_TIMESTAMP
            WHERE domain_id = $2 AND roles @> to_jsonb($1::text)
            "#,
        )
        .bind(role_name)
        .bind(domain_id)
        .execute(&self.pool)
        .await
        .map_err(AppError::from)?;
        Ok(())
    }

    async fn count_users_with_role(&self, domain_id: Uuid, role_name: &str) -> AppResult<i64> {
        let row: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM domain_end_users WHERE domain_id = $1 AND roles @> to_jsonb($2::text)",
        )
        .bind(domain_id)
        .bind(role_name)
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::from)?;
        Ok(row.0)
    }
}
