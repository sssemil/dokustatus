use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use base64::Engine;
use chrono::NaiveDateTime;
use sha2::{Digest, Sha256};
use tracing::instrument;
use uuid::Uuid;

use crate::app_error::{AppError, AppResult};
use crate::application::email_templates::{
    account_created_email, account_frozen_email, account_invited_email, account_unfrozen_email,
    account_whitelisted_email, primary_button, wrap_email,
};
use crate::application::use_cases::domain::DomainRepoTrait;
use crate::domain::entities::domain::DomainStatus;
use crate::domain::entities::webhook::{UserAuthPayload, UserIdPayload};
use crate::infra::crypto::ProcessCipher;

// ============================================================================
// Repository Traits
// ============================================================================

#[async_trait]
pub trait DomainAuthConfigRepoTrait: Send + Sync {
    async fn get_by_domain_id(&self, domain_id: Uuid)
    -> AppResult<Option<DomainAuthConfigProfile>>;
    async fn get_by_domain_ids(
        &self,
        domain_ids: &[Uuid],
    ) -> AppResult<Vec<DomainAuthConfigProfile>>;
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
pub trait DomainAuthMagicLinkRepoTrait: Send + Sync {
    async fn get_by_domain_id(
        &self,
        domain_id: Uuid,
    ) -> AppResult<Option<DomainAuthMagicLinkProfile>>;
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
pub trait DomainAuthGoogleOAuthRepoTrait: Send + Sync {
    async fn get_by_domain_id(
        &self,
        domain_id: Uuid,
    ) -> AppResult<Option<DomainAuthGoogleOAuthProfile>>;
    async fn upsert(
        &self,
        domain_id: Uuid,
        client_id: &str,
        client_secret_encrypted: &str,
    ) -> AppResult<DomainAuthGoogleOAuthProfile>;
    async fn delete(&self, domain_id: Uuid) -> AppResult<()>;
}

#[async_trait]
pub trait DomainEndUserRepoTrait: Send + Sync {
    async fn get_by_id(&self, id: Uuid) -> AppResult<Option<DomainEndUserProfile>>;
    async fn get_by_domain_and_email(
        &self,
        domain_id: Uuid,
        email: &str,
    ) -> AppResult<Option<DomainEndUserProfile>>;
    async fn get_by_domain_and_google_id(
        &self,
        domain_id: Uuid,
        google_id: &str,
    ) -> AppResult<Option<DomainEndUserProfile>>;
    async fn upsert(&self, domain_id: Uuid, email: &str) -> AppResult<DomainEndUserProfile>;
    async fn upsert_with_google_id(
        &self,
        domain_id: Uuid,
        email: &str,
        google_id: &str,
    ) -> AppResult<DomainEndUserProfile>;
    async fn mark_verified(&self, id: Uuid) -> AppResult<DomainEndUserProfile>;
    async fn update_last_login(&self, id: Uuid) -> AppResult<()>;
    async fn set_google_id(&self, id: Uuid, google_id: &str) -> AppResult<()>;
    async fn clear_google_id(&self, id: Uuid) -> AppResult<()>;
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
    async fn save(
        &self,
        token_hash: &str,
        end_user_id: Uuid,
        domain_id: Uuid,
        session_id: &str,
        ttl_minutes: i64,
    ) -> AppResult<()>;
    /// Consume a magic link. Returns the data if session matches, or SessionMismatch error if different browser/device.
    async fn consume(
        &self,
        token_hash: &str,
        session_id: &str,
    ) -> AppResult<Option<DomainMagicLinkData>>;
}

/// OAuth state data stored in Redis during the OAuth flow
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OAuthStateData {
    pub domain: String,
    pub code_verifier: String, // PKCE code_verifier for exchange
    /// Status: "pending" (initial), "in_use" (being exchanged)
    #[serde(default = "default_pending")]
    pub status: String,
    /// Unix timestamp when state was marked in-use (for retry window)
    #[serde(default)]
    pub marked_at: Option<i64>,
}

fn default_pending() -> String {
    "pending".to_string()
}

/// Result of attempting to mark state in-use
#[derive(Debug, Clone)]
pub enum MarkStateResult {
    /// State marked successfully, here's the data
    Success(OAuthStateData),
    /// State not found (doesn't exist or was deleted)
    NotFound,
    /// State is in-use and retry window has expired
    RetryWindowExpired,
}

/// OAuth completion data stored in Redis after successful OAuth exchange
/// Used to transfer auth state to the correct domain for cookie setting
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OAuthCompletionData {
    pub user_id: Uuid,
    pub domain_id: Uuid,
    pub domain: String,
}

/// OAuth link confirmation data stored in Redis when a Google account needs
/// to be linked to an existing user account (email match, no google_id yet).
/// Server-derived data only - never trust client-provided values.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OAuthLinkConfirmationData {
    pub existing_user_id: Uuid,
    pub google_id: String,
    pub domain_id: Uuid,
    pub domain: String,
    // Note: email already verified via id_token email_verified=true check in exchange
}

#[async_trait]
pub trait OAuthStateStoreTrait: Send + Sync {
    /// Store state with domain and PKCE verifier. Single-use, expires after TTL.
    async fn store_state(
        &self,
        state: &str,
        data: &OAuthStateData,
        ttl_minutes: i64,
    ) -> AppResult<()>;
    /// Consume state atomically (single-use) and return stored data
    async fn consume_state(&self, state: &str) -> AppResult<Option<OAuthStateData>>;
    /// Mark state as "in_use". Refreshes TTL to ensure retry window is available.
    /// Returns structured result instead of Option/Error.
    async fn mark_state_in_use(
        &self,
        state: &str,
        retry_window_secs: i64,
    ) -> AppResult<MarkStateResult>;
    /// Delete state unconditionally after successful completion.
    /// This is called only after user creation succeeds.
    async fn complete_state(&self, state: &str) -> AppResult<()>;
    /// Abort state for terminal errors (unconditional delete).
    /// Called when error is non-retryable (invalid_grant, validation failure).
    async fn abort_state(&self, state: &str) -> AppResult<()>;
    /// Store completion token after successful OAuth exchange
    async fn store_completion(
        &self,
        token: &str,
        data: &OAuthCompletionData,
        ttl_minutes: i64,
    ) -> AppResult<()>;
    /// Consume completion token atomically
    async fn consume_completion(&self, token: &str) -> AppResult<Option<OAuthCompletionData>>;

    /// Store link confirmation token after OAuth exchange when user needs to confirm linking.
    /// TTL: 5 minutes (short-lived, single-use)
    async fn store_link_confirmation(
        &self,
        token: &str,
        data: &OAuthLinkConfirmationData,
        ttl_minutes: i64,
    ) -> AppResult<()>;

