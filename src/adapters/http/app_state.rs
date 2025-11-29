use std::sync::Arc;

use axum::extract::FromRef;

use crate::{
    infra::config::AppConfig,
    infra::rate_limit::RateLimiter,
    use_cases::{
        pass_status::PassStatusUseCases,
        user::{AuthUseCases, UserRepo},
    },
};

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<AppConfig>,
    pub auth_use_cases: Arc<AuthUseCases>,
    pub pass_status_use_cases: Arc<PassStatusUseCases>,
    pub user_repo: Arc<dyn UserRepo>,
    pub rate_limiter: Arc<RateLimiter>,
}

impl FromRef<AppState> for Arc<AuthUseCases> {
    fn from_ref(app_state: &AppState) -> Self {
        app_state.auth_use_cases.clone()
    }
}

impl FromRef<AppState> for Arc<PassStatusUseCases> {
    fn from_ref(app_state: &AppState) -> Self {
        app_state.pass_status_use_cases.clone()
    }
}
