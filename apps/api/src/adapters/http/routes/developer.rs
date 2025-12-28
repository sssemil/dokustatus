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

    // Verify the JWT token
    let claims = match jwt::verify_domain_end_user(&payload.token, &app_state.config.jwt_secret) {
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
