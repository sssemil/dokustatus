use async_trait::async_trait;
use chrono::NaiveDateTime;
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
}
