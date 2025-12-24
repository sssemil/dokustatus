use std::sync::Arc;

use async_trait::async_trait;
use base64::Engine;
use chrono::NaiveDateTime;
use sha2::{Digest, Sha256};
use tracing::instrument;
use uuid::Uuid;

use crate::app_error::{AppError, AppResult};
use crate::application::email_templates::{primary_button, wrap_email};
use crate::application::use_cases::domain::DomainRepo;
use crate::domain::entities::domain::DomainStatus;
use crate::infra::crypto::ProcessCipher;

// ============================================================================
// Repository Traits
// ============================================================================

#[async_trait]
pub trait DomainAuthConfigRepo: Send + Sync {
    async fn get_by_domain_id(&self, domain_id: Uuid) -> AppResult<Option<DomainAuthConfigProfile>>;
    async fn upsert(
        &self,
        domain_id: Uuid,
        magic_link_enabled: bool,
        google_oauth_enabled: bool,
        redirect_url: Option<&str>,
    ) -> AppResult<DomainAuthConfigProfile>;
    async fn delete(&self, domain_id: Uuid) -> AppResult<()>;
}

#[async_trait]
pub trait DomainAuthMagicLinkRepo: Send + Sync {
    async fn get_by_domain_id(&self, domain_id: Uuid) -> AppResult<Option<DomainAuthMagicLinkProfile>>;
    async fn upsert(
        &self,
        domain_id: Uuid,
        resend_api_key_encrypted: &str,
        from_email: &str,
    ) -> AppResult<DomainAuthMagicLinkProfile>;
    async fn delete(&self, domain_id: Uuid) -> AppResult<()>;
}

#[async_trait]
pub trait DomainEndUserRepo: Send + Sync {
    async fn get_by_id(&self, id: Uuid) -> AppResult<Option<DomainEndUserProfile>>;
    async fn get_by_domain_and_email(&self, domain_id: Uuid, email: &str) -> AppResult<Option<DomainEndUserProfile>>;
    async fn upsert(&self, domain_id: Uuid, email: &str) -> AppResult<DomainEndUserProfile>;
    async fn mark_verified(&self, id: Uuid) -> AppResult<DomainEndUserProfile>;
    async fn update_last_login(&self, id: Uuid) -> AppResult<()>;
    async fn list_by_domain(&self, domain_id: Uuid) -> AppResult<Vec<DomainEndUserProfile>>;
}

#[async_trait]
pub trait DomainMagicLinkStore: Send + Sync {
    async fn save(&self, token_hash: &str, end_user_id: Uuid, domain_id: Uuid, ttl_minutes: i64) -> AppResult<()>;
    async fn consume(&self, token_hash: &str) -> AppResult<Option<DomainMagicLinkData>>;
}

#[async_trait]
pub trait DomainEmailSender: Send + Sync {
    async fn send(&self, api_key: &str, from_email: &str, to: &str, subject: &str, html: &str) -> AppResult<()>;
}

// ============================================================================
// Profile Types
// ============================================================================

#[derive(Debug, Clone)]
pub struct DomainAuthConfigProfile {
    pub id: Uuid,
    pub domain_id: Uuid,
    pub magic_link_enabled: bool,
    pub google_oauth_enabled: bool,
    pub redirect_url: Option<String>,
    pub created_at: Option<NaiveDateTime>,
    pub updated_at: Option<NaiveDateTime>,
}

#[derive(Debug, Clone)]
pub struct DomainAuthMagicLinkProfile {
    pub id: Uuid,
    pub domain_id: Uuid,
    pub resend_api_key_encrypted: String,
    pub from_email: String,
    pub created_at: Option<NaiveDateTime>,
    pub updated_at: Option<NaiveDateTime>,
}

#[derive(Debug, Clone)]
pub struct DomainEndUserProfile {
    pub id: Uuid,
    pub domain_id: Uuid,
    pub email: String,
    pub email_verified_at: Option<NaiveDateTime>,
    pub last_login_at: Option<NaiveDateTime>,
    pub created_at: Option<NaiveDateTime>,
    pub updated_at: Option<NaiveDateTime>,
}

