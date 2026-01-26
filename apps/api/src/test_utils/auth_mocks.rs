//! In-memory mock implementations for auth-related repository traits.
//!
//! These mocks are designed for HTTP-level integration testing of the `/auth/token` endpoint.

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Mutex;
use uuid::Uuid;

use crate::{
    app_error::{AppError, AppResult},
    application::use_cases::{
        api_key::{ApiKeyProfile, ApiKeyRepoTrait, ApiKeyWithDomain, ApiKeyWithRaw},
        domain_auth::{
            DomainAuthConfigProfile, DomainAuthConfigRepoTrait, DomainEndUserProfile,
            DomainEndUserRepoTrait,
        },
    },
    infra::crypto::ProcessCipher,
};

// ============================================================================
// InMemoryDomainEndUserRepo
// ============================================================================

/// In-memory implementation of DomainEndUserRepoTrait for testing.
#[derive(Default)]
pub struct InMemoryDomainEndUserRepo {
    pub users: Mutex<HashMap<Uuid, DomainEndUserProfile>>,
}

impl InMemoryDomainEndUserRepo {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_users(users: Vec<DomainEndUserProfile>) -> Self {
        let map: HashMap<Uuid, DomainEndUserProfile> =
            users.into_iter().map(|u| (u.id, u)).collect();
        Self {
            users: Mutex::new(map),
        }
    }
}

#[async_trait]
impl DomainEndUserRepoTrait for InMemoryDomainEndUserRepo {
    async fn get_by_id(&self, id: Uuid) -> AppResult<Option<DomainEndUserProfile>> {
        Ok(self.users.lock().unwrap().get(&id).cloned())
    }

    async fn get_by_domain_and_email(
        &self,
        domain_id: Uuid,
        email: &str,
    ) -> AppResult<Option<DomainEndUserProfile>> {
        Ok(self
            .users
            .lock()
            .unwrap()
            .values()
            .find(|u| u.domain_id == domain_id && u.email == email)
            .cloned())
    }

    async fn get_by_domain_and_google_id(
        &self,
        domain_id: Uuid,
        google_id: &str,
    ) -> AppResult<Option<DomainEndUserProfile>> {
        Ok(self
            .users
            .lock()
            .unwrap()
            .values()
            .find(|u| u.domain_id == domain_id && u.google_id.as_deref() == Some(google_id))
            .cloned())
    }

    async fn upsert(&self, domain_id: Uuid, email: &str) -> AppResult<DomainEndUserProfile> {
        let mut users = self.users.lock().unwrap();

        // Check if user exists
        if let Some(existing) = users
            .values()
            .find(|u| u.domain_id == domain_id && u.email == email)
        {
            return Ok(existing.clone());
        }

        let now = chrono::Utc::now().naive_utc();
        let user = DomainEndUserProfile {
            id: Uuid::new_v4(),
            domain_id,
            email: email.to_string(),
            roles: vec![],
            google_id: None,
            email_verified_at: None,
            last_login_at: None,
            is_frozen: false,
            is_whitelisted: false,
            created_at: Some(now),
            updated_at: Some(now),
        };

        users.insert(user.id, user.clone());
        Ok(user)
    }

    async fn upsert_with_google_id(
        &self,
        domain_id: Uuid,
        email: &str,
        google_id: &str,
    ) -> AppResult<DomainEndUserProfile> {
        let mut users = self.users.lock().unwrap();

        // Check if user exists by google_id first
        if let Some(existing) = users
            .values_mut()
            .find(|u| u.domain_id == domain_id && u.google_id.as_deref() == Some(google_id))
        {
            existing.email = email.to_string();
            existing.updated_at = Some(chrono::Utc::now().naive_utc());
            return Ok(existing.clone());
        }

        // Check by email
        if let Some(existing) = users
            .values_mut()
            .find(|u| u.domain_id == domain_id && u.email == email)
        {
            existing.google_id = Some(google_id.to_string());
            existing.updated_at = Some(chrono::Utc::now().naive_utc());
            return Ok(existing.clone());
        }

        let now = chrono::Utc::now().naive_utc();
        let user = DomainEndUserProfile {
            id: Uuid::new_v4(),
            domain_id,
            email: email.to_string(),
            roles: vec![],
            google_id: Some(google_id.to_string()),
            email_verified_at: Some(now),
            last_login_at: None,
            is_frozen: false,
            is_whitelisted: false,
            created_at: Some(now),
            updated_at: Some(now),
        };

        users.insert(user.id, user.clone());
        Ok(user)
    }

