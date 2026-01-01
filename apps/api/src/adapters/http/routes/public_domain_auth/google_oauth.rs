//! Google OAuth authentication routes.

use super::common::*;
use crate::application::use_cases::domain_auth::{GoogleLoginResult, MarkStateResult};
use crate::infra::http_client;

// ============================================================================
// Types
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

// ============================================================================
// OAuth Exchange Error Types
// ============================================================================

/// Typed OAuth exchange error with source information
#[derive(Debug)]
enum OAuthExchangeError {
    /// Network error during Google API call
    Network { message: String },
    /// Google API returned an error response
    GoogleApi {
        status: u16,
        error_code: Option<String>,
        message: String,
    },
    /// Token parsing or validation failed
    TokenValidation { message: String },
    /// User data validation failed (e.g., email not verified)
    UserValidation { message: String },
    /// Database error during user creation
    Database { message: String },
    /// Redis error during state management
    #[allow(dead_code)]
    Redis { message: String },
}

impl OAuthExchangeError {
    /// Classify error as retryable or terminal based on error type and codes
    fn is_retryable(&self) -> bool {
        match self {
            OAuthExchangeError::Network { .. } => true,
            OAuthExchangeError::GoogleApi { status, .. } => *status >= 500,
            OAuthExchangeError::TokenValidation { .. } => false,
            OAuthExchangeError::UserValidation { .. } => false,
            OAuthExchangeError::Database { .. } => true,
            OAuthExchangeError::Redis { .. } => true,
        }
    }
}

impl From<OAuthExchangeError> for AppError {
    fn from(value: OAuthExchangeError) -> Self {
        match value {
            OAuthExchangeError::Network { message } => {
                AppError::Internal(format!("Network error during OAuth: {message}"))
            }
            OAuthExchangeError::GoogleApi {
                status,
                error_code,
                message,
            } => {
                if let Some(code) = error_code {
                    if code == "invalid_grant" {
                        return AppError::InvalidInput(
                            "Authorization code expired or already used".into(),
                        );
                    }
                }
                if status >= 500 {
                    AppError::Internal(format!("Google API error ({status}): {message}"))
                } else {
                    AppError::InvalidInput("Failed to authenticate with Google".into())
                }
            }
            OAuthExchangeError::TokenValidation { message } => {
                AppError::Internal(format!("Token validation failed: {message}"))
            }
            OAuthExchangeError::UserValidation { message } => AppError::InvalidInput(message),
            OAuthExchangeError::Database { message } => {
                AppError::Internal(format!("Database error: {message}"))
            }
            OAuthExchangeError::Redis { message } => {
                AppError::Internal(format!("Redis error: {message}"))
            }
        }
    }
}

// ============================================================================
// Handlers
// ============================================================================

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
    const RETRY_WINDOW_SECS: i64 = 90;

    // Mark state as in-use - the domain comes FROM the state, not the URL
    // This is because Google OAuth uses a single callback URL (reauth.reauth.dev)
    // but the OAuth flow could have been initiated from any domain
    let state_data = match app_state
        .domain_auth_use_cases
        .mark_google_oauth_state_in_use(&payload.state, RETRY_WINDOW_SECS)
        .await?
    {
        MarkStateResult::Success(data) => data,
        MarkStateResult::NotFound => {
            return Err(AppError::InvalidInput(
                "Invalid or expired OAuth state".into(),
            ));
        }
        MarkStateResult::RetryWindowExpired => {
            let _ = app_state
                .domain_auth_use_cases
                .abort_google_oauth_state(&payload.state)
                .await;
            return Err(AppError::OAuthRetryExpired);
        }
    };

    // Use the domain from the state (this is the domain that initiated the OAuth flow)
    let root_domain = &state_data.domain;

    // Get domain
    let domain = match app_state
        .domain_auth_use_cases
        .get_domain_by_name(root_domain)
        .await
    {
        Ok(Some(domain)) => domain,
        Ok(None) => {
            let _ = app_state
                .domain_auth_use_cases
                .abort_google_oauth_state(&payload.state)
                .await;
            return Err(AppError::NotFound);
        }
        Err(e) => {
            if should_abort_state(&e) {
                let _ = app_state
                    .domain_auth_use_cases
                    .abort_google_oauth_state(&payload.state)
                    .await;
            }
            return Err(e);
        }
    };

    // Verify Google OAuth is still enabled
    match app_state
        .domain_auth_use_cases
        .is_google_oauth_enabled(domain.id)
        .await
    {
        Ok(true) => {}
        Ok(false) => {
            let _ = app_state
                .domain_auth_use_cases
                .abort_google_oauth_state(&payload.state)
                .await;
            return Err(AppError::InvalidInput(
                "Google OAuth is not enabled for this domain".into(),
            ));
        }
        Err(e) => {
            if should_abort_state(&e) {
                let _ = app_state
                    .domain_auth_use_cases
                    .abort_google_oauth_state(&payload.state)
                    .await;
            }
            return Err(e);
        }
    }

    // Get OAuth credentials
    let (client_id, client_secret, is_fallback) = match app_state
        .domain_auth_use_cases
        .get_google_oauth_config(domain.id)
        .await
    {
        Ok(result) => result,
        Err(e) => {
            if should_abort_state(&e) {
                let _ = app_state
                    .domain_auth_use_cases
                    .abort_google_oauth_state(&payload.state)
                    .await;
            }
            return Err(e);
        }
    };

    // Exchange code with Google
    // Must use same redirect_uri as google_start (fallback vs custom)
    let redirect_uri = if is_fallback {
        let main_domain = &app_state.config.main_domain;
        format!("https://reauth.{}/callback/google", main_domain)
    } else {
        format!("https://reauth.{}/callback/google", root_domain)
    };

    let token_response = match exchange_google_code_typed(
        &payload.code,
        &client_id,
        &client_secret,
        &redirect_uri,
        &state_data.code_verifier,
    )
    .await
    {
        Ok(response) => response,
        Err(e) => {
            handle_oauth_exchange_error(&app_state, &payload.state, &e).await;
            return Err(e.into());
        }
    };

    // Parse and validate id_token (with signature verification)
    let (google_id, email, email_verified) =
        match parse_google_id_token(&token_response.id_token, &client_id).await {
            Ok(result) => result,
            Err(e) => {
                let _ = app_state
                    .domain_auth_use_cases
                    .abort_google_oauth_state(&payload.state)
                    .await;
                return Err(e);
            }
        };

    // Verify email is verified by Google
    if !email_verified {
        let _ = app_state
            .domain_auth_use_cases
            .abort_google_oauth_state(&payload.state)
            .await;
        return Err(AppError::InvalidInput(
            "Google account email is not verified".into(),
        ));
    }

    // Find or create end user
    let result = match app_state
        .domain_auth_use_cases
        .find_or_create_end_user_by_google(domain.id, &google_id, &email)
        .await
    {
        Ok(result) => result,
        Err(e) => {
            let exchange_err = classify_user_creation_error(&e);
            handle_oauth_exchange_error(&app_state, &payload.state, &exchange_err).await;
            return Err(e);
        }
    };

    let response = match result {
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
    };

    if response.is_ok() {
        if let Err(e) = app_state
            .domain_auth_use_cases
            .complete_google_oauth_state(&payload.state)
            .await
        {
            tracing::warn!(
                state = %payload.state,
                error = %e,
                "Failed to delete OAuth state after successful login (best-effort)"
            );
        }
    }

    response
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
    let result = super::common::complete_login(&app_state, &user, &root_domain).await?;

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
// Helper Functions
// ============================================================================

