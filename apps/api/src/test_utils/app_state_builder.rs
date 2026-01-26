//! Test app state builder for HTTP-level integration testing.
//!
//! This module provides `TestAppStateBuilder` which creates a minimal `AppState`
//! with in-memory mocks for testing HTTP endpoints.

use std::sync::Arc;

use uuid::Uuid;

use std::net::SocketAddr;

use axum::http::HeaderValue;
use secrecy::SecretString;
use time::Duration;
use url::Url;

use crate::{
    adapters::http::app_state::AppState,
    adapters::persistence::domain_role::{DomainRoleRepoTrait, DomainRoleWithCount},
    app_error::AppResult,
    application::use_cases::{
        api_key::ApiKeyUseCases,
        domain::{DnsVerifier, DomainProfile, DomainUseCases},
        domain_auth::{
            DomainAuthConfigProfile, DomainAuthUseCases, DomainEmailSender, DomainEndUserProfile,
            DomainMagicLinkStore, OAuthStateStoreTrait,
        },
        domain_billing::DomainBillingUseCases,
        domain_roles::DomainRolesUseCases,
        payment_provider_factory::PaymentProviderFactory,
    },
    domain::entities::domain_role::DomainRole,
    infra::{RateLimiterTrait, config::AppConfig, crypto::ProcessCipher},
    test_utils::{
        InMemoryApiKeyRepo, InMemoryBillingPaymentRepo, InMemoryBillingStripeConfigRepo,
        InMemoryDomainAuthConfigRepo, InMemoryDomainEndUserRepo, InMemoryDomainRepo,
        InMemoryEmailSender, InMemoryEnabledPaymentProvidersRepo, InMemoryMagicLinkStore,
        InMemoryOAuthStateStore, InMemoryRateLimiter, InMemorySubscriptionEventRepo,
        InMemorySubscriptionPlanRepo, InMemoryUserSubscriptionRepo, StubEmailSender,
        StubGoogleOAuthConfigRepo, StubMagicLinkConfigRepo, StubMagicLinkStore,
    },
};

use async_trait::async_trait;

// ============================================================================
// Stub Implementations for Unused Dependencies
// ============================================================================

/// Stub DNS verifier - not used in token tests.
#[derive(Default)]
pub struct StubDnsVerifier;

#[async_trait]
impl DnsVerifier for StubDnsVerifier {
    async fn check_cname(&self, _domain: &str, _expected_target: &str) -> AppResult<bool> {
        Ok(false)
    }

    async fn check_txt(&self, _domain: &str, _expected_value: &str) -> AppResult<bool> {
        Ok(false)
    }
}

/// Stub domain role repo - not used in token tests.
#[derive(Default)]
pub struct StubDomainRoleRepo;

#[async_trait]
impl DomainRoleRepoTrait for StubDomainRoleRepo {
    async fn create(&self, _domain_id: Uuid, _name: &str) -> AppResult<DomainRole> {
        unimplemented!("not needed for token tests")
    }

    async fn list_by_domain(&self, _domain_id: Uuid) -> AppResult<Vec<DomainRole>> {
        Ok(vec![])
    }

    async fn list_by_domain_with_counts(
        &self,
        _domain_id: Uuid,
    ) -> AppResult<Vec<DomainRoleWithCount>> {
        Ok(vec![])
    }

    async fn get_by_name(&self, _domain_id: Uuid, _name: &str) -> AppResult<Option<DomainRole>> {
        Ok(None)
    }

    async fn delete(&self, _domain_id: Uuid, _name: &str) -> AppResult<()> {
        Ok(())
    }

    async fn exists(&self, _domain_id: Uuid, _name: &str) -> AppResult<bool> {
        Ok(false)
    }
}

// ============================================================================
// TestAppStateBuilder
// ============================================================================

