use async_trait::async_trait;
use sqlx::FromRow;
use uuid::Uuid;

use crate::{
    adapters::persistence::PostgresPersistence,
    app_error::AppResult,
    use_cases::pass_status::{DocumentTrack, PassStatusRepo},
};

#[derive(FromRow)]
struct DbDocumentTrack {
    id: Uuid,
    user_id: Uuid,
    number_ciphertext: String,
    #[allow(dead_code)]
    number_hash: String,
    last_status: Option<String>,
    last_pickup: Option<String>,
    last_checked_at: Option<chrono::NaiveDateTime>,
    status_changed_at: Option<chrono::NaiveDateTime>,
    created_at: chrono::NaiveDateTime,
}

impl PostgresPersistence {
    fn decrypt_track(&self, db: DbDocumentTrack) -> AppResult<DocumentTrack> {
        let plaintext = self.cipher.decrypt(&db.number_ciphertext)?;

        Ok(DocumentTrack {
            id: db.id,
            user_id: db.user_id,
            number: plaintext,
            last_status: db.last_status,
            last_pickup: db.last_pickup,
            last_checked_at: db.last_checked_at,
            status_changed_at: db.status_changed_at,
            created_at: db.created_at,
            typ: None,
        })
    }
}

#[async_trait]
impl PassStatusRepo for PostgresPersistence {
    async fn upsert_track(&self, user_id: Uuid, number: &str) -> AppResult<DocumentTrack> {
        let id = Uuid::new_v4();
        let number_hash = self.cipher.hash(number);
        let ciphertext = self.cipher.encrypt(number)?;
        let rec = sqlx::query_as::<_, DbDocumentTrack>(
            r#"INSERT INTO document_tracks (id, user_id, number_ciphertext, number_hash)
               VALUES ($1, $2, $3, $4)
               ON CONFLICT (user_id, number_hash)
               DO UPDATE SET
                    number_ciphertext = EXCLUDED.number_ciphertext,
                    number_hash = EXCLUDED.number_hash
               RETURNING id, user_id, number_ciphertext, number_hash, last_status, last_pickup, last_checked_at, status_changed_at, created_at"#,
        )
        .bind(id)
        .bind(user_id)
        .bind(ciphertext)
        .bind(number_hash)
        .fetch_one(&self.pool)
        .await?;

        self.decrypt_track(rec)
    }

    async fn list_tracks_for_user(&self, user_id: Uuid) -> AppResult<Vec<DocumentTrack>> {
        let recs = sqlx::query_as::<_, DbDocumentTrack>(
            r#"SELECT id, user_id, number_ciphertext, number_hash, last_status, last_pickup, last_checked_at, status_changed_at, created_at
               FROM document_tracks
               WHERE user_id = $1
               ORDER BY created_at DESC"#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        recs.into_iter().map(|r| self.decrypt_track(r)).collect()
    }

    async fn list_all_tracks(&self) -> AppResult<Vec<DocumentTrack>> {
        let recs = sqlx::query_as::<_, DbDocumentTrack>(
            r#"SELECT id, user_id, number_ciphertext, number_hash, last_status, last_pickup, last_checked_at, status_changed_at, created_at
               FROM document_tracks
               ORDER BY created_at DESC"#,
        )
        .fetch_all(&self.pool)
        .await?;

        recs.into_iter().map(|r| self.decrypt_track(r)).collect()
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