    async fn mark_verified(&self, id: Uuid) -> AppResult<DomainEndUserProfile> {
        let mut users = self.users.lock().unwrap();
        let user = users.get_mut(&id).ok_or(AppError::NotFound)?;
        user.email_verified_at = Some(chrono::Utc::now().naive_utc());
        user.updated_at = Some(chrono::Utc::now().naive_utc());
        Ok(user.clone())
    }

    async fn update_last_login(&self, id: Uuid) -> AppResult<()> {
        let mut users = self.users.lock().unwrap();
        if let Some(user) = users.get_mut(&id) {
            user.last_login_at = Some(chrono::Utc::now().naive_utc());
            user.updated_at = Some(chrono::Utc::now().naive_utc());
        }
        Ok(())
    }

    async fn set_google_id(&self, id: Uuid, google_id: &str) -> AppResult<()> {
        let mut users = self.users.lock().unwrap();
        let user = users.get_mut(&id).ok_or(AppError::NotFound)?;
        user.google_id = Some(google_id.to_string());
        user.updated_at = Some(chrono::Utc::now().naive_utc());
        Ok(())
    }

    async fn clear_google_id(&self, id: Uuid) -> AppResult<()> {
        let mut users = self.users.lock().unwrap();
        let user = users.get_mut(&id).ok_or(AppError::NotFound)?;
        user.google_id = None;
        user.updated_at = Some(chrono::Utc::now().naive_utc());
        Ok(())
    }

    async fn list_by_domain(&self, domain_id: Uuid) -> AppResult<Vec<DomainEndUserProfile>> {
        Ok(self
            .users
            .lock()
            .unwrap()
            .values()
            .filter(|u| u.domain_id == domain_id)
            .cloned()
            .collect())
    }

    async fn delete(&self, id: Uuid) -> AppResult<()> {
        self.users.lock().unwrap().remove(&id);
        Ok(())
    }

    async fn set_frozen(&self, id: Uuid, frozen: bool) -> AppResult<()> {
        let mut users = self.users.lock().unwrap();
        let user = users.get_mut(&id).ok_or(AppError::NotFound)?;
        user.is_frozen = frozen;
        user.updated_at = Some(chrono::Utc::now().naive_utc());
        Ok(())
    }

    async fn set_whitelisted(&self, id: Uuid, whitelisted: bool) -> AppResult<()> {
        let mut users = self.users.lock().unwrap();
        let user = users.get_mut(&id).ok_or(AppError::NotFound)?;
        user.is_whitelisted = whitelisted;
        user.updated_at = Some(chrono::Utc::now().naive_utc());
        Ok(())
    }

    async fn whitelist_all_in_domain(&self, domain_id: Uuid) -> AppResult<()> {
        let mut users = self.users.lock().unwrap();
        for user in users.values_mut().filter(|u| u.domain_id == domain_id) {
            user.is_whitelisted = true;
            user.updated_at = Some(chrono::Utc::now().naive_utc());
        }
        Ok(())
    }

    async fn count_by_domain_ids(&self, domain_ids: &[Uuid]) -> AppResult<i64> {
        Ok(self
            .users
            .lock()
            .unwrap()
            .values()
            .filter(|u| domain_ids.contains(&u.domain_id))
            .count() as i64)
    }

    async fn get_waitlist_position(&self, domain_id: Uuid, user_id: Uuid) -> AppResult<i64> {
        let users = self.users.lock().unwrap();
        let mut waitlist: Vec<_> = users
            .values()
            .filter(|u| u.domain_id == domain_id && !u.is_whitelisted)
            .collect();
        waitlist.sort_by(|a, b| a.created_at.cmp(&b.created_at));

        waitlist
            .iter()
            .position(|u| u.id == user_id)
            .map(|pos| (pos + 1) as i64)
            .ok_or(AppError::NotFound)
    }