    /// Consume link confirmation token atomically (single-use)
    async fn consume_link_confirmation(
        &self,
        token: &str,
    ) -> AppResult<Option<OAuthLinkConfirmationData>>;
}

#[async_trait]
pub trait DomainEmailSender: Send + Sync {
    async fn send(
        &self,
        api_key: &str,
        from_email: &str,
        to: &str,
        subject: &str,
        html: &str,
    ) -> AppResult<()>;
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
pub struct DomainAuthGoogleOAuthProfile {
    pub id: Uuid,
    pub domain_id: Uuid,
    pub client_id: String,
    pub client_secret_encrypted: String,
    pub created_at: Option<NaiveDateTime>,
    pub updated_at: Option<NaiveDateTime>,
}

#[derive(Debug, Clone)]
pub struct DomainEndUserProfile {
    pub id: Uuid,
    pub domain_id: Uuid,
    pub email: String,
    pub roles: Vec<String>,
    pub google_id: Option<String>,
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
    domain_repo: Arc<dyn DomainRepoTrait>,
    auth_config_repo: Arc<dyn DomainAuthConfigRepoTrait>,
    magic_link_config_repo: Arc<dyn DomainAuthMagicLinkRepoTrait>,
    google_oauth_config_repo: Arc<dyn DomainAuthGoogleOAuthRepoTrait>,
    end_user_repo: Arc<dyn DomainEndUserRepoTrait>,
    magic_link_store: Arc<dyn DomainMagicLinkStore>,
    oauth_state_store: Arc<dyn OAuthStateStoreTrait>,
    email_sender: Arc<dyn DomainEmailSender>,
    cipher: ProcessCipher,
    fallback_resend_api_key: String,
    fallback_email_domain: String,
    fallback_google_client_id: String,
    fallback_google_client_secret: String,
    webhook_emitter: Option<Arc<crate::application::use_cases::webhook::WebhookUseCases>>,
}

impl DomainAuthUseCases {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        domain_repo: Arc<dyn DomainRepoTrait>,
        auth_config_repo: Arc<dyn DomainAuthConfigRepoTrait>,
        magic_link_config_repo: Arc<dyn DomainAuthMagicLinkRepoTrait>,
        google_oauth_config_repo: Arc<dyn DomainAuthGoogleOAuthRepoTrait>,
        end_user_repo: Arc<dyn DomainEndUserRepoTrait>,
        magic_link_store: Arc<dyn DomainMagicLinkStore>,
        oauth_state_store: Arc<dyn OAuthStateStoreTrait>,
        email_sender: Arc<dyn DomainEmailSender>,
        cipher: ProcessCipher,
        fallback_resend_api_key: String,
        fallback_email_domain: String,
        fallback_google_client_id: String,
        fallback_google_client_secret: String,
    ) -> Self {
        Self {
            domain_repo,
            auth_config_repo,
            magic_link_config_repo,
            google_oauth_config_repo,
            end_user_repo,
            magic_link_store,
            oauth_state_store,
            email_sender,
            cipher,
            fallback_resend_api_key,
            fallback_email_domain,
            fallback_google_client_id,
            fallback_google_client_secret,
            webhook_emitter: None,
        }
    }

    pub fn set_webhook_emitter(
        &mut self,
        emitter: Arc<crate::application::use_cases::webhook::WebhookUseCases>,
    ) {
        self.webhook_emitter = Some(emitter);
    }

    fn emit_webhook(
        &self,
        domain_id: Uuid,
        event_type: crate::domain::entities::webhook::WebhookEventType,
        data: impl serde::Serialize + Send + 'static,
    ) {
        if let Some(emitter) = &self.webhook_emitter {
            let emitter = Arc::clone(emitter);
            tokio::spawn(async move {
                if let Err(e) = emitter.emit_event(domain_id, event_type, data).await {
                    tracing::error!(
                        error = %e,
                        event_type = %event_type,
                        "Failed to emit webhook event"
                    );
                }
            });
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

        // Use domain config if set, otherwise default to enabled
        let magic_link_enabled = auth_config
            .as_ref()
            .map(|c| c.magic_link_enabled)
            .unwrap_or(true);

        let google_oauth_enabled = auth_config
            .as_ref()
            .map(|c| c.google_oauth_enabled)
            .unwrap_or(true);

        Ok(PublicDomainConfig {
            domain_id: domain.id,
            domain: domain.domain,
            magic_link_enabled,
            google_oauth_enabled,
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
            .ok_or_else(|| {
                AppError::InvalidInput("Authentication not configured for this domain".into())
            })?;

        if !auth_config.magic_link_enabled {
            return Err(AppError::InvalidInput(
                "Magic link login is not enabled for this domain".into(),
            ));
        }

        // Get Resend config (domain-specific or fallback to global)
        let (api_key, from_email, _) = self.get_email_config(domain.id, domain_name).await?;

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
        let footer_note = "This one-time link keeps your account protected; delete this email if you did not request it.";

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

        self.email_sender
            .send(&api_key, &from_email, email, subject, &html)
            .await
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
        if let Some(data) = consume_magic_link_from_store(
            self.magic_link_store.as_ref(),
            raw_token,
            domain_name,
            session_id,
        )
        .await?
        {
            // Get the end user first to check access
            let end_user = self
                .end_user_repo
                .get_by_id(data.end_user_id)
                .await?
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
            if is_first_login
                && let Ok((api_key, from_email, _)) =
                    self.get_email_config(data.domain_id, domain_name).await
            {
                let app_origin = format!("https://reauth.{}", domain_name);
                let (subject, html) = account_created_email(&app_origin, domain_name);
                // Fire and forget - don't fail login if email fails
                let _ = self
                    .email_sender
                    .send(&api_key, &from_email, &end_user.email, &subject, &html)
                    .await;
            }

            // Emit webhook events
            use crate::domain::entities::webhook::WebhookEventType;
            if is_first_login {
                self.emit_webhook(
                    data.domain_id,
                    WebhookEventType::UserCreated,
                    UserAuthPayload {
                        user_id: end_user.id.to_string(),
                        auth_method: "magic_link".into(),
                    },
                );
            }
            self.emit_webhook(
                data.domain_id,
                WebhookEventType::UserLogin,
                UserAuthPayload {
                    user_id: end_user.id.to_string(),
                    auth_method: "magic_link".into(),
                },
            );

            return Ok(Some(end_user));
        }

        Ok(None)
    }

    /// Get waitlist position for a non-whitelisted user
    /// Returns the count of non-whitelisted users created before this user + 1
    #[instrument(skip(self))]
    pub async fn get_waitlist_position(&self, domain_id: Uuid, user_id: Uuid) -> AppResult<i64> {
        self.end_user_repo
            .get_waitlist_position(domain_id, user_id)
            .await
    }

    // ========================================================================
    // Protected endpoints (for dashboard)
    // ========================================================================

    /// Batch check if domains have auth methods enabled.
    ///
    /// IMPORTANT: This method does NOT verify domain ownership. It must only be
    /// called with domain IDs that have already been authorized.
    /// Domains without explicit config default to `true` (matches get_auth_config).
    pub(crate) async fn has_auth_methods_for_owner_domains(
        &self,
        domain_ids: &[Uuid],
    ) -> AppResult<HashMap<Uuid, bool>> {
        if domain_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let configs = self.auth_config_repo.get_by_domain_ids(domain_ids).await?;
        let mut result = HashMap::with_capacity(domain_ids.len());

        for config in configs {
            let has_methods = config.magic_link_enabled || config.google_oauth_enabled;
            result.insert(config.domain_id, has_methods);
        }

        for &domain_id in domain_ids {
            result.entry(domain_id).or_insert(true);
        }

        Ok(result)
    }

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
                magic_link_enabled: true,
                google_oauth_enabled: false,
                redirect_url: None,
                whitelist_enabled: false,
                access_token_ttl_secs: 86400,
                refresh_token_ttl_days: 30,
                created_at: None,
                updated_at: None,
            });

        let magic_link_config = self
            .magic_link_config_repo
            .get_by_domain_id(domain_id)
            .await?;

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
            return Err(AppError::InvalidInput(
                "Domain must be verified before configuring authentication".into(),
            ));
        }

        // Validate redirect URL is on the domain or a subdomain
        if let Some(url) = redirect_url
            && !is_valid_redirect_url(url, &domain.domain)
        {
            return Err(AppError::InvalidInput(format!(
                "Redirect URL must be on {} or a subdomain",
                domain.domain
            )));
        }

        // If enabling whitelist and requested, whitelist all existing users
        if whitelist_enabled && whitelist_all_existing {
            self.end_user_repo
                .whitelist_all_in_domain(domain_id)
                .await?;
        }

        // Update general auth config
        self.auth_config_repo
            .upsert(
                domain_id,
                magic_link_enabled,
                google_oauth_enabled,
                redirect_url,
                whitelist_enabled,
            )
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

