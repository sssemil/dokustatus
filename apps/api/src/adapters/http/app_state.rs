use std::sync::Arc;

use crate::{
    application::use_cases::api_key::ApiKeyUseCases,
    application::use_cases::domain_auth::DomainAuthUseCases,
    application::use_cases::domain_roles::DomainRolesUseCases,
    infra::config::AppConfig,
    infra::rate_limit::RateLimiter,
    use_cases::domain::DomainUseCases,
};

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<AppConfig>,
    pub domain_use_cases: Arc<DomainUseCases>,
    pub domain_auth_use_cases: Arc<DomainAuthUseCases>,
    pub api_key_use_cases: Arc<ApiKeyUseCases>,
    pub domain_roles_use_cases: Arc<DomainRolesUseCases>,
    pub rate_limiter: Arc<RateLimiter>,
}
