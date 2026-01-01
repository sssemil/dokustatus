use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    response::IntoResponse,
    routing::{delete, get, post},
};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use serde::{Deserialize, Serialize};
use time;
use tracing::error;
use uuid::Uuid;

use crate::{
    adapters::http::app_state::AppState,
    app_error::{AppError, AppResult},
    application::{
        jwt,
        use_cases::{
            domain::extract_root_from_reauth_hostname, domain_auth::DomainEndUserProfile,
            domain_billing::SubscriptionClaims,
        },
        validators::is_valid_email,
    },
    domain::entities::{
        payment_mode::PaymentMode, payment_provider::PaymentProvider,
        payment_scenario::PaymentScenario,
    },
    infra::http_client,
};

/// Appends a cookie to the headers, handling parse errors gracefully
fn append_cookie(headers: &mut HeaderMap, cookie: Cookie<'_>) -> Result<(), AppError> {
    let value = HeaderValue::from_str(&cookie.to_string())
        .map_err(|_| AppError::Internal("Failed to build cookie header".into()))?;
    headers.append("set-cookie", value);
    Ok(())
}

/// Result of completing a login (magic link or OAuth)
struct LoginCompletionResult {
    headers: HeaderMap,
    redirect_url: Option<String>,
    waitlist_position: Option<i64>,
}

