use std::sync::Arc;

use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use serde::Deserialize;
use time;

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
        .route("/verify", get(verify))
        .route("/logout", post(logout))
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
        // Get user email
        let email = app_state
            .repo
            .get_email_by_id(user_id)
            .await?
            .unwrap_or_default();

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
        let access_cookie = Cookie::build(("access_token", access))
            .http_only(true)
            .same_site(SameSite::Lax)
            .path("/")
            .build();
        let refresh_cookie = Cookie::build(("refresh_token", refresh))
            .http_only(true)
            .same_site(SameSite::Lax)
            .path("/")
            .build();
        let email_cookie = Cookie::build(("user_email", email))
            .http_only(false)
            .same_site(SameSite::Lax)
            .path("/")
            .build();
        headers.append("set-cookie", access_cookie.to_string().parse().unwrap());
        headers.append("set-cookie", refresh_cookie.to_string().parse().unwrap());
        headers.append("set-cookie", email_cookie.to_string().parse().unwrap());
        return Ok((StatusCode::OK, headers));
    }
    let headers = HeaderMap::new();
    Ok((StatusCode::UNAUTHORIZED, headers))
}

async fn verify(
    cookies: CookieJar,
    State(app_state): State<AppState>,
) -> AppResult<impl IntoResponse> {
    if let Some(access_token) = cookies.get("access_token")
        && jwt::verify(access_token.value(), &app_state.config.jwt_secret).is_ok()
    {
        return Ok(StatusCode::OK);
    }
    Ok(StatusCode::UNAUTHORIZED)
}

async fn logout() -> impl IntoResponse {
    let mut headers = HeaderMap::new();
    let access = Cookie::build(("access_token", ""))
        .http_only(true)
        .same_site(SameSite::Lax)
        .path("/")
        .max_age(time::Duration::seconds(0))
        .build();
    let refresh = Cookie::build(("refresh_token", ""))
        .http_only(true)
        .same_site(SameSite::Lax)
        .path("/")
        .max_age(time::Duration::seconds(0))
        .build();
    let email = Cookie::build(("user_email", ""))
        .http_only(false)
        .same_site(SameSite::Lax)
        .path("/")
        .max_age(time::Duration::seconds(0))
        .build();
    headers.append("set-cookie", access.to_string().parse().unwrap());
    headers.append("set-cookie", refresh.to_string().parse().unwrap());
    headers.append("set-cookie", email.to_string().parse().unwrap());
    (StatusCode::OK, headers)
}
