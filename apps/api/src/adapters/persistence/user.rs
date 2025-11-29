use async_trait::async_trait;
use chrono::{NaiveDateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

use crate::{
    adapters::persistence::PostgresPersistence,
    app_error::{AppError, AppResult},
    use_cases::user::UserRepo,
};

// User struct as stored in the db.
#[derive(sqlx::FromRow, Debug, Serialize)]
pub struct UserDb {
    pub id: Uuid,
    pub created_at: NaiveDateTime,
    pub email: String,
}

#[async_trait]
impl UserRepo for PostgresPersistence {
    async fn find_or_create_by_email(&self, email: &str) -> AppResult<Uuid> {
        // Try find
        if let Some(rec) = sqlx::query!("SELECT id FROM users WHERE email = $1", email)
            .fetch_optional(&self.pool)
            .await?
        {
            return Ok(rec.id);
        }
        // Insert
        let id = Uuid::new_v4();
        sqlx::query!("INSERT INTO users (id, email) VALUES ($1, $2)", id, email)
            .execute(&self.pool)
            .await
            .map_err(AppError::from)?;
        Ok(id)
    }

    async fn get_email_by_id(&self, user_id: Uuid) -> AppResult<Option<String>> {
        let rec = sqlx::query!("SELECT email FROM users WHERE id = $1", user_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(AppError::from)?;
        Ok(rec.map(|r| r.email))
    }

    async fn create_magic_link(
        &self,
        user_id: Uuid,
        token_hash: &str,
        expires_at: NaiveDateTime,
    ) -> AppResult<()> {
        sqlx::query!(
            "INSERT INTO magic_links (token_hash, user_id, expires_at) VALUES ($1, $2, $3)",
            token_hash,
            user_id,
            expires_at
        )
        .execute(&self.pool)
        .await
        .map_err(AppError::from)?;
        Ok(())
    }

    async fn get_valid_magic_link(
        &self,
        token_hash: &str,
        now: NaiveDateTime,
    ) -> AppResult<Option<Uuid>> {
        let rec = sqlx::query!(
            r#"SELECT user_id FROM magic_links
               WHERE token_hash = $1 AND consumed_at IS NULL AND expires_at > $2"#,
            token_hash,
            now
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::from)?;
        Ok(rec.map(|r| r.user_id))
    }

    async fn consume_magic_link(&self, token_hash: &str) -> AppResult<()> {
        sqlx::query!(
            "UPDATE magic_links SET consumed_at = $2 WHERE token_hash = $1 AND consumed_at IS NULL",
            token_hash,
            Utc::now().naive_utc()
        )
        .execute(&self.pool)
        .await
        .map_err(AppError::from)?;
        Ok(())
    }
}
