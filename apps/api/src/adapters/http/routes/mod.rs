pub mod auth;
pub mod dictionary;
pub mod pass;
pub mod user;

use axum::Router;

use crate::adapters::http::app_state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .nest("/auth", auth::router())
        .nest("/dictionaries", dictionary::router())
        .nest("/pass", pass::router())
        .nest("/user", user::router())
}
