use std::sync::Arc;

use async_trait::async_trait;
use base64::Engine;
use chrono::NaiveDateTime;
use sha2::{Digest, Sha256};
use tracing::instrument;
use uuid::Uuid;

use crate::app_error::{AppError, AppResult};
use crate::application::email_templates::{
    account_created_email, account_frozen_email, account_invited_email,
    account_unfrozen_email, account_whitelisted_email, primary_button, wrap_email,
};
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
        whitelist_enabled: bool,
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
    async fn update_from_email(&self, domain_id: Uuid, from_email: &str) -> AppResult<()>;
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
    async fn delete(&self, id: Uuid) -> AppResult<()>;
    async fn set_frozen(&self, id: Uuid, frozen: bool) -> AppResult<()>;
    async fn set_whitelisted(&self, id: Uuid, whitelisted: bool) -> AppResult<()>;
    async fn whitelist_all_in_domain(&self, domain_id: Uuid) -> AppResult<()>;
    async fn count_by_domain_ids(&self, domain_ids: &[Uuid]) -> AppResult<i64>;
    async fn get_waitlist_position(&self, domain_id: Uuid, user_id: Uuid) -> AppResult<i64>;
    async fn set_roles(&self, id: Uuid, roles: &[String]) -> AppResult<()>;
    async fn remove_role_from_all_users(&self, domain_id: Uuid, role_name: &str) -> AppResult<()>;
    async fn count_users_with_role(&self, domain_id: Uuid, role_name: &str) -> AppResult<i64>;
}

#[async_trait]
pub trait DomainMagicLinkStore: Send + Sync {
    async fn save(&self, token_hash: &str, end_user_id: Uuid, domain_id: Uuid, session_id: &str, ttl_minutes: i64) -> AppResult<()>;
    /// Consume a magic link. Returns the data if session matches, or SessionMismatch error if different browser/device.
    async fn consume(&self, token_hash: &str, session_id: &str) -> AppResult<Option<DomainMagicLinkData>>;
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
    pub whitelist_enabled: bool,
    pub access_token_ttl_secs: i32,
    pub refresh_token_ttl_days: i32,
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
    pub roles: Vec<String>,
    pub email_verified_at: Option<NaiveDateTime>,
    pub last_login_at: Option<NaiveDateTime>,
    pub is_frozen: bool,
    pub is_whitelisted: bool,
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
}

