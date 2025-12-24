pub mod auth;
pub mod domain;
pub mod user;

use axum::Router;

use crate::adapters::http::app_state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .nest("/auth", auth::router())
        .nest("/user", user::router())
        .nest("/domains", domain::router())
}
