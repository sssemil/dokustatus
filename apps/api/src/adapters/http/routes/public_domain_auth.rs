use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    response::IntoResponse,
    routing::{delete, get, post},
};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use serde::{Deserialize, Serialize};
use time;
use uuid::Uuid;

use crate::{
    adapters::http::app_state::AppState,
    app_error::{AppError, AppResult},
    application::{
        jwt, use_cases::domain::extract_root_from_reauth_hostname, validators::is_valid_email,
    },
};

/// Appends a cookie to the headers, handling parse errors gracefully
fn append_cookie(headers: &mut HeaderMap, cookie: Cookie<'_>) -> Result<(), AppError> {
    let value = HeaderValue::from_str(&cookie.to_string())
        .map_err(|_| AppError::Internal("Failed to build cookie header".into()))?;
    headers.append("set-cookie", value);
    Ok(())
}

#[derive(Serialize)]
struct PublicConfigResponse {
    domain: String,
    auth_methods: AuthMethodsResponse,
    redirect_url: Option<String>,
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
    waitlist_position: Option<i64>,
}

#[derive(Serialize)]
struct SessionResponse {
    valid: bool,
    end_user_id: Option<String>,
    email: Option<String>,
    roles: Option<Vec<String>>,
    waitlist_position: Option<i64>,
    error: Option<String>,
    error_code: Option<String>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/{domain}/config", get(get_config))
        .route(
            "/{domain}/auth/request-magic-link",
            post(request_magic_link),
        )
        .route("/{domain}/auth/verify-magic-link", post(verify_magic_link))
        .route("/{domain}/auth/session", get(check_session))
        .route("/{domain}/auth/refresh", post(refresh_token))
        .route("/{domain}/auth/logout", post(logout))
        .route("/{domain}/auth/account", delete(delete_account))
}

/// GET /api/public/domain/{domain}/config
/// Returns public config for a domain (enabled auth methods)
/// The {domain} param is the hostname (e.g., "reauth.example.com")
async fn get_config(
    State(app_state): State<AppState>,
    Path(hostname): Path<String>,
) -> AppResult<impl IntoResponse> {
    // Extract root domain from reauth.* hostname
    let root_domain = extract_root_from_reauth_hostname(&hostname);

    let config = app_state
        .domain_auth_use_cases
        .get_public_config(&root_domain)
        .await?;

    let response = PublicConfigResponse {
        domain: config.domain,
        auth_methods: AuthMethodsResponse {
            magic_link: config.magic_link_enabled,
            google_oauth: config.google_oauth_enabled,
        },
        redirect_url: config.redirect_url,
    };

    Ok(Json(response))
}

/// POST /api/public/domain/{domain}/auth/request-magic-link
/// Sends a magic link email to the end-user
/// The {domain} param is the hostname (e.g., "reauth.example.com")
async fn request_magic_link(
    State(app_state): State<AppState>,
    Path(hostname): Path<String>,
    jar: CookieJar,
    Json(payload): Json<RequestMagicLinkPayload>,
) -> AppResult<impl IntoResponse> {
    // Validate email format
    let email = payload.email.trim();
    if !is_valid_email(email) {
        return Err(AppError::InvalidInput("Invalid email format".into()));
    }

    // Extract root domain from reauth.* hostname
    let root_domain = extract_root_from_reauth_hostname(&hostname);

    let (jar, session_id) =
        ensure_login_session(jar, &root_domain, app_state.config.magic_link_ttl_minutes);

    app_state
        .domain_auth_use_cases
        .request_magic_link(
            &root_domain,
            email,
            &session_id,
            app_state.config.magic_link_ttl_minutes,
        )
        .await?;

    Ok((StatusCode::ACCEPTED, jar))
}

