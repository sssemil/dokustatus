use std::sync::Arc;

use async_trait::async_trait;
use chrono::{NaiveDateTime, Utc};
use sha2::{Digest, Sha256};
use tracing::instrument;
use uuid::Uuid;
use base64::Engine;

use crate::app_error::AppResult;

#[async_trait]
pub trait UserRepo: Send + Sync {
    async fn find_or_create_by_email(&self, email: &str) -> AppResult<Uuid>;
    async fn create_magic_link(&self, user_id: Uuid, token_hash: &str, expires_at: NaiveDateTime) -> AppResult<()>;
    async fn get_valid_magic_link(&self, token_hash: &str, now: NaiveDateTime) -> AppResult<Option<Uuid>>;
    async fn consume_magic_link(&self, token_hash: &str) -> AppResult<()>;
}

#[async_trait]
pub trait EmailSender: Send + Sync {
    async fn send(&self, to: &str, subject: &str, html: &str) -> AppResult<()>;
}

#[derive(Clone)]
pub struct AuthUseCases {
    repo: Arc<dyn UserRepo>,
    email: Arc<dyn EmailSender>,
    app_origin: String,
}

impl AuthUseCases {
    pub fn new(repo: Arc<dyn UserRepo>, email: Arc<dyn EmailSender>, app_origin: String) -> Self {
        Self { repo, email, app_origin }
    }

    #[instrument(skip(self))]
    pub async fn request_magic_link(&self, email: &str, ttl_minutes: i64) -> AppResult<()> {
        let user_id = self.repo.find_or_create_by_email(email).await?;
        let raw = generate_token();
        let token_hash = hash_token(&raw);
        let expires_at = (Utc::now() + chrono::Duration::minutes(ttl_minutes)).naive_utc();
        self.repo.create_magic_link(user_id, &token_hash, expires_at).await?;
        let link = format!("{}/magic?token={}", self.app_origin, raw);
        self.email.send(email, "Your login link", &format!("<a href=\"{}\">Sign in</a>", link)).await
    }

    #[instrument(skip(self))]
    pub async fn consume_magic_link(&self, raw_token: &str) -> AppResult<Option<Uuid>> {
        let token_hash = hash_token(raw_token);
        let now = Utc::now().naive_utc();
        if let Some(user_id) = self.repo.get_valid_magic_link(&token_hash, now).await? {
            self.repo.consume_magic_link(&token_hash).await?;
            return Ok(Some(user_id));
        }
        Ok(None)
    }
}

fn generate_token() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

fn hash_token(raw: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(raw.as_bytes());
    let out = hasher.finalize();
    hex::encode(out)
}