/// Unified login completion logic shared by verify_magic_link and google_complete.
/// Handles waitlist check, token issuance, and cookie setting.
async fn complete_login(
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

/// Subscription info returned in session response (matches SDK types)
#[derive(Serialize)]
struct SessionSubscriptionInfo {
    status: String,
    plan_code: Option<String>,
    plan_name: Option<String>,
    current_period_end: Option<i64>,
    cancel_at_period_end: Option<bool>,
    trial_ends_at: Option<i64>,
}

#[derive(Serialize)]
struct SessionResponse {
    valid: bool,
    end_user_id: Option<String>,
    email: Option<String>,
    roles: Option<Vec<String>>,
    waitlist_position: Option<i64>,
    google_linked: Option<bool>,
    error: Option<String>,
    error_code: Option<String>,
    /// Subscription info (if billing is configured)
    subscription: Option<SessionSubscriptionInfo>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/{domain}/config", get(get_config))
        .route(
            "/{domain}/auth/request-magic-link",
            post(request_magic_link),
        )
        .route("/{domain}/auth/verify-magic-link", post(verify_magic_link))
        .route("/{domain}/auth/google/start", post(google_start))
        .route("/{domain}/auth/google/exchange", post(google_exchange))
        .route(
            "/{domain}/auth/google/confirm-link",
            post(google_confirm_link),
        )
        .route("/{domain}/auth/google/complete", post(google_complete))
        .route("/{domain}/auth/google/unlink", post(unlink_google))
        .route("/{domain}/auth/session", get(check_session))
        .route("/{domain}/auth/refresh", post(refresh_token))
        .route("/{domain}/auth/logout", post(logout))
        .route("/{domain}/auth/account", delete(delete_account))
        // Billing routes
        .route("/{domain}/billing/plans", get(get_public_plans))
        .route("/{domain}/billing/subscription", get(get_user_subscription))
        .route("/{domain}/billing/checkout", post(create_checkout))
        .route("/{domain}/billing/portal", post(create_portal))
        .route("/{domain}/billing/cancel", post(cancel_subscription))
        .route("/{domain}/billing/payments", get(get_user_payments))
        .route(
            "/{domain}/billing/plan-change/preview",
            get(preview_plan_change),
        )
        .route("/{domain}/billing/plan-change", post(change_plan))
        // Provider routes
        .route("/{domain}/billing/providers", get(get_available_providers))
        .route(
            "/{domain}/billing/checkout/dummy",
            post(create_dummy_checkout),
        )
        .route(
            "/{domain}/billing/dummy/confirm",
            post(confirm_dummy_checkout),
        )
        .route(
            "/{domain}/billing/dummy/scenarios",
            get(get_dummy_scenarios),
        )
        // Mode-specific webhook endpoints
        .route("/{domain}/billing/webhook/test", post(handle_webhook_test))
        .route("/{domain}/billing/webhook/live", post(handle_webhook_live))
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
            // Use unified login completion logic (handles waitlist, tokens, cookies)
            let result = complete_login(&app_state, &user, &root_domain).await?;

            Ok((
                StatusCode::OK,
                result.headers,
                Json(VerifyMagicLinkResponse {
                    success: true,
                    redirect_url: result.redirect_url,
                    end_user_id: Some(user.id.to_string()),
                    email: Some(user.email),
                    waitlist_position: result.waitlist_position,
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
                                email: Some(user.email.clone()),
                                roles: None,
                                waitlist_position: None,
                                google_linked: Some(user.google_id.is_some()),
                                error: Some("Your account has been suspended".to_string()),
                                error_code: Some("ACCOUNT_SUSPENDED".to_string()),
                                subscription: None, // Don't show subscription for suspended accounts
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
                                email: Some(user.email.clone()),
                                roles: Some(claims.roles.clone()),
                                waitlist_position,
                                google_linked: Some(user.google_id.is_some()),
                                error: None,
                                error_code: None,
                                subscription: Some(SessionSubscriptionInfo {
                                    status: claims.subscription.status.clone(),
                                    plan_code: claims.subscription.plan_code.clone(),
                                    plan_name: claims.subscription.plan_name.clone(),
                                    current_period_end: claims.subscription.current_period_end,
                                    cancel_at_period_end: claims.subscription.cancel_at_period_end,
                                    trial_ends_at: claims.subscription.trial_ends_at,
                                }),
                            }));
                        }

                        // User is fully authorized
                        return Ok(Json(SessionResponse {
                            valid: true,
                            end_user_id: Some(claims.sub.clone()),
                            email: Some(user.email),
                            roles: Some(claims.roles.clone()),
                            waitlist_position: None,
                            google_linked: Some(user.google_id.is_some()),
                            error: None,
                            error_code: None,
                            subscription: Some(SessionSubscriptionInfo {
                                status: claims.subscription.status.clone(),
                                plan_code: claims.subscription.plan_code.clone(),
                                plan_name: claims.subscription.plan_name.clone(),
                                current_period_end: claims.subscription.current_period_end,
                                cancel_at_period_end: claims.subscription.cancel_at_period_end,
                                trial_ends_at: claims.subscription.trial_ends_at,
                            }),
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
                    google_linked: None,
                    error: Some("Session verification failed".to_string()),
                    error_code: Some("SESSION_VERIFICATION_FAILED".to_string()),
                    subscription: None,
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
                    google_linked: None,
                    error: None,
                    error_code: None,
                    subscription: None,
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
        google_linked: None,
        error: None,
        error_code: None,
        subscription: None,
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

    // Fetch fresh subscription claims for the new token
    let subscription_claims = app_state
        .billing_use_cases
        .get_subscription_claims(domain_id, end_user_id)
        .await
        .unwrap_or_else(|_| SubscriptionClaims::none());

    // Issue new access token
    let access_token = jwt::issue_domain_end_user(
        end_user_id,
        domain_id,
        &root_domain,
        claims.roles,
        subscription_claims,
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

// ============================================================================
// Google OAuth Routes
// ============================================================================

#[derive(Serialize)]
struct GoogleStartResponse {
    state: String,
    auth_url: String,
}

#[derive(Deserialize)]
struct GoogleExchangePayload {
    code: String,
    state: String,
}

#[derive(Serialize)]
#[serde(tag = "status")]
enum GoogleExchangeResponse {
    /// User is authenticated - redirect to completion URL to set cookies
    #[serde(rename = "logged_in")]
    LoggedIn {
        /// URL to complete the login (sets cookies on correct domain)
        completion_url: String,
    },
    #[serde(rename = "needs_link_confirmation")]
    NeedsLinkConfirmation {
        /// Token to confirm linking (server-derived, single-use, 5 min TTL)
        link_token: String,
        /// Email for UI display only (already verified)
        email: String,
    },
}

#[derive(Deserialize)]
struct GoogleConfirmLinkPayload {
    /// Token from the NeedsLinkConfirmation response
    link_token: String,
}

#[derive(Serialize)]
struct GoogleConfirmLinkResponse {
    /// URL to complete the OAuth flow on the correct domain
    completion_url: String,
}

/// POST /api/public/domain/{domain}/auth/google/start
/// Creates OAuth state and returns the Google authorization URL
async fn google_start(
    State(app_state): State<AppState>,
    Path(hostname): Path<String>,
) -> AppResult<impl IntoResponse> {
    // Extract root domain from reauth.* hostname
    let root_domain = extract_root_from_reauth_hostname(&hostname);

    // Create OAuth state
    let (state, code_verifier) = app_state
        .domain_auth_use_cases
        .create_google_oauth_state(&root_domain)
        .await?;

    // Get OAuth config to build auth URL
    let domain = app_state
        .domain_auth_use_cases
        .get_domain_by_name(&root_domain)
        .await?
        .ok_or(AppError::NotFound)?;

    let (client_id, _, is_fallback) = app_state
        .domain_auth_use_cases
        .get_google_oauth_config(domain.id)
        .await?;

    // Build PKCE code challenge (S256)
    use base64::Engine;
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(code_verifier.as_bytes());
    let code_challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(hasher.finalize());

    // Build Google OAuth URL
    // - Fallback credentials: use reauth.{main_domain}/callback/google (shared OAuth app)
    // - Custom credentials: use reauth.{root_domain}/callback/google (user's own OAuth app)
    let redirect_uri = if is_fallback {
        let main_domain = &app_state.config.main_domain;
        format!("https://reauth.{}/callback/google", main_domain)
    } else {
        format!("https://reauth.{}/callback/google", root_domain)
    };

    // Use url crate for proper URL encoding
    let mut auth_url = url::Url::parse("https://accounts.google.com/o/oauth2/v2/auth").unwrap();
    auth_url
        .query_pairs_mut()
        .append_pair("client_id", &client_id)
        .append_pair("redirect_uri", &redirect_uri)
        .append_pair("response_type", "code")
        .append_pair("scope", "openid email")
        .append_pair("state", &state)
        .append_pair("code_challenge", &code_challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("prompt", "select_account");

    Ok(Json(GoogleStartResponse {
        state,
        auth_url: auth_url.to_string(),
    }))
}

/// POST /api/public/domain/{domain}/auth/google/exchange
/// Exchanges the authorization code for tokens and handles account matching
/// Note: The {domain} in the path is ignored - we use the domain from the OAuth state
/// to support the single callback URL pattern (all callbacks go to reauth.reauth.dev)
async fn google_exchange(
    State(app_state): State<AppState>,
    Path(_hostname): Path<String>,
    Json(payload): Json<GoogleExchangePayload>,
) -> AppResult<impl IntoResponse> {
    // Consume state - the domain comes FROM the state, not the URL
    // This is because Google OAuth uses a single callback URL (reauth.reauth.dev)
    // but the OAuth flow could have been initiated from any domain
    let state_data = app_state
        .domain_auth_use_cases
        .consume_google_oauth_state(&payload.state)
        .await?
        .ok_or_else(|| AppError::InvalidInput("Invalid or expired OAuth state".into()))?;

    // Use the domain from the state (this is the domain that initiated the OAuth flow)
    let root_domain = &state_data.domain;

    // Get domain
    let domain = app_state
        .domain_auth_use_cases
        .get_domain_by_name(root_domain)
        .await?
        .ok_or(AppError::NotFound)?;

    // Verify Google OAuth is still enabled
    if !app_state
        .domain_auth_use_cases
        .is_google_oauth_enabled(domain.id)
        .await?
    {
        return Err(AppError::InvalidInput(
            "Google OAuth is not enabled for this domain".into(),
        ));
    }

    // Get OAuth credentials
    let (client_id, client_secret, is_fallback) = app_state
        .domain_auth_use_cases
        .get_google_oauth_config(domain.id)
        .await?;

    // Exchange code with Google
    // Must use same redirect_uri as google_start (fallback vs custom)
    let redirect_uri = if is_fallback {
        let main_domain = &app_state.config.main_domain;
        format!("https://reauth.{}/callback/google", main_domain)
    } else {
        format!("https://reauth.{}/callback/google", root_domain)
    };

    let token_response = exchange_google_code(
        &payload.code,
        &client_id,
        &client_secret,
        &redirect_uri,
        &state_data.code_verifier,
    )
    .await?;

    // Parse and validate id_token (with signature verification)
    let (google_id, email, email_verified) =
        parse_google_id_token(&token_response.id_token, &client_id).await?;

    // Verify email is verified by Google
    if !email_verified {
        return Err(AppError::InvalidInput(
            "Google account email is not verified".into(),
        ));
    }

    // Find or create end user
    use crate::application::use_cases::domain_auth::GoogleLoginResult;
    let result = app_state
        .domain_auth_use_cases
        .find_or_create_end_user_by_google(domain.id, &google_id, &email)
        .await?;

    match result {
        GoogleLoginResult::LoggedIn(user) => {
            // Generate a completion token - this will be used to set cookies on the correct domain
            let completion_token = app_state
                .domain_auth_use_cases
                .create_google_completion_token(user.id, user.domain_id, root_domain)
                .await?;

            // Build completion URL - redirect to the domain's ingress to set cookies
            let completion_url = format!(
                "https://reauth.{}/google-complete?token={}",
                root_domain, completion_token
            );

            Ok((
                StatusCode::OK,
                HeaderMap::new(),
                Json(GoogleExchangeResponse::LoggedIn { completion_url }),
            ))
        }
        GoogleLoginResult::NeedsLinkConfirmation {
            existing_user_id,
            email,
            google_id,
        } => {
            // Generate a link confirmation token - stores server-derived data
            // (existing_user_id, google_id, domain_id, domain) for later verification
            let link_token = app_state
                .domain_auth_use_cases
                .create_google_link_confirmation_token(
                    existing_user_id,
                    &google_id,
                    domain.id,
                    root_domain,
                )
                .await?;

            // Return only the token and email (for UI display)
            Ok((
                StatusCode::OK,
                HeaderMap::new(),
                Json(GoogleExchangeResponse::NeedsLinkConfirmation { link_token, email }),
            ))
        }
    }
}

/// POST /api/public/domain/{domain}/auth/google/confirm-link
/// Confirms linking a Google account to an existing user.
/// Consumes the link_token from the exchange response (single-use, server-derived data).
/// Returns a completion URL to redirect to the correct domain for cookie setting.
async fn google_confirm_link(
    State(app_state): State<AppState>,
    Path(_hostname): Path<String>,
    Json(payload): Json<GoogleConfirmLinkPayload>,
) -> AppResult<impl IntoResponse> {
    // Consume the link confirmation token (single-use, contains server-derived data)
    let link_data = app_state
        .domain_auth_use_cases
        .consume_google_link_confirmation_token(&payload.link_token)
        .await?
        .ok_or_else(|| AppError::InvalidInput("Invalid or expired link token".into()))?;

    // Re-verify user at consume time (per Codex security review):
    // 1. Check user still exists
    let user = app_state
        .domain_auth_use_cases
        .get_end_user_by_id(link_data.existing_user_id)
        .await?
        .ok_or_else(|| AppError::NotFound)?;

    // 2. Check user belongs to correct domain (cross-tenant protection)
    if user.domain_id != link_data.domain_id {
        return Err(AppError::InvalidInput("User domain mismatch".into()));
    }

    // 3. Check user not frozen
    if user.is_frozen {
        return Err(AppError::AccountSuspended);
    }

    // 4. Confirm the link (this checks google_id not already linked to another user)
    let linked_user = app_state
        .domain_auth_use_cases
        .confirm_google_link(link_data.existing_user_id, &link_data.google_id)
        .await?;

    // Generate a completion token for cross-domain cookie setting
    let completion_token = app_state
        .domain_auth_use_cases
        .create_google_completion_token(linked_user.id, link_data.domain_id, &link_data.domain)
        .await?;

    // Build completion URL - redirect to the domain's ingress to set cookies
    let completion_url = format!(
        "https://reauth.{}/google-complete?token={}",
        link_data.domain, completion_token
    );

    Ok((
        StatusCode::OK,
        HeaderMap::new(),
        Json(GoogleConfirmLinkResponse { completion_url }),
    ))
}

#[derive(Deserialize)]
struct GoogleCompletePayload {
    token: String,
}

#[derive(Serialize)]
struct GoogleCompleteResponse {
    success: bool,
    /// Redirect URL (None if user is on waitlist)
    redirect_url: Option<String>,
    end_user_id: String,
    email: String,
    /// Waitlist position if user is on waitlist
    waitlist_position: Option<i64>,
}

/// POST /api/public/domain/{domain}/auth/google/complete
/// Completes the Google OAuth flow by consuming the completion token and setting cookies.
/// This endpoint is called from reauth.{domain} (the correct domain for cookies).
async fn google_complete(
    State(app_state): State<AppState>,
    Path(hostname): Path<String>,
    Json(payload): Json<GoogleCompletePayload>,
) -> AppResult<impl IntoResponse> {
    // Extract root domain from reauth.* hostname
    let root_domain = extract_root_from_reauth_hostname(&hostname);

    // Consume completion token
    let completion_data = app_state
        .domain_auth_use_cases
        .consume_google_completion_token(&payload.token)
        .await?
        .ok_or_else(|| AppError::InvalidInput("Invalid or expired completion token".into()))?;

    // Verify domain matches (defense in depth)
    if completion_data.domain != root_domain {
        return Err(AppError::InvalidInput("Token domain mismatch".into()));
    }

    // Get the user
    let user = app_state
        .domain_auth_use_cases
        .get_end_user_by_id(completion_data.user_id)
        .await?
        .ok_or(AppError::NotFound)?;

    // Use unified login completion logic (handles waitlist, tokens, cookies)
    let result = complete_login(&app_state, &user, &root_domain).await?;

    Ok((
        StatusCode::OK,
        result.headers,
        Json(GoogleCompleteResponse {
            success: true,
            redirect_url: result.redirect_url,
            end_user_id: user.id.to_string(),
            email: user.email,
            waitlist_position: result.waitlist_position,
        }),
    ))
}

// ============================================================================
// Billing Routes
// ============================================================================

#[derive(Serialize)]
struct PublicPlanResponse {
    id: Uuid,
    code: String,
    name: String,
    description: Option<String>,
    price_cents: i32,
    currency: String,
    interval: String,
    interval_count: i32,
    trial_days: i32,
    features: Vec<String>,
    display_order: i32,
}

/// GET /api/public/domain/{domain}/billing/plans
/// Returns public subscription plans for a domain
async fn get_public_plans(
    State(app_state): State<AppState>,
    Path(hostname): Path<String>,
) -> AppResult<impl IntoResponse> {
    let root_domain = extract_root_from_reauth_hostname(&hostname);

    let domain = app_state
        .domain_auth_use_cases
        .get_domain_by_name(&root_domain)
        .await?
        .ok_or(AppError::NotFound)?;

    let plans = app_state
        .billing_use_cases
        .get_public_plans(domain.id)
        .await?;

    let response: Vec<PublicPlanResponse> = plans
        .into_iter()
        .map(|p| PublicPlanResponse {
            id: p.id,
            code: p.code,
            name: p.name,
            description: p.description,
            price_cents: p.price_cents,
            currency: p.currency,
            interval: p.interval,
            interval_count: p.interval_count,
            trial_days: p.trial_days,
            features: p.features,
            display_order: p.display_order,
        })
        .collect();

    Ok(Json(response))
}

#[derive(Serialize)]
struct UserSubscriptionResponse {
    id: Option<Uuid>,
    plan_code: Option<String>,
    plan_name: Option<String>,
    status: String,
    current_period_end: Option<i64>,
    trial_end: Option<i64>,
    cancel_at_period_end: Option<bool>,
}

/// GET /api/public/domain/{domain}/billing/subscription
/// Returns the current user's subscription status
async fn get_user_subscription(
    State(app_state): State<AppState>,
    Path(hostname): Path<String>,
    cookies: CookieJar,
) -> AppResult<impl IntoResponse> {
    let root_domain = extract_root_from_reauth_hostname(&hostname);

    // Get user from token
    let (user_id, domain_id) = get_current_user(&app_state, &cookies, &root_domain)?;

    let sub = app_state
        .billing_use_cases
        .get_user_subscription_with_plan(domain_id, user_id)
        .await?;

    match sub {
        Some((subscription, plan)) => Ok(Json(UserSubscriptionResponse {
            id: Some(subscription.id),
            plan_code: Some(plan.code),
            plan_name: Some(plan.name),
            status: subscription.status.as_str().to_string(),
            current_period_end: subscription
                .current_period_end
                .map(|dt| dt.and_utc().timestamp()),
            trial_end: subscription.trial_end.map(|dt| dt.and_utc().timestamp()),
            cancel_at_period_end: Some(subscription.cancel_at_period_end),
        })),
        None => Ok(Json(UserSubscriptionResponse {
            id: None,
            plan_code: None,
            plan_name: None,
            status: "none".to_string(),
            current_period_end: None,
            trial_end: None,
            cancel_at_period_end: None,
        })),
    }
}

#[derive(Deserialize)]
struct CreateCheckoutPayload {
    plan_code: String,
    success_url: String,
    cancel_url: String,
}

#[derive(Serialize)]
struct CheckoutResponse {
    checkout_url: String,
}

/// POST /api/public/domain/{domain}/billing/checkout
/// Creates a Stripe checkout session for subscribing to a plan
async fn create_checkout(
    State(app_state): State<AppState>,
    Path(hostname): Path<String>,
    cookies: CookieJar,
    Json(payload): Json<CreateCheckoutPayload>,
) -> AppResult<impl IntoResponse> {
    use crate::infra::stripe_client::StripeClient;
    use std::collections::HashMap;

    let root_domain = extract_root_from_reauth_hostname(&hostname);

    // Get user from token
    let (user_id, domain_id) = get_current_user(&app_state, &cookies, &root_domain)?;

    // Get user details
    let user = app_state
        .domain_auth_use_cases
        .get_end_user_by_id(user_id)
        .await?
        .ok_or(AppError::NotFound)?;

    // Get the plan
    let mut plan = app_state
        .billing_use_cases
        .get_plan_by_code(domain_id, &payload.plan_code)
        .await?
        .ok_or(AppError::NotFound)?;

    // Verify plan is public (users can only subscribe to public plans)
    if !plan.is_public {
        return Err(AppError::NotFound);
    }

    // Get Stripe client
    let secret_key = app_state
        .billing_use_cases
        .get_stripe_secret_key(domain_id)
        .await?;
    let stripe = StripeClient::new(secret_key);

    // Lazily create Stripe product/price if not set
    if plan.stripe_product_id.is_none() || plan.stripe_price_id.is_none() {
        // Create Stripe product if needed
        let product_id = if let Some(ref id) = plan.stripe_product_id {
            id.clone()
        } else {
            let product = stripe
                .create_product(&plan.name, plan.description.as_deref())
                .await?;
            product.id
        };

        // Create Stripe price if needed
        let price_id = if let Some(ref id) = plan.stripe_price_id {
            id.clone()
        } else {
            // Convert interval to Stripe format (month/year)
            let stripe_interval = match plan.interval.as_str() {
                "monthly" => "month",
                "yearly" => "year",
                other => other, // Allow custom intervals
            };
            let price = stripe
                .create_price(
                    &product_id,
                    plan.price_cents as i64,
                    &plan.currency,
                    stripe_interval,
                    plan.interval_count,
                )
                .await?;
            price.id
        };

        // Update plan with Stripe IDs
        app_state
            .billing_use_cases
            .set_stripe_ids(plan.id, &product_id, &price_id)
            .await?;

        plan.stripe_product_id = Some(product_id);
        plan.stripe_price_id = Some(price_id.clone());
    }

    let price_id = plan.stripe_price_id.as_ref().unwrap();

    // Get or create customer
    let mut metadata = HashMap::new();
    metadata.insert("user_id".to_string(), user_id.to_string());
    metadata.insert("domain_id".to_string(), domain_id.to_string());
    let customer = stripe
        .get_or_create_customer(&user.email, Some(metadata))
        .await?;

    // Create checkout session
    let session = stripe
        .create_checkout_session(
            &customer.id,
            &price_id,
            &payload.success_url,
            &payload.cancel_url,
            Some(&user_id.to_string()),
            Some(plan.trial_days),
        )
        .await?;

    let checkout_url = session.url.ok_or(AppError::Internal(
        "Stripe checkout session missing URL".into(),
    ))?;

    Ok(Json(CheckoutResponse { checkout_url }))
}

#[derive(Deserialize)]
struct CreatePortalPayload {
    return_url: String,
}

#[derive(Serialize)]
struct PortalResponse {
    portal_url: String,
}

/// POST /api/public/domain/{domain}/billing/portal
/// Creates a Stripe customer portal session
async fn create_portal(
    State(app_state): State<AppState>,
    Path(hostname): Path<String>,
    cookies: CookieJar,
    Json(payload): Json<CreatePortalPayload>,
) -> AppResult<impl IntoResponse> {
    use crate::infra::stripe_client::StripeClient;

    let root_domain = extract_root_from_reauth_hostname(&hostname);

    // Get user from token
    let (user_id, domain_id) = get_current_user(&app_state, &cookies, &root_domain)?;

    // Get user's subscription to find Stripe customer ID
    let subscription = app_state
        .billing_use_cases
        .get_user_subscription(domain_id, user_id)
        .await?
        .ok_or(AppError::NotFound)?;

    // Get Stripe client
    let secret_key = app_state
        .billing_use_cases
        .get_stripe_secret_key(domain_id)
        .await?;
    let stripe = StripeClient::new(secret_key);

    // Create portal session
    let portal = stripe
        .create_portal_session(&subscription.stripe_customer_id, &payload.return_url)
        .await?;

    Ok(Json(PortalResponse {
        portal_url: portal.url,
    }))
}

/// POST /api/public/domain/{domain}/billing/cancel
/// Cancels the user's subscription at period end
async fn cancel_subscription(
    State(app_state): State<AppState>,
    Path(hostname): Path<String>,
    cookies: CookieJar,
) -> AppResult<impl IntoResponse> {
    use crate::infra::stripe_client::StripeClient;

    let root_domain = extract_root_from_reauth_hostname(&hostname);

    // Get user from token
    let (user_id, domain_id) = get_current_user(&app_state, &cookies, &root_domain)?;

    // Get user's subscription
    let subscription = app_state
        .billing_use_cases
        .get_user_subscription(domain_id, user_id)
        .await?
        .ok_or(AppError::NotFound)?;

    // Get Stripe subscription ID
    let stripe_subscription_id =
        subscription
            .stripe_subscription_id
            .ok_or(AppError::InvalidInput(
                "No active Stripe subscription".into(),
            ))?;

    // Get Stripe client
    let secret_key = app_state
        .billing_use_cases
        .get_stripe_secret_key(domain_id)
        .await?;
    let stripe = StripeClient::new(secret_key);

    // Cancel at period end
    stripe
        .cancel_subscription(&stripe_subscription_id, true)
        .await?;

    Ok(StatusCode::OK)
}

/// Query params for payment list
#[derive(Debug, Deserialize)]
struct PaymentListQuery {
    page: Option<i32>,
    per_page: Option<i32>,
}

/// Response for paginated payments
#[derive(Debug, Serialize)]
struct PaymentListResponse {
    payments: Vec<PaymentResponse>,
    total: i64,
    page: i32,
    per_page: i32,
    total_pages: i32,
}

#[derive(Debug, Serialize)]
struct PaymentResponse {
    id: String,
    amount_cents: i32,
    amount_paid_cents: i32,
    amount_refunded_cents: i32,
    currency: String,
    status: String,
    payment_provider: Option<PaymentProvider>,
    payment_mode: Option<PaymentMode>,
    plan_name: Option<String>,
    plan_code: Option<String>,
    invoice_url: Option<String>,
    invoice_pdf: Option<String>,
    invoice_number: Option<String>,
    payment_date: Option<i64>,
    created_at: Option<i64>,
}

/// GET /api/public/domain/{domain}/billing/payments
/// Returns the user's payment history
async fn get_user_payments(
    State(app_state): State<AppState>,
    Path(hostname): Path<String>,
    Query(query): Query<PaymentListQuery>,
    cookies: CookieJar,
) -> AppResult<impl IntoResponse> {
    let root_domain = extract_root_from_reauth_hostname(&hostname);

    // Get user from token
    let (user_id, domain_id) = get_current_user(&app_state, &cookies, &root_domain)?;

    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(10).clamp(1, 100);

    let paginated = app_state
        .billing_use_cases
        .get_user_payments(domain_id, user_id, page, per_page)
        .await?;

    let payments: Vec<PaymentResponse> = paginated
        .payments
        .into_iter()
        .map(|p| PaymentResponse {
            id: p.payment.id.to_string(),
            amount_cents: p.payment.amount_cents,
            amount_paid_cents: p.payment.amount_paid_cents,
            amount_refunded_cents: p.payment.amount_refunded_cents,
            currency: p.payment.currency,
            status: p.payment.status.as_str().to_string(),
            payment_provider: p.payment.payment_provider,
            payment_mode: p.payment.payment_mode,
            plan_name: p.payment.plan_name,
            plan_code: p.payment.plan_code,
            invoice_url: p.payment.hosted_invoice_url,
            invoice_pdf: p.payment.invoice_pdf_url,
            invoice_number: p.payment.invoice_number,
            payment_date: p.payment.payment_date.map(|dt| dt.and_utc().timestamp()),
            created_at: p.payment.created_at.map(|dt| dt.and_utc().timestamp()),
        })
        .collect();

    Ok(Json(PaymentListResponse {
        payments,
        total: paginated.total,
        page: paginated.page,
        per_page: paginated.per_page,
        total_pages: paginated.total_pages,
    }))
}

// ============================================================================
// Plan Change (Upgrade/Downgrade) Types
// ============================================================================

/// Query params for plan change preview
#[derive(Debug, Deserialize)]
struct PlanChangePreviewQuery {
    plan_code: String,
}

/// Response for plan change preview
#[derive(Debug, Serialize)]
struct PlanChangePreviewResponse {
    prorated_amount_cents: i64,
    currency: String,
    period_end: i64,
    new_plan_name: String,
    new_plan_price_cents: i64,
    change_type: String,
    effective_at: i64,
}

/// Request body for plan change
#[derive(Debug, Deserialize)]
struct PlanChangeRequest {
    plan_code: String,
}

/// Response for plan change
#[derive(Debug, Serialize)]
struct PlanChangeResponse {
    success: bool,
    change_type: String,
    invoice_id: Option<String>,
    amount_charged_cents: Option<i64>,
    currency: Option<String>,
    client_secret: Option<String>,
    hosted_invoice_url: Option<String>,
    payment_intent_status: Option<String>,
    new_plan: PlanChangeNewPlanResponse,
    effective_at: i64,
    schedule_id: Option<String>,
}

#[derive(Debug, Serialize)]
struct PlanChangeNewPlanResponse {
    code: String,
    name: String,
    price_cents: i32,
    currency: String,
    interval: String,
    interval_count: i32,
    features: Vec<String>,
}

/// GET /api/public/domain/{domain}/billing/plan-change/preview
/// Preview the cost of upgrading or downgrading a subscription
async fn preview_plan_change(
    State(app_state): State<AppState>,
    Path(hostname): Path<String>,
    Query(query): Query<PlanChangePreviewQuery>,
    cookies: CookieJar,
) -> AppResult<impl IntoResponse> {
    let root_domain = extract_root_from_reauth_hostname(&hostname);

    // Get user from token
    let (user_id, domain_id) = get_current_user(&app_state, &cookies, &root_domain)?;

    // Get preview from use cases
    let preview = app_state
        .billing_use_cases
        .preview_plan_change(domain_id, user_id, &query.plan_code)
        .await?;

    Ok(Json(PlanChangePreviewResponse {
        prorated_amount_cents: preview.prorated_amount_cents,
        currency: preview.currency,
        period_end: preview.period_end,
        new_plan_name: preview.new_plan_name,
        new_plan_price_cents: preview.new_plan_price_cents,
        change_type: preview.change_type.as_str().to_string(),
        effective_at: preview.effective_at,
    }))
}

/// POST /api/public/domain/{domain}/billing/plan-change
/// Execute a plan change (upgrade or downgrade)
async fn change_plan(
    State(app_state): State<AppState>,
    Path(hostname): Path<String>,
    headers: HeaderMap,
    cookies: CookieJar,
    Json(payload): Json<PlanChangeRequest>,
) -> AppResult<impl IntoResponse> {
    let root_domain = extract_root_from_reauth_hostname(&hostname);

    // Get user from token
    let (user_id, domain_id) = get_current_user(&app_state, &cookies, &root_domain)?;

    // Get or generate idempotency key
    let idempotency_key = headers
        .get("Idempotency-Key")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    // Execute plan change
    let result = app_state
        .billing_use_cases
        .change_plan(domain_id, user_id, &payload.plan_code, &idempotency_key)
        .await?;

    Ok(Json(PlanChangeResponse {
        success: result.success,
        change_type: result.change_type.as_str().to_string(),
        invoice_id: result.invoice_id,
        amount_charged_cents: result.amount_charged_cents,
        currency: result.currency,
        client_secret: result.client_secret,
        hosted_invoice_url: result.hosted_invoice_url,
        payment_intent_status: result.payment_intent_status,
        new_plan: PlanChangeNewPlanResponse {
            code: result.new_plan.code,
            name: result.new_plan.name,
            price_cents: result.new_plan.price_cents,
            currency: result.new_plan.currency,
            interval: result.new_plan.interval,
            interval_count: result.new_plan.interval_count,
            features: result.new_plan.features,
        },
        effective_at: result.effective_at,
        schedule_id: result.schedule_id,
    }))
}

use crate::domain::entities::stripe_mode::StripeMode;

/// POST /api/public/domain/{domain}/billing/webhook/test
/// Handles Stripe webhook events for test mode
async fn handle_webhook_test(
    state: State<AppState>,
    path: Path<String>,
    headers: HeaderMap,
    body: String,
) -> AppResult<impl IntoResponse> {
    handle_webhook_for_mode(state, path, headers, body, StripeMode::Test).await
}

/// POST /api/public/domain/{domain}/billing/webhook/live
/// Handles Stripe webhook events for live mode
async fn handle_webhook_live(
    state: State<AppState>,
    path: Path<String>,
    headers: HeaderMap,
    body: String,
) -> AppResult<impl IntoResponse> {
    handle_webhook_for_mode(state, path, headers, body, StripeMode::Live).await
}

/// Internal webhook handler that processes events for a specific mode
async fn handle_webhook_for_mode(
    State(app_state): State<AppState>,
    Path(hostname): Path<String>,
    headers: HeaderMap,
    body: String,
    stripe_mode: StripeMode,
) -> AppResult<impl IntoResponse> {
    use crate::domain::entities::user_subscription::SubscriptionStatus;
    use crate::infra::stripe_client::StripeClient;

    let root_domain = extract_root_from_reauth_hostname(&hostname);

    // Get domain
    let domain = app_state
        .domain_auth_use_cases
        .get_domain_by_name(&root_domain)
        .await?
        .ok_or(AppError::NotFound)?;

    // Get webhook secret for the specific mode
    let webhook_secret = app_state
        .billing_use_cases
        .get_stripe_webhook_secret_for_mode(domain.id, stripe_mode)
        .await?;

    // Get Stripe signature
    let signature = headers
        .get("stripe-signature")
        .and_then(|v| v.to_str().ok())
        .ok_or(AppError::InvalidInput("Missing Stripe signature".into()))?;

    // Verify signature
    StripeClient::verify_webhook_signature(&body, signature, &webhook_secret)?;

    // Parse event
    let event: serde_json::Value = serde_json::from_str(&body)
        .map_err(|e| AppError::InvalidInput(format!("Invalid webhook payload: {}", e)))?;

    let event_type = event["type"].as_str().unwrap_or("");
    let event_id = event["id"].as_str().unwrap_or("");

    // Check idempotency
    if app_state
        .billing_use_cases
        .is_event_processed(event_id)
        .await?
    {
        return Ok(StatusCode::OK);
    }

    // Handle event types
    match event_type {
        "checkout.session.completed" => {
            let session = &event["data"]["object"];
            let customer_id = session["customer"].as_str().unwrap_or("");
            let subscription_id = session["subscription"].as_str();
            let client_reference_id = session["client_reference_id"].as_str();

            if let (Some(sub_id), Some(user_id_str)) = (subscription_id, client_reference_id) {
                if let Ok(user_id) = Uuid::parse_str(user_id_str) {
                    // Get the Stripe subscription to find the price ID
                    let secret_key = app_state
                        .billing_use_cases
                        .get_stripe_secret_key(domain.id)
                        .await?;
                    let stripe = StripeClient::new(secret_key);

                    if let Ok(stripe_sub) = stripe.get_subscription(sub_id).await {
                        // Use the webhook mode for plan lookup

                        // Find plan by Stripe price ID - search ALL plans (not just public)
                        // because plan visibility can change after purchase
                        let plan = app_state
                            .billing_use_cases
                            .get_plan_by_stripe_price_id(
                                domain.id,
                                stripe_mode,
                                &stripe_sub.price_id(),
                            )
                            .await?;

                        if let Some(plan) = plan {
                            use crate::application::use_cases::domain_billing::CreateSubscriptionInput;

                            // Map Stripe status to our SubscriptionStatus - don't assume Active
                            let status = match stripe_sub.status.as_str() {
                                "active" => SubscriptionStatus::Active,
                                "past_due" => SubscriptionStatus::PastDue,
                                "canceled" => SubscriptionStatus::Canceled,
                                "trialing" => SubscriptionStatus::Trialing,
                                "incomplete" => SubscriptionStatus::Incomplete,
                                "incomplete_expired" => SubscriptionStatus::IncompleteExpired,
                                "unpaid" => SubscriptionStatus::Unpaid,
                                "paused" => SubscriptionStatus::Paused,
                                // Default to Incomplete - never grant access by default
                                _ => SubscriptionStatus::Incomplete,
                            };

                            let input = CreateSubscriptionInput {
                                domain_id: domain.id,
                                stripe_mode,
                                end_user_id: user_id,
                                plan_id: plan.id,
                                stripe_customer_id: customer_id.to_string(),
                                stripe_subscription_id: Some(sub_id.to_string()),
                                status,
                                current_period_start: Some(
                                    chrono::NaiveDateTime::from_timestamp_opt(
                                        stripe_sub.current_period_start,
                                        0,
                                    )
                                    .unwrap_or_default(),
                                ),
                                current_period_end: Some(
                                    chrono::NaiveDateTime::from_timestamp_opt(
                                        stripe_sub.current_period_end,
                                        0,
                                    )
                                    .unwrap_or_default(),
                                ),
                                trial_start: stripe_sub.trial_start.and_then(|ts| {
                                    chrono::NaiveDateTime::from_timestamp_opt(ts, 0)
                                }),
                                trial_end: stripe_sub.trial_end.and_then(|ts| {
                                    chrono::NaiveDateTime::from_timestamp_opt(ts, 0)
                                }),
                            };

                            let created_sub = app_state
                                .billing_use_cases
                                .create_or_update_subscription(&input)
                                .await?;

                            // Log event with actual status
                            app_state
                                .billing_use_cases
                                .log_webhook_event(
                                    created_sub.id,
                                    event_type,
                                    None,
                                    Some(status),
                                    event_id,
                                    serde_json::json!({"customer_id": customer_id, "stripe_status": &stripe_sub.status}),
                                )
                                .await?;
                        }
                    }
                }
            }
        }
        "customer.subscription.updated" | "customer.subscription.deleted" => {
            let subscription = &event["data"]["object"];
            let stripe_sub_id = subscription["id"].as_str().unwrap_or("");
            let status_str = subscription["status"].as_str().unwrap_or("");

            let new_status = match status_str {
                "active" => SubscriptionStatus::Active,
                "past_due" => SubscriptionStatus::PastDue,
                "canceled" => SubscriptionStatus::Canceled,
                "trialing" => SubscriptionStatus::Trialing,
                "incomplete" => SubscriptionStatus::Incomplete,
                "incomplete_expired" => SubscriptionStatus::IncompleteExpired,
                "unpaid" => SubscriptionStatus::Unpaid,
                "paused" => SubscriptionStatus::Paused,
                // Default to Incomplete for unknown statuses - never grant access by default
                _ => SubscriptionStatus::Incomplete,
            };

            // Extract price_id from subscription items to handle plan upgrades/downgrades
            let stripe_price_id = subscription["items"]["data"]
                .as_array()
                .and_then(|items| items.first())
                .and_then(|item| item["price"]["id"].as_str());

            // Look up plan by stripe_price_id to handle plan changes
            // Use the webhook mode for plan lookup
            let plan_id = if let Some(price_id) = stripe_price_id {
                app_state
                    .billing_use_cases
                    .get_plan_by_stripe_price_id(domain.id, stripe_mode, price_id)
                    .await
                    .ok()
                    .flatten()
                    .map(|p| p.id)
            } else {
                None
            };

            use crate::application::use_cases::domain_billing::StripeSubscriptionUpdate;

            let update = StripeSubscriptionUpdate {
                status: new_status,
                plan_id, // Update plan if it changed (upgrade/downgrade via Stripe portal)
                stripe_subscription_id: None, // Already set, don't overwrite
                current_period_start: subscription["current_period_start"]
                    .as_i64()
                    .and_then(|ts| chrono::NaiveDateTime::from_timestamp_opt(ts, 0)),
                current_period_end: subscription["current_period_end"]
                    .as_i64()
                    .and_then(|ts| chrono::NaiveDateTime::from_timestamp_opt(ts, 0)),
                cancel_at_period_end: subscription["cancel_at_period_end"]
                    .as_bool()
                    .unwrap_or(false),
                canceled_at: subscription["canceled_at"]
                    .as_i64()
                    .and_then(|ts| chrono::NaiveDateTime::from_timestamp_opt(ts, 0)),
                trial_start: subscription["trial_start"]
                    .as_i64()
                    .and_then(|ts| chrono::NaiveDateTime::from_timestamp_opt(ts, 0)),
                trial_end: subscription["trial_end"]
                    .as_i64()
                    .and_then(|ts| chrono::NaiveDateTime::from_timestamp_opt(ts, 0)),
            };

            if let Ok(updated_sub) = app_state
                .billing_use_cases
                .update_subscription_from_stripe(stripe_sub_id, &update)
                .await
            {
                app_state
                    .billing_use_cases
                    .log_webhook_event(
                        updated_sub.id,
                        event_type,
                        None,
                        Some(new_status),
                        event_id,
                        serde_json::json!({"stripe_status": status_str}),
                    )
                    .await?;
            }
        }
        // Invoice events for payment history tracking
        // Note: invoice.payment_succeeded is the newer event name (some Stripe configs use it)
        "invoice.created"
        | "invoice.paid"
        | "invoice.payment_succeeded"
        | "invoice.updated"
        | "invoice.finalized" => {
            let invoice = &event["data"]["object"];

            // Try to sync the invoice to our payments table
            match app_state
                .billing_use_cases
                .sync_invoice_from_webhook(domain.id, stripe_mode, invoice)
                .await
            {
                Ok(_payment) => {
                    tracing::info!("Synced payment from {} event: {}", event_type, event_id);
                }
                Err(e) => {
                    // Log but don't fail - the invoice might be for a customer we don't know
                    tracing::warn!(
                        "Could not sync invoice from {} event: {} - {}",
                        event_type,
                        event_id,
                        e
                    );
                }
            }
        }
        "invoice.payment_failed" => {
            let invoice = &event["data"]["object"];
            let invoice_id = invoice["id"].as_str().unwrap_or("");

            // First try to sync/create the invoice
            let _ = app_state
                .billing_use_cases
                .sync_invoice_from_webhook(domain.id, stripe_mode, invoice)
                .await;

            // Extract failure message from the invoice
            let failure_message = invoice["last_finalization_error"]["message"]
                .as_str()
                .or_else(|| invoice["last_payment_error"]["message"].as_str())
                .map(|s| s.to_string());

            // Update status to failed
            if let Err(e) = app_state
                .billing_use_cases
                .update_payment_status(
                    invoice_id,
                    crate::domain::entities::payment_status::PaymentStatus::Failed,
                    None,
                    failure_message,
                )
                .await
            {
                tracing::warn!(
                    "Could not update payment status for failed invoice {}: {}",
                    invoice_id,
                    e
                );
            }
        }
        "invoice.voided" => {
            let invoice = &event["data"]["object"];
            let invoice_id = invoice["id"].as_str().unwrap_or("");

            if let Err(e) = app_state
                .billing_use_cases
                .update_payment_status(
                    invoice_id,
                    crate::domain::entities::payment_status::PaymentStatus::Void,
                    None,
                    None,
                )
                .await
            {
                tracing::warn!(
                    "Could not update payment status for voided invoice {}: {}",
                    invoice_id,
                    e
                );
            }
        }
        "invoice.marked_uncollectible" => {
            let invoice = &event["data"]["object"];
            let invoice_id = invoice["id"].as_str().unwrap_or("");

            if let Err(e) = app_state
                .billing_use_cases
                .update_payment_status(
                    invoice_id,
                    crate::domain::entities::payment_status::PaymentStatus::Uncollectible,
                    None,
                    None,
                )
                .await
            {
                tracing::warn!(
                    "Could not update payment status for uncollectible invoice {}: {}",
                    invoice_id,
                    e
                );
            }
        }
        "charge.refunded" => {
            // Handle refunds - need to find the associated invoice
            let charge = &event["data"]["object"];
            let invoice_id = charge["invoice"].as_str();
            let amount_refunded = charge["amount_refunded"].as_i64().unwrap_or(0) as i32;
            let amount = charge["amount"].as_i64().unwrap_or(0) as i32;

            if let Some(invoice_id) = invoice_id {
                // Determine if it's a full or partial refund
                let status = if amount_refunded >= amount {
                    crate::domain::entities::payment_status::PaymentStatus::Refunded
                } else {
                    crate::domain::entities::payment_status::PaymentStatus::PartialRefund
                };

                if let Err(e) = app_state
                    .billing_use_cases
                    .update_payment_status(invoice_id, status, Some(amount_refunded), None)
                    .await
                {
                    tracing::warn!(
                        "Could not update payment status for refund on invoice {}: {}",
                        invoice_id,
                        e
                    );
                }
            }
        }
        "charge.succeeded" => {
            // Backup confirmation of payment - sync invoice if we have one
            let charge = &event["data"]["object"];
            if let Some(invoice_id) = charge["invoice"].as_str() {
                // Fetch and sync the invoice data
                tracing::debug!(
                    "Charge succeeded for invoice {}, invoice event should handle sync",
                    invoice_id
                );
            }
        }
        "charge.failed" => {
            // Payment failed - update invoice status if we have one
            let charge = &event["data"]["object"];
            if let Some(invoice_id) = charge["invoice"].as_str() {
                let failure_message = charge["failure_message"].as_str().map(|s| s.to_string());
                if let Err(e) = app_state
                    .billing_use_cases
                    .update_payment_status(
                        invoice_id,
                        crate::domain::entities::payment_status::PaymentStatus::Failed,
                        None,
                        failure_message,
                    )
                    .await
                {
                    tracing::warn!(
                        "Could not update payment status for failed charge on invoice {}: {}",
                        invoice_id,
                        e
                    );
                }
            }
        }
        "charge.dispute.created" => {
            // Dispute opened - log for awareness (could add dispute tracking later)
            let dispute = &event["data"]["object"];
            let charge_id = dispute["charge"].as_str().unwrap_or("unknown");
            let amount = dispute["amount"].as_i64().unwrap_or(0);
            tracing::warn!(
                "Dispute opened for charge {} (amount: {} cents) on domain {}",
                charge_id,
                amount,
                domain.domain
            );
        }
        "charge.dispute.closed" => {
            let dispute = &event["data"]["object"];
            let status = dispute["status"].as_str().unwrap_or("unknown");
            let charge_id = dispute["charge"].as_str().unwrap_or("unknown");
            tracing::info!(
                "Dispute closed for charge {} with status: {}",
                charge_id,
                status
            );
        }
        "checkout.session.async_payment_failed" => {
            // Async payment (bank transfer, etc.) failed
            let session = &event["data"]["object"];
            let session_id = session["id"].as_str().unwrap_or("unknown");
            tracing::warn!("Async payment failed for checkout session {}", session_id);
        }
        "checkout.session.expired" => {
            // Checkout was abandoned
            let session = &event["data"]["object"];
            let session_id = session["id"].as_str().unwrap_or("unknown");
            tracing::debug!("Checkout session {} expired", session_id);
        }
        "customer.subscription.trial_will_end" => {
            // Trial ending soon - could trigger notification
            let subscription = &event["data"]["object"];
            let sub_id = subscription["id"].as_str().unwrap_or("unknown");
            let trial_end = subscription["trial_end"].as_i64();
            tracing::info!(
                "Trial will end for subscription {}: {:?}",
                sub_id,
                trial_end
            );
        }
        _ => {
            tracing::debug!("Unhandled webhook event type: {}", event_type);
        }
    }

    Ok(StatusCode::OK)
}

/// Helper to extract current user from cookies
fn get_current_user(
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

/// POST /api/public/domain/{domain}/auth/google/unlink
/// Unlinks the Google account from the current end-user's account
async fn unlink_google(
    State(app_state): State<AppState>,
    Path(hostname): Path<String>,
    cookies: CookieJar,
) -> AppResult<impl IntoResponse> {
    // Extract root domain from reauth.* hostname
    let root_domain = extract_root_from_reauth_hostname(&hostname);

    // Get end_user_id from access or refresh token (same pattern as delete_account)
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
        return Err(AppError::InvalidCredentials);
    };

    // Unlink Google account
    app_state
        .domain_auth_use_cases
        .unlink_google_account(end_user_id)
        .await?;

    Ok(StatusCode::OK)
}

// ============================================================================
// Google OAuth Helper Functions
// ============================================================================

#[derive(Deserialize)]
struct GoogleTokenResponse {
    #[allow(dead_code)]
    access_token: String,
    id_token: String,
    #[allow(dead_code)]
    token_type: String,
    #[allow(dead_code)]
    expires_in: Option<i64>,
}

/// Exchange authorization code with Google for tokens
async fn exchange_google_code(
    code: &str,
    client_id: &str,
    client_secret: &str,
    redirect_uri: &str,
    code_verifier: &str,
) -> AppResult<GoogleTokenResponse> {
    let client = http_client::try_build_client().map_err(|e| {
        error!(
            error = %e,
            "Failed to build HTTP client for Google OAuth token exchange"
        );
        AppError::Internal("Failed to build HTTP client".into())
    })?;

    let response = client
        .post("https://oauth2.googleapis.com/token")
        .form(&[
            ("code", code),
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("redirect_uri", redirect_uri),
            ("grant_type", "authorization_code"),
            ("code_verifier", code_verifier),
        ])
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("Failed to exchange code with Google: {}", e)))?;

    if !response.status().is_success() {
        let error_text = response.text().await.unwrap_or_default();
        tracing::error!("Google token exchange failed: {}", error_text);
        return Err(AppError::InvalidInput(
            "Failed to authenticate with Google".into(),
        ));
    }

    response
        .json::<GoogleTokenResponse>()
        .await
        .map_err(|e| AppError::Internal(format!("Failed to parse Google token response: {}", e)))
}

/// Google OIDC claims from id_token
#[derive(Debug, serde::Deserialize)]
struct GoogleIdTokenClaims {
    /// Google user ID (stable identifier)
    sub: String,
    /// User's email address
    email: String,
    /// Whether the email has been verified by Google
    #[serde(default)]
    email_verified: bool,
    /// Issuer (validated by jsonwebtoken)
    #[allow(dead_code)]
    iss: String,
    /// Audience (validated by jsonwebtoken)
    #[allow(dead_code)]
    aud: String,
    /// Authorized party (if present, should match client_id)
    #[serde(default)]
    azp: Option<String>,
}

/// Google JWKs response
#[derive(Debug, serde::Deserialize)]
struct GoogleJwks {
    keys: Vec<GoogleJwk>,
}

#[derive(Debug, serde::Deserialize)]
struct GoogleJwk {
    kid: String,
    n: String,
    e: String,
    #[allow(dead_code)]
    kty: String,
    #[allow(dead_code)]
    alg: Option<String>,
}

/// Fetch Google's public keys for JWT verification
async fn fetch_google_jwks() -> AppResult<GoogleJwks> {
    let client = http_client::try_build_client().map_err(|e| {
        error!(
            error = %e,
            "Failed to build HTTP client for Google JWKS fetch"
        );
        AppError::Internal("Failed to build HTTP client".into())
    })?;
    let response = client
        .get("https://www.googleapis.com/oauth2/v3/certs")
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("Failed to fetch Google JWKs: {}", e)))?;

    if !response.status().is_success() {
        return Err(AppError::Internal("Failed to fetch Google JWKs".into()));
    }

    response
        .json::<GoogleJwks>()
        .await
        .map_err(|e| AppError::Internal(format!("Failed to parse Google JWKs: {}", e)))
}

/// Parse and verify Google id_token with signature verification
async fn parse_google_id_token(
    id_token: &str,
    expected_client_id: &str,
) -> AppResult<(String, String, bool)> {
    use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};

    // Decode the header to get the key ID (kid)
    let header = decode_header(id_token)
        .map_err(|e| AppError::InvalidInput(format!("Invalid id_token header: {}", e)))?;

    let kid = header
        .kid
        .ok_or_else(|| AppError::InvalidInput("Missing kid in id_token header".into()))?;

    // Fetch Google's JWKs
    let jwks = fetch_google_jwks().await?;

    // Find the matching key
    let jwk = jwks
        .keys
        .iter()
        .find(|k| k.kid == kid)
        .ok_or_else(|| AppError::InvalidInput("No matching key found in Google JWKs".into()))?;

    // Create decoding key from JWK
    let decoding_key = DecodingKey::from_rsa_components(&jwk.n, &jwk.e)
        .map_err(|e| AppError::Internal(format!("Failed to create decoding key: {}", e)))?;

    // Set up validation
    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_audience(&[expected_client_id]);
    validation.set_issuer(&["https://accounts.google.com", "accounts.google.com"]);

    // Decode and verify the token
    let token_data = decode::<GoogleIdTokenClaims>(id_token, &decoding_key, &validation)
        .map_err(|e| AppError::InvalidInput(format!("Invalid id_token: {}", e)))?;

    let claims = token_data.claims;

    // Additional validation: check azp if present
    if let Some(ref azp) = claims.azp {
        if azp != expected_client_id {
            return Err(AppError::InvalidInput("Invalid id_token azp claim".into()));
        }
    }

    Ok((claims.sub, claims.email, claims.email_verified))
}

// ============================================================================
// Payment Provider Handlers
// ============================================================================

#[derive(Serialize)]
struct AvailableProvider {
    id: Uuid,
    domain_id: Uuid,
    provider: PaymentProvider,
    mode: PaymentMode,
    is_active: bool,
    display_order: i32,
    created_at: Option<chrono::NaiveDateTime>,
}

/// GET /api/public/domain/{domain}/billing/providers
/// Returns the list of active payment providers for this domain
async fn get_available_providers(
    State(app_state): State<AppState>,
    Path(hostname): Path<String>,
) -> AppResult<impl IntoResponse> {
    let root_domain = extract_root_from_reauth_hostname(&hostname);

    let domain = app_state
        .domain_auth_use_cases
        .get_domain_by_name(&root_domain)
        .await?
        .ok_or(AppError::NotFound)?;

    let active_providers = app_state
        .billing_use_cases
        .list_active_providers(domain.id)
        .await?;

    let response: Vec<AvailableProvider> = active_providers
        .into_iter()
        .map(|p| AvailableProvider {
            id: p.id,
            domain_id: p.domain_id,
            provider: p.provider,
            mode: p.mode,
            is_active: p.is_active,
            display_order: p.display_order,
            created_at: p.created_at,
        })
        .collect();

    Ok(Json(response))
}

#[derive(Serialize)]
struct DummyScenarioInfo {
    scenario: PaymentScenario,
    display_name: String,
    description: String,
    test_card: String,
}

/// GET /api/public/domain/{domain}/billing/dummy/scenarios
/// Returns available test scenarios for the dummy payment provider
async fn get_dummy_scenarios(
    State(_app_state): State<AppState>,
    Path(_hostname): Path<String>,
) -> AppResult<impl IntoResponse> {
    let scenarios = vec![
        DummyScenarioInfo {
            scenario: PaymentScenario::Success,
            display_name: "Success".to_string(),
            description: "Payment completes successfully".to_string(),
            test_card: "4242 4242 4242 4242".to_string(),
        },
        DummyScenarioInfo {
            scenario: PaymentScenario::Decline,
            display_name: "Card Declined".to_string(),
            description: "Card is declined by the issuer".to_string(),
            test_card: "4000 0000 0000 0002".to_string(),
        },
        DummyScenarioInfo {
            scenario: PaymentScenario::InsufficientFunds,
            display_name: "Insufficient Funds".to_string(),
            description: "Card has insufficient funds".to_string(),
            test_card: "4000 0000 0000 9995".to_string(),
        },
        DummyScenarioInfo {
            scenario: PaymentScenario::ThreeDSecure,
            display_name: "3D Secure Required".to_string(),
            description: "Requires additional authentication".to_string(),
            test_card: "4000 0000 0000 3220".to_string(),
        },
        DummyScenarioInfo {
            scenario: PaymentScenario::ExpiredCard,
            display_name: "Expired Card".to_string(),
            description: "Card has expired".to_string(),
            test_card: "4000 0000 0000 0069".to_string(),
        },
        DummyScenarioInfo {
            scenario: PaymentScenario::ProcessingError,
            display_name: "Processing Error".to_string(),
            description: "A processing error occurred".to_string(),
            test_card: "4000 0000 0000 0119".to_string(),
        },
    ];

    Ok(Json(scenarios))
}

#[derive(Deserialize)]
struct DummyCheckoutPayload {
    plan_code: String,
    scenario: PaymentScenario,
}

#[derive(Serialize)]
struct DummyCheckoutResponse {
    success: bool,
    requires_confirmation: bool,
    confirmation_token: Option<String>,
    error_message: Option<String>,
    subscription_id: Option<String>,
}

/// POST /api/public/domain/{domain}/billing/checkout/dummy
/// Creates a test subscription using the dummy payment provider
async fn create_dummy_checkout(
    State(app_state): State<AppState>,
    Path(hostname): Path<String>,
    cookies: CookieJar,
    Json(payload): Json<DummyCheckoutPayload>,
) -> AppResult<impl IntoResponse> {
    let root_domain = extract_root_from_reauth_hostname(&hostname);

    // Get user from token
    let (user_id, domain_id) = get_current_user(&app_state, &cookies, &root_domain)?;

    // Verify dummy provider is enabled
    let is_enabled = app_state
        .billing_use_cases
        .is_provider_enabled(domain_id, PaymentProvider::Dummy, PaymentMode::Test)
        .await?;

    if !is_enabled {
        return Err(AppError::InvalidInput(
            "Dummy payment provider is not enabled for this domain".into(),
        ));
    }

    // Get the plan
    let plan = app_state
        .billing_use_cases
        .get_plan_by_code(domain_id, &payload.plan_code)
        .await?
        .ok_or(AppError::NotFound)?;

    // Build response based on scenario
    let response = match payload.scenario {
        PaymentScenario::Success => {
            // Generate subscription ID
            let subscription_id_str = format!("dummy_sub_{}", Uuid::new_v4());

            // Create subscription and payment records
            use crate::application::use_cases::domain_billing::CreateSubscriptionInput;
            use crate::domain::entities::stripe_mode::StripeMode;
            use crate::domain::entities::user_subscription::SubscriptionStatus;

            let now = chrono::Utc::now().naive_utc();
            let period_end = now
                + chrono::Duration::days(match plan.interval.as_str() {
                    "yearly" => 365 * plan.interval_count as i64,
                    _ => 30 * plan.interval_count as i64, // monthly default
                });

            let subscription = app_state
                .billing_use_cases
                .create_or_update_subscription(&CreateSubscriptionInput {
                    domain_id,
                    stripe_mode: StripeMode::Test,
                    end_user_id: user_id,
                    plan_id: plan.id,
                    stripe_customer_id: format!("dummy_cus_{}", user_id),
                    stripe_subscription_id: Some(subscription_id_str.clone()),
                    status: SubscriptionStatus::Active,
                    current_period_start: Some(now),
                    current_period_end: Some(period_end),
                    trial_start: None,
                    trial_end: None,
                })
                .await?;

            // Create payment record
            app_state
                .billing_use_cases
                .create_dummy_payment(domain_id, user_id, subscription.id, &plan)
                .await?;

            DummyCheckoutResponse {
                success: true,
                requires_confirmation: false,
                confirmation_token: None,
                error_message: None,
                subscription_id: Some(subscription_id_str),
            }
        }
        PaymentScenario::ThreeDSecure => {
            // Encode plan_code in the token so confirm endpoint can use it
            DummyCheckoutResponse {
                success: false,
                requires_confirmation: true,
                confirmation_token: Some(format!(
                    "3ds_token_{}_{}",
                    payload.plan_code,
                    Uuid::new_v4()
                )),
                error_message: None,
                subscription_id: None,
            }
        }
        PaymentScenario::Decline => DummyCheckoutResponse {
            success: false,
            requires_confirmation: false,
            confirmation_token: None,
            error_message: Some("Your card was declined".into()),
            subscription_id: None,
        },
        PaymentScenario::InsufficientFunds => DummyCheckoutResponse {
            success: false,
            requires_confirmation: false,
            confirmation_token: None,
            error_message: Some("Your card has insufficient funds".into()),
            subscription_id: None,
        },
        PaymentScenario::ExpiredCard => DummyCheckoutResponse {
            success: false,
            requires_confirmation: false,
            confirmation_token: None,
            error_message: Some("Your card has expired".into()),
            subscription_id: None,
        },
        PaymentScenario::ProcessingError => DummyCheckoutResponse {
            success: false,
            requires_confirmation: false,
            confirmation_token: None,
            error_message: Some("A processing error occurred. Please try again.".into()),
            subscription_id: None,
        },
    };

    // Log for debugging
    tracing::info!(
        domain_id = %domain_id,
        user_id = %user_id,
        scenario = ?payload.scenario,
        success = response.success,
        "Dummy checkout processed"
    );

    Ok(Json(response))
}

#[derive(Deserialize)]
struct DummyConfirmPayload {
    confirmation_token: String,
}

/// POST /api/public/domain/{domain}/billing/dummy/confirm
/// Confirms a 3DS payment for the dummy provider
async fn confirm_dummy_checkout(
    State(app_state): State<AppState>,
    Path(hostname): Path<String>,
    cookies: CookieJar,
    Json(payload): Json<DummyConfirmPayload>,
) -> AppResult<impl IntoResponse> {
    let root_domain = extract_root_from_reauth_hostname(&hostname);

    // Get user from token
    let (user_id, domain_id) = get_current_user(&app_state, &cookies, &root_domain)?;

    // Verify and parse the token (format: 3ds_token_{plan_code}_{uuid})
    if !payload.confirmation_token.starts_with("3ds_token_") {
        return Err(AppError::InvalidInput("Invalid confirmation token".into()));
    }

    // Extract plan_code from token: 3ds_token_{plan_code}_{uuid}
    let token_parts: Vec<&str> = payload.confirmation_token.splitn(4, '_').collect();
    if token_parts.len() < 4 {
        return Err(AppError::InvalidInput(
            "Invalid confirmation token format".into(),
        ));
    }
    let plan_code = token_parts[2]; // 3ds, token, {plan_code}, {uuid}

    // Get the plan
    let plan = app_state
        .billing_use_cases
        .get_plan_by_code(domain_id, plan_code)
        .await?
        .ok_or(AppError::NotFound)?;

    // Create subscription and payment records
    use crate::application::use_cases::domain_billing::CreateSubscriptionInput;
    use crate::domain::entities::stripe_mode::StripeMode;
    use crate::domain::entities::user_subscription::SubscriptionStatus;

    let subscription_id_str = format!("dummy_sub_{}", Uuid::new_v4());
    let now = chrono::Utc::now().naive_utc();
    let period_end = now
        + chrono::Duration::days(match plan.interval.as_str() {
            "yearly" => 365 * plan.interval_count as i64,
            _ => 30 * plan.interval_count as i64,
        });

    let subscription = app_state
        .billing_use_cases
        .create_or_update_subscription(&CreateSubscriptionInput {
            domain_id,
            stripe_mode: StripeMode::Test,
            end_user_id: user_id,
            plan_id: plan.id,
            stripe_customer_id: format!("dummy_cus_{}", user_id),
            stripe_subscription_id: Some(subscription_id_str.clone()),
            status: SubscriptionStatus::Active,
            current_period_start: Some(now),
            current_period_end: Some(period_end),
            trial_start: None,
            trial_end: None,
        })
        .await?;

    // Create payment record
    app_state
        .billing_use_cases
        .create_dummy_payment(domain_id, user_id, subscription.id, &plan)
        .await?;

    let response = DummyCheckoutResponse {
        success: true,
        requires_confirmation: false,
        confirmation_token: None,
        error_message: None,
        subscription_id: Some(subscription_id_str),
    };

    tracing::info!(
        domain_id = %domain_id,
        user_id = %user_id,
        plan_code = %plan_code,
        "Dummy 3DS confirmation processed"
    );

    Ok(Json(response))
}
