use async_trait::async_trait;
use chrono::NaiveDateTime;
use serde::Serialize;
use uuid::Uuid;

use crate::{
    adapters::persistence::PostgresPersistence,
    app_error::{AppError, AppResult},
    application::language::UserLanguage,
    use_cases::user::{UserProfile, UserRepo},
};

// User struct as stored in the db.
#[derive(sqlx::FromRow, Debug, Serialize)]
pub struct UserDb {
    pub id: Uuid,
    pub created_at: Option<NaiveDateTime>,
    pub updated_at: Option<NaiveDateTime>,
    pub email: String,
    pub language: String,
}

#[async_trait]
impl UserRepo for PostgresPersistence {
    async fn upsert_by_email(&self, email: &str, language: Option<&str>) -> AppResult<UserProfile> {
        let lang = UserLanguage::from_raw(language.map(|l| l.trim()));
        let id = Uuid::new_v4();
        let rec = sqlx::query_as!(
            UserDb,
            r#"
                INSERT INTO users (id, email, language)
                VALUES ($1, $2, $3)
                ON CONFLICT (email) DO UPDATE
                SET language = COALESCE($3, users.language)
                RETURNING id, email, created_at, updated_at, language
            "#,
            id,
            email,
            lang.as_str(),
        )
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::from)?;
        Ok(UserProfile {
            id: rec.id,
            email: rec.email,
            language: rec.language,
            updated_at: rec.updated_at,
        })
    }

    async fn get_profile_by_id(&self, user_id: Uuid) -> AppResult<Option<UserProfile>> {
        let rec = sqlx::query_as!(
            UserDb,
            "SELECT id, email, created_at, updated_at, language FROM users WHERE id = $1",
            user_id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::from)?;
        Ok(rec.map(|r| UserProfile {
            id: r.id,
            email: r.email,
            language: r.language,
            updated_at: r.updated_at,
        }))
    }

    async fn update_language(&self, user_id: Uuid, language: &str) -> AppResult<()> {
        let lang = UserLanguage::from_raw(Some(language.trim()));
        sqlx::query!(
            "UPDATE users SET language = $2 WHERE id = $1",
            user_id,
            lang.as_str()
        )
        .execute(&self.pool)
        .await
        .map_err(AppError::from)?;
        Ok(())
    }

    async fn delete_user(&self, user_id: Uuid) -> AppResult<()> {
        sqlx::query!("DELETE FROM users WHERE id = $1", user_id)
            .execute(&self.pool)
            .await
            .map_err(AppError::from)?;
        Ok(())
    }
}
