use std::sync::Arc;

use async_trait::async_trait;
use base64::Engine;
use sha2::{Digest, Sha256};
use tracing::instrument;
use uuid::Uuid;

use crate::app_error::AppResult;

#[async_trait]
pub trait UserRepo: Send + Sync {
    async fn find_or_create_by_email(&self, email: &str) -> AppResult<Uuid>;
    async fn get_email_by_id(&self, user_id: Uuid) -> AppResult<Option<String>>;
}

#[async_trait]
pub trait EmailSender: Send + Sync {
    async fn send(&self, to: &str, subject: &str, html: &str) -> AppResult<()>;
}

#[async_trait]
pub trait MagicLinkStore: Send + Sync {
    async fn save(&self, token_hash: &str, user_id: Uuid, ttl_minutes: i64) -> AppResult<()>;
    async fn consume(&self, token_hash: &str) -> AppResult<Option<Uuid>>;
}

#[derive(Clone)]
pub struct AuthUseCases {
    repo: Arc<dyn UserRepo>,
    magic_links: Arc<dyn MagicLinkStore>,
    email: Arc<dyn EmailSender>,
    app_origin: String,
}

impl AuthUseCases {
    pub fn new(
        repo: Arc<dyn UserRepo>,
        magic_links: Arc<dyn MagicLinkStore>,
        email: Arc<dyn EmailSender>,
        app_origin: String,
    ) -> Self {
        Self {
            repo,
            magic_links,
            email,
            app_origin,
        }
    }

    #[instrument(skip(self))]
    pub async fn request_magic_link(
        &self,
        email: &str,
        session_id: &str,
        ttl_minutes: i64,
    ) -> AppResult<()> {
        let user_id = self.repo.find_or_create_by_email(email).await?;
        let raw = generate_token();
        let token_hash = hash_token(&raw, session_id);
        self.magic_links
            .save(&token_hash, user_id, ttl_minutes)
            .await?;
        let link = format!("{}/magic?token={}", self.app_origin, raw);
        self.email
            .send(
                email,
                "Your login link",
                &format!("<a href=\"{}\">Sign in</a>", link),
            )
            .await
    }

    #[instrument(skip(self))]
    pub async fn consume_magic_link(
        &self,
        raw_token: &str,
        session_id: &str,
    ) -> AppResult<Option<Uuid>> {
        let token_hash = hash_token(raw_token, session_id);
        if let Some(user_id) = self.magic_links.consume(&token_hash).await? {
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

fn hash_token(raw: &str, session_id: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(raw.as_bytes());
    hasher.update(session_id.as_bytes());
    let out = hasher.finalize();
    hex::encode(out)
}