fn should_abort_state(error: &AppError) -> bool {
    !matches!(
        error,
        AppError::Database(_) | AppError::Internal(_) | AppError::RateLimited
    )
}

fn classify_user_creation_error(error: &AppError) -> OAuthExchangeError {
    match error {
        AppError::InvalidInput(message) | AppError::ValidationError(message) => {
            OAuthExchangeError::UserValidation {
                message: message.clone(),
            }
        }
        AppError::Database(message) | AppError::Internal(message) => OAuthExchangeError::Database {
            message: message.clone(),
        },
        other => OAuthExchangeError::UserValidation {
            message: other.to_string(),
        },
    }
}

async fn handle_oauth_exchange_error(
    app_state: &AppState,
    state: &str,
    error: &OAuthExchangeError,
) {
    if error.is_retryable() {
        tracing::warn!(
            state = %state,
            error = ?error,
            "OAuth exchange failed (retryable), state preserved for retry"
        );
    } else {
        tracing::warn!(
            state = %state,
            error = ?error,
            "OAuth exchange failed (terminal), aborting state"
        );
        let _ = app_state
            .domain_auth_use_cases
            .abort_google_oauth_state(state)
            .await;
    }
}

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
async fn exchange_google_code_typed(
    code: &str,
    client_id: &str,
    client_secret: &str,
    redirect_uri: &str,
    code_verifier: &str,
) -> Result<GoogleTokenResponse, OAuthExchangeError> {
    let client = http_client::try_build_client().map_err(|e| {
        error!(
            error = %e,
            "Failed to build HTTP client for Google OAuth token exchange"
        );
        OAuthExchangeError::Network {
            message: format!("Failed to build HTTP client: {e}"),
        }
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
        .map_err(|e| OAuthExchangeError::Network {
            message: e.to_string(),
        })?;

    let status = response.status().as_u16();

    if !response.status().is_success() {
        let error_body = response.text().await.unwrap_or_default();
        let error_code = serde_json::from_str::<serde_json::Value>(&error_body)
            .ok()
            .and_then(|value| {
                value
                    .get("error")
                    .and_then(|error| error.as_str())
                    .map(str::to_string)
            });
        return Err(OAuthExchangeError::GoogleApi {
            status,
            error_code,
            message: error_body,
        });
    }

    response
        .json::<GoogleTokenResponse>()
        .await
        .map_err(|e| OAuthExchangeError::TokenValidation {
            message: format!("Failed to parse token response: {e}"),
        })
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
// Router
// ============================================================================

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/{domain}/auth/google/start", post(google_start))
        .route("/{domain}/auth/google/exchange", post(google_exchange))
        .route(
            "/{domain}/auth/google/confirm-link",
            post(google_confirm_link),
        )
        .route("/{domain}/auth/google/complete", post(google_complete))
        .route("/{domain}/auth/google/unlink", post(unlink_google))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod oauth_exchange_tests {
    use super::OAuthExchangeError;

    #[test]
    fn test_error_classification() {
        let err = OAuthExchangeError::Network {
            message: "timeout".into(),
        };
        assert!(err.is_retryable());

        let err = OAuthExchangeError::GoogleApi {
            status: 503,
            error_code: None,
            message: "Service unavailable".into(),
        };
        assert!(err.is_retryable());

        let err = OAuthExchangeError::GoogleApi {
            status: 400,
            error_code: Some("invalid_grant".into()),
            message: "Code already used".into(),
        };
        assert!(!err.is_retryable());

        let err = OAuthExchangeError::TokenValidation {
            message: "bad token".into(),
        };
        assert!(!err.is_retryable());

        let err = OAuthExchangeError::Database {
            message: "connection reset".into(),
        };
        assert!(err.is_retryable());
    }
}
