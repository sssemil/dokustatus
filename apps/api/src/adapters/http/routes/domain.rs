use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, patch, post},
};
use axum_extra::extract::cookie::CookieJar;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    adapters::http::app_state::AppState,
    app_error::AppResult,
    application::jwt,
    domain::entities::domain::DomainStatus,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", post(create_domain))
        .route("/", get(list_domains))
        .route("/stats", get(get_usage_stats))
        .route("/check-allowed", get(check_allowed))
        .route("/{domain_id}", get(get_domain))
        .route("/{domain_id}/verify", post(start_verification))
        .route("/{domain_id}/status", get(get_verification_status))
        .route("/{domain_id}", delete(delete_domain))
        .route("/{domain_id}/auth-config", get(get_auth_config))
        .route("/{domain_id}/auth-config", patch(update_auth_config))
        .route("/{domain_id}/end-users", get(list_end_users))
        .route("/{domain_id}/end-users/{user_id}", get(get_end_user))
        .route("/{domain_id}/end-users/{user_id}", delete(delete_end_user))
        .route("/{domain_id}/end-users/{user_id}/freeze", post(freeze_end_user))
        .route("/{domain_id}/end-users/{user_id}/freeze", delete(unfreeze_end_user))
        .route("/{domain_id}/end-users/{user_id}/whitelist", post(whitelist_end_user))
        .route("/{domain_id}/end-users/{user_id}/whitelist", delete(unwhitelist_end_user))
}

#[derive(Deserialize)]
struct CreateDomainPayload {
    domain: String,
}

#[derive(Deserialize)]
struct CheckAllowedParams {
    domain: String,
}

/// Used by Caddy's on_demand_tls to check if a domain is allowed for SSL provisioning.
/// Returns 200 if domain is verified, 404 otherwise.
/// This endpoint is public (no auth required).
async fn check_allowed(
    State(app_state): State<AppState>,
    Query(params): Query<CheckAllowedParams>,
) -> impl IntoResponse {
    match app_state.domain_use_cases.is_domain_allowed(&params.domain).await {
        Ok(true) => StatusCode::OK,
        _ => StatusCode::NOT_FOUND,
    }
}

#[derive(Serialize)]
struct UsageStatsResponse {
    domains_count: usize,
    total_users: i64,
}

async fn get_usage_stats(
    State(app_state): State<AppState>,
    jar: CookieJar,
) -> AppResult<impl IntoResponse> {
    let (_, user_id) = current_user(&jar, &app_state)?;

    let domains = app_state.domain_use_cases.list_domains(user_id).await?;
    let domain_ids: Vec<_> = domains.iter().map(|d| d.id).collect();
    let total_users = app_state
        .domain_auth_use_cases
        .count_users_by_domain_ids(&domain_ids)
        .await?;

    Ok(Json(UsageStatsResponse {
        domains_count: domains.len(),
        total_users,
    }))
}

#[derive(Serialize)]
struct DomainResponse {
    id: Uuid,
    domain: String,
    status: String,
    dns_records: Option<DnsRecordsResponse>,
    verified_at: Option<chrono::NaiveDateTime>,
    created_at: Option<chrono::NaiveDateTime>,
}

#[derive(Serialize)]
struct DnsRecordsResponse {
    cname_name: String,
    cname_value: String,
    txt_name: String,
    txt_value: String,
}

async fn create_domain(
    State(app_state): State<AppState>,
    jar: CookieJar,
    Json(payload): Json<CreateDomainPayload>,
) -> AppResult<impl IntoResponse> {
    let (_, user_id) = current_user(&jar, &app_state)?;

    let domain = app_state
        .domain_use_cases
        .add_domain(user_id, &payload.domain)
        .await?;

    let dns_records = app_state
        .domain_use_cases
        .get_dns_records(&domain.domain, domain.id);

    Ok((
        StatusCode::CREATED,
        Json(DomainResponse {
            id: domain.id,
            domain: domain.domain,
            status: domain.status.as_str().to_string(),
            dns_records: Some(DnsRecordsResponse {
                cname_name: dns_records.cname_name,
                cname_value: dns_records.cname_value,
                txt_name: dns_records.txt_name,
                txt_value: dns_records.txt_value,
            }),
            verified_at: domain.verified_at,
            created_at: domain.created_at,
        }),
    ))
}

async fn list_domains(
    State(app_state): State<AppState>,
    jar: CookieJar,
) -> AppResult<impl IntoResponse> {
    let (_, user_id) = current_user(&jar, &app_state)?;

    let domains = app_state.domain_use_cases.list_domains(user_id).await?;

    let response: Vec<DomainResponse> = domains
        .into_iter()
        .map(|d| {
            let dns_records = app_state.domain_use_cases.get_dns_records(&d.domain, d.id);
            DomainResponse {
                id: d.id,
                domain: d.domain,
                status: d.status.as_str().to_string(),
                dns_records: Some(DnsRecordsResponse {
                    cname_name: dns_records.cname_name,
                    cname_value: dns_records.cname_value,
                    txt_name: dns_records.txt_name,
                    txt_value: dns_records.txt_value,
                }),
                verified_at: d.verified_at,
                created_at: d.created_at,
            }
        })
        .collect();

    Ok(Json(response))
}

