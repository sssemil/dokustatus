//! Config route for public domain auth.

use super::common::*;

/// GET /api/public/domain/{domain}/config
/// Returns public config for a domain (enabled auth methods)
/// The {domain} param is the hostname (e.g., "reauth.example.com")
async fn get_config(
    State(app_state): State<AppState>,
    Path(hostname): Path<String>,
) -> AppResult<impl IntoResponse> {
    // Extract root domain from reauth.* hostname
    let root_domain = extract_root_from_reauth_hostname(&hostname);

    let config = app_state
        .domain_auth_use_cases
        .get_public_config(&root_domain)
        .await?;

    let response = PublicConfigResponse {
        domain: config.domain,
        auth_methods: AuthMethodsResponse {
            magic_link: config.magic_link_enabled,
            google_oauth: config.google_oauth_enabled,
        },
        redirect_url: config.redirect_url,
    };

    Ok(Json(response))
}

pub(crate) fn router() -> Router<AppState> {
    Router::new().route("/{domain}/config", get(get_config))
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use axum_test::TestServer;

    use crate::{
        domain::entities::domain::DomainStatus,
        test_utils::{
            TestAppStateBuilder, create_test_auth_config, create_test_domain, create_test_end_user,
        },
    };

    use super::*;

    // ========================================================================
    // GET /config Tests
    // ========================================================================

    #[tokio::test]
    async fn get_config_returns_not_found_for_unknown_domain() {
        let domain = create_test_domain(|d| d.domain = "example.com".to_string());
        let user = create_test_end_user(domain.id, |_| {});
        let api_key_raw = "test_api_key_123456789012345678901234";

        let app_state = TestAppStateBuilder::new()
            .with_domain(domain.clone())
            .with_user(user)
            .with_api_key(domain.id, &domain.domain, api_key_raw)
            .build();

        let app = router().with_state(app_state);
        let server = TestServer::new(app).unwrap();

        // Request config for a domain that doesn't exist
        let response = server.get("/reauth.unknown.com/config").await;

        assert_eq!(response.status_code(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn get_config_returns_not_found_for_unverified_domain() {
        let domain = create_test_domain(|d| {
            d.domain = "example.com".to_string();
            d.status = DomainStatus::PendingDns;
        });
        let user = create_test_end_user(domain.id, |_| {});
        let api_key_raw = "test_api_key_123456789012345678901234";

        let app_state = TestAppStateBuilder::new()
            .with_domain(domain.clone())
            .with_user(user)
            .with_api_key(domain.id, &domain.domain, api_key_raw)
            .build();

        let app = router().with_state(app_state);
        let server = TestServer::new(app).unwrap();

        let response = server.get("/reauth.example.com/config").await;

        assert_eq!(response.status_code(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn get_config_returns_defaults_when_no_auth_config() {
        let domain = create_test_domain(|d| {
            d.domain = "example.com".to_string();
            d.status = DomainStatus::Verified;
        });
        let user = create_test_end_user(domain.id, |_| {});
        let api_key_raw = "test_api_key_123456789012345678901234";

        // No auth config added - should use defaults
        let app_state = TestAppStateBuilder::new()
            .with_domain(domain.clone())
            .with_user(user)
            .with_api_key(domain.id, &domain.domain, api_key_raw)
            .build();

        let app = router().with_state(app_state);
        let server = TestServer::new(app).unwrap();

        let response = server.get("/reauth.example.com/config").await;

        assert_eq!(response.status_code(), StatusCode::OK);

        let body: serde_json::Value = response.json();
        assert_eq!(body.get("domain").unwrap(), "example.com");
        // Defaults when no config: both enabled
        assert_eq!(body["auth_methods"]["magic_link"], true);
        assert_eq!(body["auth_methods"]["google_oauth"], true);
        // Default redirect URL
        assert_eq!(body.get("redirect_url").unwrap(), "https://example.com");
    }

    #[tokio::test]
    async fn get_config_returns_configured_auth_methods() {
        let domain = create_test_domain(|d| {
            d.domain = "example.com".to_string();
            d.status = DomainStatus::Verified;
        });
        let user = create_test_end_user(domain.id, |_| {});
        let api_key_raw = "test_api_key_123456789012345678901234";

        let auth_config = create_test_auth_config(domain.id, |c| {
            c.magic_link_enabled = false;
            c.google_oauth_enabled = true;
            c.redirect_url = Some("https://app.example.com/dashboard".to_string());
        });

        let app_state = TestAppStateBuilder::new()
            .with_domain(domain.clone())
            .with_user(user)
            .with_auth_config(auth_config)
            .with_api_key(domain.id, &domain.domain, api_key_raw)
            .build();

        let app = router().with_state(app_state);
        let server = TestServer::new(app).unwrap();

        let response = server.get("/reauth.example.com/config").await;

        assert_eq!(response.status_code(), StatusCode::OK);

        let body: serde_json::Value = response.json();
        assert_eq!(body.get("domain").unwrap(), "example.com");
        assert_eq!(body["auth_methods"]["magic_link"], false);
        assert_eq!(body["auth_methods"]["google_oauth"], true);
        assert_eq!(
            body.get("redirect_url").unwrap(),
            "https://app.example.com/dashboard"
        );
    }

    #[tokio::test]
    async fn get_config_extracts_root_domain_from_reauth_hostname() {
        let domain = create_test_domain(|d| {
            d.domain = "myapp.io".to_string();
            d.status = DomainStatus::Verified;
        });
        let user = create_test_end_user(domain.id, |_| {});
        let api_key_raw = "test_api_key_123456789012345678901234";

        let app_state = TestAppStateBuilder::new()
            .with_domain(domain.clone())
            .with_user(user)
            .with_api_key(domain.id, &domain.domain, api_key_raw)
            .build();

        let app = router().with_state(app_state);
        let server = TestServer::new(app).unwrap();

        // Request with reauth. prefix - should extract myapp.io
        let response = server.get("/reauth.myapp.io/config").await;

        assert_eq!(response.status_code(), StatusCode::OK);

        let body: serde_json::Value = response.json();
        assert_eq!(body.get("domain").unwrap(), "myapp.io");
    }
}