    async fn set_roles(&self, id: Uuid, roles: &[String]) -> AppResult<()> {
        let mut users = self.users.lock().unwrap();
        let user = users.get_mut(&id).ok_or(AppError::NotFound)?;
        user.roles = roles.to_vec();
        user.updated_at = Some(chrono::Utc::now().naive_utc());
        Ok(())
    }

    async fn remove_role_from_all_users(&self, domain_id: Uuid, role_name: &str) -> AppResult<()> {
        let mut users = self.users.lock().unwrap();
        for user in users.values_mut().filter(|u| u.domain_id == domain_id) {
            user.roles.retain(|r| r != role_name);
            user.updated_at = Some(chrono::Utc::now().naive_utc());
        }
        Ok(())
    }

    async fn count_users_with_role(&self, domain_id: Uuid, role_name: &str) -> AppResult<i64> {
        Ok(self
            .users
            .lock()
            .unwrap()
            .values()
            .filter(|u| u.domain_id == domain_id && u.roles.contains(&role_name.to_string()))
            .count() as i64)
    }
}

// ============================================================================
// InMemoryDomainAuthConfigRepo
// ============================================================================

/// In-memory implementation of DomainAuthConfigRepoTrait for testing.
#[derive(Default)]
pub struct InMemoryDomainAuthConfigRepo {
    pub configs: Mutex<HashMap<Uuid, DomainAuthConfigProfile>>,
}

impl InMemoryDomainAuthConfigRepo {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_configs(configs: Vec<DomainAuthConfigProfile>) -> Self {
        let map: HashMap<Uuid, DomainAuthConfigProfile> =
            configs.into_iter().map(|c| (c.domain_id, c)).collect();
        Self {
            configs: Mutex::new(map),
        }
    }
}

#[async_trait]
impl DomainAuthConfigRepoTrait for InMemoryDomainAuthConfigRepo {
    async fn get_by_domain_id(
        &self,
        domain_id: Uuid,
    ) -> AppResult<Option<DomainAuthConfigProfile>> {
        Ok(self.configs.lock().unwrap().get(&domain_id).cloned())
    }

    async fn get_by_domain_ids(
        &self,
        domain_ids: &[Uuid],
    ) -> AppResult<Vec<DomainAuthConfigProfile>> {
        Ok(self
            .configs
            .lock()
            .unwrap()
            .values()
            .filter(|c| domain_ids.contains(&c.domain_id))
            .cloned()
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
        let mut configs = self.configs.lock().unwrap();
        let now = chrono::Utc::now().naive_utc();

        let config = DomainAuthConfigProfile {
            id: configs
                .get(&domain_id)
                .map(|c| c.id)
                .unwrap_or_else(Uuid::new_v4),
            domain_id,
            magic_link_enabled,
            google_oauth_enabled,
            redirect_url: redirect_url.map(|s| s.to_string()),
            whitelist_enabled,
            access_token_ttl_secs: 86400,
            refresh_token_ttl_days: 30,
            created_at: configs
                .get(&domain_id)
                .and_then(|c| c.created_at)
                .or(Some(now)),
            updated_at: Some(now),
        };

        configs.insert(domain_id, config.clone());
        Ok(config)
    }

    async fn delete(&self, domain_id: Uuid) -> AppResult<()> {
        self.configs.lock().unwrap().remove(&domain_id);
        Ok(())
    }
}

// ============================================================================
// InMemoryApiKeyRepo
// ============================================================================

/// Internal storage for API keys including encrypted value.
#[derive(Clone)]
struct ApiKeyStorage {
    pub profile: ApiKeyProfile,
    pub key_hash: String,
    pub key_encrypted: String,
    #[allow(dead_code)]
    pub created_by_end_user_id: Uuid,
    pub domain_name: String,
}

/// In-memory implementation of ApiKeyRepoTrait for testing.
#[derive(Default)]
pub struct InMemoryApiKeyRepo {
    keys: Mutex<HashMap<Uuid, ApiKeyStorage>>,
}

impl InMemoryApiKeyRepo {
    pub fn new() -> Self {
        Self::default()
    }

