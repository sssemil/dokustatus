use std::sync::Arc;

use axum::extract::FromRef;

use crate::{
    infra::config::AppConfig,
    use_cases::user::{AuthUseCases, UserRepo},
};

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<AppConfig>,
    pub auth_use_cases: Arc<AuthUseCases>,
    pub repo: Arc<dyn UserRepo>,
}

impl FromRef<AppState> for Arc<AuthUseCases> {
    fn from_ref(app_state: &AppState) -> Self {
        app_state.auth_use_cases.clone()
    }
}