/// Builder for creating `AppState` with in-memory mocks for testing.
///
/// # Example
///
/// ```ignore
/// let domain = create_test_domain(|d| d.domain = "example.com".to_string());
/// let user = create_test_end_user(domain.id, |u| u.is_frozen = false);
///
/// let app_state = TestAppStateBuilder::new()
///     .with_domain(domain)
///     .with_user(user)
///     .with_api_key(domain.id, "test_api_key_12345678")
///     .build();
/// ```
pub struct TestAppStateBuilder {
    domains: Vec<DomainProfile>,
    users: Vec<DomainEndUserProfile>,
    auth_configs: Vec<DomainAuthConfigProfile>,
    api_keys: Vec<(Uuid, Uuid, String, String)>, // (domain_id, key_id, raw_key, domain_name)
    cipher: ProcessCipher,
    magic_link_store: Option<Arc<dyn DomainMagicLinkStore>>,
    email_sender: Option<Arc<dyn DomainEmailSender>>,
    main_domain: String,
}

impl TestAppStateBuilder {
    /// Create a new builder with default test cipher.
    pub fn new() -> Self {
        // Use a fixed test key for reproducible tests (base64-encoded 32 bytes)
        let test_key_b64 = "QkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkI="; // 32 'B' bytes
        let cipher =
            ProcessCipher::new_from_base64(test_key_b64).expect("Test cipher key should be valid");

        Self {
            domains: vec![],
            users: vec![],
            auth_configs: vec![],
            api_keys: vec![],
            cipher,
            magic_link_store: None,
            email_sender: None,
            main_domain: "reauth.test".to_string(),
        }
    }

    /// Add a domain to the test state.
    pub fn with_domain(mut self, domain: DomainProfile) -> Self {
        self.domains.push(domain);
        self
    }

    /// Add an end user to the test state.
    pub fn with_user(mut self, user: DomainEndUserProfile) -> Self {
        self.users.push(user);
        self
    }

    /// Add an auth config to the test state.
    pub fn with_auth_config(mut self, config: DomainAuthConfigProfile) -> Self {
        self.auth_configs.push(config);
        self
    }

    /// Add an API key for a domain.
    pub fn with_api_key(mut self, domain_id: Uuid, domain_name: &str, raw_key: &str) -> Self {
        let key_id = Uuid::new_v4();
        self.api_keys.push((
            domain_id,
            key_id,
            raw_key.to_string(),
            domain_name.to_string(),
        ));
        self
    }

    /// Get the cipher used by this builder (for signing test tokens).
    pub fn cipher(&self) -> &ProcessCipher {
        &self.cipher
    }

    /// Set a custom magic link store (for testing magic link flow).
    pub fn with_magic_link_store(mut self, store: Arc<dyn DomainMagicLinkStore>) -> Self {
        self.magic_link_store = Some(store);
        self
    }

    /// Set a custom email sender (for testing email sending).
    pub fn with_email_sender(mut self, sender: Arc<dyn DomainEmailSender>) -> Self {
        self.email_sender = Some(sender);
        self
    }

    /// Set the main domain (for dashboard user tests that check main_domain access).
    pub fn with_main_domain(mut self, main_domain: String) -> Self {
        self.main_domain = main_domain;
        self
    }

    /// Create app state with in-memory magic link store and email sender.
    /// Returns (AppState, Arc<InMemoryMagicLinkStore>, Arc<InMemoryEmailSender>) for test assertions.
    pub fn build_with_magic_link_mocks(
        self,
    ) -> (
        AppState,
        Arc<InMemoryMagicLinkStore>,
        Arc<InMemoryEmailSender>,
    ) {
        let magic_link_store = Arc::new(InMemoryMagicLinkStore::new());
        let email_sender = Arc::new(InMemoryEmailSender::new());

        let app_state = self
            .with_magic_link_store(magic_link_store.clone())
            .with_email_sender(email_sender.clone())
            .build();

        (app_state, magic_link_store, email_sender)
    }