    /// Seed with pre-configured API keys for testing.
    /// The `raw_keys` map provides domain_id -> (raw_key, domain_name) for signing.
    pub fn with_signing_keys(
        keys: Vec<(Uuid, Uuid, String, String)>, // (domain_id, key_id, raw_key, domain_name)
        cipher: &ProcessCipher,
    ) -> Self {
        use sha2::{Digest, Sha256};

        let mut map = HashMap::new();
        let now = chrono::Utc::now().naive_utc();

        for (domain_id, key_id, raw_key, domain_name) in keys {
            let key_encrypted = cipher.encrypt(&raw_key).unwrap_or_default();
            // Use real hash so validate_api_key works
            let key_hash = hex::encode(Sha256::digest(raw_key.as_bytes()));
            let key_prefix = raw_key.chars().take(8).collect::<String>();

            let storage = ApiKeyStorage {
                profile: ApiKeyProfile {
                    id: key_id,
                    domain_id,
                    key_prefix,
                    name: "Test Key".to_string(),
                    last_used_at: None,
                    revoked_at: None,
                    created_at: Some(now),
                },
                key_hash,
                key_encrypted,
                created_by_end_user_id: Uuid::new_v4(),
                domain_name,
            };

            map.insert(key_id, storage);
        }

        Self {
            keys: Mutex::new(map),
        }
    }
}

#[async_trait]
impl ApiKeyRepoTrait for InMemoryApiKeyRepo {
    async fn create(
        &self,
        domain_id: Uuid,
        key_prefix: &str,
        key_hash: &str,
        key_encrypted: &str,
        name: &str,
        created_by_end_user_id: Uuid,
    ) -> AppResult<ApiKeyProfile> {
        let mut keys = self.keys.lock().unwrap();
        let now = chrono::Utc::now().naive_utc();

        let profile = ApiKeyProfile {
            id: Uuid::new_v4(),
            domain_id,
            key_prefix: key_prefix.to_string(),
            name: name.to_string(),
            last_used_at: None,
            revoked_at: None,
            created_at: Some(now),
        };

        let storage = ApiKeyStorage {
            profile: profile.clone(),
            key_hash: key_hash.to_string(),
            key_encrypted: key_encrypted.to_string(),
            created_by_end_user_id,
            domain_name: String::new(), // Not used for create
        };

        keys.insert(profile.id, storage);
        Ok(profile)
    }

    async fn get_by_hash(&self, key_hash: &str) -> AppResult<Option<ApiKeyWithDomain>> {
        Ok(self
            .keys
            .lock()
            .unwrap()
            .values()
            .find(|k| k.key_hash == key_hash)
            .map(|k| ApiKeyWithDomain {
                id: k.profile.id,
                domain_id: k.profile.domain_id,
                domain_name: k.domain_name.clone(),
                revoked_at: k.profile.revoked_at,
            }))
    }

    async fn list_by_domain(&self, domain_id: Uuid) -> AppResult<Vec<ApiKeyProfile>> {
        Ok(self
            .keys
            .lock()
            .unwrap()
            .values()
            .filter(|k| k.profile.domain_id == domain_id)
            .map(|k| k.profile.clone())
            .collect())
    }

    async fn revoke(&self, id: Uuid) -> AppResult<()> {
        let mut keys = self.keys.lock().unwrap();
        if let Some(key) = keys.get_mut(&id) {
            key.profile.revoked_at = Some(chrono::Utc::now().naive_utc());
        }
        Ok(())
    }

    async fn update_last_used(&self, id: Uuid) -> AppResult<()> {
        let mut keys = self.keys.lock().unwrap();
        if let Some(key) = keys.get_mut(&id) {
            key.profile.last_used_at = Some(chrono::Utc::now().naive_utc());
        }
        Ok(())
    }

