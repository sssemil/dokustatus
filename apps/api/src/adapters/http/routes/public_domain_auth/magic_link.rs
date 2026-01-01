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