impl DomainAuthUseCases {
    pub fn new(
        domain_repo: Arc<dyn DomainRepo>,
        auth_config_repo: Arc<dyn DomainAuthConfigRepo>,
        magic_link_config_repo: Arc<dyn DomainAuthMagicLinkRepo>,
        end_user_repo: Arc<dyn DomainEndUserRepo>,
        magic_link_store: Arc<dyn DomainMagicLinkStore>,
        email_sender: Arc<dyn DomainEmailSender>,
        cipher: ProcessCipher,
    ) -> Self {
        Self {
            domain_repo,
            auth_config_repo,
            magic_link_config_repo,
            end_user_repo,
            magic_link_store,
            email_sender,
            cipher,
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

        // Default redirect URL to https://{domain} if not configured
        let redirect_url = auth_config
            .as_ref()
            .and_then(|c| c.redirect_url.clone())
            .unwrap_or_else(|| format!("https://{}", domain.domain));

        Ok(PublicDomainConfig {
            domain_id: domain.id,
            domain: domain.domain,
            magic_link_enabled: auth_config.as_ref().map(|c| c.magic_link_enabled).unwrap_or(false),
            google_oauth_enabled: auth_config.as_ref().map(|c| c.google_oauth_enabled).unwrap_or(false),
            redirect_url: Some(redirect_url),
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

        // Generate token (bound to domain only, session stored separately for verification)
        let raw = generate_token();
        let token_hash = hash_domain_token(&raw, domain_name);

        // Save to Redis with session_id for browser verification
        self.magic_link_store
            .save(&token_hash, end_user.id, domain.id, session_id, ttl_minutes)
            .await?;

        // Build magic link URL (uses reauth.{domain} for the login page)
        let reauth_hostname = format!("reauth.{}", domain_name);
        let link = format!("https://{}/magic?token={}", reauth_hostname, raw);

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
        let origin = format!("https://{}", reauth_hostname);
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
    /// Returns the user even if not whitelisted (caller should handle waitlist logic)
    /// Only blocks frozen users
    /// Sends welcome email on first login (when email_verified_at was null)
    #[instrument(skip(self))]
    pub async fn consume_magic_link(
        &self,
        domain_name: &str,
        raw_token: &str,
        session_id: &str,
    ) -> AppResult<Option<DomainEndUserProfile>> {
        let token_hash = hash_domain_token(raw_token, domain_name);

        if let Some(data) = self.magic_link_store.consume(&token_hash, session_id).await? {
            // Get the end user first to check access
            let end_user = self.end_user_repo.get_by_id(data.end_user_id).await?
                .ok_or(AppError::NotFound)?;

            // Check if user is frozen
            if end_user.is_frozen {
                return Err(AppError::AccountSuspended);
            }

            // Check if this is first login (email not verified yet)
            let is_first_login = end_user.email_verified_at.is_none();

            // Mark user as verified and update last login
            let end_user = self.end_user_repo.mark_verified(data.end_user_id).await?;

            // Send welcome email on first login
            if is_first_login {
                if let Ok((api_key, from_email)) = self.get_email_config(data.domain_id).await {
                    let app_origin = format!("https://reauth.{}", domain_name);
                    let (subject, html) = account_created_email(&app_origin, domain_name);
                    // Fire and forget - don't fail login if email fails
                    let _ = self.email_sender.send(&api_key, &from_email, &end_user.email, &subject, &html).await;
                }
            }

            return Ok(Some(end_user));
        }

        Ok(None)
    }

    /// Get waitlist position for a non-whitelisted user
    /// Returns the count of non-whitelisted users created before this user + 1
    #[instrument(skip(self))]
    pub async fn get_waitlist_position(&self, domain_id: Uuid, user_id: Uuid) -> AppResult<i64> {
        self.end_user_repo.get_waitlist_position(domain_id, user_id).await
    }

    // ========================================================================
    // Protected endpoints (for dashboard)
    // ========================================================================

    /// Get auth config for a domain (domain owner only)
    #[instrument(skip(self))]
    pub async fn get_auth_config(
        &self,
        end_user_id: Uuid,
        domain_id: Uuid,
    ) -> AppResult<(DomainAuthConfigProfile, Option<DomainAuthMagicLinkProfile>)> {
        // Verify ownership
        let domain = self
            .domain_repo
            .get_by_id(domain_id)
            .await?
            .ok_or(AppError::NotFound)?;

        if domain.owner_end_user_id != Some(end_user_id) {
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
                whitelist_enabled: false,
                access_token_ttl_secs: 86400,
                refresh_token_ttl_days: 30,
                created_at: None,
                updated_at: None,
            });

        let magic_link_config = self.magic_link_config_repo.get_by_domain_id(domain_id).await?;

        Ok((auth_config, magic_link_config))
    }

    /// Update auth config for a domain (domain owner only)
    #[instrument(skip(self, resend_api_key))]
    pub async fn update_auth_config(
        &self,
        end_user_id: Uuid,
        domain_id: Uuid,
        magic_link_enabled: bool,
        google_oauth_enabled: bool,
        redirect_url: Option<&str>,
        whitelist_enabled: bool,
        whitelist_all_existing: bool,
        resend_api_key: Option<&str>,
        from_email: Option<&str>,
    ) -> AppResult<()> {
        // Verify ownership
        let domain = self
            .domain_repo
            .get_by_id(domain_id)
            .await?
            .ok_or(AppError::NotFound)?;

        if domain.owner_end_user_id != Some(end_user_id) {
            return Err(AppError::InvalidCredentials);
        }

        if domain.status != DomainStatus::Verified {
            return Err(AppError::InvalidInput("Domain must be verified before configuring authentication".into()));
        }

        // Validate redirect URL is on the domain or a subdomain
        if let Some(url) = redirect_url {
            if !is_valid_redirect_url(url, &domain.domain) {
                return Err(AppError::InvalidInput(
                    format!("Redirect URL must be on {} or a subdomain", domain.domain)
                ));
            }
        }

        // If enabling whitelist and requested, whitelist all existing users
        if whitelist_enabled && whitelist_all_existing {
            self.end_user_repo.whitelist_all_in_domain(domain_id).await?;
        }

        // Update general auth config
        self.auth_config_repo
            .upsert(domain_id, magic_link_enabled, google_oauth_enabled, redirect_url, whitelist_enabled)
            .await?;

        // Update magic link config if provided
        match (resend_api_key, from_email) {
            // Both provided: upsert full config
            (Some(api_key), Some(from)) => {
                let encrypted_key = self.cipher.encrypt(api_key)?;
                self.magic_link_config_repo
                    .upsert(domain_id, &encrypted_key, from)
                    .await?;
            }
            // Only from_email provided: update just the from_email (config must exist)
            (None, Some(from)) => {
                self.magic_link_config_repo
                    .update_from_email(domain_id, from)
                    .await?;
            }
            // Only api_key or neither: do nothing
            _ => {}
        }

        Ok(())
    }

    /// List end-users for a domain (domain owner only)
    #[instrument(skip(self))]
    pub async fn list_end_users(&self, end_user_id: Uuid, domain_id: Uuid) -> AppResult<Vec<DomainEndUserProfile>> {
        // Verify ownership
        let domain = self
            .domain_repo
            .get_by_id(domain_id)
            .await?
            .ok_or(AppError::NotFound)?;

        if domain.owner_end_user_id != Some(end_user_id) {
            return Err(AppError::InvalidCredentials);
        }

        self.end_user_repo.list_by_domain(domain_id).await
    }

    /// Get a single end-user by ID (domain owner only)
    #[instrument(skip(self))]
    pub async fn get_end_user(
        &self,
        owner_end_user_id: Uuid,
        domain_id: Uuid,
        user_id: Uuid,
    ) -> AppResult<DomainEndUserProfile> {
        self.verify_domain_ownership(owner_end_user_id, domain_id).await?;

        let user = self
            .end_user_repo
            .get_by_id(user_id)
            .await?
            .ok_or(AppError::NotFound)?;

        if user.domain_id != domain_id {
            return Err(AppError::NotFound);
        }

        Ok(user)
    }

    /// Delete an end-user account (domain owner only)
    #[instrument(skip(self))]
    pub async fn delete_end_user(
        &self,
        owner_end_user_id: Uuid,
        domain_id: Uuid,
        user_id: Uuid,
    ) -> AppResult<()> {
        self.verify_domain_ownership(owner_end_user_id, domain_id).await?;

        let user = self
            .end_user_repo
            .get_by_id(user_id)
            .await?
            .ok_or(AppError::NotFound)?;

        if user.domain_id != domain_id {
            return Err(AppError::NotFound);
        }

        self.end_user_repo.delete(user_id).await
    }

    /// Freeze an end-user account (domain owner only)
    /// Sends suspension notification email
    #[instrument(skip(self))]
    pub async fn freeze_end_user(
        &self,
        owner_end_user_id: Uuid,
        domain_id: Uuid,
        user_id: Uuid,
    ) -> AppResult<()> {
        let domain = self.verify_domain_ownership_get_domain(owner_end_user_id, domain_id).await?;

        let user = self
            .end_user_repo
            .get_by_id(user_id)
            .await?
            .ok_or(AppError::NotFound)?;

        if user.domain_id != domain_id {
            return Err(AppError::NotFound);
        }

        // Only send email if user wasn't already frozen
        let was_not_frozen = !user.is_frozen;

        self.end_user_repo.set_frozen(user_id, true).await?;

        // Send suspension email
        if was_not_frozen {
            if let Ok((api_key, from_email)) = self.get_email_config(domain_id).await {
                let app_origin = format!("https://reauth.{}", domain.domain);
                let (subject, html) = account_frozen_email(&app_origin, &domain.domain);
                let _ = self.email_sender.send(&api_key, &from_email, &user.email, &subject, &html).await;
            }
        }

        Ok(())
    }

    /// Unfreeze an end-user account (domain owner only)
    /// Sends restoration notification email
    #[instrument(skip(self))]
    pub async fn unfreeze_end_user(
        &self,
        owner_end_user_id: Uuid,
        domain_id: Uuid,
        user_id: Uuid,
    ) -> AppResult<()> {
        let domain = self.verify_domain_ownership_get_domain(owner_end_user_id, domain_id).await?;

        let user = self
            .end_user_repo
            .get_by_id(user_id)
            .await?
            .ok_or(AppError::NotFound)?;

        if user.domain_id != domain_id {
            return Err(AppError::NotFound);
        }

        // Only send email if user was frozen
        let was_frozen = user.is_frozen;

        self.end_user_repo.set_frozen(user_id, false).await?;

        // Send restoration email
        if was_frozen {
            if let Ok((api_key, from_email)) = self.get_email_config(domain_id).await {
                let app_origin = format!("https://reauth.{}", domain.domain);
                let login_url = format!("https://reauth.{}/", domain.domain);
                let (subject, html) = account_unfrozen_email(&app_origin, &domain.domain, &login_url);
                let _ = self.email_sender.send(&api_key, &from_email, &user.email, &subject, &html).await;
            }
        }

        Ok(())
    }

    /// Whitelist an end-user (domain owner only)
    /// Sends approval email to the user
    #[instrument(skip(self))]
    pub async fn whitelist_end_user(
        &self,
        owner_end_user_id: Uuid,
        domain_id: Uuid,
        user_id: Uuid,
    ) -> AppResult<()> {
        let domain = self.verify_domain_ownership_get_domain(owner_end_user_id, domain_id).await?;

        let user = self
            .end_user_repo
            .get_by_id(user_id)
            .await?
            .ok_or(AppError::NotFound)?;

        if user.domain_id != domain_id {
            return Err(AppError::NotFound);
        }

        // Only send email if user wasn't already whitelisted
        let was_not_whitelisted = !user.is_whitelisted;

        self.end_user_repo.set_whitelisted(user_id, true).await?;

        // Send approval email
        if was_not_whitelisted {
            if let Ok((api_key, from_email)) = self.get_email_config(domain_id).await {
                let app_origin = format!("https://reauth.{}", domain.domain);
                let login_url = format!("https://reauth.{}/", domain.domain);
                let (subject, html) = account_whitelisted_email(&app_origin, &domain.domain, &login_url);
                let _ = self.email_sender.send(&api_key, &from_email, &user.email, &subject, &html).await;
            }
        }

        Ok(())
    }

    /// Remove end-user from whitelist (domain owner only)
    #[instrument(skip(self))]
    pub async fn unwhitelist_end_user(
        &self,
        owner_end_user_id: Uuid,
        domain_id: Uuid,
        user_id: Uuid,
    ) -> AppResult<()> {
        self.verify_domain_ownership(owner_end_user_id, domain_id).await?;

        let user = self
            .end_user_repo
            .get_by_id(user_id)
            .await?
            .ok_or(AppError::NotFound)?;

        if user.domain_id != domain_id {
            return Err(AppError::NotFound);
        }

        self.end_user_repo.set_whitelisted(user_id, false).await
    }

    /// Invite a user to the domain (domain owner only)
    /// Creates the user if they don't exist, optionally pre-whitelists them
    /// Sends an invitation email
    #[instrument(skip(self))]
    pub async fn invite_end_user(
        &self,
        owner_end_user_id: Uuid,
        domain_id: Uuid,
        email: &str,
        pre_whitelist: bool,
    ) -> AppResult<DomainEndUserProfile> {
        let domain = self.verify_domain_ownership_get_domain(owner_end_user_id, domain_id).await?;

        // Check if user already exists
        let existing = self.end_user_repo.get_by_domain_and_email(domain_id, email).await?;
        if existing.is_some() {
            return Err(AppError::InvalidInput("User already exists".into()));
        }

        // Create the user
        let user = self.end_user_repo.upsert(domain_id, email).await?;

        // Pre-whitelist if requested
        if pre_whitelist {
            self.end_user_repo.set_whitelisted(user.id, true).await?;
        }

        // Send invitation email
        if let Ok((api_key, from_email)) = self.get_email_config(domain_id).await {
            let app_origin = format!("https://reauth.{}", domain.domain);
            let login_url = format!("https://reauth.{}/", domain.domain);
            let (subject, html) = account_invited_email(&app_origin, &domain.domain, &login_url);
            let _ = self.email_sender.send(&api_key, &from_email, email, &subject, &html).await;
        }

        // Return user with updated whitelist status
        self.end_user_repo
            .get_by_id(user.id)
            .await?
            .ok_or(AppError::NotFound)
    }

    /// Delete own account (for self-service account deletion)
    #[instrument(skip(self))]
    pub async fn delete_own_account(&self, end_user_id: Uuid) -> AppResult<()> {
        self.end_user_repo.delete(end_user_id).await
    }

    /// Get auth config for a domain by name (no ownership check, for public endpoints)
    #[instrument(skip(self))]
    pub async fn get_auth_config_for_domain(&self, domain_name: &str) -> AppResult<DomainAuthConfigProfile> {
        let domain = self
            .domain_repo
            .get_by_domain(domain_name)
            .await?
            .ok_or(AppError::NotFound)?;

        self.auth_config_repo
            .get_by_domain_id(domain.id)
            .await?
            .ok_or(AppError::NotFound)
    }

    // ========================================================================
    // Helpers
    // ========================================================================

    /// Verify that the end-user owns the specified domain
    async fn verify_domain_ownership(&self, owner_end_user_id: Uuid, domain_id: Uuid) -> AppResult<()> {
        self.verify_domain_ownership_get_domain(owner_end_user_id, domain_id).await?;
        Ok(())
    }

    /// Verify ownership and return the domain (for when we need domain info)
    async fn verify_domain_ownership_get_domain(
        &self,
        owner_end_user_id: Uuid,
        domain_id: Uuid,
    ) -> AppResult<crate::application::use_cases::domain::DomainProfile> {
        let domain = self
            .domain_repo
            .get_by_id(domain_id)
            .await?
            .ok_or(AppError::NotFound)?;

        if domain.owner_end_user_id != Some(owner_end_user_id) {
            return Err(AppError::InvalidCredentials);
        }

        Ok(domain)
    }

    /// Get email config for a domain (required - no fallback)
    async fn get_email_config(&self, domain_id: Uuid) -> AppResult<(String, String)> {
        let config = self
            .magic_link_config_repo
            .get_by_domain_id(domain_id)
            .await?
            .ok_or_else(|| {
                AppError::InvalidInput(
                    "Email not configured for this domain. Please add a Resend API key.".into(),
                )
            })?;

        let api_key = self.cipher.decrypt(&config.resend_api_key_encrypted)?;
        Ok((api_key, config.from_email))
    }

    /// Count total users across multiple domains
    pub async fn count_users_by_domain_ids(&self, domain_ids: &[Uuid]) -> AppResult<i64> {
        self.end_user_repo.count_by_domain_ids(domain_ids).await
    }

    /// Get end-user by ID (for session validation, no ownership check)
    #[instrument(skip(self))]
    pub async fn get_end_user_by_id(&self, user_id: Uuid) -> AppResult<Option<DomainEndUserProfile>> {
        self.end_user_repo.get_by_id(user_id).await
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
/// Note: session_id is stored separately in Redis for verification, not in the hash
fn hash_domain_token(raw: &str, domain: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(raw.as_bytes());
    hasher.update(domain.as_bytes());
    let out = hasher.finalize();
    hex::encode(out)
}

/// Validate that a redirect URL is on the specified domain or a subdomain
fn is_valid_redirect_url(url: &str, domain: &str) -> bool {
    let Ok(parsed) = url::Url::parse(url) else {
        return false;
    };

    let Some(host) = parsed.host_str() else {
        return false;
    };

    // Check if host matches domain exactly or is a subdomain
    host == domain || host.ends_with(&format!(".{}", domain))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_valid_redirect_url() {
        // Valid: exact domain match
        assert!(is_valid_redirect_url("https://example.com/callback", "example.com"));
        assert!(is_valid_redirect_url("https://example.com", "example.com"));
        assert!(is_valid_redirect_url("http://example.com/path", "example.com"));

        // Valid: subdomain
        assert!(is_valid_redirect_url("https://app.example.com/callback", "example.com"));
        assert!(is_valid_redirect_url("https://login.example.com", "example.com"));
        assert!(is_valid_redirect_url("https://deep.nested.example.com/path", "example.com"));

        // Invalid: different domain
        assert!(!is_valid_redirect_url("https://evil.com/callback", "example.com"));
        assert!(!is_valid_redirect_url("https://notexample.com", "example.com"));

        // Invalid: domain suffix attack (evil.com shouldn't match example.com)
        assert!(!is_valid_redirect_url("https://fakeexample.com", "example.com"));

        // Invalid: malformed URLs
        assert!(!is_valid_redirect_url("not-a-url", "example.com"));
        assert!(!is_valid_redirect_url("", "example.com"));
    }
}
