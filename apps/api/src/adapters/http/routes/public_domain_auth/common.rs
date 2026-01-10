//! Shared types, helpers, and cookie utilities for public domain auth routes.

// Core framework - re-exported for use by sibling modules
pub use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    response::IntoResponse,
    routing::{delete, get, post},
};
pub use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
pub use serde::{Deserialize, Serialize};
pub use time;
pub use tracing::error;
pub use uuid::Uuid;

// App-level imports
pub use crate::adapters::http::app_state::AppState;
pub use crate::app_error::{AppError, AppResult};
pub use crate::application::jwt;
pub use crate::application::use_cases::domain::extract_root_from_reauth_hostname;
pub use crate::application::use_cases::domain_billing::SubscriptionClaims;
pub use crate::domain::entities::stripe_mode::StripeMode;

/// Appends a cookie to the headers, handling parse errors gracefully
pub(crate) fn append_cookie(headers: &mut HeaderMap, cookie: Cookie<'_>) -> Result<(), AppError> {
    let value = HeaderValue::from_str(&cookie.to_string())
        .map_err(|_| AppError::Internal("Failed to build cookie header".into()))?;
    headers.append("set-cookie", value);
    Ok(())
}

/// Clears all auth cookies (access, refresh, email) for logout/delete
pub(crate) fn clear_auth_cookies(
    headers: &mut HeaderMap,
    root_domain: &str,
) -> Result<(), AppError> {
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

    append_cookie(headers, access_cookie)?;
    append_cookie(headers, refresh_cookie)?;
    append_cookie(headers, email_cookie)?;
    Ok(())
}

/// Result of completing a login (magic link or OAuth)
pub(crate) struct LoginCompletionResult {
    pub headers: HeaderMap,
    pub redirect_url: Option<String>,
    pub waitlist_position: Option<i64>,
}

use crate::application::use_cases::domain_auth::DomainEndUserProfile;

/// Unified login completion logic shared by verify_magic_link and google_complete.
/// Handles waitlist check, token issuance, and cookie setting.
pub(crate) async fn complete_login(
    app_state: &AppState,
    user: &DomainEndUserProfile,
    root_domain: &str,
) -> AppResult<LoginCompletionResult> {
    // Get config for redirect URL and TTL settings
    let config = app_state
        .domain_auth_use_cases
        .get_auth_config_for_domain(root_domain)
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

    // Fetch subscription claims for JWT
    let subscription_claims = app_state
        .billing_use_cases
        .get_subscription_claims(user.domain_id, user.id)
        .await
        .unwrap_or_else(|_| SubscriptionClaims::none());

    // Issue access token (short-lived)
    let access_token = jwt::issue_domain_end_user(
        user.id,
        user.domain_id,
        root_domain,
        user.roles.clone(),
        subscription_claims.clone(),
        &app_state.config.jwt_secret,
        time::Duration::seconds(access_ttl_secs as i64),
    )?;

    // Issue refresh token (long-lived)
    let refresh_token = jwt::issue_domain_end_user(
        user.id,
        user.domain_id,
        root_domain,
        user.roles.clone(),
        subscription_claims,
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

    Ok(LoginCompletionResult {
        headers,
        redirect_url,
        waitlist_position,
    })
}

/// Ensures a login session exists (domain-scoped)
/// The domain parameter should be the root domain (e.g., "example.com")
pub(crate) fn ensure_login_session(
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

/// Helper to extract current user from cookies
pub(crate) fn get_current_user(
    app_state: &AppState,
    cookies: &CookieJar,
    root_domain: &str,
) -> AppResult<(Uuid, Uuid)> {
    let token = cookies
        .get("end_user_access_token")
        .or_else(|| cookies.get("end_user_refresh_token"))
        .ok_or(AppError::InvalidCredentials)?;

    let claims = jwt::verify_domain_end_user(token.value(), &app_state.config.jwt_secret)
        .map_err(|_| AppError::InvalidCredentials)?;

    if claims.domain != root_domain {
        return Err(AppError::InvalidCredentials);
    }

    let user_id = Uuid::parse_str(&claims.sub).map_err(|_| AppError::InvalidCredentials)?;
    let domain_id = Uuid::parse_str(&claims.domain_id).map_err(|_| AppError::InvalidCredentials)?;

    Ok((user_id, domain_id))
}

// Response types used by multiple modules

/// Public config response
#[derive(Serialize)]
pub(crate) struct PublicConfigResponse {
    pub domain: String,
    pub auth_methods: AuthMethodsResponse,
    pub redirect_url: Option<String>,
}

#[derive(Serialize)]
pub(crate) struct AuthMethodsResponse {
    pub magic_link: bool,
    pub google_oauth: bool,
}

/// Subscription info returned in session response (matches SDK types)
#[derive(Serialize)]
pub(crate) struct SessionSubscriptionInfo {
    pub status: String,
    pub plan_code: Option<String>,
    pub plan_name: Option<String>,
    pub current_period_end: Option<i64>,
    pub cancel_at_period_end: Option<bool>,
    pub trial_ends_at: Option<i64>,
}

#[derive(Serialize)]
pub(crate) struct SessionResponse {
    pub valid: bool,
    pub end_user_id: Option<String>,
    pub email: Option<String>,
    pub roles: Option<Vec<String>>,
    pub waitlist_position: Option<i64>,
    pub google_linked: Option<bool>,
    pub error: Option<String>,
    pub error_code: Option<String>,
    /// Subscription info (if billing is configured)
    pub subscription: Option<SessionSubscriptionInfo>,
}
