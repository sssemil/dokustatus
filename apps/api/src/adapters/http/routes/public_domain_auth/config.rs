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