/// POST /api/public/domain/{domain}/auth/verify-magic-link
/// Verifies the magic link token and creates a session with access + refresh tokens
/// The {domain} param is the hostname (e.g., "reauth.example.com")
async fn verify_magic_link(
    State(app_state): State<AppState>,
    Path(hostname): Path<String>,
    jar: CookieJar,
    Json(payload): Json<VerifyMagicLinkPayload>,
) -> AppResult<impl IntoResponse> {
    // Extract root domain from reauth.* hostname
    let root_domain = extract_root_from_reauth_hostname(&hostname);

    // Get session ID from cookie - if missing, user is on a different browser/device
    let session_id = match jar.get("login_session") {
        Some(cookie) => cookie.value().to_owned(),
        None => {
            // No session cookie = different browser/device than where link was requested
            return Err(AppError::SessionMismatch);
        }
    };

    let end_user = app_state
        .domain_auth_use_cases
        .consume_magic_link(&root_domain, &payload.token, &session_id)
        .await?;

    match end_user {
        Some(user) => {
            // Get config for redirect URL and TTL settings
            let config = app_state
                .domain_auth_use_cases
                .get_auth_config_for_domain(&root_domain)
                .await
                .ok();

            let access_ttl_secs = config
                .as_ref()
                .map(|c| c.access_token_ttl_secs)
                .unwrap_or(86400);
            let refresh_ttl_days = config
                .as_ref()
                .map(|c| c.refresh_token_ttl_days)
                .unwrap_or(30);

            // Check if user is on waitlist (whitelist enabled but user not whitelisted)
            let whitelist_enabled = config
                .as_ref()
                .map(|c| c.whitelist_enabled)
                .unwrap_or(false);
            let on_waitlist = whitelist_enabled && !user.is_whitelisted;

            // Get waitlist position if on waitlist
            let waitlist_position = if on_waitlist {
                app_state
                    .domain_auth_use_cases
                    .get_waitlist_position(user.domain_id, user.id)
                    .await
                    .ok()
            } else {
                None
            };

            // Only provide redirect_url if user is whitelisted (or whitelist not enabled)
            // Default to https://{domain} if not configured
            let redirect_url = if on_waitlist {
                None
            } else {
                Some(
                    config
                        .as_ref()
                        .and_then(|c| c.redirect_url.clone())
                        .unwrap_or_else(|| format!("https://{}", root_domain)),
                )
            };

            // Issue access token (short-lived)
            let access_token = jwt::issue_domain_end_user(
                user.id,
                user.domain_id,
                &root_domain,
                user.roles.clone(),
                &app_state.config.jwt_secret,
                time::Duration::seconds(access_ttl_secs as i64),
            )?;

            // Issue refresh token (long-lived)
            let refresh_token = jwt::issue_domain_end_user(
                user.id,
                user.domain_id,
                &root_domain,
                user.roles.clone(),
                &app_state.config.jwt_secret,
                time::Duration::days(refresh_ttl_days as i64),
            )?;

            // Set cookies on root domain
            let mut headers = HeaderMap::new();

            let access_cookie = Cookie::build(("end_user_access_token", access_token))
                .http_only(true)
                .secure(true)
                .same_site(SameSite::Lax)
                .domain(format!(".{}", root_domain))
                .path("/")
                .max_age(time::Duration::seconds(access_ttl_secs as i64))
                .build();

            let refresh_cookie = Cookie::build(("end_user_refresh_token", refresh_token))
                .http_only(true)
                .secure(true)
                .same_site(SameSite::Lax)
                .domain(format!(".{}", root_domain))
                .path("/")
                .max_age(time::Duration::days(refresh_ttl_days as i64))
                .build();

            let email_cookie = Cookie::build(("end_user_email", user.email.clone()))
                .http_only(false)
                .secure(true)
                .same_site(SameSite::Lax)
                .domain(format!(".{}", root_domain))
                .path("/")
                .max_age(time::Duration::days(refresh_ttl_days as i64))
                .build();

            append_cookie(&mut headers, access_cookie)?;
            append_cookie(&mut headers, refresh_cookie)?;
            append_cookie(&mut headers, email_cookie)?;

            Ok((
                StatusCode::OK,
                headers,
                Json(VerifyMagicLinkResponse {
                    success: true,
                    redirect_url,
                    end_user_id: Some(user.id.to_string()),
                    email: Some(user.email),
                    waitlist_position,
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
                waitlist_position: None,
            }),
        )),
    }
}

/// GET /api/public/domain/{domain}/auth/session
/// Checks if the end-user session is valid (checks access token first, then refresh)
/// Also checks real-time frozen/whitelist status from database
/// The {domain} param is the hostname (e.g., "reauth.example.com")
async fn check_session(
    State(app_state): State<AppState>,
    Path(hostname): Path<String>,
    cookies: CookieJar,
) -> AppResult<impl IntoResponse> {
    // Extract root domain from reauth.* hostname
    let root_domain = extract_root_from_reauth_hostname(&hostname);

    // Check access token first
    if let Some(access_token) = cookies.get("end_user_access_token") {
        if let Ok(claims) =
            jwt::verify_domain_end_user(access_token.value(), &app_state.config.jwt_secret)
        {
            if claims.domain == root_domain {
                // Parse user ID and check real-time status from database
                if let Ok(user_id) = uuid::Uuid::parse_str(&claims.sub) {
                    // Check user's current status
                    if let Ok(Some(user)) = app_state
                        .domain_auth_use_cases
                        .get_end_user_by_id(user_id)
                        .await
                    {
                        // Check if frozen
                        if user.is_frozen {
                            return Ok(Json(SessionResponse {
                                valid: false,
                                end_user_id: Some(claims.sub.clone()),
                                email: Some(user.email),
                                roles: None,
                                waitlist_position: None,
                                error: Some("Your account has been suspended".to_string()),
                                error_code: Some("ACCOUNT_SUSPENDED".to_string()),
                            }));
                        }

                        // Check whitelist status
                        let config = app_state
                            .domain_auth_use_cases
                            .get_auth_config_for_domain(&root_domain)
                            .await
                            .ok();

                        let whitelist_enabled = config
                            .as_ref()
                            .map(|c| c.whitelist_enabled)
                            .unwrap_or(false);

                        if whitelist_enabled && !user.is_whitelisted {
                            // User is on waitlist
                            let waitlist_position = app_state
                                .domain_auth_use_cases
                                .get_waitlist_position(user.domain_id, user.id)
                                .await
                                .ok();

                            return Ok(Json(SessionResponse {
                                valid: true,
                                end_user_id: Some(claims.sub.clone()),
                                email: Some(user.email),
                                roles: Some(claims.roles),
                                waitlist_position,
                                error: None,
                                error_code: None,
                            }));
                        }

                        // User is fully authorized
                        return Ok(Json(SessionResponse {
                            valid: true,
                            end_user_id: Some(claims.sub.clone()),
                            email: Some(user.email),
                            roles: Some(claims.roles),
                            waitlist_position: None,
                            error: None,
                            error_code: None,
                        }));
                    }
                }

                // User lookup failed - don't trust the token, require re-authentication
                return Ok(Json(SessionResponse {
                    valid: false,
                    end_user_id: None,
                    email: None,
                    roles: None,
                    waitlist_position: None,
                    error: Some("Session verification failed".to_string()),
                    error_code: Some("SESSION_VERIFICATION_FAILED".to_string()),
                }));
            }
        }
    }

    // Fallback: check refresh token (client should call /refresh if access expired)
    if let Some(refresh_token) = cookies.get("end_user_refresh_token") {
        if let Ok(claims) =
            jwt::verify_domain_end_user(refresh_token.value(), &app_state.config.jwt_secret)
        {
            if claims.domain == root_domain {
                // Refresh token is valid but access token expired - return 401 to prompt refresh
                return Ok(Json(SessionResponse {
                    valid: false,
                    end_user_id: None,
                    email: None,
                    roles: None,
                    waitlist_position: None,
                    error: None,
                    error_code: None,
                }));
            }
        }
    }

    Ok(Json(SessionResponse {
        valid: false,
        end_user_id: None,
        email: None,
        roles: None,
        waitlist_position: None,
        error: None,
        error_code: None,
    }))
}

/// POST /api/public/domain/{domain}/auth/refresh
/// Refreshes the access token using the refresh token
/// Checks real-time frozen status before issuing new token
/// The {domain} param is the hostname (e.g., "reauth.example.com")
async fn refresh_token(
    State(app_state): State<AppState>,
    Path(hostname): Path<String>,
    cookies: CookieJar,
) -> AppResult<impl IntoResponse> {
    // Extract root domain from reauth.* hostname
    let root_domain = extract_root_from_reauth_hostname(&hostname);

    let Some(refresh_cookie) = cookies.get("end_user_refresh_token") else {
        return Ok((StatusCode::UNAUTHORIZED, HeaderMap::new()));
    };

    let claims = jwt::verify_domain_end_user(refresh_cookie.value(), &app_state.config.jwt_secret)
        .map_err(|_| crate::app_error::AppError::InvalidCredentials)?;

    if claims.domain != root_domain {
        return Ok((StatusCode::UNAUTHORIZED, HeaderMap::new()));
    }

    // Parse end_user_id from claims
    let end_user_id =
        Uuid::parse_str(&claims.sub).map_err(|_| crate::app_error::AppError::InvalidCredentials)?;
    let domain_id = Uuid::parse_str(&claims.domain_id)
        .map_err(|_| crate::app_error::AppError::InvalidCredentials)?;

    // Check user's current status from database before issuing new token
    if let Ok(Some(user)) = app_state
        .domain_auth_use_cases
        .get_end_user_by_id(end_user_id)
        .await
    {
        if user.is_frozen {
            return Err(crate::app_error::AppError::AccountSuspended);
        }
    }

    // Get TTL config
    let config = app_state
        .domain_auth_use_cases
        .get_auth_config_for_domain(&root_domain)
        .await
        .ok();

    let access_ttl_secs = config
        .as_ref()
        .map(|c| c.access_token_ttl_secs)
        .unwrap_or(86400);

    // Issue new access token
    let access_token = jwt::issue_domain_end_user(
        end_user_id,
        domain_id,
        &root_domain,
        claims.roles,
        &app_state.config.jwt_secret,
        time::Duration::seconds(access_ttl_secs as i64),
    )?;

    let mut headers = HeaderMap::new();

    let access_cookie = Cookie::build(("end_user_access_token", access_token))
        .http_only(true)
        .secure(true)
        .same_site(SameSite::Lax)
        .domain(format!(".{}", root_domain))
        .path("/")
        .max_age(time::Duration::seconds(access_ttl_secs as i64))
        .build();

    append_cookie(&mut headers, access_cookie)?;

    Ok((StatusCode::OK, headers))
}

/// POST /api/public/domain/{domain}/auth/logout
/// Clears the end-user session
/// The {domain} param is the hostname (e.g., "reauth.example.com")
async fn logout(Path(hostname): Path<String>) -> AppResult<impl IntoResponse> {
    // Extract root domain from reauth.* hostname
    let root_domain = extract_root_from_reauth_hostname(&hostname);
    let mut headers = HeaderMap::new();

    let access_cookie = Cookie::build(("end_user_access_token", ""))
        .http_only(true)
        .secure(true)
        .same_site(SameSite::Lax)
        .domain(format!(".{}", root_domain))
        .path("/")
        .max_age(time::Duration::seconds(0))
        .build();

    let refresh_cookie = Cookie::build(("end_user_refresh_token", ""))
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

    append_cookie(&mut headers, access_cookie)?;
    append_cookie(&mut headers, refresh_cookie)?;
    append_cookie(&mut headers, email_cookie)?;

    Ok((StatusCode::OK, headers))
}

/// DELETE /api/public/domain/{domain}/auth/account
/// Deletes the end-user's account
/// The {domain} param is the hostname (e.g., "reauth.example.com")
async fn delete_account(
    State(app_state): State<AppState>,
    Path(hostname): Path<String>,
    cookies: CookieJar,
) -> AppResult<impl IntoResponse> {
    // Extract root domain from reauth.* hostname
    let root_domain = extract_root_from_reauth_hostname(&hostname);

    // Get end_user_id from access or refresh token
    let end_user_id = if let Some(access_token) = cookies.get("end_user_access_token") {
        if let Ok(claims) =
            jwt::verify_domain_end_user(access_token.value(), &app_state.config.jwt_secret)
        {
            if claims.domain == root_domain {
                Some(Uuid::parse_str(&claims.sub).ok())
            } else {
                None
            }
        } else {
            None
        }
    } else if let Some(refresh_token) = cookies.get("end_user_refresh_token") {
        if let Ok(claims) =
            jwt::verify_domain_end_user(refresh_token.value(), &app_state.config.jwt_secret)
        {
            if claims.domain == root_domain {
                Some(Uuid::parse_str(&claims.sub).ok())
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    let Some(Some(end_user_id)) = end_user_id else {
        return Ok((StatusCode::UNAUTHORIZED, HeaderMap::new()));
    };

    // Delete the account
    app_state
        .domain_auth_use_cases
        .delete_own_account(end_user_id)
        .await?;

    // Clear cookies
    let mut headers = HeaderMap::new();

    let access_cookie = Cookie::build(("end_user_access_token", ""))
        .http_only(true)
        .secure(true)
        .same_site(SameSite::Lax)
        .domain(format!(".{}", root_domain))
        .path("/")
        .max_age(time::Duration::seconds(0))
        .build();

    let refresh_cookie = Cookie::build(("end_user_refresh_token", ""))
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

    append_cookie(&mut headers, access_cookie)?;
    append_cookie(&mut headers, refresh_cookie)?;
    append_cookie(&mut headers, email_cookie)?;

    Ok((StatusCode::OK, headers))
}

/// Ensures a login session exists (domain-scoped)
/// The domain parameter should be the root domain (e.g., "example.com")
fn ensure_login_session(
    jar: CookieJar,
    root_domain: &str,
    ttl_minutes: i64,
) -> (CookieJar, String) {
    let session_id = jar
        .get("login_session")
        .map(|c| c.value().to_owned())
        .unwrap_or_else(|| Uuid::new_v4().to_string());

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
