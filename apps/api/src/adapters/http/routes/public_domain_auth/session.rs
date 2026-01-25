//! Session management routes: check session, refresh token, get token, logout, delete account.

use super::common::*;

/// Response for GET /auth/token endpoint
#[derive(Serialize)]
struct TokenResponse {
    access_token: String,
    expires_in: i64,
    token_type: String,
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
    if let Some(access_token) = cookies.get("end_user_access_token")
        && let Ok(claims) = verify_token_with_domain_keys(&app_state, access_token.value()).await
        && claims.domain == root_domain
    {
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

    // Fallback: check refresh token (client should call /refresh if access expired)
    if let Some(refresh_token) = cookies.get("end_user_refresh_token")
        && let Ok(claims) = verify_token_with_domain_keys(&app_state, refresh_token.value()).await
        && claims.domain == root_domain
    {
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

/// GET /api/public/domain/{domain}/auth/token
/// Returns an access token in the response body for Bearer auth.
/// SECURITY: Only accepts refresh token (not access token) to prevent indefinite refresh.
/// The {domain} param is the hostname (e.g., "reauth.example.com")
async fn get_token(
    State(app_state): State<AppState>,
    Path(hostname): Path<String>,
    cookies: CookieJar,
) -> AppResult<impl IntoResponse> {
    // Extract root domain from reauth.* hostname
    let root_domain = extract_root_from_reauth_hostname(&hostname);

    // SECURITY: Only accept refresh token (not access token)
    // This prevents indefinite refresh if access token is stolen
    let Some(refresh_cookie) = cookies.get("end_user_refresh_token") else {
        return Err(AppError::InvalidCredentials);
    };

    let claims = verify_token_with_domain_keys(&app_state, refresh_cookie.value())
        .await
        .map_err(|_| AppError::InvalidCredentials)?;

    if claims.domain != root_domain {
        return Err(AppError::InvalidCredentials);
    }

    // Parse end_user_id from claims
    let end_user_id =
        Uuid::parse_str(&claims.sub).map_err(|_| AppError::InvalidCredentials)?;
    let domain_id = Uuid::parse_str(&claims.domain_id)
        .map_err(|_| AppError::InvalidCredentials)?;

    // Check user's current status from database before issuing new token
    if let Ok(Some(user)) = app_state
        .domain_auth_use_cases
        .get_end_user_by_id(end_user_id)
        .await
        && user.is_frozen
    {
        return Err(AppError::AccountSuspended);
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

    // Get signing key for this domain (derived from API key)
    let signing_key = app_state
        .api_key_use_cases
        .get_signing_key_for_domain(domain_id)
        .await?
        .ok_or(AppError::NoApiKeyConfigured)?;

    // Issue new access token using derived secret
    // Use roles from refresh token claims (they're refreshed on login)
    let access_token = jwt::issue_domain_end_user_derived(
        end_user_id,
        domain_id,
        &root_domain,
        claims.roles,
        subscription_claims,
        &signing_key,
        time::Duration::seconds(access_ttl_secs as i64),
    )?;

    Ok(Json(TokenResponse {
        access_token,
        expires_in: access_ttl_secs as i64,
        token_type: "Bearer".to_string(),
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

    let claims = verify_token_with_domain_keys(&app_state, refresh_cookie.value())
        .await
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
        && user.is_frozen
    {
        return Err(crate::app_error::AppError::AccountSuspended);
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

    // Get signing key for this domain (derived from API key)
    let signing_key = app_state
        .api_key_use_cases
        .get_signing_key_for_domain(domain_id)
        .await?
        .ok_or(AppError::NoApiKeyConfigured)?;

    // Issue new access token using derived secret
    let access_token = jwt::issue_domain_end_user_derived(
        end_user_id,
        domain_id,
        &root_domain,
        claims.roles,
        subscription_claims,
        &signing_key,
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

    clear_auth_cookies(&mut headers, &root_domain)?;

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
        if let Ok(claims) = verify_token_with_domain_keys(&app_state, access_token.value()).await {
            if claims.domain == root_domain {
                Some(Uuid::parse_str(&claims.sub).ok())
            } else {
                None
            }
        } else {
            None
        }
    } else if let Some(refresh_token) = cookies.get("end_user_refresh_token") {
        if let Ok(claims) = verify_token_with_domain_keys(&app_state, refresh_token.value()).await
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
    clear_auth_cookies(&mut headers, &root_domain)?;

    Ok((StatusCode::OK, headers))
}

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/{domain}/auth/session", get(check_session))
        .route("/{domain}/auth/token", get(get_token))
        .route("/{domain}/auth/refresh", post(refresh_token))
        .route("/{domain}/auth/logout", post(logout))
        .route("/{domain}/auth/account", delete(delete_account))
}
