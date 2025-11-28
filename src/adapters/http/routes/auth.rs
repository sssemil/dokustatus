use std::sync::Arc;

use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::post,
};
use axum_extra::extract::cookie::{Cookie, SameSite};
use serde::Deserialize;

use crate::{
    adapters::http::app_state::AppState, app_error::AppResult, application::jwt,
    use_cases::user::AuthUseCases,
};

#[derive(Deserialize)]
struct RequestPayload {
    email: String,
}

#[derive(Deserialize)]
struct ConsumePayload {
    token: String,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/request", post(request))
        .route("/consume", post(consume))
}

async fn request(
    State(app_state): State<AppState>,
    Json(payload): Json<RequestPayload>,
) -> AppResult<impl IntoResponse> {
    let auth: Arc<AuthUseCases> = app_state.auth_use_cases.clone();
    auth.request_magic_link(&payload.email, app_state.config.magic_link_ttl_minutes)
        .await?;
    Ok((StatusCode::ACCEPTED, ()))
}

async fn consume(
    State(app_state): State<AppState>,
    Json(payload): Json<ConsumePayload>,
) -> AppResult<impl IntoResponse> {
    let auth = app_state.auth_use_cases.clone();
    if let Some(user_id) = auth.consume_magic_link(&payload.token).await? {
        let access = jwt::issue(
            user_id,
            &app_state.config.jwt_secret,
            app_state.config.access_token_ttl,
        )?;
        let refresh = jwt::issue(
            user_id,
            &app_state.config.jwt_secret,
            app_state.config.refresh_token_ttl,
        )?;

        let mut headers = HeaderMap::new();
        let access = Cookie::build(("access_token", access))
            .http_only(true)
            .same_site(SameSite::Lax)
            .path("/")
            .build();
        let refresh = Cookie::build(("refresh_token", refresh))
            .http_only(true)
            .same_site(SameSite::Lax)
            .path("/")
            .build();
        headers.append("set-cookie", access.to_string().parse().unwrap());
        headers.append("set-cookie", refresh.to_string().parse().unwrap());
        return Ok((StatusCode::OK, headers));
    }
    let headers = HeaderMap::new();
    Ok((StatusCode::UNAUTHORIZED, headers))
}