    async fn count_active_by_domain(&self, domain_id: Uuid) -> AppResult<i64> {
        Ok(self
            .keys
            .lock()
            .unwrap()
            .values()
            .filter(|k| k.profile.domain_id == domain_id && k.profile.revoked_at.is_none())
            .count() as i64)
    }

    async fn get_signing_key_for_domain(
        &self,
        domain_id: Uuid,
        cipher: &ProcessCipher,
    ) -> AppResult<Option<ApiKeyWithRaw>> {
        let keys = self.keys.lock().unwrap();

        // Get newest active key with encrypted value
        let key = keys
            .values()
            .filter(|k| {
                k.profile.domain_id == domain_id
                    && k.profile.revoked_at.is_none()
                    && !k.key_encrypted.is_empty()
            })
            .max_by_key(|k| k.profile.created_at);

        match key {
            Some(k) => {
                let raw_key = cipher.decrypt(&k.key_encrypted)?;
                Ok(Some(ApiKeyWithRaw {
                    id: k.profile.id,
                    domain_id: k.profile.domain_id,
                    raw_key,
                }))
            }
            None => Ok(None),
        }
    }

    async fn get_all_active_keys_for_domain(
        &self,
        domain_id: Uuid,
        cipher: &ProcessCipher,
    ) -> AppResult<Vec<ApiKeyWithRaw>> {
        let keys = self.keys.lock().unwrap();

        keys.values()
            .filter(|k| {
                k.profile.domain_id == domain_id
                    && k.profile.revoked_at.is_none()
                    && !k.key_encrypted.is_empty()
            })
            .map(|k| {
                let raw_key = cipher.decrypt(&k.key_encrypted)?;
                Ok(ApiKeyWithRaw {
                    id: k.profile.id,
                    domain_id: k.profile.domain_id,
                    raw_key,
                })
            })
            .collect()
    }
}

// ============================================================================
// Test Factories
// ============================================================================

/// Create a test end user with sensible defaults.
pub fn create_test_end_user(
    domain_id: Uuid,
    overrides: impl FnOnce(&mut DomainEndUserProfile),
) -> DomainEndUserProfile {
    let now = chrono::Utc::now().naive_utc();
    let mut user = DomainEndUserProfile {
        id: Uuid::new_v4(),
        domain_id,
        email: "test@example.com".to_string(),
        roles: vec![],
        google_id: None,
        email_verified_at: Some(now),
        last_login_at: Some(now),
        is_frozen: false,
        is_whitelisted: true,
        created_at: Some(now),
        updated_at: Some(now),
    };
    overrides(&mut user);
    user
}

/// Create a test auth config with sensible defaults.
pub fn create_test_auth_config(
    domain_id: Uuid,
    overrides: impl FnOnce(&mut DomainAuthConfigProfile),
) -> DomainAuthConfigProfile {
    let now = chrono::Utc::now().naive_utc();
    let mut config = DomainAuthConfigProfile {
        id: Uuid::new_v4(),
        domain_id,
        magic_link_enabled: true,
        google_oauth_enabled: true,
        redirect_url: Some("https://example.com".to_string()),
        whitelist_enabled: false,
        access_token_ttl_secs: 86400,
        refresh_token_ttl_days: 30,
        created_at: Some(now),
        updated_at: Some(now),
    };
    overrides(&mut config);
    config
}

// ============================================================================
// Stub Implementations for Unused Dependencies
// ============================================================================

/// Stub implementation of DomainAuthMagicLinkRepoTrait - not used in token tests.
#[derive(Default)]
pub struct StubMagicLinkConfigRepo;

#[async_trait]
impl crate::application::use_cases::domain_auth::DomainAuthMagicLinkRepoTrait
    for StubMagicLinkConfigRepo
{
    async fn get_by_domain_id(
        &self,
        _domain_id: Uuid,
    ) -> AppResult<Option<crate::application::use_cases::domain_auth::DomainAuthMagicLinkProfile>>
    {
        Ok(None)
    }

    async fn upsert(
        &self,
        _domain_id: Uuid,
        _resend_api_key_encrypted: &str,
        _from_email: &str,
    ) -> AppResult<crate::application::use_cases::domain_auth::DomainAuthMagicLinkProfile> {
        unimplemented!("not needed for token tests")
    }

    async fn update_from_email(&self, _domain_id: Uuid, _from_email: &str) -> AppResult<()> {
        unimplemented!("not needed for token tests")
    }

    async fn delete(&self, _domain_id: Uuid) -> AppResult<()> {
        unimplemented!("not needed for token tests")
    }
}

/// Stub implementation of DomainAuthGoogleOAuthRepoTrait - not used in token tests.
#[derive(Default)]
pub struct StubGoogleOAuthConfigRepo;

#[async_trait]
impl crate::application::use_cases::domain_auth::DomainAuthGoogleOAuthRepoTrait
    for StubGoogleOAuthConfigRepo
{
    async fn get_by_domain_id(
        &self,
        _domain_id: Uuid,
    ) -> AppResult<Option<crate::application::use_cases::domain_auth::DomainAuthGoogleOAuthProfile>>
    {
        Ok(None)
    }

    async fn upsert(
        &self,
        _domain_id: Uuid,
        _client_id_encrypted: &str,
        _client_secret_encrypted: &str,
    ) -> AppResult<crate::application::use_cases::domain_auth::DomainAuthGoogleOAuthProfile> {
        unimplemented!("not needed for token tests")
    }

    async fn delete(&self, _domain_id: Uuid) -> AppResult<()> {
        unimplemented!("not needed for token tests")
    }
}

/// Stub implementation of DomainMagicLinkStore - not used in token tests.
#[derive(Default)]
pub struct StubMagicLinkStore;

#[async_trait]
impl crate::application::use_cases::domain_auth::DomainMagicLinkStore for StubMagicLinkStore {
    async fn save(
        &self,
        _token_hash: &str,
        _end_user_id: Uuid,
        _domain_id: Uuid,
        _session_id: &str,
        _ttl_minutes: i64,
    ) -> AppResult<()> {
        unimplemented!("not needed for token tests")
    }

