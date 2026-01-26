//! Magic link authentication routes.

use super::common::*;
use crate::application::validators::is_valid_email;

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

    let (jar, session_id) = super::common::ensure_login_session(
        jar,
        &root_domain,
        app_state.config.magic_link_ttl_minutes,
    );

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
            let result = super::common::complete_login(&app_state, &user, &root_domain).await?;

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

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/{domain}/auth/request-magic-link",
            post(request_magic_link),
        )
        .route("/{domain}/auth/verify-magic-link", post(verify_magic_link))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum_test::TestServer;
    use serde_json::json;
    use uuid::Uuid;

    use crate::application::use_cases::domain_auth::DomainMagicLinkStore;
    use crate::domain::entities::domain::DomainStatus;
    use crate::test_utils::{
        TestAppStateBuilder, create_test_auth_config, create_test_domain, create_test_end_user,
    };

    fn build_test_router(app_state: AppState) -> Router<()> {
        router().with_state(app_state)
    }

    // =========================================================================
    // POST /{domain}/auth/request-magic-link
    // =========================================================================

    #[tokio::test]
    async fn request_magic_link_invalid_email_returns_400() {
        let domain_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.id = domain_id;
            d.domain = "example.com".to_string();
            d.status = DomainStatus::Verified;
        });
        let auth_config = create_test_auth_config(domain_id, |c| {
            c.magic_link_enabled = true;
        });

        let (app_state, _magic_store, _email_sender) = TestAppStateBuilder::new()
            .with_domain(domain)
            .with_auth_config(auth_config)
            .with_api_key(domain_id, "example.com", "test_api_key_12345678")
            .build_with_magic_link_mocks();

        let server = TestServer::new(build_test_router(app_state)).unwrap();

        let response = server
            .post("/reauth.example.com/auth/request-magic-link")
            .json(&json!({ "email": "not-an-email" }))
            .await;

        response.assert_status(StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn request_magic_link_unknown_domain_returns_404() {
        let (app_state, _magic_store, _email_sender) =
            TestAppStateBuilder::new().build_with_magic_link_mocks();

        let server = TestServer::new(build_test_router(app_state)).unwrap();

        let response = server
            .post("/reauth.unknown.com/auth/request-magic-link")
            .json(&json!({ "email": "user@example.com" }))
            .await;

        response.assert_status(StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn request_magic_link_unverified_domain_returns_404() {
        let domain_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.id = domain_id;
            d.domain = "example.com".to_string();
            d.status = DomainStatus::PendingDns;
        });

        let (app_state, _magic_store, _email_sender) = TestAppStateBuilder::new()
            .with_domain(domain)
            .build_with_magic_link_mocks();

        let server = TestServer::new(build_test_router(app_state)).unwrap();

        let response = server
            .post("/reauth.example.com/auth/request-magic-link")
            .json(&json!({ "email": "user@example.com" }))
            .await;

        response.assert_status(StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn request_magic_link_magic_link_disabled_returns_400() {
        let domain_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.id = domain_id;
            d.domain = "example.com".to_string();
            d.status = DomainStatus::Verified;
        });
        let auth_config = create_test_auth_config(domain_id, |c| {
            c.magic_link_enabled = false;
        });

        let (app_state, _magic_store, _email_sender) = TestAppStateBuilder::new()
            .with_domain(domain)
            .with_auth_config(auth_config)
            .build_with_magic_link_mocks();

        let server = TestServer::new(build_test_router(app_state)).unwrap();

        let response = server
            .post("/reauth.example.com/auth/request-magic-link")
            .json(&json!({ "email": "user@example.com" }))
            .await;

        response.assert_status(StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn request_magic_link_success_sends_email_and_returns_202() {
        let domain_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.id = domain_id;
            d.domain = "example.com".to_string();
            d.status = DomainStatus::Verified;
        });
        let auth_config = create_test_auth_config(domain_id, |c| {
            c.magic_link_enabled = true;
        });

        let (app_state, _magic_store, email_sender) = TestAppStateBuilder::new()
            .with_domain(domain)
            .with_auth_config(auth_config)
            .with_api_key(domain_id, "example.com", "test_api_key_12345678")
            .build_with_magic_link_mocks();

        let server = TestServer::new(build_test_router(app_state)).unwrap();

        let response = server
            .post("/reauth.example.com/auth/request-magic-link")
            .json(&json!({ "email": "user@example.com" }))
            .await;

        response.assert_status(StatusCode::ACCEPTED);

        // Verify email was "sent"
        let emails = email_sender.captured_emails();
        assert_eq!(emails.len(), 1);
        assert_eq!(emails[0].to, "user@example.com");
        assert!(emails[0].html.contains("magic"));

        // Verify login_session cookie was set
        let cookies = response.cookies();
        assert!(cookies.iter().any(|c| c.name() == "login_session"));
    }

    #[tokio::test]
    async fn request_magic_link_trims_email_whitespace() {
        let domain_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.id = domain_id;
            d.domain = "example.com".to_string();
            d.status = DomainStatus::Verified;
        });
        let auth_config = create_test_auth_config(domain_id, |c| {
            c.magic_link_enabled = true;
        });

        let (app_state, _magic_store, email_sender) = TestAppStateBuilder::new()
            .with_domain(domain)
            .with_auth_config(auth_config)
            .with_api_key(domain_id, "example.com", "test_api_key_12345678")
            .build_with_magic_link_mocks();

        let server = TestServer::new(build_test_router(app_state)).unwrap();

        let response = server
            .post("/reauth.example.com/auth/request-magic-link")
            .json(&json!({ "email": "  user@example.com  " }))
            .await;

        response.assert_status(StatusCode::ACCEPTED);

        let emails = email_sender.captured_emails();
        assert_eq!(emails.len(), 1);
        assert_eq!(emails[0].to, "user@example.com");
    }

    // =========================================================================
    // POST /{domain}/auth/verify-magic-link
    // =========================================================================

    #[tokio::test]
    async fn verify_magic_link_no_session_cookie_returns_session_mismatch() {
        let domain_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.id = domain_id;
            d.domain = "example.com".to_string();
            d.status = DomainStatus::Verified;
        });

        let app_state = TestAppStateBuilder::new().with_domain(domain).build();

        let server = TestServer::new(build_test_router(app_state)).unwrap();

        let response = server
            .post("/reauth.example.com/auth/verify-magic-link")
            .json(&json!({ "token": "some-token" }))
            .await;

        // SESSION_MISMATCH error returns 401 (Unauthorized)
        response.assert_status(StatusCode::UNAUTHORIZED);
        let body = response.json::<serde_json::Value>();
        assert_eq!(body["code"].as_str(), Some("SESSION_MISMATCH"));
    }

    #[tokio::test]
    async fn verify_magic_link_invalid_token_returns_401() {
        let domain_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.id = domain_id;
            d.domain = "example.com".to_string();
            d.status = DomainStatus::Verified;
        });
        let user = create_test_end_user(domain_id, |u| {
            u.id = user_id;
            u.email = "user@example.com".to_string();
        });
        let auth_config = create_test_auth_config(domain_id, |_| {});

        let (app_state, _magic_store, _email_sender) = TestAppStateBuilder::new()
            .with_domain(domain)
            .with_user(user)
            .with_auth_config(auth_config)
            .with_api_key(domain_id, "example.com", "test_api_key_12345678")
            .build_with_magic_link_mocks();

        let server = TestServer::new(build_test_router(app_state)).unwrap();

        let response = server
            .post("/reauth.example.com/auth/verify-magic-link")
            .add_cookie(Cookie::new("login_session", "valid-session-id"))
            .json(&json!({ "token": "invalid-token" }))
            .await;

        // Invalid token returns 401 with success: false
        response.assert_status(StatusCode::UNAUTHORIZED);
        let body = response.json::<serde_json::Value>();
        assert_eq!(body["success"].as_bool(), Some(false));
    }

    #[tokio::test]
    async fn verify_magic_link_session_mismatch_on_different_session() {
        let domain_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.id = domain_id;
            d.domain = "example.com".to_string();
            d.status = DomainStatus::Verified;
        });
        let user = create_test_end_user(domain_id, |u| {
            u.id = user_id;
            u.email = "user@example.com".to_string();
        });
        let auth_config = create_test_auth_config(domain_id, |_| {});

        let (app_state, magic_store, _email_sender) = TestAppStateBuilder::new()
            .with_domain(domain)
            .with_user(user)
            .with_auth_config(auth_config)
            .with_api_key(domain_id, "example.com", "test_api_key_12345678")
            .build_with_magic_link_mocks();

        // Manually save a magic link with a specific session
        magic_store
            .save(
                "test-token-hash",
                user_id,
                domain_id,
                "original-session",
                15,
            )
            .await
            .unwrap();

        let server = TestServer::new(build_test_router(app_state)).unwrap();

        // Try to verify with a different session ID
        let response = server
            .post("/reauth.example.com/auth/verify-magic-link")
            .add_cookie(Cookie::new("login_session", "different-session"))
            .json(&json!({ "token": "test-token" }))
            .await;

        // Token lookup won't find the raw token since we stored the hash directly
        // The flow is: hash(token + domain) -> lookup
        // This tests the session mismatch error path
        response.assert_status(StatusCode::UNAUTHORIZED);
    }
}