#[derive(Debug, Clone)]
pub struct DomainMagicLinkData {
    pub end_user_id: Uuid,
    pub domain_id: Uuid,
}

// ============================================================================
// Public Config (for ingress page)
// ============================================================================

#[derive(Debug, Clone)]
pub struct PublicDomainConfig {
    pub domain_id: Uuid,
    pub domain: String,
    pub magic_link_enabled: bool,
    pub google_oauth_enabled: bool,
    pub redirect_url: Option<String>,
}

// ============================================================================
// Use Cases
// ============================================================================

#[derive(Clone)]
pub struct DomainAuthUseCases {
    domain_repo: Arc<dyn DomainRepo>,
    auth_config_repo: Arc<dyn DomainAuthConfigRepo>,
    magic_link_config_repo: Arc<dyn DomainAuthMagicLinkRepo>,
    end_user_repo: Arc<dyn DomainEndUserRepo>,
    magic_link_store: Arc<dyn DomainMagicLinkStore>,
    email_sender: Arc<dyn DomainEmailSender>,
    cipher: ProcessCipher,
    global_resend_api_key: Option<String>,
    global_from_email: Option<String>,
}

impl DomainAuthUseCases {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        domain_repo: Arc<dyn DomainRepo>,
        auth_config_repo: Arc<dyn DomainAuthConfigRepo>,
        magic_link_config_repo: Arc<dyn DomainAuthMagicLinkRepo>,
        end_user_repo: Arc<dyn DomainEndUserRepo>,
        magic_link_store: Arc<dyn DomainMagicLinkStore>,
        email_sender: Arc<dyn DomainEmailSender>,
        cipher: ProcessCipher,
        global_resend_api_key: Option<String>,
        global_from_email: Option<String>,
    ) -> Self {
        Self {
            domain_repo,
            auth_config_repo,
            magic_link_config_repo,
            end_user_repo,
            magic_link_store,
            email_sender,
            cipher,
            global_resend_api_key,
            global_from_email,
        }
    }

    // ========================================================================
    // Public endpoints (for ingress page)
    // ========================================================================

    /// Get public config for a domain (used by login page to show available auth methods)
    #[instrument(skip(self))]
    pub async fn get_public_config(&self, domain_name: &str) -> AppResult<PublicDomainConfig> {
        let domain = self
            .domain_repo
            .get_by_domain(domain_name)
            .await?
            .ok_or(AppError::NotFound)?;

        if domain.status != DomainStatus::Verified {
            return Err(AppError::NotFound);
        }

        let auth_config = self.auth_config_repo.get_by_domain_id(domain.id).await?;

        Ok(PublicDomainConfig {
            domain_id: domain.id,
            domain: domain.domain,
            magic_link_enabled: auth_config.as_ref().map(|c| c.magic_link_enabled).unwrap_or(false),
            google_oauth_enabled: auth_config.as_ref().map(|c| c.google_oauth_enabled).unwrap_or(false),
            redirect_url: auth_config.and_then(|c| c.redirect_url),
        })
    }

    /// Request a magic link for domain end-user login
    #[instrument(skip(self))]
    pub async fn request_magic_link(
        &self,
        domain_name: &str,
        email: &str,
        session_id: &str,
        ttl_minutes: i64,
    ) -> AppResult<()> {
        // Get domain and verify it's active
        let domain = self
            .domain_repo
            .get_by_domain(domain_name)
            .await?
            .ok_or(AppError::NotFound)?;

        if domain.status != DomainStatus::Verified {
            return Err(AppError::NotFound);
        }

        // Check if magic link is enabled
        let auth_config = self
            .auth_config_repo
            .get_by_domain_id(domain.id)
            .await?
            .ok_or_else(|| AppError::InvalidInput("Authentication not configured for this domain".into()))?;

        if !auth_config.magic_link_enabled {
            return Err(AppError::InvalidInput("Magic link login is not enabled for this domain".into()));
        }

        // Get Resend config (domain-specific or fallback to global)
        let (api_key, from_email) = self.get_email_config(domain.id).await?;

        // Create or get end-user
        let end_user = self.end_user_repo.upsert(domain.id, email).await?;

        // Generate token (bound to domain)
        let raw = generate_token();
        let token_hash = hash_domain_token(&raw, session_id, domain_name);

        // Save to Redis
        self.magic_link_store
            .save(&token_hash, end_user.id, domain.id, ttl_minutes)
            .await?;

        // Build magic link URL (uses the custom domain)
        let link = format!("https://{}/magic?token={}", domain_name, raw);

        // Send email
        let subject = "Sign in to your account";
        let headline = "Your sign-in link is ready";
        let lead = format!(
            "Use this secure link to finish signing in. It expires in {} minutes.",
            ttl_minutes
        );
        let button_label = "Sign in";
        let reason = format!("you requested to sign in to {}", domain_name);
        let footer_note =
            "This one-time link keeps your account protected; delete this email if you did not request it.";

        let button = primary_button(&link, button_label);
        let origin = format!("https://{}", domain_name);
        let html = wrap_email(
            &origin,
            headline,
            &lead,
            &format!(
                "{button}<p style=\"margin:12px 0 0;font-size:14px;color:#4b5563;\">If the button does not work, copy and paste this URL:<br><span style=\"word-break:break-all;color:#111827;\">{link}</span></p>"
            ),
            &reason,
            Some(footer_note),
        );

        self.email_sender.send(&api_key, &from_email, email, subject, &html).await
    }

    /// Consume a magic link token and return end-user info
    #[instrument(skip(self))]
    pub async fn consume_magic_link(
        &self,
        domain_name: &str,
        raw_token: &str,
        session_id: &str,
    ) -> AppResult<Option<DomainEndUserProfile>> {
        let token_hash = hash_domain_token(raw_token, session_id, domain_name);

        if let Some(data) = self.magic_link_store.consume(&token_hash).await? {
            // Mark user as verified and update last login
            let end_user = self.end_user_repo.mark_verified(data.end_user_id).await?;
            return Ok(Some(end_user));
        }

        Ok(None)
    }

    // ========================================================================
    // Protected endpoints (for dashboard)
    // ========================================================================

    /// Get auth config for a domain (workspace owner only)
    #[instrument(skip(self))]
    pub async fn get_auth_config(
        &self,
        user_id: Uuid,
        domain_id: Uuid,
    ) -> AppResult<(DomainAuthConfigProfile, Option<DomainAuthMagicLinkProfile>)> {
        // Verify ownership
        let domain = self
            .domain_repo
            .get_by_id(domain_id)
            .await?
            .ok_or(AppError::NotFound)?;

        if domain.user_id != user_id {
            return Err(AppError::InvalidCredentials);
        }

        let auth_config = self
            .auth_config_repo
            .get_by_domain_id(domain_id)
            .await?
            .unwrap_or(DomainAuthConfigProfile {
                id: Uuid::nil(),
                domain_id,
                magic_link_enabled: false,
                google_oauth_enabled: false,
                redirect_url: None,
                created_at: None,
                updated_at: None,
            });

        let magic_link_config = self.magic_link_config_repo.get_by_domain_id(domain_id).await?;

        Ok((auth_config, magic_link_config))
    }

    /// Update auth config for a domain (workspace owner only)
    #[instrument(skip(self, resend_api_key))]
    pub async fn update_auth_config(
        &self,
        user_id: Uuid,
        domain_id: Uuid,
        magic_link_enabled: bool,
        google_oauth_enabled: bool,
        redirect_url: Option<&str>,
        resend_api_key: Option<&str>,
        from_email: Option<&str>,
    ) -> AppResult<()> {
        // Verify ownership
        let domain = self
            .domain_repo
            .get_by_id(domain_id)
            .await?
            .ok_or(AppError::NotFound)?;

        if domain.user_id != user_id {
            return Err(AppError::InvalidCredentials);
        }

        if domain.status != DomainStatus::Verified {
            return Err(AppError::InvalidInput("Domain must be verified before configuring authentication".into()));
        }

        // Update general auth config
        self.auth_config_repo
            .upsert(domain_id, magic_link_enabled, google_oauth_enabled, redirect_url)
            .await?;

        // Update magic link config if provided
        if let (Some(api_key), Some(from)) = (resend_api_key, from_email) {
            let encrypted_key = self.cipher.encrypt(api_key)?;
            self.magic_link_config_repo
                .upsert(domain_id, &encrypted_key, from)
                .await?;
        }

        Ok(())
    }

    /// List end-users for a domain (workspace owner only)
    #[instrument(skip(self))]
    pub async fn list_end_users(&self, user_id: Uuid, domain_id: Uuid) -> AppResult<Vec<DomainEndUserProfile>> {
        // Verify ownership
        let domain = self
            .domain_repo
            .get_by_id(domain_id)
            .await?
            .ok_or(AppError::NotFound)?;

        if domain.user_id != user_id {
            return Err(AppError::InvalidCredentials);
        }

        self.end_user_repo.list_by_domain(domain_id).await
    }

    // ========================================================================
    // Helpers
    // ========================================================================

    /// Get email config: domain-specific if available, otherwise global
    async fn get_email_config(&self, domain_id: Uuid) -> AppResult<(String, String)> {
        // Try domain-specific config first
        if let Some(config) = self.magic_link_config_repo.get_by_domain_id(domain_id).await? {
            let api_key = self.cipher.decrypt(&config.resend_api_key_encrypted)?;
            return Ok((api_key, config.from_email));
        }

        // Fall back to global config
        match (&self.global_resend_api_key, &self.global_from_email) {
            (Some(key), Some(from)) => Ok((key.clone(), from.clone())),
            _ => Err(AppError::InvalidInput("Email sending not configured for this domain".into())),
        }
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

fn generate_token() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

/// Hash token bound to domain to prevent cross-domain reuse
fn hash_domain_token(raw: &str, session_id: &str, domain: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(raw.as_bytes());
    hasher.update(session_id.as_bytes());
    hasher.update(domain.as_bytes());
    let out = hasher.finalize();
    hex::encode(out)
}

/// Extract root domain from hostname for cookie scoping
/// e.g., "login.example.com" -> "example.com"
/// e.g., "app.staging.example.co.uk" -> "example.co.uk"
pub fn get_root_domain(hostname: &str) -> String {
    let parts: Vec<&str> = hostname.split('.').collect();

    // Handle common multi-part TLDs
    let multi_part_tlds = ["co.uk", "com.au", "co.nz", "com.br", "co.jp"];
    let hostname_lower = hostname.to_lowercase();
    for tld in &multi_part_tlds {
        if hostname_lower.ends_with(tld) {
            // For multi-part TLDs, we need 3 parts minimum (domain + tld)
            if parts.len() >= 3 {
                let tld_parts: Vec<&str> = tld.split('.').collect();
                let domain_start = parts.len() - tld_parts.len() - 1;
                return parts[domain_start..].join(".");
            }
        }
    }

    // Standard TLDs: take last 2 parts
    if parts.len() >= 2 {
        return parts[parts.len() - 2..].join(".");
    }

    // Fallback: return as-is
    hostname.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_root_domain() {
        assert_eq!(get_root_domain("login.example.com"), "example.com");
        assert_eq!(get_root_domain("app.staging.example.com"), "example.com");
        assert_eq!(get_root_domain("example.com"), "example.com");
        assert_eq!(get_root_domain("login.example.co.uk"), "example.co.uk");
        assert_eq!(get_root_domain("app.example.co.uk"), "example.co.uk");
    }
}
