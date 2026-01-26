use axum::{
    Extension, Json, Router,
    extract::{Path, State},
    response::IntoResponse,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    adapters::http::{app_state::AppState, middleware::ApiKeyContext},
    app_error::{AppError, AppResult},
    application::jwt,
};

/// Returns a router for developer API endpoints.
/// Note: The api_key_auth middleware is applied in mod.rs when nesting this router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/{domain}/auth/verify-token", post(verify_token))
        .route("/{domain}/users/{user_id}", get(get_user))
}

// ============================================================================
// Request/Response Types
// ============================================================================

#[derive(Deserialize)]
struct VerifyTokenPayload {
    token: String,
}

#[derive(Serialize)]
struct VerifyTokenResponse {
    valid: bool,
    user: Option<UserDetailsResponse>,
}

#[derive(Serialize)]
struct UserDetailsResponse {
    id: String,
    email: String,
    roles: Vec<String>,
    email_verified_at: Option<chrono::NaiveDateTime>,
    last_login_at: Option<chrono::NaiveDateTime>,
    is_frozen: bool,
    is_whitelisted: bool,
    created_at: Option<chrono::NaiveDateTime>,
}

// ============================================================================
// Handlers
// ============================================================================

/// POST /api/developer/{domain}/auth/verify-token
/// Verifies a user's JWT token and returns user details if valid.
async fn verify_token(
    State(app_state): State<AppState>,
    Extension(api_key_ctx): Extension<ApiKeyContext>,
    Path(domain): Path<String>,
    Json(payload): Json<VerifyTokenPayload>,
) -> AppResult<impl IntoResponse> {
    // Verify the domain in the path matches the API key's domain
    if domain != api_key_ctx.domain_name {
        return Err(AppError::InvalidApiKey);
    }

    // Get all active API keys for this domain (for multi-key verification)
    let keys = app_state
        .api_key_use_cases
        .get_all_active_keys_for_domain(api_key_ctx.domain_id)
        .await?;

    if keys.is_empty() {
        return Err(AppError::NoApiKeyConfigured);
    }

    // Verify the JWT token using derived secrets
    let claims = match jwt::verify_domain_end_user_multi(&payload.token, &keys) {
        Ok(claims) => claims,
        Err(_) => {
            return Ok(Json(VerifyTokenResponse {
                valid: false,
                user: None,
            }));
        }
    };

    // Check if the token's domain matches
    if claims.domain != domain {
        return Ok(Json(VerifyTokenResponse {
            valid: false,
            user: None,
        }));
    }

    // Parse user ID
    let user_id = match Uuid::parse_str(&claims.sub) {
        Ok(id) => id,
        Err(_) => {
            return Ok(Json(VerifyTokenResponse {
                valid: false,
                user: None,
            }));
        }
    };

    // Get full user details from database
    match app_state
        .api_key_use_cases
        .get_user_by_id(api_key_ctx.domain_id, user_id)
        .await
    {
        Ok(user) => {
            // Check if user is frozen
            if user.is_frozen {
                return Ok(Json(VerifyTokenResponse {
                    valid: false,
                    user: Some(UserDetailsResponse {
                        id: user.id.to_string(),
                        email: user.email,
                        roles: user.roles,
                        email_verified_at: user.email_verified_at,
                        last_login_at: user.last_login_at,
                        is_frozen: user.is_frozen,
                        is_whitelisted: user.is_whitelisted,
                        created_at: user.created_at,
                    }),
                }));
            }

            Ok(Json(VerifyTokenResponse {
                valid: true,
                user: Some(UserDetailsResponse {
                    id: user.id.to_string(),
                    email: user.email,
                    roles: user.roles,
                    email_verified_at: user.email_verified_at,
                    last_login_at: user.last_login_at,
                    is_frozen: user.is_frozen,
                    is_whitelisted: user.is_whitelisted,
                    created_at: user.created_at,
                }),
            }))
        }
        Err(_) => Ok(Json(VerifyTokenResponse {
            valid: false,
            user: None,
        })),
    }
}

/// GET /api/developer/{domain}/users/{user_id}
/// Returns user details by ID.
async fn get_user(
    State(app_state): State<AppState>,
    Extension(api_key_ctx): Extension<ApiKeyContext>,
    Path((domain, user_id)): Path<(String, Uuid)>,
) -> AppResult<impl IntoResponse> {
    // Verify the domain in the path matches the API key's domain
    if domain != api_key_ctx.domain_name {
        return Err(AppError::InvalidApiKey);
    }

    let user = app_state
        .api_key_use_cases
        .get_user_by_id(api_key_ctx.domain_id, user_id)
        .await?;

    Ok(Json(UserDetailsResponse {
        id: user.id.to_string(),
        email: user.email,
        roles: user.roles,
        email_verified_at: user.email_verified_at,
        last_login_at: user.last_login_at,
        is_frozen: user.is_frozen,
        is_whitelisted: user.is_whitelisted,
        created_at: user.created_at,
    }))
}

