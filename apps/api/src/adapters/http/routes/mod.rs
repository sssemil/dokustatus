pub mod domain;
pub mod public_domain_auth;
pub mod user;

use axum::Router;

use crate::adapters::http::app_state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .nest("/user", user::router())
        .nest("/domains", domain::router())
        .nest("/public/domain", public_domain_auth::router())
}