    async fn consume(
        &self,
        _token_hash: &str,
        _session_id: &str,
    ) -> AppResult<Option<crate::application::use_cases::domain_auth::DomainMagicLinkData>> {
        unimplemented!("not needed for token tests")
    }
}

/// Stub implementation of OAuthStateStoreTrait - not used in token tests.
#[derive(Default)]
pub struct StubOAuthStateStore;

#[async_trait]
impl crate::application::use_cases::domain_auth::OAuthStateStoreTrait for StubOAuthStateStore {
    async fn store_state(
        &self,
        _state: &str,
        _data: &crate::application::use_cases::domain_auth::OAuthStateData,
        _ttl_minutes: i64,
    ) -> AppResult<()> {
        unimplemented!("not needed for token tests")
    }

    async fn consume_state(
        &self,
        _state: &str,
    ) -> AppResult<Option<crate::application::use_cases::domain_auth::OAuthStateData>> {
        unimplemented!("not needed for token tests")
    }

    async fn mark_state_in_use(
        &self,
        _state: &str,
        _retry_window_secs: i64,
    ) -> AppResult<crate::application::use_cases::domain_auth::MarkStateResult> {
        unimplemented!("not needed for token tests")
    }

    async fn complete_state(&self, _state: &str) -> AppResult<()> {
        unimplemented!("not needed for token tests")
    }

    async fn abort_state(&self, _state: &str) -> AppResult<()> {
        unimplemented!("not needed for token tests")
    }

    async fn store_completion(
        &self,
        _token: &str,
        _data: &crate::application::use_cases::domain_auth::OAuthCompletionData,
        _ttl_minutes: i64,
    ) -> AppResult<()> {
        unimplemented!("not needed for token tests")
    }

    async fn consume_completion(
        &self,
        _token: &str,
    ) -> AppResult<Option<crate::application::use_cases::domain_auth::OAuthCompletionData>> {
        unimplemented!("not needed for token tests")
    }

