use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{delete, get},
};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use serde::Serialize;
use time;
use uuid::Uuid;

use crate::{adapters::http::app_state::AppState, app_error::AppResult, application::jwt};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/me", get(get_me))
        .route("/delete", delete(delete_account))
}

#[derive(Serialize)]
struct MeResponse {
    email: String,
    roles: Vec<String>,
}

async fn get_me(State(app_state): State<AppState>, jar: CookieJar) -> AppResult<impl IntoResponse> {
    let (_, claims) = current_user(&jar, &app_state).await?;

    Ok(Json(MeResponse {
        email: jar
            .get("end_user_email")
            .map(|c| c.value().to_string())
            .unwrap_or_default(),
        roles: claims.roles,
    }))
}

async fn delete_account(
    State(app_state): State<AppState>,
    jar: CookieJar,
) -> AppResult<(StatusCode, HeaderMap)> {
    let (_, claims) = current_user(&jar, &app_state).await?;

    let end_user_id =
        Uuid::parse_str(&claims.sub).map_err(|_| crate::app_error::AppError::InvalidCredentials)?;

    app_state
        .domain_auth_use_cases
        .delete_own_account(end_user_id)
        .await?;

    let mut headers = HeaderMap::new();
    for (name, value, http_only) in [
        ("end_user_access_token", "", true),
        ("end_user_refresh_token", "", true),
        ("end_user_email", "", false),
        ("login_session", "", true),
    ] {
        let cookie = Cookie::build((name, value))
            .http_only(http_only)
            .same_site(SameSite::Lax)
            .path("/")
            .max_age(time::Duration::seconds(0))
            .build();
        headers.append("set-cookie", cookie.to_string().parse().unwrap());
    }

    Ok((StatusCode::NO_CONTENT, headers))
}

/// Extracts the current end-user from the session.
/// Only allows access if the user is a main domain end-user (dashboard users).
async fn current_user(
    jar: &CookieJar,
    app_state: &AppState,
) -> AppResult<(CookieJar, jwt::DomainEndUserClaims)> {
    let Some(access_cookie) = jar.get("end_user_access_token") else {
        return Err(crate::app_error::AppError::InvalidCredentials);
    };

    // Peek at domain_id to know which keys to fetch
    let domain_id = jwt::peek_domain_id_from_token(access_cookie.value())?;

    // Get all active API keys for this domain
    let keys = app_state
        .api_key_use_cases
        .get_all_active_keys_for_domain(domain_id)
        .await?;

    if keys.is_empty() {
        return Err(crate::app_error::AppError::NoApiKeyConfigured);
    }

    // Verify with multi-key verification
    let claims = jwt::verify_domain_end_user_multi(access_cookie.value(), &keys)?;

    // Only allow main domain end-users to access dashboard
    if claims.domain != app_state.config.main_domain {
        return Err(crate::app_error::AppError::InvalidCredentials);
    }

    Ok((jar.clone(), claims))
}
