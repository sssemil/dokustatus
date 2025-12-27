use std::sync::Arc;

use async_trait::async_trait;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use chrono::NaiveDateTime;
use rand::RngCore;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::app_error::{AppError, AppResult};
use crate::application::use_cases::domain::DomainRepo;
use crate::application::use_cases::domain_auth::{DomainEndUserProfile, DomainEndUserRepo};

// ============================================================================
// Repository Trait
// ============================================================================

#[async_trait]
pub trait ApiKeyRepo: Send + Sync {
    async fn create(
        &self,
        domain_id: Uuid,
        key_prefix: &str,
        key_hash: &str,
        name: &str,
        created_by_end_user_id: Uuid,
    ) -> AppResult<ApiKeyProfile>;

    async fn get_by_hash(&self, key_hash: &str) -> AppResult<Option<ApiKeyWithDomain>>;

    async fn list_by_domain(&self, domain_id: Uuid) -> AppResult<Vec<ApiKeyProfile>>;

    async fn revoke(&self, id: Uuid) -> AppResult<()>;

    async fn update_last_used(&self, id: Uuid) -> AppResult<()>;
}

// ============================================================================
// Profile Types
// ============================================================================

#[derive(Debug, Clone)]
pub struct ApiKeyProfile {
    pub id: Uuid,
    pub domain_id: Uuid,
    pub key_prefix: String,
    pub name: String,
    pub last_used_at: Option<NaiveDateTime>,
    pub revoked_at: Option<NaiveDateTime>,
    pub created_at: Option<NaiveDateTime>,
}

/// ApiKey with domain info for validation
#[derive(Debug, Clone)]
pub struct ApiKeyWithDomain {
    pub id: Uuid,
    pub domain_id: Uuid,
    pub domain_name: String,
    pub revoked_at: Option<NaiveDateTime>,
}

// ============================================================================
// Use Cases
// ============================================================================

#[derive(Clone)]
pub struct ApiKeyUseCases {
    api_key_repo: Arc<dyn ApiKeyRepo>,
    domain_repo: Arc<dyn DomainRepo>,
    end_user_repo: Arc<dyn DomainEndUserRepo>,
}

impl ApiKeyUseCases {
    pub fn new(
        api_key_repo: Arc<dyn ApiKeyRepo>,
        domain_repo: Arc<dyn DomainRepo>,
        end_user_repo: Arc<dyn DomainEndUserRepo>,
    ) -> Self {
        Self {
            api_key_repo,
            domain_repo,
            end_user_repo,
        }
    }

    // ========================================================================
    // Dashboard Operations (require domain ownership)
    // ========================================================================

    /// Create a new API key for a domain.
    /// Returns the profile and the raw key (shown only once).
    pub async fn create_api_key(
        &self,
        owner_end_user_id: Uuid,
        domain_id: Uuid,
        name: &str,
    ) -> AppResult<(ApiKeyProfile, String)> {
        // Verify ownership
        self.verify_domain_ownership(owner_end_user_id, domain_id)
            .await?;

        // Generate key
        let raw_key = generate_api_key();
        let key_prefix = &raw_key[..16]; // "sk_live_" + first 8 chars of random
        let key_hash = hash_api_key(&raw_key);

        // Sanitize name
        let name = name.trim();
        let name = if name.is_empty() { "Default" } else { name };

        // Create
        let profile = self
            .api_key_repo
            .create(domain_id, key_prefix, &key_hash, name, owner_end_user_id)
            .await?;

        Ok((profile, raw_key))
    }

    /// List all API keys for a domain (for dashboard display).
    pub async fn list_api_keys(
        &self,
        owner_end_user_id: Uuid,
        domain_id: Uuid,
    ) -> AppResult<Vec<ApiKeyProfile>> {
        // Verify ownership
        self.verify_domain_ownership(owner_end_user_id, domain_id)
            .await?;

        self.api_key_repo.list_by_domain(domain_id).await
    }

    /// Revoke an API key.
    pub async fn revoke_api_key(
        &self,
        owner_end_user_id: Uuid,
        domain_id: Uuid,
        key_id: Uuid,
    ) -> AppResult<()> {
        // Verify ownership
        self.verify_domain_ownership(owner_end_user_id, domain_id)
            .await?;

        // Verify the key belongs to this domain
        let keys = self.api_key_repo.list_by_domain(domain_id).await?;
        if !keys.iter().any(|k| k.id == key_id) {
            return Err(AppError::NotFound);
        }

        self.api_key_repo.revoke(key_id).await
    }

    // ========================================================================
    // Developer API Operations (require valid API key)
    // ========================================================================

    /// Validate an API key and return the domain info if valid.
    /// Returns (domain_id, domain_name, key_id) if valid.
    pub async fn validate_api_key(
        &self,
        raw_key: &str,
    ) -> AppResult<Option<(Uuid, String, Uuid)>> {
        // Hash the key
        let key_hash = hash_api_key(raw_key);

        // Look up
        let Some(key_with_domain) = self.api_key_repo.get_by_hash(&key_hash).await? else {
            return Ok(None);
        };

        // Check if revoked
        if key_with_domain.revoked_at.is_some() {
            return Ok(None);
        }

        Ok(Some((
            key_with_domain.domain_id,
            key_with_domain.domain_name,
            key_with_domain.id,
        )))
    }

    /// Update the last_used_at timestamp for a key.
    /// This is fire-and-forget, errors are ignored.
    pub async fn update_last_used(&self, key_id: Uuid) -> AppResult<()> {
        self.api_key_repo.update_last_used(key_id).await
    }

    /// Get a user by ID for a specific domain.
    pub async fn get_user_by_id(
        &self,
        domain_id: Uuid,
        user_id: Uuid,
    ) -> AppResult<DomainEndUserProfile> {
        let user = self
            .end_user_repo
            .get_by_id(user_id)
            .await?
            .ok_or(AppError::NotFound)?;

        // Verify user belongs to this domain
        if user.domain_id != domain_id {
            return Err(AppError::NotFound);
        }

        Ok(user)
    }

    // ========================================================================
    // Private Helpers
    // ========================================================================

    async fn verify_domain_ownership(
        &self,
        owner_end_user_id: Uuid,
        domain_id: Uuid,
    ) -> AppResult<()> {
        let domain = self
            .domain_repo
            .get_by_id(domain_id)
            .await?
            .ok_or(AppError::NotFound)?;

        if domain.owner_end_user_id != Some(owner_end_user_id) {
            return Err(AppError::InvalidCredentials);
        }

        Ok(())
    }
}

// ============================================================================
// Key Generation
// ============================================================================

/// Generate a new API key with format: sk_live_<base64url_24_bytes>
fn generate_api_key() -> String {
    let mut bytes = [0u8; 24]; // 24 bytes = 32 chars base64
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    let random_part = URL_SAFE_NO_PAD.encode(bytes);
    format!("sk_live_{}", random_part)
}

/// Hash an API key using SHA-256, returning hex-encoded hash.
fn hash_api_key(raw_key: &str) -> String {
    let hash = Sha256::digest(raw_key.as_bytes());
    hex::encode(hash)
}
