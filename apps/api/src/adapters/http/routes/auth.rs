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
use uuid::Uuid;

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
    jar: CookieJar,
    Json(payload): Json<RequestPayload>,
) -> AppResult<impl IntoResponse> {
    let (jar, session_id) = ensure_login_session(jar, app_state.config.magic_link_ttl_minutes);
    let auth: Arc<AuthUseCases> = app_state.auth_use_cases.clone();
    auth.request_magic_link(
        &payload.email,
        &session_id,
        app_state.config.magic_link_ttl_minutes,
    )
    .await?;
    Ok((StatusCode::ACCEPTED, jar))
}

async fn consume(
    State(app_state): State<AppState>,
    jar: CookieJar,
    Json(payload): Json<ConsumePayload>,
) -> AppResult<impl IntoResponse> {
    let Some(session_cookie) = jar.get("login_session") else {
        return Ok((StatusCode::UNAUTHORIZED, HeaderMap::new()));
    };
    let session_id = session_cookie.value().to_owned();

    let auth = app_state.auth_use_cases.clone();
    if let Some(user_id) = auth.consume_magic_link(&payload.token, &session_id).await? {
        // Get user email
        let Some(profile) = app_state.user_repo.get_profile_by_id(user_id).await? else {
            return Ok((StatusCode::UNAUTHORIZED, HeaderMap::new()));
        };
        let email = profile.email;

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
            .max_age(app_state.config.access_token_ttl)
            .build();
        let refresh_cookie = Cookie::build(("refresh_token", refresh))
            .http_only(true)
            .same_site(SameSite::Lax)
            .path("/")
            .max_age(app_state.config.refresh_token_ttl)
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

fn ensure_login_session(jar: CookieJar, ttl_minutes: i64) -> (CookieJar, String) {
    let session_id = jar
        .get("login_session")
        .map(|c| c.value().to_owned())
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let cookie = Cookie::build(("login_session", session_id.clone()))
        .http_only(true)
        .same_site(SameSite::Lax)
        .path("/")
        .max_age(time::Duration::minutes(ttl_minutes))
        .build();
    (jar.add(cookie), session_id)
}