    async fn store_link_confirmation(
        &self,
        _token: &str,
        _data: &crate::application::use_cases::domain_auth::OAuthLinkConfirmationData,
        _ttl_minutes: i64,
    ) -> AppResult<()> {
        unimplemented!("not needed for token tests")
    }

    async fn consume_link_confirmation(
        &self,
        _token: &str,
    ) -> AppResult<Option<crate::application::use_cases::domain_auth::OAuthLinkConfirmationData>>
    {
        unimplemented!("not needed for token tests")
    }
}

/// Stub implementation of DomainEmailSender - not used in token tests.
#[derive(Default)]
pub struct StubEmailSender;

#[async_trait]
impl crate::application::use_cases::domain_auth::DomainEmailSender for StubEmailSender {
    async fn send(
        &self,
        _api_key: &str,
        _from_email: &str,
        _to: &str,
        _subject: &str,
        _html: &str,
    ) -> AppResult<()> {
        unimplemented!("not needed for token tests")
    }
}

// ============================================================================
// InMemoryRateLimiter
// ============================================================================

/// In-memory rate limiter for testing.
/// Uses HashMap to track request counts per key.
pub struct InMemoryRateLimiter {
    counts: Mutex<HashMap<String, u64>>,
    max_per_ip: u64,
    max_per_email: u64,
}

impl InMemoryRateLimiter {
    pub fn new(max_per_ip: u64, max_per_email: u64) -> Self {
        Self {
            counts: Mutex::new(HashMap::new()),
            max_per_ip,
            max_per_email,
        }
    }

    /// Create a permissive rate limiter that never blocks (for most tests).
    pub fn permissive() -> Self {
        Self::new(u64::MAX, u64::MAX)
    }
}

#[async_trait]
impl crate::infra::RateLimiterTrait for InMemoryRateLimiter {
    async fn check(&self, ip: &str, email: Option<&str>) -> AppResult<()> {
        let mut counts = self.counts.lock().unwrap();

        let ip_key = format!("rate:ip:{ip}");
        let ip_count = counts.entry(ip_key).or_insert(0);
        *ip_count += 1;
        if *ip_count > self.max_per_ip {
            return Err(AppError::RateLimited);
        }

        if let Some(email) = email {
            let email_key = format!("rate:email:{}", email.to_lowercase());
            let email_count = counts.entry(email_key).or_insert(0);
            *email_count += 1;
            if *email_count > self.max_per_email {
                return Err(AppError::RateLimited);
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_end_user_repo_get_by_id() {
        let user_id = Uuid::new_v4();
        let domain_id = Uuid::new_v4();

        let user = create_test_end_user(domain_id, |u| {
            u.id = user_id;
            u.email = "alice@example.com".to_string();
        });

        let repo = InMemoryDomainEndUserRepo::with_users(vec![user]);

        let found = repo.get_by_id(user_id).await.unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().email, "alice@example.com");
    }

    #[tokio::test]
    async fn test_end_user_repo_frozen_status() {
        let user_id = Uuid::new_v4();
        let domain_id = Uuid::new_v4();

        let user = create_test_end_user(domain_id, |u| {
            u.id = user_id;
            u.is_frozen = false;
        });

        let repo = InMemoryDomainEndUserRepo::with_users(vec![user]);

        // Set frozen
        repo.set_frozen(user_id, true).await.unwrap();

        let found = repo.get_by_id(user_id).await.unwrap().unwrap();
        assert!(found.is_frozen);
    }

    #[tokio::test]
    async fn test_auth_config_repo_upsert() {
        let repo = InMemoryDomainAuthConfigRepo::new();
        let domain_id = Uuid::new_v4();

        let config = repo
            .upsert(domain_id, true, false, Some("https://test.com"), false)
            .await
            .unwrap();

        assert_eq!(config.domain_id, domain_id);
        assert!(config.magic_link_enabled);
        assert!(!config.google_oauth_enabled);

        // Upsert again should update
        let updated = repo
            .upsert(domain_id, false, true, None, true)
            .await
            .unwrap();

        assert_eq!(updated.id, config.id); // Same ID
        assert!(!updated.magic_link_enabled);
        assert!(updated.google_oauth_enabled);
    }
}
