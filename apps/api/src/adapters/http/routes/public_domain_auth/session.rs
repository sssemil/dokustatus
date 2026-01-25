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
    let end_user_id = Uuid::parse_str(&claims.sub).map_err(|_| AppError::InvalidCredentials)?;
    let domain_id = Uuid::parse_str(&claims.domain_id).map_err(|_| AppError::InvalidCredentials)?;

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
        if let Ok(claims) = verify_token_with_domain_keys(&app_state, refresh_token.value()).await {
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

#[cfg(test)]
mod tests {
    use axum_test::TestServer;
    use uuid::Uuid;

    use crate::{
        application::{
            jwt,
            use_cases::{api_key::ApiKeyWithRaw, domain_billing::SubscriptionClaims},
        },
        test_utils::{
            TestAppStateBuilder, create_test_auth_config, create_test_domain, create_test_end_user,
        },
    };

    use super::*;

    fn create_test_app_state(
        domain: crate::application::use_cases::domain::DomainProfile,
        user: crate::application::use_cases::domain_auth::DomainEndUserProfile,
        api_key_raw: &str,
    ) -> crate::adapters::http::app_state::AppState {
        let auth_config = create_test_auth_config(domain.id, |c| {
            c.access_token_ttl_secs = 86400;
            c.refresh_token_ttl_days = 30;
        });

        TestAppStateBuilder::new()
            .with_domain(domain.clone())
            .with_user(user)
            .with_auth_config(auth_config)
            .with_api_key(domain.id, &domain.domain, api_key_raw)
            .build()
    }

    fn create_refresh_token(
        user_id: Uuid,
        domain_id: Uuid,
        domain_name: &str,
        api_key_raw: &str,
    ) -> String {
        let api_key = ApiKeyWithRaw {
            id: Uuid::new_v4(),
            domain_id,
            raw_key: api_key_raw.to_string(),
        };

        jwt::issue_domain_end_user_derived(
            user_id,
            domain_id,
            domain_name,
            vec![],
            SubscriptionClaims::none(),
            &api_key,
            time::Duration::days(30), // Refresh token TTL
        )
        .expect("Failed to issue test token")
    }

    fn create_access_token(
        user_id: Uuid,
        domain_id: Uuid,
        domain_name: &str,
        api_key_raw: &str,
    ) -> String {
        let api_key = ApiKeyWithRaw {
            id: Uuid::new_v4(),
            domain_id,
            raw_key: api_key_raw.to_string(),
        };

        jwt::issue_domain_end_user_derived(
            user_id,
            domain_id,
            domain_name,
            vec![],
            SubscriptionClaims::none(),
            &api_key,
            time::Duration::hours(24), // Access token TTL
        )
        .expect("Failed to issue test token")
    }

    // ========================================================================
    // Test: Request without refresh cookie is rejected
    // ========================================================================
    #[tokio::test]
    async fn get_token_rejects_request_without_refresh_cookie() {
        let domain = create_test_domain(|d| d.domain = "example.com".to_string());
        let user = create_test_end_user(domain.id, |_| {});
        let api_key_raw = "test_api_key_123456789012345678901234";

        let app_state = create_test_app_state(domain.clone(), user, api_key_raw);
        let app = router().with_state(app_state);
        let server = TestServer::new(app).unwrap();

        let response = server.get("/reauth.example.com/auth/token").await;

        assert_eq!(response.status_code(), StatusCode::UNAUTHORIZED);
    }

    // ========================================================================
    // Test: Request with only access token (no refresh) is rejected
    // SECURITY: Access tokens should NOT be able to mint new tokens
    // ========================================================================
    #[tokio::test]
    async fn get_token_rejects_access_token_only() {
        let domain = create_test_domain(|d| d.domain = "example.com".to_string());
        let user_id = Uuid::new_v4();
        let user = create_test_end_user(domain.id, |u| {
            u.id = user_id;
            u.is_frozen = false;
        });
        let api_key_raw = "test_api_key_123456789012345678901234";

        let access_token = create_access_token(user_id, domain.id, &domain.domain, api_key_raw);

        let app_state = create_test_app_state(domain.clone(), user, api_key_raw);
        let app = router().with_state(app_state);
        let server = TestServer::new(app).unwrap();

        // Send access token as end_user_access_token cookie, but NO refresh token
        let response = server
            .get("/reauth.example.com/auth/token")
            .add_cookie(axum_extra::extract::cookie::Cookie::new(
                "end_user_access_token",
                access_token,
            ))
            .await;

        // Should be rejected because only refresh token can mint new tokens
        assert_eq!(response.status_code(), StatusCode::UNAUTHORIZED);
    }

    // ========================================================================
    // Test: Frozen user is rejected
    // ========================================================================
    #[tokio::test]
    async fn get_token_rejects_frozen_user() {
        let domain = create_test_domain(|d| d.domain = "example.com".to_string());
        let user_id = Uuid::new_v4();
        let user = create_test_end_user(domain.id, |u| {
            u.id = user_id;
            u.is_frozen = true; // User is frozen
        });
        let api_key_raw = "test_api_key_123456789012345678901234";

        let refresh_token = create_refresh_token(user_id, domain.id, &domain.domain, api_key_raw);

        let app_state = create_test_app_state(domain.clone(), user, api_key_raw);
        let app = router().with_state(app_state);
        let server = TestServer::new(app).unwrap();

        let response = server
            .get("/reauth.example.com/auth/token")
            .add_cookie(axum_extra::extract::cookie::Cookie::new(
                "end_user_refresh_token",
                refresh_token,
            ))
            .await;

        // Should be rejected because user is frozen (ACCOUNT_SUSPENDED)
        assert_eq!(response.status_code(), StatusCode::FORBIDDEN);
    }

    // ========================================================================
    // Test: Token for wrong domain is rejected
    // ========================================================================
    #[tokio::test]
    async fn get_token_rejects_wrong_domain_token() {
        let domain = create_test_domain(|d| d.domain = "example.com".to_string());
        let user_id = Uuid::new_v4();
        let user = create_test_end_user(domain.id, |u| {
            u.id = user_id;
            u.is_frozen = false;
        });
        let api_key_raw = "test_api_key_123456789012345678901234";

        // Create a token for a DIFFERENT domain
        let wrong_domain_id = Uuid::new_v4();
        let refresh_token =
            create_refresh_token(user_id, wrong_domain_id, "other.com", api_key_raw);

        let app_state = create_test_app_state(domain.clone(), user, api_key_raw);
        let app = router().with_state(app_state);
        let server = TestServer::new(app).unwrap();

        let response = server
            .get("/reauth.example.com/auth/token")
            .add_cookie(axum_extra::extract::cookie::Cookie::new(
                "end_user_refresh_token",
                refresh_token,
            ))
            .await;

        // Should be rejected because domain doesn't match
        assert_eq!(response.status_code(), StatusCode::UNAUTHORIZED);
    }

    // ========================================================================
    // Test: Valid request returns Bearer token
    // ========================================================================
    #[tokio::test]
    async fn get_token_returns_valid_bearer_token() {
        let domain = create_test_domain(|d| d.domain = "example.com".to_string());
        let user_id = Uuid::new_v4();
        let user = create_test_end_user(domain.id, |u| {
            u.id = user_id;
            u.is_frozen = false;
        });
        let api_key_raw = "test_api_key_123456789012345678901234";

        let refresh_token = create_refresh_token(user_id, domain.id, &domain.domain, api_key_raw);

        let app_state = create_test_app_state(domain.clone(), user, api_key_raw);
        let app = router().with_state(app_state);
        let server = TestServer::new(app).unwrap();

        let response = server
            .get("/reauth.example.com/auth/token")
            .add_cookie(axum_extra::extract::cookie::Cookie::new(
                "end_user_refresh_token",
                refresh_token,
            ))
            .await;

        assert_eq!(response.status_code(), StatusCode::OK);

        // Parse response body
        let body: serde_json::Value = response.json();
        assert!(body.get("access_token").is_some());
        assert_eq!(body.get("token_type").unwrap(), "Bearer");
        assert!(body.get("expires_in").is_some());

        // Verify the token is a valid JWT (basic structure check)
        let access_token = body.get("access_token").unwrap().as_str().unwrap();
        let parts: Vec<&str> = access_token.split('.').collect();
        assert_eq!(parts.len(), 3, "Token should be a valid JWT with 3 parts");
    }

    // ========================================================================
    // GET /auth/session Tests
    // ========================================================================

    #[tokio::test]
    async fn check_session_returns_invalid_without_tokens() {
        let domain = create_test_domain(|d| d.domain = "example.com".to_string());
        let user = create_test_end_user(domain.id, |_| {});
        let api_key_raw = "test_api_key_123456789012345678901234";

        let app_state = create_test_app_state(domain.clone(), user, api_key_raw);
        let app = router().with_state(app_state);
        let server = TestServer::new(app).unwrap();

        let response = server.get("/reauth.example.com/auth/session").await;

        assert_eq!(response.status_code(), StatusCode::OK);

        let body: serde_json::Value = response.json();
        assert_eq!(body.get("valid").unwrap(), false);
        assert!(body.get("end_user_id").unwrap().is_null());
        assert!(body.get("email").unwrap().is_null());
    }

    #[tokio::test]
    async fn check_session_returns_valid_with_access_token() {
        let domain = create_test_domain(|d| d.domain = "example.com".to_string());
        let user_id = Uuid::new_v4();
        let user = create_test_end_user(domain.id, |u| {
            u.id = user_id;
            u.email = "alice@example.com".to_string();
            u.is_frozen = false;
            u.is_whitelisted = true;
        });
        let api_key_raw = "test_api_key_123456789012345678901234";

        let access_token = create_access_token(user_id, domain.id, &domain.domain, api_key_raw);

        let app_state = create_test_app_state(domain.clone(), user, api_key_raw);
        let app = router().with_state(app_state);
        let server = TestServer::new(app).unwrap();

        let response = server
            .get("/reauth.example.com/auth/session")
            .add_cookie(axum_extra::extract::cookie::Cookie::new(
                "end_user_access_token",
                access_token,
            ))
            .await;

        assert_eq!(response.status_code(), StatusCode::OK);

        let body: serde_json::Value = response.json();
        assert_eq!(body.get("valid").unwrap(), true);
        assert_eq!(
            body.get("end_user_id").unwrap().as_str().unwrap(),
            user_id.to_string()
        );
        assert_eq!(
            body.get("email").unwrap().as_str().unwrap(),
            "alice@example.com"
        );
    }

    #[tokio::test]
    async fn check_session_returns_suspended_for_frozen_user() {
        let domain = create_test_domain(|d| d.domain = "example.com".to_string());
        let user_id = Uuid::new_v4();
        let user = create_test_end_user(domain.id, |u| {
            u.id = user_id;
            u.email = "alice@example.com".to_string();
            u.is_frozen = true;
        });
        let api_key_raw = "test_api_key_123456789012345678901234";

        let access_token = create_access_token(user_id, domain.id, &domain.domain, api_key_raw);

        let app_state = create_test_app_state(domain.clone(), user, api_key_raw);
        let app = router().with_state(app_state);
        let server = TestServer::new(app).unwrap();

        let response = server
            .get("/reauth.example.com/auth/session")
            .add_cookie(axum_extra::extract::cookie::Cookie::new(
                "end_user_access_token",
                access_token,
            ))
            .await;

        assert_eq!(response.status_code(), StatusCode::OK);

        let body: serde_json::Value = response.json();
        assert_eq!(body.get("valid").unwrap(), false);
        assert_eq!(body.get("error_code").unwrap(), "ACCOUNT_SUSPENDED");
    }

    #[tokio::test]
    async fn check_session_returns_invalid_for_wrong_domain_token() {
        let domain = create_test_domain(|d| d.domain = "example.com".to_string());
        let user_id = Uuid::new_v4();
        let user = create_test_end_user(domain.id, |u| {
            u.id = user_id;
            u.is_frozen = false;
        });
        let api_key_raw = "test_api_key_123456789012345678901234";

        // Create token for different domain
        let wrong_domain_id = Uuid::new_v4();
        let access_token = create_access_token(user_id, wrong_domain_id, "other.com", api_key_raw);

        let app_state = create_test_app_state(domain.clone(), user, api_key_raw);
        let app = router().with_state(app_state);
        let server = TestServer::new(app).unwrap();

        let response = server
            .get("/reauth.example.com/auth/session")
            .add_cookie(axum_extra::extract::cookie::Cookie::new(
                "end_user_access_token",
                access_token,
            ))
            .await;

        assert_eq!(response.status_code(), StatusCode::OK);

        let body: serde_json::Value = response.json();
        assert_eq!(body.get("valid").unwrap(), false);
    }

    // ========================================================================
    // POST /auth/refresh Tests
    // ========================================================================

    #[tokio::test]
    async fn refresh_token_rejects_request_without_refresh_cookie() {
        let domain = create_test_domain(|d| d.domain = "example.com".to_string());
        let user = create_test_end_user(domain.id, |_| {});
        let api_key_raw = "test_api_key_123456789012345678901234";

        let app_state = create_test_app_state(domain.clone(), user, api_key_raw);
        let app = router().with_state(app_state);
        let server = TestServer::new(app).unwrap();

        let response = server.post("/reauth.example.com/auth/refresh").await;

        assert_eq!(response.status_code(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn refresh_token_rejects_frozen_user() {
        let domain = create_test_domain(|d| d.domain = "example.com".to_string());
        let user_id = Uuid::new_v4();
        let user = create_test_end_user(domain.id, |u| {
            u.id = user_id;
            u.is_frozen = true;
        });
        let api_key_raw = "test_api_key_123456789012345678901234";

        let refresh_token = create_refresh_token(user_id, domain.id, &domain.domain, api_key_raw);

        let app_state = create_test_app_state(domain.clone(), user, api_key_raw);
        let app = router().with_state(app_state);
        let server = TestServer::new(app).unwrap();

        let response = server
            .post("/reauth.example.com/auth/refresh")
            .add_cookie(axum_extra::extract::cookie::Cookie::new(
                "end_user_refresh_token",
                refresh_token,
            ))
            .await;

        assert_eq!(response.status_code(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn refresh_token_rejects_wrong_domain_token() {
        let domain = create_test_domain(|d| d.domain = "example.com".to_string());
        let user_id = Uuid::new_v4();
        let user = create_test_end_user(domain.id, |u| {
            u.id = user_id;
            u.is_frozen = false;
        });
        let api_key_raw = "test_api_key_123456789012345678901234";

        let wrong_domain_id = Uuid::new_v4();
        let refresh_token =
            create_refresh_token(user_id, wrong_domain_id, "other.com", api_key_raw);

        let app_state = create_test_app_state(domain.clone(), user, api_key_raw);
        let app = router().with_state(app_state);
        let server = TestServer::new(app).unwrap();

        let response = server
            .post("/reauth.example.com/auth/refresh")
            .add_cookie(axum_extra::extract::cookie::Cookie::new(
                "end_user_refresh_token",
                refresh_token,
            ))
            .await;

        assert_eq!(response.status_code(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn refresh_token_sets_new_access_cookie() {
        let domain = create_test_domain(|d| d.domain = "example.com".to_string());
        let user_id = Uuid::new_v4();
        let user = create_test_end_user(domain.id, |u| {
            u.id = user_id;
            u.is_frozen = false;
        });
        let api_key_raw = "test_api_key_123456789012345678901234";

        let refresh_token = create_refresh_token(user_id, domain.id, &domain.domain, api_key_raw);

        let app_state = create_test_app_state(domain.clone(), user, api_key_raw);
        let app = router().with_state(app_state);
        let server = TestServer::new(app).unwrap();

        let response = server
            .post("/reauth.example.com/auth/refresh")
            .add_cookie(axum_extra::extract::cookie::Cookie::new(
                "end_user_refresh_token",
                refresh_token,
            ))
            .await;

        assert_eq!(response.status_code(), StatusCode::OK);

        // Check that a set-cookie header was returned
        let set_cookie = response
            .headers()
            .get_all("set-cookie")
            .iter()
            .map(|v| v.to_str().unwrap_or(""))
            .collect::<Vec<_>>();

        let has_access_token_cookie = set_cookie
            .iter()
            .any(|c| c.starts_with("end_user_access_token="));
        assert!(
            has_access_token_cookie,
            "Response should set end_user_access_token cookie"
        );
    }

    // ========================================================================
    // POST /auth/logout Tests
    // ========================================================================

    #[tokio::test]
    async fn logout_clears_auth_cookies() {
        let domain = create_test_domain(|d| d.domain = "example.com".to_string());
        let user = create_test_end_user(domain.id, |_| {});
        let api_key_raw = "test_api_key_123456789012345678901234";

        let app_state = create_test_app_state(domain.clone(), user, api_key_raw);
        let app = router().with_state(app_state);
        let server = TestServer::new(app).unwrap();

        let response = server.post("/reauth.example.com/auth/logout").await;

        assert_eq!(response.status_code(), StatusCode::OK);

        // Verify cookies are cleared (max-age=0)
        let set_cookies = response
            .headers()
            .get_all("set-cookie")
            .iter()
            .map(|v| v.to_str().unwrap_or(""))
            .collect::<Vec<_>>();

        let has_cleared_access = set_cookies.iter().any(|c| {
            c.starts_with("end_user_access_token=")
                && (c.contains("Max-Age=0") || c.contains("max-age=0"))
        });
        let has_cleared_refresh = set_cookies.iter().any(|c| {
            c.starts_with("end_user_refresh_token=")
                && (c.contains("Max-Age=0") || c.contains("max-age=0"))
        });

        assert!(has_cleared_access, "Access token cookie should be cleared");
        assert!(
            has_cleared_refresh,
            "Refresh token cookie should be cleared"
        );
    }

    // ========================================================================
    // DELETE /auth/account Tests
    // ========================================================================

    #[tokio::test]
    async fn delete_account_rejects_unauthenticated_request() {
        let domain = create_test_domain(|d| d.domain = "example.com".to_string());
        let user = create_test_end_user(domain.id, |_| {});
        let api_key_raw = "test_api_key_123456789012345678901234";

        let app_state = create_test_app_state(domain.clone(), user, api_key_raw);
        let app = router().with_state(app_state);
        let server = TestServer::new(app).unwrap();

        let response = server.delete("/reauth.example.com/auth/account").await;

        assert_eq!(response.status_code(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn delete_account_rejects_wrong_domain_token() {
        let domain = create_test_domain(|d| d.domain = "example.com".to_string());
        let user_id = Uuid::new_v4();
        let user = create_test_end_user(domain.id, |u| {
            u.id = user_id;
            u.is_frozen = false;
        });
        let api_key_raw = "test_api_key_123456789012345678901234";

        let wrong_domain_id = Uuid::new_v4();
        let access_token = create_access_token(user_id, wrong_domain_id, "other.com", api_key_raw);

        let app_state = create_test_app_state(domain.clone(), user, api_key_raw);
        let app = router().with_state(app_state);
        let server = TestServer::new(app).unwrap();

        let response = server
            .delete("/reauth.example.com/auth/account")
            .add_cookie(axum_extra::extract::cookie::Cookie::new(
                "end_user_access_token",
                access_token,
            ))
            .await;

        assert_eq!(response.status_code(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn delete_account_succeeds_with_valid_token() {
        let domain = create_test_domain(|d| d.domain = "example.com".to_string());
        let user_id = Uuid::new_v4();
        let user = create_test_end_user(domain.id, |u| {
            u.id = user_id;
            u.is_frozen = false;
        });
        let api_key_raw = "test_api_key_123456789012345678901234";

        let access_token = create_access_token(user_id, domain.id, &domain.domain, api_key_raw);

        let app_state = create_test_app_state(domain.clone(), user, api_key_raw);
        let app = router().with_state(app_state);
        let server = TestServer::new(app).unwrap();

        let response = server
            .delete("/reauth.example.com/auth/account")
            .add_cookie(axum_extra::extract::cookie::Cookie::new(
                "end_user_access_token",
                access_token,
            ))
            .await;

        assert_eq!(response.status_code(), StatusCode::OK);

        // Verify cookies are cleared
        let set_cookies = response
            .headers()
            .get_all("set-cookie")
            .iter()
            .map(|v| v.to_str().unwrap_or(""))
            .collect::<Vec<_>>();

        let has_cleared_access = set_cookies.iter().any(|c| {
            c.starts_with("end_user_access_token=")
                && (c.contains("Max-Age=0") || c.contains("max-age=0"))
        });
        assert!(
            has_cleared_access,
            "Access token cookie should be cleared after account deletion"
        );
    }

    #[tokio::test]
    async fn delete_account_works_with_refresh_token() {
        let domain = create_test_domain(|d| d.domain = "example.com".to_string());
        let user_id = Uuid::new_v4();
        let user = create_test_end_user(domain.id, |u| {
            u.id = user_id;
            u.is_frozen = false;
        });
        let api_key_raw = "test_api_key_123456789012345678901234";

        let refresh_token = create_refresh_token(user_id, domain.id, &domain.domain, api_key_raw);

        let app_state = create_test_app_state(domain.clone(), user, api_key_raw);
        let app = router().with_state(app_state);
        let server = TestServer::new(app).unwrap();

        let response = server
            .delete("/reauth.example.com/auth/account")
            .add_cookie(axum_extra::extract::cookie::Cookie::new(
                "end_user_refresh_token",
                refresh_token,
            ))
            .await;

        assert_eq!(response.status_code(), StatusCode::OK);
    }
}