async fn get_domain(
    State(app_state): State<AppState>,
    jar: CookieJar,
    Path(domain_id): Path<Uuid>,
) -> AppResult<impl IntoResponse> {
    let (_, user_id) = current_user(&jar, &app_state)?;

    let domain = app_state
        .domain_use_cases
        .get_domain(user_id, domain_id)
        .await?;

    let dns_records = app_state
        .domain_use_cases
        .get_dns_records(&domain.domain, domain.id);

    Ok(Json(DomainResponse {
        id: domain.id,
        domain: domain.domain,
        status: domain.status.as_str().to_string(),
        dns_records: Some(DnsRecordsResponse {
            cname_name: dns_records.cname_name,
            cname_value: dns_records.cname_value,
            txt_name: dns_records.txt_name,
            txt_value: dns_records.txt_value,
        }),
        verified_at: domain.verified_at,
        created_at: domain.created_at,
    }))
}

async fn start_verification(
    State(app_state): State<AppState>,
    jar: CookieJar,
    Path(domain_id): Path<Uuid>,
) -> AppResult<impl IntoResponse> {
    let (_, user_id) = current_user(&jar, &app_state)?;

    let domain = app_state
        .domain_use_cases
        .start_verification(user_id, domain_id)
        .await?;

    Ok(Json(DomainResponse {
        id: domain.id,
        domain: domain.domain,
        status: domain.status.as_str().to_string(),
        dns_records: None,
        verified_at: domain.verified_at,
        created_at: domain.created_at,
    }))
}

#[derive(Serialize)]
struct VerificationStatusResponse {
    status: String,
    verified: bool,
}

async fn get_verification_status(
    State(app_state): State<AppState>,
    jar: CookieJar,
    Path(domain_id): Path<Uuid>,
) -> AppResult<impl IntoResponse> {
    let (_, user_id) = current_user(&jar, &app_state)?;

    let domain = app_state
        .domain_use_cases
        .get_domain(user_id, domain_id)
        .await?;

    Ok(Json(VerificationStatusResponse {
        status: domain.status.as_str().to_string(),
        verified: domain.status == DomainStatus::Verified,
    }))
}

async fn delete_domain(
    State(app_state): State<AppState>,
    jar: CookieJar,
    Path(domain_id): Path<Uuid>,
) -> AppResult<impl IntoResponse> {
    let (_, user_id) = current_user(&jar, &app_state)?;

    app_state
        .domain_use_cases
        .delete_domain(user_id, domain_id)
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

/// Extracts the current end-user from the session.
/// Only allows access if the user is a reauth.dev end-user (dashboard users).
fn current_user(jar: &CookieJar, app_state: &AppState) -> AppResult<(CookieJar, Uuid)> {
    // Check for end_user_access_token (new auth)
    let Some(access_cookie) = jar.get("end_user_access_token") else {
        return Err(crate::app_error::AppError::InvalidCredentials);
    };

    let claims = jwt::verify_domain_end_user(access_cookie.value(), &app_state.config.jwt_secret)?;

    // Only allow reauth.dev end-users to access dashboard
    if claims.domain != "reauth.dev" {
        return Err(crate::app_error::AppError::InvalidCredentials);
    }

    let end_user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| crate::app_error::AppError::InvalidCredentials)?;
    Ok((jar.clone(), end_user_id))
}

// ============================================================================
// Auth Config Endpoints
// ============================================================================

#[derive(Serialize)]
struct AuthConfigResponse {
    magic_link_enabled: bool,
    google_oauth_enabled: bool,
    redirect_url: Option<String>,
    whitelist_enabled: bool,
    magic_link_config: Option<MagicLinkConfigResponse>,
}

#[derive(Serialize)]
struct MagicLinkConfigResponse {
    from_email: String,
    has_api_key: bool,
}

async fn get_auth_config(
    State(app_state): State<AppState>,
    jar: CookieJar,
    Path(domain_id): Path<Uuid>,
) -> AppResult<impl IntoResponse> {
    let (_, user_id) = current_user(&jar, &app_state)?;

    let (auth_config, magic_link_config) = app_state
        .domain_auth_use_cases
        .get_auth_config(user_id, domain_id)
        .await?;

    let magic_link_response = magic_link_config.map(|c| MagicLinkConfigResponse {
        from_email: c.from_email,
        has_api_key: !c.resend_api_key_encrypted.is_empty(),
    });

    Ok(Json(AuthConfigResponse {
        magic_link_enabled: auth_config.magic_link_enabled,
        google_oauth_enabled: auth_config.google_oauth_enabled,
        redirect_url: auth_config.redirect_url,
        whitelist_enabled: auth_config.whitelist_enabled,
        magic_link_config: magic_link_response,
    }))
}

#[derive(Deserialize)]
struct UpdateAuthConfigPayload {
    magic_link_enabled: Option<bool>,
    google_oauth_enabled: Option<bool>,
    redirect_url: Option<String>,
    whitelist_enabled: Option<bool>,
    whitelist_all_existing: Option<bool>,
    resend_api_key: Option<String>,
    from_email: Option<String>,
}