#[cfg(test)]
mod tests {
    use axum::{http::StatusCode, middleware};
    use axum_test::TestServer;
    use uuid::Uuid;

    use crate::{
        adapters::http::middleware::api_key_auth,
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
    ) -> AppState {
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
            vec!["user".to_string()],
            SubscriptionClaims::none(),
            &api_key,
            time::Duration::hours(24),
        )
        .expect("Failed to issue test token")
    }

    fn build_test_router(app_state: AppState) -> Router<()> {
        router()
            .layer(middleware::from_fn_with_state(
                app_state.clone(),
                api_key_auth,
            ))
            .with_state(app_state)
    }

    // ========================================================================
    // POST /auth/verify-token Tests
    // ========================================================================

    #[tokio::test]
    async fn verify_token_rejects_request_without_api_key() {
        let domain = create_test_domain(|d| d.domain = "example.com".to_string());
        let user = create_test_end_user(domain.id, |_| {});
        let api_key_raw = "sk_test_123456789012345678901234567890";

        let app_state = create_test_app_state(domain.clone(), user, api_key_raw);
        let app = build_test_router(app_state);
        let server = TestServer::new(app).unwrap();

        let response = server
            .post("/example.com/auth/verify-token")
            .json(&serde_json::json!({
                "token": "some.jwt.token"
            }))
            .await;

        assert_eq!(response.status_code(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn verify_token_rejects_invalid_api_key() {
        let domain = create_test_domain(|d| d.domain = "example.com".to_string());
        let user = create_test_end_user(domain.id, |_| {});
        let api_key_raw = "sk_test_123456789012345678901234567890";

        let app_state = create_test_app_state(domain.clone(), user, api_key_raw);
        let app = build_test_router(app_state);
        let server = TestServer::new(app).unwrap();

        let response = server
            .post("/example.com/auth/verify-token")
            .add_header("Authorization", "Bearer sk_test_wrong_key")
            .json(&serde_json::json!({
                "token": "some.jwt.token"
            }))
            .await;

        assert_eq!(response.status_code(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn verify_token_returns_invalid_for_bad_jwt() {
        let domain = create_test_domain(|d| d.domain = "example.com".to_string());
        let user = create_test_end_user(domain.id, |_| {});
        let api_key_raw = "sk_test_123456789012345678901234567890";

        let app_state = create_test_app_state(domain.clone(), user, api_key_raw);
        let app = build_test_router(app_state);
        let server = TestServer::new(app).unwrap();

        let response = server
            .post("/example.com/auth/verify-token")
            .add_header("Authorization", format!("Bearer {}", api_key_raw))
            .json(&serde_json::json!({
                "token": "invalid.jwt.token"
            }))
            .await;

        assert_eq!(response.status_code(), StatusCode::OK);

        let body: serde_json::Value = response.json();
        assert_eq!(body.get("valid").unwrap(), false);
        assert!(body.get("user").unwrap().is_null());
    }

    #[tokio::test]
    async fn verify_token_returns_valid_for_good_jwt() {
        let domain = create_test_domain(|d| d.domain = "example.com".to_string());
        let user_id = Uuid::new_v4();
        let user = create_test_end_user(domain.id, |u| {
            u.id = user_id;
            u.email = "alice@example.com".to_string();
            u.is_frozen = false;
        });
        let api_key_raw = "sk_test_123456789012345678901234567890";

        let access_token = create_access_token(user_id, domain.id, &domain.domain, api_key_raw);

        let app_state = create_test_app_state(domain.clone(), user, api_key_raw);
        let app = build_test_router(app_state);
        let server = TestServer::new(app).unwrap();

        let response = server
            .post("/example.com/auth/verify-token")
            .add_header("Authorization", format!("Bearer {}", api_key_raw))
            .json(&serde_json::json!({
                "token": access_token
            }))
            .await;

        assert_eq!(response.status_code(), StatusCode::OK);

        let body: serde_json::Value = response.json();
        assert_eq!(body.get("valid").unwrap(), true);
        assert!(body.get("user").is_some());

        let user_data = body.get("user").unwrap();
        assert_eq!(
            user_data.get("id").unwrap().as_str().unwrap(),
            user_id.to_string()
        );
        assert_eq!(user_data.get("email").unwrap(), "alice@example.com");
    }

    #[tokio::test]
    async fn verify_token_returns_invalid_for_frozen_user() {
        let domain = create_test_domain(|d| d.domain = "example.com".to_string());
        let user_id = Uuid::new_v4();
        let user = create_test_end_user(domain.id, |u| {
            u.id = user_id;
            u.email = "alice@example.com".to_string();
            u.is_frozen = true;
        });
        let api_key_raw = "sk_test_123456789012345678901234567890";

        let access_token = create_access_token(user_id, domain.id, &domain.domain, api_key_raw);

        let app_state = create_test_app_state(domain.clone(), user, api_key_raw);
        let app = build_test_router(app_state);
        let server = TestServer::new(app).unwrap();

        let response = server
            .post("/example.com/auth/verify-token")
            .add_header("Authorization", format!("Bearer {}", api_key_raw))
            .json(&serde_json::json!({
                "token": access_token
            }))
            .await;

        assert_eq!(response.status_code(), StatusCode::OK);

        let body: serde_json::Value = response.json();
        assert_eq!(body.get("valid").unwrap(), false);
        // User details are still returned for frozen users
        let user_data = body.get("user").unwrap();
        assert_eq!(user_data.get("is_frozen").unwrap(), true);
    }

    #[tokio::test]
    async fn verify_token_rejects_domain_mismatch() {
        let domain = create_test_domain(|d| d.domain = "example.com".to_string());
        let user_id = Uuid::new_v4();
        let user = create_test_end_user(domain.id, |u| {
            u.id = user_id;
        });
        let api_key_raw = "sk_test_123456789012345678901234567890";

        let access_token = create_access_token(user_id, domain.id, &domain.domain, api_key_raw);

        let app_state = create_test_app_state(domain.clone(), user, api_key_raw);
        let app = build_test_router(app_state);
        let server = TestServer::new(app).unwrap();

        // Request to wrong domain
        let response = server
            .post("/other.com/auth/verify-token")
            .add_header("Authorization", format!("Bearer {}", api_key_raw))
            .json(&serde_json::json!({
                "token": access_token
            }))
            .await;

        // API key is for example.com, but path is other.com
        assert_eq!(response.status_code(), StatusCode::UNAUTHORIZED);
    }

    // ========================================================================
    // GET /users/{user_id} Tests
    // ========================================================================

    #[tokio::test]
    async fn get_user_rejects_request_without_api_key() {
        let domain = create_test_domain(|d| d.domain = "example.com".to_string());
        let user_id = Uuid::new_v4();
        let user = create_test_end_user(domain.id, |u| {
            u.id = user_id;
        });
        let api_key_raw = "sk_test_123456789012345678901234567890";

        let app_state = create_test_app_state(domain.clone(), user, api_key_raw);
        let app = build_test_router(app_state);
        let server = TestServer::new(app).unwrap();

        let response = server.get(&format!("/example.com/users/{}", user_id)).await;

        assert_eq!(response.status_code(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn get_user_returns_user_details() {
        let domain = create_test_domain(|d| d.domain = "example.com".to_string());
        let user_id = Uuid::new_v4();
        let user = create_test_end_user(domain.id, |u| {
            u.id = user_id;
            u.email = "alice@example.com".to_string();
            u.roles = vec!["admin".to_string(), "user".to_string()];
            u.is_whitelisted = true;
        });
        let api_key_raw = "sk_test_123456789012345678901234567890";

        let app_state = create_test_app_state(domain.clone(), user, api_key_raw);
        let app = build_test_router(app_state);
        let server = TestServer::new(app).unwrap();

        let response = server
            .get(&format!("/example.com/users/{}", user_id))
            .add_header("Authorization", format!("Bearer {}", api_key_raw))
            .await;

        assert_eq!(response.status_code(), StatusCode::OK);

        let body: serde_json::Value = response.json();
        assert_eq!(
            body.get("id").unwrap().as_str().unwrap(),
            user_id.to_string()
        );
        assert_eq!(body.get("email").unwrap(), "alice@example.com");
        assert_eq!(body.get("is_whitelisted").unwrap(), true);

        let roles = body.get("roles").unwrap().as_array().unwrap();
        assert!(roles.contains(&serde_json::json!("admin")));
        assert!(roles.contains(&serde_json::json!("user")));
    }

    #[tokio::test]
    async fn get_user_returns_not_found_for_unknown_user() {
        let domain = create_test_domain(|d| d.domain = "example.com".to_string());
        let user_id = Uuid::new_v4();
        let user = create_test_end_user(domain.id, |u| {
            u.id = user_id;
        });
        let api_key_raw = "sk_test_123456789012345678901234567890";

        let app_state = create_test_app_state(domain.clone(), user, api_key_raw);
        let app = build_test_router(app_state);
        let server = TestServer::new(app).unwrap();

        let unknown_user_id = Uuid::new_v4();
        let response = server
            .get(&format!("/example.com/users/{}", unknown_user_id))
            .add_header("Authorization", format!("Bearer {}", api_key_raw))
            .await;

        assert_eq!(response.status_code(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn get_user_rejects_domain_mismatch() {
        let domain = create_test_domain(|d| d.domain = "example.com".to_string());
        let user_id = Uuid::new_v4();
        let user = create_test_end_user(domain.id, |u| {
            u.id = user_id;
        });
        let api_key_raw = "sk_test_123456789012345678901234567890";

        let app_state = create_test_app_state(domain.clone(), user, api_key_raw);
        let app = build_test_router(app_state);
        let server = TestServer::new(app).unwrap();

        // Request to wrong domain path
        let response = server
            .get(&format!("/other.com/users/{}", user_id))
            .add_header("Authorization", format!("Bearer {}", api_key_raw))
            .await;

        assert_eq!(response.status_code(), StatusCode::UNAUTHORIZED);
    }
}
