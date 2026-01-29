use std::sync::Arc;

use crate::{
    application::use_cases::api_key::ApiKeyUseCases,
    application::use_cases::domain_auth::DomainAuthUseCases,
    application::use_cases::domain_billing::DomainBillingUseCases,
    application::use_cases::domain_roles::DomainRolesUseCases,
    application::use_cases::webhook::WebhookUseCases, infra::RateLimiterTrait,
    infra::config::AppConfig, use_cases::domain::DomainUseCases,
};

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<AppConfig>,
    pub domain_use_cases: Arc<DomainUseCases>,
    pub domain_auth_use_cases: Arc<DomainAuthUseCases>,
    pub api_key_use_cases: Arc<ApiKeyUseCases>,
    pub domain_roles_use_cases: Arc<DomainRolesUseCases>,
    pub billing_use_cases: Arc<DomainBillingUseCases>,
    pub webhook_use_cases: Arc<WebhookUseCases>,
    pub rate_limiter: Arc<dyn RateLimiterTrait>,
}
