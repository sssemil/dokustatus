use async_trait::async_trait;
use chrono::NaiveDateTime;
use serde::Serialize;
use uuid::Uuid;

use crate::{
    adapters::persistence::PostgresPersistence,
    app_error::{AppError, AppResult},
    use_cases::user::{UserProfile, UserRepo, WaitlistPosition},
};

#[derive(sqlx::FromRow, Debug, Serialize)]
pub struct UserDb {
    pub id: Uuid,
    pub created_at: Option<NaiveDateTime>,
    pub updated_at: Option<NaiveDateTime>,
    pub email: String,
    pub on_waitlist: bool,
}

#[async_trait]
impl UserRepo for PostgresPersistence {
    async fn upsert_by_email(&self, email: &str) -> AppResult<UserProfile> {
        let id = Uuid::new_v4();
        let rec = sqlx::query_as!(
            UserDb,
            r#"
                INSERT INTO users (id, email)
                VALUES ($1, $2)
                ON CONFLICT (email) DO UPDATE
                SET updated_at = CURRENT_TIMESTAMP
                RETURNING id, email, created_at, updated_at, on_waitlist
            "#,
            id,
            email,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::from)?;
        Ok(UserProfile {
            id: rec.id,
            email: rec.email,
            updated_at: rec.updated_at,
            on_waitlist: rec.on_waitlist,
        })
    }

    async fn get_profile_by_id(&self, user_id: Uuid) -> AppResult<Option<UserProfile>> {
        let rec = sqlx::query_as!(
            UserDb,
            "SELECT id, email, created_at, updated_at, on_waitlist FROM users WHERE id = $1",
            user_id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::from)?;
        Ok(rec.map(|r| UserProfile {
            id: r.id,
            email: r.email,
            updated_at: r.updated_at,
            on_waitlist: r.on_waitlist,
        }))
    }

    async fn get_waitlist_position(&self, user_id: Uuid) -> AppResult<Option<WaitlistPosition>> {
        let row = sqlx::query!(
            r#"
            SELECT
                position,
                total
            FROM (
                SELECT
                    id,
                    ROW_NUMBER() OVER (ORDER BY created_at ASC) as position,
                    COUNT(*) OVER () as total
                FROM users
                WHERE on_waitlist = true
            ) ranked
            WHERE id = $1
            "#,
            user_id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::from)?;

        Ok(row.map(|r| WaitlistPosition {
            position: r.position.unwrap_or(0) as u32,
            total: r.total.unwrap_or(0) as u32,
        }))
    }

    async fn delete_user(&self, user_id: Uuid) -> AppResult<()> {
        sqlx::query!("DELETE FROM users WHERE id = $1", user_id)
            .execute(&self.pool)
            .await
            .map_err(AppError::from)?;
        Ok(())
    }
}