    /// Delete magic link email config for a domain (domain owner only)
    /// This allows the domain to fall back to the global/shared email service
    #[instrument(skip(self))]
    pub async fn delete_magic_link_config(
        &self,
        end_user_id: Uuid,
        domain_id: Uuid,
    ) -> AppResult<()> {
        self.verify_domain_ownership(end_user_id, domain_id).await?;
        self.magic_link_config_repo.delete(domain_id).await
    }

    /// List end-users for a domain (domain owner only)
    #[instrument(skip(self))]
    pub async fn list_end_users(
        &self,
        end_user_id: Uuid,
        domain_id: Uuid,
    ) -> AppResult<Vec<DomainEndUserProfile>> {
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
        self.verify_domain_ownership(owner_end_user_id, domain_id)
            .await?;

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
        self.verify_domain_ownership(owner_end_user_id, domain_id)
            .await?;

        let user = self
            .end_user_repo
            .get_by_id(user_id)
            .await?
            .ok_or(AppError::NotFound)?;

        if user.domain_id != domain_id {
            return Err(AppError::NotFound);
        }

        self.end_user_repo.delete(user_id).await?;

        self.emit_webhook(
            domain_id,
            crate::domain::entities::webhook::WebhookEventType::UserDeleted,
            UserIdPayload {
                user_id: user_id.to_string(),
            },
        );

        Ok(())
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
        let domain = self
            .verify_domain_ownership_get_domain(owner_end_user_id, domain_id)
            .await?;

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
            if let Ok((api_key, from_email, _)) =
                self.get_email_config(domain_id, &domain.domain).await
            {
                let app_origin = format!("https://reauth.{}", domain.domain);
                let (subject, html) = account_frozen_email(&app_origin, &domain.domain);
                let _ = self
                    .email_sender
                    .send(&api_key, &from_email, &user.email, &subject, &html)
                    .await;
            }

            self.emit_webhook(
                domain_id,
                crate::domain::entities::webhook::WebhookEventType::UserFrozen,
                UserIdPayload {
                    user_id: user_id.to_string(),
                },
            );
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
        let domain = self
            .verify_domain_ownership_get_domain(owner_end_user_id, domain_id)
            .await?;

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
            if let Ok((api_key, from_email, _)) =
                self.get_email_config(domain_id, &domain.domain).await
            {
                let app_origin = format!("https://reauth.{}", domain.domain);
                let login_url = format!("https://reauth.{}/", domain.domain);
                let (subject, html) =
                    account_unfrozen_email(&app_origin, &domain.domain, &login_url);
                let _ = self
                    .email_sender
                    .send(&api_key, &from_email, &user.email, &subject, &html)
                    .await;
            }

            self.emit_webhook(
                domain_id,
                crate::domain::entities::webhook::WebhookEventType::UserUnfrozen,
                UserIdPayload {
                    user_id: user_id.to_string(),
                },
            );
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
        let domain = self
            .verify_domain_ownership_get_domain(owner_end_user_id, domain_id)
            .await?;

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
            if let Ok((api_key, from_email, _)) =
                self.get_email_config(domain_id, &domain.domain).await
            {
                let app_origin = format!("https://reauth.{}", domain.domain);
                let login_url = format!("https://reauth.{}/", domain.domain);
                let (subject, html) =
                    account_whitelisted_email(&app_origin, &domain.domain, &login_url);
                let _ = self
                    .email_sender
                    .send(&api_key, &from_email, &user.email, &subject, &html)
                    .await;
            }

            self.emit_webhook(
                domain_id,
                crate::domain::entities::webhook::WebhookEventType::UserWhitelisted,
                UserIdPayload {
                    user_id: user_id.to_string(),
                },
            );
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
        self.verify_domain_ownership(owner_end_user_id, domain_id)
            .await?;

        let user = self
            .end_user_repo
            .get_by_id(user_id)
            .await?
            .ok_or(AppError::NotFound)?;

        if user.domain_id != domain_id {
            return Err(AppError::NotFound);
        }

        let was_whitelisted = user.is_whitelisted;

        self.end_user_repo.set_whitelisted(user_id, false).await?;

        if was_whitelisted {
            self.emit_webhook(
                domain_id,
                crate::domain::entities::webhook::WebhookEventType::UserUnwhitelisted,
                UserIdPayload {
                    user_id: user_id.to_string(),
                },
            );
        }

        Ok(())
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
        let domain = self
            .verify_domain_ownership_get_domain(owner_end_user_id, domain_id)
            .await?;

        // Check if user already exists
        let existing = self
            .end_user_repo
            .get_by_domain_and_email(domain_id, email)
            .await?;
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
        if let Ok((api_key, from_email, _)) = self.get_email_config(domain_id, &domain.domain).await
        {
            let app_origin = format!("https://reauth.{}", domain.domain);
            let login_url = format!("https://reauth.{}/", domain.domain);
            let (subject, html) = account_invited_email(&app_origin, &domain.domain, &login_url);
            let _ = self
                .email_sender
                .send(&api_key, &from_email, email, &subject, &html)
                .await;
        }

        self.emit_webhook(
            domain_id,
            crate::domain::entities::webhook::WebhookEventType::UserInvited,
            UserIdPayload {
                user_id: user.id.to_string(),
            },
        );

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
    pub async fn get_auth_config_for_domain(
        &self,
        domain_name: &str,
    ) -> AppResult<DomainAuthConfigProfile> {
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
    async fn verify_domain_ownership(
        &self,
        owner_end_user_id: Uuid,
        domain_id: Uuid,
    ) -> AppResult<()> {
        self.verify_domain_ownership_get_domain(owner_end_user_id, domain_id)
            .await?;
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

    /// Get email config for a domain. Uses domain-specific config if set, otherwise global fallback.
    /// Returns (api_key, from_email, is_using_fallback).
    async fn get_email_config(
        &self,
        domain_id: Uuid,
        domain_name: &str,
    ) -> AppResult<(String, String, bool)> {
        // Try domain-specific config first
        if let Some(config) = self
            .magic_link_config_repo
            .get_by_domain_id(domain_id)
            .await?
        {
            let api_key = self.cipher.decrypt(&config.resend_api_key_encrypted)?;
            return Ok((api_key, config.from_email, false));
        }

        // Use global fallback config
        let sanitized_domain = sanitize_domain_for_email(domain_name);
        let from_email = format!("{}@{}", sanitized_domain, self.fallback_email_domain);
        Ok((self.fallback_resend_api_key.clone(), from_email, true))
    }

    /// Get the generated from_email for fallback email config.
    pub fn get_fallback_email_info(&self, domain_name: &str) -> String {
        let sanitized = sanitize_domain_for_email(domain_name);
        format!("{}@{}", sanitized, self.fallback_email_domain)
    }

    /// Count total users across multiple domains
    pub async fn count_users_by_domain_ids(&self, domain_ids: &[Uuid]) -> AppResult<i64> {
        self.end_user_repo.count_by_domain_ids(domain_ids).await
    }

    /// Get end-user by ID (for session validation, no ownership check)
    #[instrument(skip(self))]
    pub async fn get_end_user_by_id(
        &self,
        user_id: Uuid,
    ) -> AppResult<Option<DomainEndUserProfile>> {
        self.end_user_repo.get_by_id(user_id).await
    }

    // ========================================================================
    // Google OAuth Methods
    // ========================================================================

    /// Get Google OAuth config for a domain.
    /// Returns (client_id, client_secret, using_fallback)
    /// Tries domain-specific first, then falls back to global config.
    pub async fn get_google_oauth_config(
        &self,
        domain_id: Uuid,
    ) -> AppResult<(String, String, bool)> {
        // Try domain-specific config first
        if let Some(config) = self
            .google_oauth_config_repo
            .get_by_domain_id(domain_id)
            .await?
        {
            let client_secret = self.cipher.decrypt(&config.client_secret_encrypted)?;
            return Ok((config.client_id, client_secret, false));
        }

        // Use global fallback config
        Ok((
            self.fallback_google_client_id.clone(),
            self.fallback_google_client_secret.clone(),
            true,
        ))
    }

    /// Create OAuth state for Google OAuth flow.
    /// Returns (state_token, code_verifier) - caller builds the auth URL.
    #[instrument(skip(self))]
    pub async fn create_google_oauth_state(
        &self,
        domain_name: &str,
    ) -> AppResult<(String, String)> {
        // Verify domain exists and is verified
        let domain = self
            .domain_repo
            .get_by_domain(domain_name)
            .await?
            .ok_or(AppError::NotFound)?;

        if domain.status != DomainStatus::Verified {
            return Err(AppError::NotFound);
        }

        // Check if Google OAuth is enabled for this domain
        let auth_config = self.auth_config_repo.get_by_domain_id(domain.id).await?;
        let google_oauth_enabled = auth_config
            .as_ref()
            .map(|c| c.google_oauth_enabled)
            .unwrap_or(true);

        if !google_oauth_enabled {
            return Err(AppError::InvalidInput(
                "Google OAuth is not enabled for this domain".into(),
            ));
        }

        // Generate state token and PKCE code verifier
        let state = generate_token();
        let code_verifier = generate_token();

        // Store state in Redis
        let state_data = OAuthStateData {
            domain: domain_name.to_string(),
            code_verifier: code_verifier.clone(),
            status: default_pending(),
            marked_at: None,
        };

        self.oauth_state_store
            .store_state(&state, &state_data, 10) // 10 minute TTL
            .await?;

        Ok((state, code_verifier))
    }

    /// Consume OAuth state and return stored data.
    /// Returns None if state is invalid/expired/already consumed.
    #[instrument(skip(self))]
    pub async fn consume_google_oauth_state(
        &self,
        state: &str,
    ) -> AppResult<Option<OAuthStateData>> {
        self.oauth_state_store.consume_state(state).await
    }

    /// Mark OAuth state as in-use (two-phase exchange).
    #[instrument(skip(self))]
    pub async fn mark_google_oauth_state_in_use(
        &self,
        state: &str,
        retry_window_secs: i64,
    ) -> AppResult<MarkStateResult> {
        self.oauth_state_store
            .mark_state_in_use(state, retry_window_secs)
            .await
    }

    /// Complete OAuth state after successful exchange (best-effort delete).
    #[instrument(skip(self))]
    pub async fn complete_google_oauth_state(&self, state: &str) -> AppResult<()> {
        self.oauth_state_store.complete_state(state).await
    }

    /// Abort OAuth state for terminal errors (delete).
    #[instrument(skip(self))]
    pub async fn abort_google_oauth_state(&self, state: &str) -> AppResult<()> {
        self.oauth_state_store.abort_state(state).await
    }

    /// Get domain by name (for OAuth callback to look up domain from state)
    pub async fn get_domain_by_name(
        &self,
        domain_name: &str,
    ) -> AppResult<Option<crate::application::use_cases::domain::DomainProfile>> {
        self.domain_repo.get_by_domain(domain_name).await
    }

    /// Check if Google OAuth is enabled for a domain
    #[instrument(skip(self))]
    pub async fn is_google_oauth_enabled(&self, domain_id: Uuid) -> AppResult<bool> {
        let auth_config = self.auth_config_repo.get_by_domain_id(domain_id).await?;
        Ok(auth_config
            .as_ref()
            .map(|c| c.google_oauth_enabled)
            .unwrap_or(true))
    }

    /// Find or create end user by Google ID (for OAuth login).
    /// Returns the end user and a flag indicating if it's a new user.
    #[instrument(skip(self))]
    pub async fn find_or_create_end_user_by_google(
        &self,
        domain_id: Uuid,
        google_id: &str,
        email: &str,
    ) -> AppResult<GoogleLoginResult> {
        // First, try to find by google_id (existing linked account)
        if let Some(user) = self
            .end_user_repo
            .get_by_domain_and_google_id(domain_id, google_id)
            .await?
        {
            // Existing linked account - update last login and return
            self.end_user_repo.update_last_login(user.id).await?;
            self.emit_webhook(
                domain_id,
                crate::domain::entities::webhook::WebhookEventType::UserLogin,
                UserAuthPayload {
                    user_id: user.id.to_string(),
                    auth_method: "google_oauth".into(),
                },
            );
            return Ok(GoogleLoginResult::LoggedIn(user));
        }

        // Try to find by email
        if let Some(user) = self
            .end_user_repo
            .get_by_domain_and_email(domain_id, email)
            .await?
        {
            // User exists with this email
            if user.google_id.is_some() {
                // Already linked to a different Google account - conflict
                return Err(AppError::InvalidInput(
                    "This email is already linked to a different Google account".into(),
                ));
            }

            // User exists but not linked - needs confirmation
            return Ok(GoogleLoginResult::NeedsLinkConfirmation {
                existing_user_id: user.id,
                email: email.to_string(),
                google_id: google_id.to_string(),
            });
        }

        // No existing user - create new one with Google ID
        let user = self
            .end_user_repo
            .upsert_with_google_id(domain_id, email, google_id)
            .await?;
        self.end_user_repo.update_last_login(user.id).await?;

        use crate::domain::entities::webhook::WebhookEventType;
        self.emit_webhook(
            domain_id,
            WebhookEventType::UserCreated,
            UserAuthPayload {
                user_id: user.id.to_string(),
                auth_method: "google_oauth".into(),
            },
        );
        self.emit_webhook(
            domain_id,
            WebhookEventType::UserLogin,
            UserAuthPayload {
                user_id: user.id.to_string(),
                auth_method: "google_oauth".into(),
            },
        );

        Ok(GoogleLoginResult::LoggedIn(user))
    }

    /// Confirm linking a Google account to an existing user.
    /// Called after user confirms the link in the UI.
    #[instrument(skip(self))]
    pub async fn confirm_google_link(
        &self,
        existing_user_id: Uuid,
        google_id: &str,
    ) -> AppResult<DomainEndUserProfile> {
        // Get the user to verify it exists and get domain_id
        let user = self
            .end_user_repo
            .get_by_id(existing_user_id)
            .await?
            .ok_or(AppError::NotFound)?;

        // Verify google_id is not already linked to another user
        if let Some(existing) = self
            .end_user_repo
            .get_by_domain_and_google_id(user.domain_id, google_id)
            .await?
            && existing.id != existing_user_id
        {
            return Err(AppError::InvalidInput(
                "This Google account is already linked to a different user".into(),
            ));
        }

        // Link the Google account
        self.end_user_repo
            .set_google_id(existing_user_id, google_id)
            .await?;
        self.end_user_repo
            .update_last_login(existing_user_id)
            .await?;

        // Return the updated user
        self.end_user_repo
            .get_by_id(existing_user_id)
            .await?
            .ok_or(AppError::NotFound)
    }

    /// Update Google OAuth config for a domain (domain owner only)
    #[instrument(skip(self, client_secret))]
    pub async fn update_google_oauth_config(
        &self,
        end_user_id: Uuid,
        domain_id: Uuid,
        client_id: &str,
        client_secret: &str,
    ) -> AppResult<()> {
        self.verify_domain_ownership(end_user_id, domain_id).await?;

        let encrypted_secret = self.cipher.encrypt(client_secret)?;
        self.google_oauth_config_repo
            .upsert(domain_id, client_id, &encrypted_secret)
            .await?;

        Ok(())
    }

    /// Delete Google OAuth config for a domain (domain owner only)
    #[instrument(skip(self))]
    pub async fn delete_google_oauth_config(
        &self,
        end_user_id: Uuid,
        domain_id: Uuid,
    ) -> AppResult<()> {
        self.verify_domain_ownership(end_user_id, domain_id).await?;
        self.google_oauth_config_repo.delete(domain_id).await
    }

    /// Get Google OAuth config info for dashboard display
    pub async fn get_google_oauth_config_info(
        &self,
        domain_id: Uuid,
    ) -> AppResult<Option<GoogleOAuthConfigInfo>> {
        if let Some(config) = self
            .google_oauth_config_repo
            .get_by_domain_id(domain_id)
            .await?
        {
            Ok(Some(GoogleOAuthConfigInfo {
                client_id_prefix: config.client_id.chars().take(10).collect(),
                has_client_secret: true,
            }))
        } else {
            Ok(None)
        }
    }

    /// Unlink Google account from end user (for profile page)
    #[instrument(skip(self))]
    pub async fn unlink_google_account(&self, end_user_id: Uuid) -> AppResult<()> {
        // Verify user exists
        let _ = self
            .end_user_repo
            .get_by_id(end_user_id)
            .await?
            .ok_or(AppError::NotFound)?;

        self.end_user_repo.clear_google_id(end_user_id).await
    }

    /// Create a completion token for cross-domain cookie setting.
    /// After Google OAuth exchange on reauth.reauth.dev, we redirect to reauth.{domain}
    /// with this token so cookies can be set on the correct domain.
    #[instrument(skip(self))]
    pub async fn create_google_completion_token(
        &self,
        user_id: Uuid,
        domain_id: Uuid,
        domain: &str,
    ) -> AppResult<String> {
        let token = generate_token();
        let data = OAuthCompletionData {
            user_id,
            domain_id,
            domain: domain.to_string(),
        };

        self.oauth_state_store
            .store_completion(&token, &data, 5) // 5 minute TTL (short-lived)
            .await?;

        Ok(token)
    }

    /// Consume a completion token and return the stored data.
    /// Returns None if token is invalid/expired/already consumed.
    #[instrument(skip(self))]
    pub async fn consume_google_completion_token(
        &self,
        token: &str,
    ) -> AppResult<Option<OAuthCompletionData>> {
        self.oauth_state_store.consume_completion(token).await
    }

    /// Create a link confirmation token for when a Google account needs to be linked
    /// to an existing user account (email match, no google_id yet).
    /// All data is server-derived (never from client).
    #[instrument(skip(self))]
    pub async fn create_google_link_confirmation_token(
        &self,
        existing_user_id: Uuid,
        google_id: &str,
        domain_id: Uuid,
        domain: &str,
    ) -> AppResult<String> {
        let token = generate_token();
        let data = OAuthLinkConfirmationData {
            existing_user_id,
            google_id: google_id.to_string(),
            domain_id,
            domain: domain.to_string(),
        };

        self.oauth_state_store
            .store_link_confirmation(&token, &data, 5) // 5 minute TTL (short-lived)
            .await?;

        Ok(token)
    }

    /// Consume a link confirmation token and return the stored data.
    /// Returns None if token is invalid/expired/already consumed.
    #[instrument(skip(self))]
    pub async fn consume_google_link_confirmation_token(
        &self,
        token: &str,
    ) -> AppResult<Option<OAuthLinkConfirmationData>> {
        self.oauth_state_store
            .consume_link_confirmation(token)
            .await
    }
}

/// Result of a Google OAuth login attempt
#[derive(Debug)]
pub enum GoogleLoginResult {
    /// Successfully logged in (existing or new user)
    LoggedIn(DomainEndUserProfile),
    /// User with this email exists but needs to confirm linking
    NeedsLinkConfirmation {
        existing_user_id: Uuid,
        email: String,
        google_id: String,
    },
}

/// Google OAuth config info for dashboard display
#[derive(Debug, Clone)]
pub struct GoogleOAuthConfigInfo {
    pub client_id_prefix: String,
    pub has_client_secret: bool,
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Sanitize a domain name for use in email local part.
/// Replaces dots with hyphens: "myapp.com" -> "myapp-com"
fn sanitize_domain_for_email(domain: &str) -> String {
    domain.replace('.', "-")
}

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
    let raw_bytes = raw.as_bytes();
    let domain_bytes = domain.as_bytes();
    hasher.update((raw_bytes.len() as u32).to_be_bytes());
    hasher.update(raw_bytes);
    hasher.update((domain_bytes.len() as u32).to_be_bytes());
    hasher.update(domain_bytes);
    let out = hasher.finalize();
    hex::encode(out)
}

async fn consume_magic_link_from_store(
    magic_link_store: &dyn DomainMagicLinkStore,
    raw_token: &str,
    domain_name: &str,
    session_id: &str,
) -> AppResult<Option<DomainMagicLinkData>> {
    let token_hash = hash_domain_token(raw_token, domain_name);
    magic_link_store.consume(&token_hash, session_id).await
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
    use crate::application::use_cases::domain::DomainProfile;
    use crate::domain::entities::payment_mode::PaymentMode;
    use async_trait::async_trait;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct InMemoryMagicLinkStore {
        entries: Mutex<HashMap<String, StoredMagicLink>>,
    }

    struct StoredMagicLink {
        data: DomainMagicLinkData,
        session_id: String,
    }

    #[async_trait]
    impl DomainMagicLinkStore for InMemoryMagicLinkStore {
        async fn save(
            &self,
            token_hash: &str,
            end_user_id: Uuid,
            domain_id: Uuid,
            session_id: &str,
            _ttl_minutes: i64,
        ) -> AppResult<()> {
            let mut entries = self.entries.lock().expect("magic link store lock");
            entries.insert(
                token_hash.to_string(),
                StoredMagicLink {
                    data: DomainMagicLinkData {
                        end_user_id,
                        domain_id,
                    },
                    session_id: session_id.to_string(),
                },
            );
            Ok(())
        }

        async fn consume(
            &self,
            token_hash: &str,
            session_id: &str,
        ) -> AppResult<Option<DomainMagicLinkData>> {
            let mut entries = self.entries.lock().expect("magic link store lock");
            let Some(stored) = entries.remove(token_hash) else {
                return Ok(None);
            };

            if stored.session_id != session_id {
                entries.insert(token_hash.to_string(), stored);
                return Err(AppError::SessionMismatch);
            }

            Ok(Some(stored.data))
        }
    }

    /// Controllable clock for OAuth state testing
    #[derive(Clone)]
    struct TestClock {
        now: Arc<Mutex<i64>>,
    }

    impl TestClock {
        fn new(initial: i64) -> Self {
            Self {
                now: Arc::new(Mutex::new(initial)),
            }
        }

        fn now(&self) -> i64 {
            *self.now.lock().expect("clock lock")
        }

        fn advance(&self, seconds: i64) {
            let mut now = self.now.lock().expect("clock lock");
            *now += seconds;
        }
    }

    struct InMemoryOAuthStateStore {
        states: Mutex<HashMap<String, (OAuthStateData, i64)>>,
        clock: TestClock,
    }

    impl InMemoryOAuthStateStore {
        fn new(clock: TestClock) -> Self {
            Self {
                states: Mutex::new(HashMap::new()),
                clock,
            }
        }
    }

    #[async_trait]
    impl OAuthStateStoreTrait for InMemoryOAuthStateStore {
        async fn store_state(
            &self,
            state: &str,
            data: &OAuthStateData,
            ttl_minutes: i64,
        ) -> AppResult<()> {
            let mut states = self.states.lock().expect("oauth state lock");
            let ttl_secs = ttl_minutes.max(1) * 60;
            let expires_at = self.clock.now() + ttl_secs;
            states.insert(state.to_string(), (data.clone(), expires_at));
            Ok(())
        }

        async fn consume_state(&self, state: &str) -> AppResult<Option<OAuthStateData>> {
            let mut states = self.states.lock().expect("oauth state lock");
            let now = self.clock.now();
            let Some((data, expires_at)) = states.remove(state) else {
                return Ok(None);
            };
            if now > expires_at {
                return Ok(None);
            }
            Ok(Some(data))
        }

        async fn mark_state_in_use(
            &self,
            state: &str,
            retry_window_secs: i64,
        ) -> AppResult<MarkStateResult> {
            let mut states = self.states.lock().expect("oauth state lock");
            let now = self.clock.now();
            let Some((data, expires_at)) = states.get_mut(state) else {
                return Ok(MarkStateResult::NotFound);
            };

            if now > *expires_at {
                states.remove(state);
                return Ok(MarkStateResult::NotFound);
            }

            if data.status == "in_use" {
                let marked_at = data.marked_at.unwrap_or(0);
                if (now - marked_at) > retry_window_secs {
                    return Ok(MarkStateResult::RetryWindowExpired);
                }
                *expires_at = now + retry_window_secs + 30;
                return Ok(MarkStateResult::Success(data.clone()));
            }

            data.status = "in_use".to_string();
            data.marked_at = Some(now);
            *expires_at = now + retry_window_secs + 30;
            Ok(MarkStateResult::Success(data.clone()))
        }

        async fn complete_state(&self, state: &str) -> AppResult<()> {
            self.states.lock().expect("oauth state lock").remove(state);
            Ok(())
        }

        async fn abort_state(&self, state: &str) -> AppResult<()> {
            self.states.lock().expect("oauth state lock").remove(state);
            Ok(())
        }

        async fn store_completion(
            &self,
            _token: &str,
            _data: &OAuthCompletionData,
            _ttl_minutes: i64,
        ) -> AppResult<()> {
            Err(AppError::Internal("not implemented".into()))
        }

        async fn consume_completion(&self, _token: &str) -> AppResult<Option<OAuthCompletionData>> {
            Err(AppError::Internal("not implemented".into()))
        }

        async fn store_link_confirmation(
            &self,
            _token: &str,
            _data: &OAuthLinkConfirmationData,
            _ttl_minutes: i64,
        ) -> AppResult<()> {
            Err(AppError::Internal("not implemented".into()))
        }

        async fn consume_link_confirmation(
            &self,
            _token: &str,
        ) -> AppResult<Option<OAuthLinkConfirmationData>> {
            Err(AppError::Internal("not implemented".into()))
        }
    }

    #[derive(Default)]
    struct InMemoryAuthConfigRepo {
        configs: Mutex<HashMap<Uuid, DomainAuthConfigProfile>>,
    }

    #[async_trait]
    impl DomainAuthConfigRepoTrait for InMemoryAuthConfigRepo {
        async fn get_by_domain_id(
            &self,
            domain_id: Uuid,
        ) -> AppResult<Option<DomainAuthConfigProfile>> {
            Ok(self
                .configs
                .lock()
                .expect("auth config lock")
                .get(&domain_id)
                .cloned())
        }

        async fn get_by_domain_ids(
            &self,
            domain_ids: &[Uuid],
        ) -> AppResult<Vec<DomainAuthConfigProfile>> {
            let configs = self.configs.lock().expect("auth config lock");
            Ok(domain_ids
                .iter()
                .filter_map(|id| configs.get(id).cloned())
                .collect())
        }

        async fn upsert(
            &self,
            domain_id: Uuid,
            magic_link_enabled: bool,
            google_oauth_enabled: bool,
            redirect_url: Option<&str>,
            whitelist_enabled: bool,
        ) -> AppResult<DomainAuthConfigProfile> {
            let profile = DomainAuthConfigProfile {
                id: Uuid::new_v4(),
                domain_id,
                magic_link_enabled,
                google_oauth_enabled,
                redirect_url: redirect_url.map(|value| value.to_string()),
                whitelist_enabled,
                access_token_ttl_secs: 0,
                refresh_token_ttl_days: 0,
                created_at: None,
                updated_at: None,
            };
            self.configs
                .lock()
                .expect("auth config lock")
                .insert(domain_id, profile.clone());
            Ok(profile)
        }

        async fn delete(&self, domain_id: Uuid) -> AppResult<()> {
            self.configs
                .lock()
                .expect("auth config lock")
                .remove(&domain_id);
            Ok(())
        }
    }

    #[derive(Default)]
    struct NoopRepo;

    #[async_trait]
    impl DomainRepoTrait for NoopRepo {
        async fn create(
            &self,
            _owner_end_user_id: Uuid,
            _domain: &str,
        ) -> AppResult<DomainProfile> {
            Err(AppError::Internal("not implemented".into()))
        }

        async fn get_by_id(&self, _domain_id: Uuid) -> AppResult<Option<DomainProfile>> {
            Err(AppError::Internal("not implemented".into()))
        }

        async fn get_by_domain(&self, _domain: &str) -> AppResult<Option<DomainProfile>> {
            Err(AppError::Internal("not implemented".into()))
        }

        async fn list_by_owner(&self, _owner_end_user_id: Uuid) -> AppResult<Vec<DomainProfile>> {
            Err(AppError::Internal("not implemented".into()))
        }

        async fn update_status(&self, _domain_id: Uuid, _status: &str) -> AppResult<DomainProfile> {
            Err(AppError::Internal("not implemented".into()))
        }

        async fn set_verifying(&self, _domain_id: Uuid) -> AppResult<DomainProfile> {
            Err(AppError::Internal("not implemented".into()))
        }

        async fn set_verified(&self, _domain_id: Uuid) -> AppResult<DomainProfile> {
            Err(AppError::Internal("not implemented".into()))
        }

        async fn set_failed(&self, _domain_id: Uuid) -> AppResult<DomainProfile> {
            Err(AppError::Internal("not implemented".into()))
        }

        async fn delete(&self, _domain_id: Uuid) -> AppResult<()> {
            Err(AppError::Internal("not implemented".into()))
        }

        async fn get_verifying_domains(&self) -> AppResult<Vec<DomainProfile>> {
            Err(AppError::Internal("not implemented".into()))
        }

        async fn set_active_payment_mode(
            &self,
            _domain_id: Uuid,
            _mode: PaymentMode,
        ) -> AppResult<DomainProfile> {
            Err(AppError::Internal("not implemented".into()))
        }
    }

    #[async_trait]
    impl DomainAuthMagicLinkRepoTrait for NoopRepo {
        async fn get_by_domain_id(
            &self,
            _domain_id: Uuid,
        ) -> AppResult<Option<DomainAuthMagicLinkProfile>> {
            Err(AppError::Internal("not implemented".into()))
        }

        async fn upsert(
            &self,
            _domain_id: Uuid,
            _resend_api_key_encrypted: &str,
            _from_email: &str,
        ) -> AppResult<DomainAuthMagicLinkProfile> {
            Err(AppError::Internal("not implemented".into()))
        }

        async fn update_from_email(&self, _domain_id: Uuid, _from_email: &str) -> AppResult<()> {
            Err(AppError::Internal("not implemented".into()))
        }

        async fn delete(&self, _domain_id: Uuid) -> AppResult<()> {
            Err(AppError::Internal("not implemented".into()))
        }
    }

    #[async_trait]
    impl DomainAuthGoogleOAuthRepoTrait for NoopRepo {
        async fn get_by_domain_id(
            &self,
            _domain_id: Uuid,
        ) -> AppResult<Option<DomainAuthGoogleOAuthProfile>> {
            Err(AppError::Internal("not implemented".into()))
        }

        async fn upsert(
            &self,
            _domain_id: Uuid,
            _client_id: &str,
            _client_secret_encrypted: &str,
        ) -> AppResult<DomainAuthGoogleOAuthProfile> {
            Err(AppError::Internal("not implemented".into()))
        }

        async fn delete(&self, _domain_id: Uuid) -> AppResult<()> {
            Err(AppError::Internal("not implemented".into()))
        }
    }

    #[async_trait]
    impl DomainEndUserRepoTrait for NoopRepo {
        async fn get_by_id(&self, _id: Uuid) -> AppResult<Option<DomainEndUserProfile>> {
            Err(AppError::Internal("not implemented".into()))
        }

        async fn get_by_domain_and_email(
            &self,
            _domain_id: Uuid,
            _email: &str,
        ) -> AppResult<Option<DomainEndUserProfile>> {
            Err(AppError::Internal("not implemented".into()))
        }

        async fn get_by_domain_and_google_id(
            &self,
            _domain_id: Uuid,
            _google_id: &str,
        ) -> AppResult<Option<DomainEndUserProfile>> {
            Err(AppError::Internal("not implemented".into()))
        }

        async fn upsert(&self, _domain_id: Uuid, _email: &str) -> AppResult<DomainEndUserProfile> {
            Err(AppError::Internal("not implemented".into()))
        }

        async fn upsert_with_google_id(
            &self,
            _domain_id: Uuid,
            _email: &str,
            _google_id: &str,
        ) -> AppResult<DomainEndUserProfile> {
            Err(AppError::Internal("not implemented".into()))
        }

        async fn mark_verified(&self, _id: Uuid) -> AppResult<DomainEndUserProfile> {
            Err(AppError::Internal("not implemented".into()))
        }

        async fn update_last_login(&self, _id: Uuid) -> AppResult<()> {
            Err(AppError::Internal("not implemented".into()))
        }

        async fn set_google_id(&self, _id: Uuid, _google_id: &str) -> AppResult<()> {
            Err(AppError::Internal("not implemented".into()))
        }

        async fn clear_google_id(&self, _id: Uuid) -> AppResult<()> {
            Err(AppError::Internal("not implemented".into()))
        }

        async fn list_by_domain(&self, _domain_id: Uuid) -> AppResult<Vec<DomainEndUserProfile>> {
            Err(AppError::Internal("not implemented".into()))
        }

        async fn delete(&self, _id: Uuid) -> AppResult<()> {
            Err(AppError::Internal("not implemented".into()))
        }

        async fn set_frozen(&self, _id: Uuid, _frozen: bool) -> AppResult<()> {
            Err(AppError::Internal("not implemented".into()))
        }

        async fn set_whitelisted(&self, _id: Uuid, _whitelisted: bool) -> AppResult<()> {
            Err(AppError::Internal("not implemented".into()))
        }

        async fn whitelist_all_in_domain(&self, _domain_id: Uuid) -> AppResult<()> {
            Err(AppError::Internal("not implemented".into()))
        }

        async fn count_by_domain_ids(&self, _domain_ids: &[Uuid]) -> AppResult<i64> {
            Err(AppError::Internal("not implemented".into()))
        }

        async fn get_waitlist_position(&self, _domain_id: Uuid, _user_id: Uuid) -> AppResult<i64> {
            Err(AppError::Internal("not implemented".into()))
        }

        async fn set_roles(&self, _id: Uuid, _roles: &[String]) -> AppResult<()> {
            Err(AppError::Internal("not implemented".into()))
        }

        async fn remove_role_from_all_users(
            &self,
            _domain_id: Uuid,
            _role_name: &str,
        ) -> AppResult<()> {
            Err(AppError::Internal("not implemented".into()))
        }

        async fn count_users_with_role(
            &self,
            _domain_id: Uuid,
            _role_name: &str,
        ) -> AppResult<i64> {
            Err(AppError::Internal("not implemented".into()))
        }
    }

    #[async_trait]
    impl OAuthStateStoreTrait for NoopRepo {
        async fn store_state(
            &self,
            _state: &str,
            _data: &OAuthStateData,
            _ttl_minutes: i64,
        ) -> AppResult<()> {
            Err(AppError::Internal("not implemented".into()))
        }

        async fn consume_state(&self, _state: &str) -> AppResult<Option<OAuthStateData>> {
            Err(AppError::Internal("not implemented".into()))
        }

        async fn mark_state_in_use(
            &self,
            _state: &str,
            _retry_window_secs: i64,
        ) -> AppResult<MarkStateResult> {
            Err(AppError::Internal("not implemented".into()))
        }

        async fn complete_state(&self, _state: &str) -> AppResult<()> {
            Err(AppError::Internal("not implemented".into()))
        }

        async fn abort_state(&self, _state: &str) -> AppResult<()> {
            Err(AppError::Internal("not implemented".into()))
        }

        async fn store_completion(
            &self,
            _token: &str,
            _data: &OAuthCompletionData,
            _ttl_minutes: i64,
        ) -> AppResult<()> {
            Err(AppError::Internal("not implemented".into()))
        }

        async fn consume_completion(&self, _token: &str) -> AppResult<Option<OAuthCompletionData>> {
            Err(AppError::Internal("not implemented".into()))
        }

        async fn store_link_confirmation(
            &self,
            _token: &str,
            _data: &OAuthLinkConfirmationData,
            _ttl_minutes: i64,
        ) -> AppResult<()> {
            Err(AppError::Internal("not implemented".into()))
        }

        async fn consume_link_confirmation(
            &self,
            _token: &str,
        ) -> AppResult<Option<OAuthLinkConfirmationData>> {
            Err(AppError::Internal("not implemented".into()))
        }
    }

    #[async_trait]
    impl DomainEmailSender for NoopRepo {
        async fn send(
            &self,
            _api_key: &str,
            _from_email: &str,
            _to: &str,
            _subject: &str,
            _html: &str,
        ) -> AppResult<()> {
            Err(AppError::Internal("not implemented".into()))
        }
    }

    fn build_use_cases(auth_repo: Arc<dyn DomainAuthConfigRepoTrait>) -> DomainAuthUseCases {
        let key_b64 = base64::engine::general_purpose::STANDARD.encode([0u8; 32]);
        let cipher = ProcessCipher::new_from_base64(&key_b64).expect("cipher");
        let noop = Arc::new(NoopRepo::default());
        DomainAuthUseCases::new(
            noop.clone(),
            auth_repo,
            noop.clone(),
            noop.clone(),
            noop.clone(),
            Arc::new(InMemoryMagicLinkStore::default()),
            noop.clone(),
            noop,
            cipher,
            "test_resend_key".to_string(),
            "test.example.com".to_string(),
            "test_google_client_id".to_string(),
            "test_google_client_secret".to_string(),
        )
    }

    fn auth_config_profile(
        domain_id: Uuid,
        magic_link_enabled: bool,
        google_oauth_enabled: bool,
    ) -> DomainAuthConfigProfile {
        DomainAuthConfigProfile {
            id: Uuid::new_v4(),
            domain_id,
            magic_link_enabled,
            google_oauth_enabled,
            redirect_url: None,
            whitelist_enabled: false,
            access_token_ttl_secs: 0,
            refresh_token_ttl_days: 0,
            created_at: None,
            updated_at: None,
        }
    }

    fn oauth_state_data(domain: &str, code_verifier: &str) -> OAuthStateData {
        OAuthStateData {
            domain: domain.to_string(),
            code_verifier: code_verifier.to_string(),
            status: default_pending(),
            marked_at: None,
        }
    }

    #[test]
    fn test_is_valid_redirect_url() {
        // Valid: exact domain match
        assert!(is_valid_redirect_url(
            "https://example.com/callback",
            "example.com"
        ));
        assert!(is_valid_redirect_url("https://example.com", "example.com"));
        assert!(is_valid_redirect_url(
            "http://example.com/path",
            "example.com"
        ));

        // Valid: subdomain
        assert!(is_valid_redirect_url(
            "https://app.example.com/callback",
            "example.com"
        ));
        assert!(is_valid_redirect_url(
            "https://login.example.com",
            "example.com"
        ));
        assert!(is_valid_redirect_url(
            "https://deep.nested.example.com/path",
            "example.com"
        ));

        // Invalid: different domain
        assert!(!is_valid_redirect_url(
            "https://evil.com/callback",
            "example.com"
        ));
        assert!(!is_valid_redirect_url(
            "https://notexample.com",
            "example.com"
        ));

        // Invalid: domain suffix attack (evil.com shouldn't match example.com)
        assert!(!is_valid_redirect_url(
            "https://fakeexample.com",
            "example.com"
        ));

        // Invalid: malformed URLs
        assert!(!is_valid_redirect_url("not-a-url", "example.com"));
        assert!(!is_valid_redirect_url("", "example.com"));
    }

    #[test]
    fn test_hash_domain_token_avoids_collisions() {
        // "ab" + "c" vs "a" + "bc" both produce "abc" if naively concatenated.
        let raw_a = "ab";
        let domain_a = "c";
        let raw_b = "a";
        let domain_b = "bc";

        let scoped_a = hash_domain_token(raw_a, domain_a);
        let scoped_b = hash_domain_token(raw_b, domain_b);
        assert_ne!(scoped_a, scoped_b);
    }

    #[tokio::test]
    async fn test_has_auth_methods_for_owner_domains_empty_input() {
        let repo = Arc::new(InMemoryAuthConfigRepo::default());
        let use_cases = build_use_cases(repo);

        let result = use_cases
            .has_auth_methods_for_owner_domains(&[])
            .await
            .expect("has auth methods");

        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_has_auth_methods_for_owner_domains_with_config() {
        let repo = Arc::new(InMemoryAuthConfigRepo::default());
        let domain_id = Uuid::new_v4();
        repo.configs
            .lock()
            .expect("auth config lock")
            .insert(domain_id, auth_config_profile(domain_id, true, false));

        let use_cases = build_use_cases(repo);
        let result = use_cases
            .has_auth_methods_for_owner_domains(&[domain_id])
            .await
            .expect("has auth methods");

        assert_eq!(result.get(&domain_id), Some(&true));
    }

    #[tokio::test]
    async fn test_has_auth_methods_for_owner_domains_disabled_config() {
        let repo = Arc::new(InMemoryAuthConfigRepo::default());
        let domain_id = Uuid::new_v4();
        repo.configs
            .lock()
            .expect("auth config lock")
            .insert(domain_id, auth_config_profile(domain_id, false, false));

        let use_cases = build_use_cases(repo);
        let result = use_cases
            .has_auth_methods_for_owner_domains(&[domain_id])
            .await
            .expect("has auth methods");

        assert_eq!(result.get(&domain_id), Some(&false));
    }

    #[tokio::test]
    async fn test_has_auth_methods_for_owner_domains_missing_config_defaults_to_true() {
        let repo = Arc::new(InMemoryAuthConfigRepo::default());
        let domain_id = Uuid::new_v4();
        let use_cases = build_use_cases(repo);
        let result = use_cases
            .has_auth_methods_for_owner_domains(&[domain_id])
            .await
            .expect("has auth methods");

        assert_eq!(result.get(&domain_id), Some(&true));
    }

    #[tokio::test]
    async fn test_has_auth_methods_for_owner_domains_mixed_configs() {
        let repo = Arc::new(InMemoryAuthConfigRepo::default());
        let domain_a = Uuid::new_v4();
        let domain_b = Uuid::new_v4();
        let domain_c = Uuid::new_v4();
        {
            let mut configs = repo.configs.lock().expect("auth config lock");
            configs.insert(domain_a, auth_config_profile(domain_a, true, false));
            configs.insert(domain_b, auth_config_profile(domain_b, false, false));
        }

        let use_cases = build_use_cases(repo);
        let result = use_cases
            .has_auth_methods_for_owner_domains(&[domain_a, domain_b, domain_c])
            .await
            .expect("has auth methods");

        assert_eq!(result.get(&domain_a), Some(&true));
        assert_eq!(result.get(&domain_b), Some(&false));
        assert_eq!(result.get(&domain_c), Some(&true));
    }

    #[tokio::test]
    async fn test_two_phase_happy_path() {
        let clock = TestClock::new(1000);
        let store = InMemoryOAuthStateStore::new(clock);

        store
            .store_state("abc", &oauth_state_data("example.com", "verifier"), 10)
            .await
            .unwrap();

        let result = store.mark_state_in_use("abc", 90).await.unwrap();
        assert!(matches!(result, MarkStateResult::Success(_)));

        store.complete_state("abc").await.unwrap();

        let result = store.mark_state_in_use("abc", 90).await.unwrap();
        assert!(matches!(result, MarkStateResult::NotFound));
    }

    #[tokio::test]
    async fn test_retry_within_window() {
        let clock = TestClock::new(1000);
        let store = InMemoryOAuthStateStore::new(clock.clone());

        store
            .store_state("abc", &oauth_state_data("example.com", "verifier"), 10)
            .await
            .unwrap();

        store.mark_state_in_use("abc", 90).await.unwrap();

        clock.advance(30);

        let result = store.mark_state_in_use("abc", 90).await.unwrap();
        assert!(matches!(result, MarkStateResult::Success(_)));
    }

    #[tokio::test]
    async fn test_retry_after_window_expires() {
        let clock = TestClock::new(1000);
        let store = InMemoryOAuthStateStore::new(clock.clone());

        store
            .store_state("abc", &oauth_state_data("example.com", "verifier"), 10)
            .await
            .unwrap();

        store.mark_state_in_use("abc", 90).await.unwrap();

        clock.advance(100);

        let result = store.mark_state_in_use("abc", 90).await.unwrap();
        assert!(matches!(result, MarkStateResult::RetryWindowExpired));
    }

    #[tokio::test]
    async fn test_retry_expired_abort_removes_state() {
        let clock = TestClock::new(1000);
        let store = InMemoryOAuthStateStore::new(clock.clone());

        store
            .store_state("abc", &oauth_state_data("example.com", "verifier"), 10)
            .await
            .unwrap();
        store.mark_state_in_use("abc", 90).await.unwrap();

        clock.advance(100);

        let result = store.mark_state_in_use("abc", 90).await.unwrap();
        assert!(matches!(result, MarkStateResult::RetryWindowExpired));

        store.abort_state("abc").await.unwrap();

        let result = store.mark_state_in_use("abc", 90).await.unwrap();
        assert!(matches!(result, MarkStateResult::NotFound));
    }

    #[tokio::test]
    async fn test_abort_removes_state() {
        let clock = TestClock::new(1000);
        let store = InMemoryOAuthStateStore::new(clock.clone());

        store
            .store_state("abc", &oauth_state_data("example.com", "verifier"), 10)
            .await
            .unwrap();
        store.mark_state_in_use("abc", 90).await.unwrap();

        store.abort_state("abc").await.unwrap();

        let result = store.mark_state_in_use("abc", 90).await.unwrap();
        assert!(matches!(result, MarkStateResult::NotFound));
    }

    #[tokio::test]
    async fn test_ttl_refresh_on_mark() {
        let clock = TestClock::new(1000);
        let store = InMemoryOAuthStateStore::new(clock.clone());

        store
            .store_state("abc", &oauth_state_data("example.com", "verifier"), 1)
            .await
            .unwrap();

        store.mark_state_in_use("abc", 90).await.unwrap();

        clock.advance(80);

        let result = store.mark_state_in_use("abc", 90).await.unwrap();
        assert!(matches!(result, MarkStateResult::Success(_)));
    }
}
