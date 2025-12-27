use axum::{Router, http, middleware};
use http::header::{AUTHORIZATION, CONTENT_TYPE, HOST, HeaderValue};
use tower_http::{
    cors::{AllowOrigin, CorsLayer},
    set_header::SetResponseHeaderLayer,
    trace::TraceLayer,
};
use uuid::Uuid;

use crate::{
    adapters::{
        self,
        http::{app_state::AppState, middleware::{api_key_auth, rate_limit_middleware}},
    },
    infra::setup::init_tracing,
};

/// Validates that the Origin header is allowed for public SDK routes.
///
/// Security model:
/// - Requests come to reauth.{domain} (e.g., reauth.anypost.xyz)
/// - Host header tells us which domain's auth subdomain is being accessed (TLS-enforced, can't be spoofed)
/// - Path contains the domain as well: /public/domain/{domain}/...
/// - We validate Host and path agree (defense in depth)
/// - We allow Origin if it matches the domain or any subdomain (HTTPS only)
///
/// This prevents evil.com from stealing session data:
/// - evil.com makes request to reauth.anypost.xyz/api/public/domain/anypost.xyz/...
/// - Browser sends Origin: https://evil.com
/// - We check: is evil.com == anypost.xyz or *.anypost.xyz? No → reject
fn validate_public_origin(origin: &HeaderValue, parts: &http::request::Parts) -> bool {
    // 1. Extract domain from Host header: "reauth.anypost.xyz" → "anypost.xyz"
    let host = parts
        .headers
        .get(HOST)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");

    let host_domain = match host.strip_prefix("reauth.") {
        Some(d) if !d.is_empty() => d,
        _ => return false,
    };

    // 2. Extract domain from path: /public/domain/{domain}/... → {domain}
    let path = parts.uri.path();
    let path_domain = path
        .strip_prefix("/public/domain/")
        .and_then(|p| p.split('/').next())
        .unwrap_or("");

    // 3. Host and path domains must match (defense in depth)
    if path_domain.is_empty() || host_domain != path_domain {
        return false;
    }

    // 4. Parse origin - must be HTTPS
    let origin_str = origin.to_str().unwrap_or("");
    let origin_host = match origin_str.strip_prefix("https://") {
        Some(rest) => rest.split('/').next().unwrap_or(""),
        None => return false, // Reject non-HTTPS origins
    };

    if origin_host.is_empty() {
        return false;
    }

    // 5. Origin must be the base domain or a subdomain of it
    origin_host == host_domain || origin_host.ends_with(&format!(".{}", host_domain))
}

pub fn create_app(app_state: AppState) -> Router {
    init_tracing();

    // Restrictive CORS for dashboard/internal routes
    let dashboard_cors = CorsLayer::new()
        .allow_origin(app_state.config.cors_origin.clone())
        .allow_methods([
            http::Method::GET,
            http::Method::POST,
            http::Method::PATCH,
            http::Method::DELETE,
        ])
        .allow_headers([CONTENT_TYPE, AUTHORIZATION])
        .allow_credentials(true);

    // Secure CORS for public SDK routes - only allows origins matching the domain in the URL
    // This prevents evil.com from making requests to /api/public/domain/anypost.xyz/...
    let public_cors = CorsLayer::new()
        .allow_origin(AllowOrigin::predicate(validate_public_origin))
        .allow_methods([
            http::Method::GET,
            http::Method::POST,
            http::Method::DELETE,
        ])
        .allow_headers([CONTENT_TYPE])
        .allow_credentials(true);

    // Public routes with domain-validated CORS (for SDK usage)
    let public_routes = Router::new()
        .nest("/public/domain", adapters::http::routes::public_domain_auth::router())
        .layer(public_cors);

    // Dashboard routes with restrictive CORS
    let dashboard_routes = Router::new()
        .nest("/user", adapters::http::routes::user::router())
        .nest("/domains", adapters::http::routes::domain::router())
        .layer(dashboard_cors);

    // Developer API routes (server-to-server, API key authenticated)
    // No CORS restrictions since these are called from developer backends
    let developer_cors = CorsLayer::new()
        .allow_origin(http::header::HeaderValue::from_static("*"))
        .allow_methods([http::Method::GET, http::Method::POST])
        .allow_headers([CONTENT_TYPE, AUTHORIZATION]);

    let developer_routes = Router::new()
        .nest("/developer", adapters::http::routes::developer::router())
        .layer(middleware::from_fn_with_state(app_state.clone(), api_key_auth))
        .layer(developer_cors);

    Router::new()
        .nest("/api", public_routes.merge(dashboard_routes).merge(developer_routes))
        .with_state(app_state.clone())
        .layer(middleware::from_fn_with_state(
            app_state,
            rate_limit_middleware,
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            http::header::X_CONTENT_TYPE_OPTIONS,
            http::HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            http::header::X_FRAME_OPTIONS,
            http::HeaderValue::from_static("DENY"),
        ))
        .layer(
            TraceLayer::new_for_http().make_span_with(|request: &http::Request<_>| {
                let request_id = Uuid::new_v4();
                tracing::info_span!(
                    "http-request",
                    method = %request.method(),
                    uri = %request.uri(),
                    version = ?request.version(),
                    request_id = %request_id
                )
            }),
        )
}