async fn update_auth_config(
    State(app_state): State<AppState>,
    jar: CookieJar,
    Path(domain_id): Path<Uuid>,
    Json(payload): Json<UpdateAuthConfigPayload>,
) -> AppResult<impl IntoResponse> {
    let (_, user_id) = current_user(&jar, &app_state)?;

    app_state
        .domain_auth_use_cases
        .update_auth_config(
            user_id,
            domain_id,
            payload.magic_link_enabled.unwrap_or(false),
            payload.google_oauth_enabled.unwrap_or(false),
            payload.redirect_url.as_deref(),
            payload.whitelist_enabled.unwrap_or(false),
            payload.whitelist_all_existing.unwrap_or(false),
            payload.resend_api_key.as_deref(),
            payload.from_email.as_deref(),
        )
        .await?;

    Ok(StatusCode::OK)
}

#[derive(Serialize)]
struct EndUserResponse {
    id: Uuid,
    email: String,
    email_verified_at: Option<chrono::NaiveDateTime>,
    last_login_at: Option<chrono::NaiveDateTime>,
    is_frozen: bool,
    is_whitelisted: bool,
    created_at: Option<chrono::NaiveDateTime>,
}

async fn list_end_users(
    State(app_state): State<AppState>,
    jar: CookieJar,
    Path(domain_id): Path<Uuid>,
) -> AppResult<impl IntoResponse> {
    let (_, user_id) = current_user(&jar, &app_state)?;

    let end_users = app_state
        .domain_auth_use_cases
        .list_end_users(user_id, domain_id)
        .await?;

    let response: Vec<EndUserResponse> = end_users
        .into_iter()
        .map(|u| EndUserResponse {
            id: u.id,
            email: u.email,
            email_verified_at: u.email_verified_at,
            last_login_at: u.last_login_at,
            is_frozen: u.is_frozen,
            is_whitelisted: u.is_whitelisted,
            created_at: u.created_at,
        })
        .collect();

    Ok(Json(response))
}

#[derive(Deserialize)]
struct EndUserPathParams {
    domain_id: Uuid,
    user_id: Uuid,
}

async fn get_end_user(
    State(app_state): State<AppState>,
    jar: CookieJar,
    Path(params): Path<EndUserPathParams>,
) -> AppResult<impl IntoResponse> {
    let (_, owner_id) = current_user(&jar, &app_state)?;

    let user = app_state
        .domain_auth_use_cases
        .get_end_user(owner_id, params.domain_id, params.user_id)
        .await?;

    Ok(Json(EndUserResponse {
        id: user.id,
        email: user.email,
        email_verified_at: user.email_verified_at,
        last_login_at: user.last_login_at,
        is_frozen: user.is_frozen,
        is_whitelisted: user.is_whitelisted,
        created_at: user.created_at,
    }))
}

async fn delete_end_user(
    State(app_state): State<AppState>,
    jar: CookieJar,
    Path(params): Path<EndUserPathParams>,
) -> AppResult<impl IntoResponse> {
    let (_, owner_id) = current_user(&jar, &app_state)?;

    app_state
        .domain_auth_use_cases
        .delete_end_user(owner_id, params.domain_id, params.user_id)
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

async fn freeze_end_user(
    State(app_state): State<AppState>,
    jar: CookieJar,
    Path(params): Path<EndUserPathParams>,
) -> AppResult<impl IntoResponse> {
    let (_, owner_id) = current_user(&jar, &app_state)?;

    app_state
        .domain_auth_use_cases
        .freeze_end_user(owner_id, params.domain_id, params.user_id)
        .await?;

    Ok(StatusCode::OK)
}

async fn unfreeze_end_user(
    State(app_state): State<AppState>,
    jar: CookieJar,
    Path(params): Path<EndUserPathParams>,
) -> AppResult<impl IntoResponse> {
    let (_, owner_id) = current_user(&jar, &app_state)?;

    app_state
        .domain_auth_use_cases
        .unfreeze_end_user(owner_id, params.domain_id, params.user_id)
        .await?;

    Ok(StatusCode::OK)
}

async fn whitelist_end_user(
    State(app_state): State<AppState>,
    jar: CookieJar,
    Path(params): Path<EndUserPathParams>,
) -> AppResult<impl IntoResponse> {
    let (_, owner_id) = current_user(&jar, &app_state)?;

    app_state
        .domain_auth_use_cases
        .whitelist_end_user(owner_id, params.domain_id, params.user_id)
        .await?;

    Ok(StatusCode::OK)
}

async fn unwhitelist_end_user(
    State(app_state): State<AppState>,
    jar: CookieJar,
    Path(params): Path<EndUserPathParams>,
) -> AppResult<impl IntoResponse> {
    let (_, owner_id) = current_user(&jar, &app_state)?;

    app_state
        .domain_auth_use_cases
        .unwhitelist_end_user(owner_id, params.domain_id, params.user_id)
        .await?;

    Ok(StatusCode::OK)
}
