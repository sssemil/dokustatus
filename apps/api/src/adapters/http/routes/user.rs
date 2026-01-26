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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::Router;
    use axum_extra::extract::cookie::Cookie;
    use axum_test::TestServer;

    use crate::application::use_cases::api_key::ApiKeyWithRaw;
    use crate::application::use_cases::domain_billing::SubscriptionClaims;
    use crate::domain::entities::domain::DomainStatus;
    use crate::test_utils::{TestAppStateBuilder, create_test_auth_config, create_test_domain};

    fn build_test_router(app_state: AppState) -> Router<()> {
        router().with_state(app_state)
    }

    // =========================================================================
    // GET /me
    // =========================================================================

    #[tokio::test]
    async fn get_me_no_auth_returns_401() {
        let app_state = TestAppStateBuilder::new().build();

        let server = TestServer::new(build_test_router(app_state)).unwrap();

        let response = server.get("/me").await;

        response.assert_status(StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn get_me_returns_user_info() {
        let domain_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let api_key_raw = "test_api_key_12345678";
        let main_domain = "reauth.test";

        let domain = create_test_domain(|d| {
            d.id = domain_id;
            d.domain = main_domain.to_string();
            d.status = DomainStatus::Verified;
        });
        let user = crate::test_utils::create_test_end_user(domain_id, |u| {
            u.id = user_id;
            u.email = "dashboard@example.com".to_string();
        });
        let auth_config = create_test_auth_config(domain_id, |_| {});

        let app_state = TestAppStateBuilder::new()
            .with_main_domain(main_domain.to_string())
            .with_domain(domain)
            .with_user(user)
            .with_auth_config(auth_config)
            .with_api_key(domain_id, main_domain, api_key_raw)
            .build();

        // Generate a valid access token for main domain
        let api_key = ApiKeyWithRaw {
            id: Uuid::new_v4(),
            domain_id,
            raw_key: api_key_raw.to_string(),
        };

        let access_token = jwt::issue_domain_end_user_derived(
            user_id,
            domain_id,
            main_domain,
            vec!["admin".to_string()],
            SubscriptionClaims::none(),
            &api_key,
            time::Duration::hours(1),
        )
        .unwrap();

        let server = TestServer::new(build_test_router(app_state)).unwrap();

        let response = server
            .get("/me")
            .add_cookie(Cookie::new("end_user_access_token", access_token))
            .add_cookie(Cookie::new("end_user_email", "dashboard@example.com"))
            .await;

        response.assert_status(StatusCode::OK);

        let body = response.json::<serde_json::Value>();
        assert_eq!(body["email"].as_str().unwrap(), "dashboard@example.com");
        assert_eq!(body["roles"].as_array().unwrap().len(), 1);
        assert_eq!(body["roles"][0].as_str().unwrap(), "admin");
    }

    #[tokio::test]
    async fn get_me_non_main_domain_returns_401() {
        let domain_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let api_key_raw = "test_api_key_12345678";
        let other_domain = "other.example.com";

        let domain = create_test_domain(|d| {
            d.id = domain_id;
            d.domain = other_domain.to_string();
            d.status = DomainStatus::Verified;
        });
        let user = crate::test_utils::create_test_end_user(domain_id, |u| {
            u.id = user_id;
            u.email = "user@other.com".to_string();
        });
        let auth_config = create_test_auth_config(domain_id, |_| {});

        // Main domain is different from user's domain
        let app_state = TestAppStateBuilder::new()
            .with_main_domain("reauth.test".to_string())
            .with_domain(domain)
            .with_user(user)
            .with_auth_config(auth_config)
            .with_api_key(domain_id, other_domain, api_key_raw)
            .build();

        let api_key = ApiKeyWithRaw {
            id: Uuid::new_v4(),
            domain_id,
            raw_key: api_key_raw.to_string(),
        };

        let access_token = jwt::issue_domain_end_user_derived(
            user_id,
            domain_id,
            other_domain,
            vec![],
            SubscriptionClaims::none(),
            &api_key,
            time::Duration::hours(1),
        )
        .unwrap();

        let server = TestServer::new(build_test_router(app_state)).unwrap();

        let response = server
            .get("/me")
            .add_cookie(Cookie::new("end_user_access_token", access_token))
            .await;

        // Non-main domain users are rejected
        response.assert_status(StatusCode::UNAUTHORIZED);
    }

    // =========================================================================
    // DELETE /delete
    // =========================================================================

    #[tokio::test]
    async fn delete_account_no_auth_returns_401() {
        let app_state = TestAppStateBuilder::new().build();

        let server = TestServer::new(build_test_router(app_state)).unwrap();

        let response = server.delete("/delete").await;

        response.assert_status(StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn delete_account_clears_cookies() {
        let domain_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let api_key_raw = "test_api_key_12345678";
        let main_domain = "reauth.test";

        let domain = create_test_domain(|d| {
            d.id = domain_id;
            d.domain = main_domain.to_string();
            d.status = DomainStatus::Verified;
        });
        let user = crate::test_utils::create_test_end_user(domain_id, |u| {
            u.id = user_id;
            u.email = "dashboard@example.com".to_string();
        });
        let auth_config = create_test_auth_config(domain_id, |_| {});

        let app_state = TestAppStateBuilder::new()
            .with_main_domain(main_domain.to_string())
            .with_domain(domain)
            .with_user(user)
            .with_auth_config(auth_config)
            .with_api_key(domain_id, main_domain, api_key_raw)
            .build();

        let api_key = ApiKeyWithRaw {
            id: Uuid::new_v4(),
            domain_id,
            raw_key: api_key_raw.to_string(),
        };

        let access_token = jwt::issue_domain_end_user_derived(
            user_id,
            domain_id,
            main_domain,
            vec![],
            SubscriptionClaims::none(),
            &api_key,
            time::Duration::hours(1),
        )
        .unwrap();

        let server = TestServer::new(build_test_router(app_state)).unwrap();

        let response = server
            .delete("/delete")
            .add_cookie(Cookie::new("end_user_access_token", access_token))
            .await;

        response.assert_status(StatusCode::NO_CONTENT);

        // Verify cookies are cleared (set-cookie headers with max-age=0)
        let set_cookie_headers: Vec<_> = response
            .headers()
            .get_all("set-cookie")
            .iter()
            .map(|h| h.to_str().unwrap().to_string())
            .collect();

        // Should have 4 cookies being cleared
        assert_eq!(set_cookie_headers.len(), 4);

        // Check that each expected cookie is being cleared
        assert!(
            set_cookie_headers
                .iter()
                .any(|h| h.contains("end_user_access_token="))
        );
        assert!(
            set_cookie_headers
                .iter()
                .any(|h| h.contains("end_user_refresh_token="))
        );
        assert!(
            set_cookie_headers
                .iter()
                .any(|h| h.contains("end_user_email="))
        );
        assert!(
            set_cookie_headers
                .iter()
                .any(|h| h.contains("login_session="))
        );
    }

    #[tokio::test]
    async fn delete_account_non_main_domain_returns_401() {
        let domain_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let api_key_raw = "test_api_key_12345678";
        let other_domain = "other.example.com";

        let domain = create_test_domain(|d| {
            d.id = domain_id;
            d.domain = other_domain.to_string();
            d.status = DomainStatus::Verified;
        });
        let user = crate::test_utils::create_test_end_user(domain_id, |u| {
            u.id = user_id;
            u.email = "user@other.com".to_string();
        });
        let auth_config = create_test_auth_config(domain_id, |_| {});

        let app_state = TestAppStateBuilder::new()
            .with_main_domain("reauth.test".to_string())
            .with_domain(domain)
            .with_user(user)
            .with_auth_config(auth_config)
            .with_api_key(domain_id, other_domain, api_key_raw)
            .build();

        let api_key = ApiKeyWithRaw {
            id: Uuid::new_v4(),
            domain_id,
            raw_key: api_key_raw.to_string(),
        };

        let access_token = jwt::issue_domain_end_user_derived(
            user_id,
            domain_id,
            other_domain,
            vec![],
            SubscriptionClaims::none(),
            &api_key,
            time::Duration::hours(1),
        )
        .unwrap();

        let server = TestServer::new(build_test_router(app_state)).unwrap();

        let response = server
            .delete("/delete")
            .add_cookie(Cookie::new("end_user_access_token", access_token))
            .await;

        response.assert_status(StatusCode::UNAUTHORIZED);
    }
}
