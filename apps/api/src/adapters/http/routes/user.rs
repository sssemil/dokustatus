use axum::{
    Router,
    extract::State,
    http::{HeaderMap, StatusCode, header},
    routing::delete,
};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use time;

use crate::{adapters::http::app_state::AppState, app_error::AppResult, application::jwt};

pub fn router() -> Router<AppState> {
    Router::new().route("/delete", delete(delete_account))
}

async fn delete_account(
    State(app_state): State<AppState>,
    headers: HeaderMap,
    jar: CookieJar,
) -> AppResult<(StatusCode, HeaderMap)> {
    let (_, user_id) = current_user(&jar, &app_state)?;
    let lang = headers
        .get(header::ACCEPT_LANGUAGE)
        .and_then(|v| v.to_str().ok());

    app_state
        .auth_use_cases
        .delete_account(user_id, lang)
        .await?;

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
