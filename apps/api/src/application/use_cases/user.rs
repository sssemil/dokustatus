use std::sync::Arc;

use async_trait::async_trait;
use base64::Engine;
use sha2::{Digest, Sha256};
use tracing::instrument;
use uuid::Uuid;

use crate::app_error::AppResult;
use crate::application::email_templates::{primary_button, wrap_email};

#[async_trait]
pub trait UserRepo: Send + Sync {
    async fn upsert_by_email(&self, email: &str) -> AppResult<UserProfile>;
    async fn get_profile_by_id(&self, user_id: Uuid) -> AppResult<Option<UserProfile>>;
    async fn get_waitlist_position(&self, user_id: Uuid) -> AppResult<Option<WaitlistPosition>>;
    async fn delete_user(&self, user_id: Uuid) -> AppResult<()>;
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
        let profile = self.repo.upsert_by_email(email).await?;
        let user_id = profile.id;
        let raw = generate_token();
        let token_hash = hash_token(&raw, session_id);
        self.magic_links
            .save(&token_hash, user_id, ttl_minutes)
            .await?;
        let origin = self.app_origin.trim_end_matches('/');
        let link = format!("{}/magic?token={}", origin, raw);

        let subject = "Sign in to your account";
        let headline = "Your sign-in link is ready";
        let lead = format!(
            "Use this secure link to finish signing in. It expires in {} minutes.",
            ttl_minutes
        );
        let button_label = "Sign in";
        let reason = format!(
            "you requested to sign in to {}",
            self.app_origin.trim_end_matches('/')
        );
        let footer_note =
            "This one-time link keeps your account protected; delete this email if you did not request it.";

        let button = primary_button(&link, button_label);
        let html = wrap_email(
            &self.app_origin,
            headline,
            &lead,
            &format!(
                "{button}<p style=\"margin:12px 0 0;font-size:14px;color:#4b5563;\">If the button does not work, copy and paste this URL:<br><span style=\"word-break:break-all;color:#111827;\">{link}</span></p>"
            ),
            &reason,
            Some(footer_note),
        );
        self.email.send(&profile.email, subject, &html).await
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

    #[instrument(skip(self))]
    pub async fn delete_account(&self, user_id: Uuid) -> AppResult<()> {
        let profile = self
            .repo
            .get_profile_by_id(user_id)
            .await?
            .ok_or(crate::app_error::AppError::InvalidCredentials)?;

        let subject = "Your account has been deleted";
        let headline = "Account deleted";
        let lead = "Your account and all associated data have been permanently deleted.";
        let body_text = "If you did not request this, please contact support immediately.";
        let reason = format!(
            "your account on {} was deleted",
            self.app_origin.trim_end_matches('/')
        );
        let footer = "This is an automated message, no action is required.";
        let body = wrap_email(
            &self.app_origin,
            headline,
            lead,
            &format!(
                "<p style=\"margin:12px 0 0;color:#374151;\">{}</p>",
                body_text
            ),
            &reason,
            Some(footer),
        );

        self.repo.delete_user(user_id).await?;
        let _ = self.email.send(&profile.email, subject, &body).await;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct UserProfile {
    pub id: Uuid,
    pub email: String,
    pub updated_at: Option<chrono::NaiveDateTime>,
    pub on_waitlist: bool,
}

#[derive(Debug, Clone)]
pub struct WaitlistPosition {
    pub position: u32,
    pub total: u32,
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