    /// Build the AppState with all configured mocks.
    pub fn build(self) -> AppState {
        // Create shared repos
        let domain_repo = Arc::new(InMemoryDomainRepo::with_domains(self.domains));
        let end_user_repo = Arc::new(InMemoryDomainEndUserRepo::with_users(self.users));
        let auth_config_repo = Arc::new(InMemoryDomainAuthConfigRepo::with_configs(
            self.auth_configs,
        ));
        let api_key_repo = Arc::new(InMemoryApiKeyRepo::with_signing_keys(
            self.api_keys,
            &self.cipher,
        ));

        // Billing repos (mostly unused but needed for DomainBillingUseCases)
        let stripe_config_repo = Arc::new(InMemoryBillingStripeConfigRepo::new());
        let enabled_providers_repo = Arc::new(InMemoryEnabledPaymentProvidersRepo::new());
        let plan_repo = Arc::new(InMemorySubscriptionPlanRepo::new());
        let subscription_repo = Arc::new(InMemoryUserSubscriptionRepo::new());
        let event_repo = Arc::new(InMemorySubscriptionEventRepo::new());
        let payment_repo = Arc::new(InMemoryBillingPaymentRepo::new());

        // Stub repos for auth
        let magic_link_config_repo = Arc::new(StubMagicLinkConfigRepo);
        let google_oauth_config_repo = Arc::new(StubGoogleOAuthConfigRepo);
        let magic_link_store: Arc<dyn DomainMagicLinkStore> = self
            .magic_link_store
            .unwrap_or_else(|| Arc::new(StubMagicLinkStore));
        let oauth_state_store: Arc<dyn OAuthStateStoreTrait> =
            Arc::new(InMemoryOAuthStateStore::new());
        let email_sender: Arc<dyn DomainEmailSender> = self
            .email_sender
            .unwrap_or_else(|| Arc::new(StubEmailSender));

        // Create use cases
        let domain_use_cases = Arc::new(DomainUseCases::new(
            domain_repo.clone(),
            Arc::new(StubDnsVerifier),
            "ingress.test".to_string(),
        ));

        let domain_auth_use_cases = Arc::new(DomainAuthUseCases::new(
            domain_repo.clone(),
            auth_config_repo,
            magic_link_config_repo,
            google_oauth_config_repo,
            end_user_repo.clone(),
            magic_link_store,
            oauth_state_store,
            email_sender,
            self.cipher.clone(),
            "fallback_key".to_string(),
            "fallback.test".to_string(),
            "fallback_client_id".to_string(),
            "fallback_client_secret".to_string(),
        ));

        let api_key_use_cases = Arc::new(ApiKeyUseCases::new(
            api_key_repo,
            domain_repo.clone(),
            end_user_repo.clone(),
            self.cipher.clone(),
        ));

        let domain_roles_use_cases = Arc::new(DomainRolesUseCases::new(
            domain_repo.clone(),
            Arc::new(StubDomainRoleRepo),
            end_user_repo,
        ));

        let provider_factory = Arc::new(PaymentProviderFactory::new(
            self.cipher.clone(),
            stripe_config_repo.clone(),
        ));

        let billing_use_cases = Arc::new(DomainBillingUseCases::new(
            domain_repo,
            stripe_config_repo,
            enabled_providers_repo,
            plan_repo,
            subscription_repo,
            event_repo,
            payment_repo,
            self.cipher,
            provider_factory,
        ));

        // Create minimal config for testing
        let config = Arc::new(AppConfig {
            jwt_secret: SecretString::new("test_jwt_secret".into()),
            access_token_ttl: Duration::hours(24),
            refresh_token_ttl: Duration::days(30),
            app_origin: Url::parse("http://localhost:3000").unwrap(),
            cors_origin: HeaderValue::from_static("http://localhost:3000"),
            magic_link_ttl_minutes: 15,
            bind_addr: "127.0.0.1:3001".parse::<SocketAddr>().unwrap(),
            redis_url: String::new(),
            rate_limit_window_secs: 60,
            rate_limit_per_ip: 60,
            rate_limit_per_email: 30,
            database_url: String::new(),
            trust_proxy: false,
            ingress_domain: "ingress.test".to_string(),
            dns_server: None,
            main_domain: self.main_domain,
            fallback_resend_api_key: "fallback_key".to_string(),
            fallback_email_domain: "fallback.test".to_string(),
            fallback_google_client_id: "fallback_client_id".to_string(),
            fallback_google_client_secret: "fallback_client_secret".to_string(),
        });

        let rate_limiter: Arc<dyn RateLimiterTrait> = Arc::new(InMemoryRateLimiter::permissive());

        AppState {
            config,
            domain_use_cases,
            domain_auth_use_cases,
            api_key_use_cases,
            domain_roles_use_cases,
            billing_use_cases,
            rate_limiter,
        }
    }
}

impl Default for TestAppStateBuilder {
    fn default() -> Self {
        Self::new()
    }
}
