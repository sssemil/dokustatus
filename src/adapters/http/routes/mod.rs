pub mod user;
pub mod auth;

use axum::Router;

use crate::adapters::http::app_state::AppState;

pub fn router() -> Router<AppState> { Router::new().nest("/auth", auth::router()) }
