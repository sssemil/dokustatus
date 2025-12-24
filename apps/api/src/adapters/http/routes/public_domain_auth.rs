use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use serde::{Deserialize, Serialize};
use time;
use uuid::Uuid;

use crate::{
    adapters::http::app_state::AppState,
    app_error::AppResult,
    application::{jwt, use_cases::domain_auth::get_root_domain},
};

#[derive(Serialize)]
struct PublicConfigResponse {
    domain: String,
    auth_methods: AuthMethodsResponse,
}

#[derive(Serialize)]
struct AuthMethodsResponse {
    magic_link: bool,
    google_oauth: bool,
}

#[derive(Deserialize)]
struct RequestMagicLinkPayload {
    email: String,
}

#[derive(Deserialize)]
struct VerifyMagicLinkPayload {
    token: String,
}

#[derive(Serialize)]
struct VerifyMagicLinkResponse {
    success: bool,
    redirect_url: Option<String>,
    end_user_id: Option<String>,
    email: Option<String>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/{domain}/config", get(get_config))
        .route("/{domain}/auth/request-magic-link", post(request_magic_link))
        .route("/{domain}/auth/verify-magic-link", post(verify_magic_link))
        .route("/{domain}/auth/session", get(check_session))
        .route("/{domain}/auth/logout", post(logout))
}

/// GET /api/public/domain/{domain}/config
/// Returns public config for a domain (enabled auth methods)
async fn get_config(
    State(app_state): State<AppState>,
    Path(domain): Path<String>,
) -> AppResult<impl IntoResponse> {
    let config = app_state
        .domain_auth_use_cases
        .get_public_config(&domain)
        .await?;

    let response = PublicConfigResponse {
        domain: config.domain,
        auth_methods: AuthMethodsResponse {
            magic_link: config.magic_link_enabled,
            google_oauth: config.google_oauth_enabled,
        },
    };

    Ok(Json(response))
}

/// POST /api/public/domain/{domain}/auth/request-magic-link
/// Sends a magic link email to the end-user
async fn request_magic_link(
    State(app_state): State<AppState>,
    Path(domain): Path<String>,
    jar: CookieJar,
    Json(payload): Json<RequestMagicLinkPayload>,
) -> AppResult<impl IntoResponse> {
    let (jar, session_id) = ensure_login_session(jar, &domain, app_state.config.magic_link_ttl_minutes);

    app_state
        .domain_auth_use_cases
        .request_magic_link(
            &domain,
            &payload.email,
            &session_id,
            app_state.config.magic_link_ttl_minutes,
        )
        .await?;

    Ok((StatusCode::ACCEPTED, jar))
}

/// POST /api/public/domain/{domain}/auth/verify-magic-link
/// Verifies the magic link token and creates a session
async fn verify_magic_link(
    State(app_state): State<AppState>,
    Path(domain): Path<String>,
    jar: CookieJar,
    Json(payload): Json<VerifyMagicLinkPayload>,
) -> AppResult<impl IntoResponse> {
    let Some(session_cookie) = jar.get("login_session") else {
        return Ok((
            StatusCode::UNAUTHORIZED,
            HeaderMap::new(),
            Json(VerifyMagicLinkResponse {
                success: false,
                redirect_url: None,
                end_user_id: None,
                email: None,
            }),
        ));
    };
    let session_id = session_cookie.value().to_owned();

    let end_user = app_state
        .domain_auth_use_cases
        .consume_magic_link(&domain, &payload.token, &session_id)
        .await?;

    match end_user {
        Some(user) => {
            // Get redirect URL from config
            let config = app_state
                .domain_auth_use_cases
                .get_public_config(&domain)
                .await
                .ok();
            let redirect_url = config.and_then(|c| c.redirect_url);

            // Issue JWT for end-user session
            let jwt = jwt::issue_domain_end_user(
                user.id,
                user.domain_id,
                &domain,
                &app_state.config.jwt_secret,
                app_state.config.access_token_ttl,
            )?;

            // Set cookie on root domain
            let root_domain = get_root_domain(&domain);
            let mut headers = HeaderMap::new();

            let session_cookie = Cookie::build(("end_user_session", jwt))
                .http_only(true)
                .secure(true)
                .same_site(SameSite::Lax)
                .domain(format!(".{}", root_domain))
                .path("/")
                .max_age(time::Duration::days(30))
                .build();

            let email_cookie = Cookie::build(("end_user_email", user.email.clone()))
                .http_only(false)
                .secure(true)
                .same_site(SameSite::Lax)
                .domain(format!(".{}", root_domain))
                .path("/")
                .max_age(time::Duration::days(30))
                .build();

            headers.append("set-cookie", session_cookie.to_string().parse().unwrap());
            headers.append("set-cookie", email_cookie.to_string().parse().unwrap());

            Ok((
                StatusCode::OK,
                headers,
                Json(VerifyMagicLinkResponse {
                    success: true,
                    redirect_url,
                    end_user_id: Some(user.id.to_string()),
                    email: Some(user.email),
                }),
            ))
        }
        None => Ok((
            StatusCode::UNAUTHORIZED,
            HeaderMap::new(),
            Json(VerifyMagicLinkResponse {
                success: false,
                redirect_url: None,
                end_user_id: None,
                email: None,
            }),
        )),
    }
}

/// GET /api/public/domain/{domain}/auth/session
/// Checks if the end-user session is valid
async fn check_session(
    State(app_state): State<AppState>,
    Path(domain): Path<String>,
    cookies: CookieJar,
) -> AppResult<impl IntoResponse> {
    if let Some(session_token) = cookies.get("end_user_session") {
        if let Ok(claims) = jwt::verify_domain_end_user(session_token.value(), &app_state.config.jwt_secret) {
            // Verify the token is for this domain
            if claims.domain == domain || claims.domain == get_root_domain(&domain) {
                return Ok(StatusCode::OK);
            }
        }
    }
    Ok(StatusCode::UNAUTHORIZED)
}

/// POST /api/public/domain/{domain}/auth/logout
/// Clears the end-user session
async fn logout(Path(domain): Path<String>) -> impl IntoResponse {
    let root_domain = get_root_domain(&domain);
    let mut headers = HeaderMap::new();

    let session_cookie = Cookie::build(("end_user_session", ""))
        .http_only(true)
        .secure(true)
        .same_site(SameSite::Lax)
        .domain(format!(".{}", root_domain))
        .path("/")
        .max_age(time::Duration::seconds(0))
        .build();

    let email_cookie = Cookie::build(("end_user_email", ""))
        .http_only(false)
        .secure(true)
        .same_site(SameSite::Lax)
        .domain(format!(".{}", root_domain))
        .path("/")
        .max_age(time::Duration::seconds(0))
        .build();

    headers.append("set-cookie", session_cookie.to_string().parse().unwrap());
    headers.append("set-cookie", email_cookie.to_string().parse().unwrap());

    (StatusCode::OK, headers)
}

/// Ensures a login session exists (domain-scoped)
fn ensure_login_session(jar: CookieJar, domain: &str, ttl_minutes: i64) -> (CookieJar, String) {
    let session_id = jar
        .get("login_session")
        .map(|c| c.value().to_owned())
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    let root_domain = get_root_domain(domain);
    let cookie = Cookie::build(("login_session", session_id.clone()))
        .http_only(true)
        .secure(true)
        .same_site(SameSite::Lax)
        .domain(format!(".{}", root_domain))
        .path("/")
        .max_age(time::Duration::minutes(ttl_minutes))
        .build();

    (jar.add(cookie), session_id)
}
