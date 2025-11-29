use async_trait::async_trait;
use uuid::Uuid;

use crate::{
    adapters::persistence::PostgresPersistence,
    app_error::AppResult,
    use_cases::pass_status::{DocumentTrack, PassStatusRepo},
};

#[async_trait]
impl PassStatusRepo for PostgresPersistence {
    async fn upsert_track(&self, user_id: Uuid, number: &str) -> AppResult<DocumentTrack> {
        let id = Uuid::new_v4();
        let rec = sqlx::query_as::<_, DocumentTrack>(
            r#"INSERT INTO document_tracks (id, user_id, number)
               VALUES ($1, $2, $3)
               ON CONFLICT (user_id, number)
               DO UPDATE SET number = EXCLUDED.number
               RETURNING id, user_id, number, last_status, last_pickup, last_checked_at, status_changed_at, created_at"#,
        )
        .bind(id)
        .bind(user_id)
        .bind(number)
        .fetch_one(&self.pool)
        .await?;

        Ok(rec)
    }

    async fn list_tracks_for_user(&self, user_id: Uuid) -> AppResult<Vec<DocumentTrack>> {
        let recs = sqlx::query_as::<_, DocumentTrack>(
            r#"SELECT id, user_id, number, last_status, last_pickup, last_checked_at, status_changed_at, created_at
               FROM document_tracks
               WHERE user_id = $1
               ORDER BY created_at DESC"#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(recs)
    }

    async fn list_all_tracks(&self) -> AppResult<Vec<DocumentTrack>> {
        let recs = sqlx::query_as::<_, DocumentTrack>(
            r#"SELECT id, user_id, number, last_status, last_pickup, last_checked_at, status_changed_at, created_at
               FROM document_tracks
               ORDER BY created_at DESC"#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(recs)
    }

    async fn delete_track(&self, user_id: Uuid, track_id: Uuid) -> AppResult<()> {
        sqlx::query(r#"DELETE FROM document_tracks WHERE id = $1 AND user_id = $2"#)
            .bind(track_id)
            .bind(user_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn save_status(
        &self,
        track_id: Uuid,
        status: Option<String>,
        pickup: Option<String>,
        checked_at: chrono::NaiveDateTime,
    ) -> AppResult<()> {
        sqlx::query(
            r#"UPDATE document_tracks
               SET last_status = $2,
                   last_pickup = $3,
                   last_checked_at = $4,
                   status_changed_at = CASE WHEN last_status IS DISTINCT FROM $2 THEN $4 ELSE status_changed_at END
               WHERE id = $1"#,
        )
        .bind(track_id)
        .bind(status)
        .bind(pickup)
        .bind(checked_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}
