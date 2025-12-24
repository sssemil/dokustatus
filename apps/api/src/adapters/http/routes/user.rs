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

use crate::{adapters::http::app_state::AppState, app_error::AppResult, application::jwt};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/me", get(get_me))
        .route("/waitlist", get(get_waitlist_position))
        .route("/delete", delete(delete_account))
}

#[derive(Serialize)]
struct MeResponse {
    email: String,
    on_waitlist: bool,
}

async fn get_me(
    State(app_state): State<AppState>,
    jar: CookieJar,
) -> AppResult<impl IntoResponse> {
    let (_, user_id) = current_user(&jar, &app_state)?;

    let profile = app_state
        .user_repo
        .get_profile_by_id(user_id)
        .await?
        .ok_or(crate::app_error::AppError::InvalidCredentials)?;

    Ok(Json(MeResponse {
        email: profile.email,
        on_waitlist: profile.on_waitlist,
    }))
}

#[derive(Serialize)]
struct WaitlistResponse {
    position: u32,
    total: u32,
}

async fn get_waitlist_position(
    State(app_state): State<AppState>,
    jar: CookieJar,
) -> AppResult<impl IntoResponse> {
    let (_, user_id) = current_user(&jar, &app_state)?;

    let position = app_state
        .user_repo
        .get_waitlist_position(user_id)
        .await?
        .ok_or(crate::app_error::AppError::InvalidCredentials)?;

    Ok(Json(WaitlistResponse {
        position: position.position,
        total: position.total,
    }))
}

async fn delete_account(
    State(app_state): State<AppState>,
    jar: CookieJar,
) -> AppResult<(StatusCode, HeaderMap)> {
    let (_, user_id) = current_user(&jar, &app_state)?;

    app_state.auth_use_cases.delete_account(user_id).await?;

    let mut headers = HeaderMap::new();
    for (name, value, http_only) in [
        ("access_token", "", true),
        ("refresh_token", "", true),
        ("user_email", "", false),
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

fn current_user(jar: &CookieJar, app_state: &AppState) -> AppResult<(CookieJar, uuid::Uuid)> {
    let Some(access_cookie) = jar.get("access_token") else {
        return Err(crate::app_error::AppError::InvalidCredentials);
    };
    let claims = jwt::verify(access_cookie.value(), &app_state.config.jwt_secret)?;
    let user_id = uuid::Uuid::parse_str(&claims.sub)
        .map_err(|_| crate::app_error::AppError::InvalidCredentials)?;
    Ok((jar.clone(), user_id))
}
