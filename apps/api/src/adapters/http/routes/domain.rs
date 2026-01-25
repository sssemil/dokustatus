use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, patch, post, put},
};
use axum_extra::extract::cookie::CookieJar;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    adapters::http::app_state::AppState,
    app_error::{AppError, AppResult},
    application::{
        jwt,
        use_cases::domain_billing::{CreatePlanInput, UpdatePlanInput},
        validators::is_valid_email,
    },
    domain::entities::{
        domain::DomainStatus, payment_mode::PaymentMode, payment_provider::PaymentProvider,
    },
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
        .route(
            "/{domain_id}/auth-config/magic-link",
            delete(delete_magic_link_config),
        )
        .route(
            "/{domain_id}/auth-config/google-oauth",
            delete(delete_google_oauth_config),
        )
        .route("/{domain_id}/end-users", get(list_end_users))
        .route("/{domain_id}/end-users/invite", post(invite_end_user))
        .route("/{domain_id}/end-users/{user_id}", get(get_end_user))
        .route("/{domain_id}/end-users/{user_id}", delete(delete_end_user))
        .route(
            "/{domain_id}/end-users/{user_id}/freeze",
            post(freeze_end_user),
        )
        .route(
            "/{domain_id}/end-users/{user_id}/freeze",
            delete(unfreeze_end_user),
        )
        .route(
            "/{domain_id}/end-users/{user_id}/whitelist",
            post(whitelist_end_user),
        )
        .route(
            "/{domain_id}/end-users/{user_id}/whitelist",
            delete(unwhitelist_end_user),
        )
        .route(
            "/{domain_id}/end-users/{user_id}/roles",
            put(set_user_roles),
        )
        // API Keys
        .route("/{domain_id}/api-keys", get(list_api_keys))
        .route("/{domain_id}/api-keys", post(create_api_key))
        .route("/{domain_id}/api-keys/{key_id}", delete(revoke_api_key))
        // Roles
        .route("/{domain_id}/roles", get(list_roles))
        .route("/{domain_id}/roles", post(create_role))
        .route("/{domain_id}/roles/{role_name}", delete(delete_role))
        .route(
            "/{domain_id}/roles/{role_name}/user-count",
            get(get_role_user_count),
        )
        // Billing
        .route("/{domain_id}/billing/config", get(get_billing_config))
        .route("/{domain_id}/billing/config", patch(update_billing_config))
        .route("/{domain_id}/billing/config", delete(delete_billing_config))
        .route("/{domain_id}/billing/mode", patch(set_billing_mode))
        .route("/{domain_id}/billing/plans", get(list_billing_plans))
        .route("/{domain_id}/billing/plans", post(create_billing_plan))
        .route(
            "/{domain_id}/billing/plans/reorder",
            put(reorder_billing_plans),
        )
        .route(
            "/{domain_id}/billing/plans/{plan_id}",
            patch(update_billing_plan),
        )
        .route(
            "/{domain_id}/billing/plans/{plan_id}",
            delete(archive_billing_plan),
        )
        .route(
            "/{domain_id}/billing/subscribers",
            get(list_billing_subscribers),
        )
        .route(
            "/{domain_id}/billing/subscribers/{user_id}/grant",
            post(grant_subscription),
        )
        .route(
            "/{domain_id}/billing/subscribers/{user_id}/revoke",
            delete(revoke_subscription),
        )
        .route("/{domain_id}/billing/analytics", get(get_billing_analytics))
        .route("/{domain_id}/billing/payments", get(list_billing_payments))
        .route(
            "/{domain_id}/billing/payments/export",
            get(export_billing_payments),
        )
        // Payment Providers
        .route(
            "/{domain_id}/billing/providers",
            get(list_billing_providers),
        )
        .route(
            "/{domain_id}/billing/providers",
            post(enable_billing_provider),
        )
        .route(
            "/{domain_id}/billing/providers/{provider}/{mode}",
            delete(disable_billing_provider),
        )
        .route(
            "/{domain_id}/billing/providers/{provider}/{mode}/active",
            patch(set_provider_active),
        )
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
    match app_state
        .domain_use_cases
        .is_domain_allowed(&params.domain)
        .await
    {
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
    has_auth_methods: bool,
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
            has_auth_methods: true, // New domains don't need warning yet
        }),
    ))
}

async fn list_domains(
    State(app_state): State<AppState>,
    jar: CookieJar,
) -> AppResult<impl IntoResponse> {
    let (_, user_id) = current_user(&jar, &app_state)?;

    let domains = app_state.domain_use_cases.list_domains(user_id).await?;

    let verified_domain_ids: Vec<Uuid> = domains
        .iter()
        .filter(|domain| domain.status == DomainStatus::Verified)
        .map(|domain| domain.id)
        .collect();

    let auth_methods_map = app_state
        .domain_auth_use_cases
        .has_auth_methods_for_owner_domains(&verified_domain_ids)
        .await;

    let mut response: Vec<DomainResponse> = Vec::with_capacity(domains.len());

    for d in domains {
        let dns_records = app_state.domain_use_cases.get_dns_records(&d.domain, d.id);

        // Check if domain has any auth methods enabled (only matters for verified domains)
        let has_auth_methods = if d.status == DomainStatus::Verified {
            match &auth_methods_map {
                Ok(map) => map.get(&d.id).copied().unwrap_or(true),
                Err(_) => false,
            }
        } else {
            true // Non-verified domains don't need this warning
        };

        response.push(DomainResponse {
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
            has_auth_methods,
        });
    }

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

    // Check if domain has any auth methods enabled
    let has_auth_methods = if domain.status == DomainStatus::Verified {
        if let Ok((config, _)) = app_state
            .domain_auth_use_cases
            .get_auth_config(user_id, domain_id)
            .await
        {
            config.magic_link_enabled || config.google_oauth_enabled
        } else {
            false
        }
    } else {
        true
    };

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
        has_auth_methods,
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
        has_auth_methods: true, // Verifying domains don't need this warning
    }))
}

#[derive(Serialize)]
struct VerificationStatusResponse {
    status: String,
    verified: bool,
    cname_verified: bool,
    txt_verified: bool,
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

    // Check individual DNS record status if domain is verifying
    let (cname_verified, txt_verified) = if domain.status == DomainStatus::Verifying {
        let dns_status = app_state
            .domain_use_cases
            .check_dns_records_status(domain_id)
            .await
            .unwrap_or_default();
        (dns_status.cname_verified, dns_status.txt_verified)
    } else if domain.status == DomainStatus::Verified {
        (true, true)
    } else {
        (false, false)
    };

    Ok(Json(VerificationStatusResponse {
        status: domain.status.as_str().to_string(),
        verified: domain.status == DomainStatus::Verified,
        cname_verified,
        txt_verified,
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
/// Only allows access if the user is a main domain end-user (dashboard users).
fn current_user(jar: &CookieJar, app_state: &AppState) -> AppResult<(CookieJar, Uuid)> {
    // Check for end_user_access_token (new auth)
    let Some(access_cookie) = jar.get("end_user_access_token") else {
        return Err(crate::app_error::AppError::InvalidCredentials);
    };

    let claims = jwt::verify_domain_end_user(access_cookie.value(), &app_state.config.jwt_secret)?;

    // Only allow main domain end-users to access dashboard
    if claims.domain != app_state.config.main_domain {
        return Err(crate::app_error::AppError::InvalidCredentials);
    }

    let end_user_id =
        Uuid::parse_str(&claims.sub).map_err(|_| crate::app_error::AppError::InvalidCredentials)?;
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
    using_fallback: bool,
    fallback_from_email: String,
    google_oauth_config: Option<GoogleOAuthConfigResponse>,
    using_google_fallback: bool,
}

#[derive(Serialize)]
struct MagicLinkConfigResponse {
    from_email: String,
    has_api_key: bool,
}

#[derive(Serialize)]
struct GoogleOAuthConfigResponse {
    client_id_prefix: String,
    has_client_secret: bool,
}

async fn get_auth_config(
    State(app_state): State<AppState>,
    jar: CookieJar,
    Path(domain_id): Path<Uuid>,
) -> AppResult<impl IntoResponse> {
    let (_, user_id) = current_user(&jar, &app_state)?;

    // Get domain to access domain name
    let domain = app_state
        .domain_use_cases
        .get_domain(user_id, domain_id)
        .await?;

    let (auth_config, magic_link_config) = app_state
        .domain_auth_use_cases
        .get_auth_config(user_id, domain_id)
        .await?;

    // Check if using fallback or custom config for magic link
    let has_custom_config = magic_link_config.is_some();
    let fallback_from_email = app_state
        .domain_auth_use_cases
        .get_fallback_email_info(&domain.domain);
    let using_fallback = !has_custom_config;

    let magic_link_response = magic_link_config.map(|c| MagicLinkConfigResponse {
        from_email: c.from_email,
        has_api_key: !c.resend_api_key_encrypted.is_empty(),
    });

    // Get Google OAuth config info
    let google_oauth_config_info = app_state
        .domain_auth_use_cases
        .get_google_oauth_config_info(domain_id)
        .await?;

    let google_oauth_response =
        google_oauth_config_info
            .as_ref()
            .map(|c| GoogleOAuthConfigResponse {
                client_id_prefix: c.client_id_prefix.clone(),
                has_client_secret: c.has_client_secret,
            });

    let using_google_fallback = google_oauth_config_info.is_none();

    Ok(Json(AuthConfigResponse {
        magic_link_enabled: auth_config.magic_link_enabled,
        google_oauth_enabled: auth_config.google_oauth_enabled,
        redirect_url: auth_config.redirect_url,
        whitelist_enabled: auth_config.whitelist_enabled,
        magic_link_config: magic_link_response,
        using_fallback,
        fallback_from_email,
        google_oauth_config: google_oauth_response,
        using_google_fallback,
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
    google_client_id: Option<String>,
    google_client_secret: Option<String>,
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

    // If Google OAuth credentials were provided, update them
    if payload.google_client_id.is_some() && payload.google_client_secret.is_some() {
        app_state
            .domain_auth_use_cases
            .update_google_oauth_config(
                user_id,
                domain_id,
                payload.google_client_id.as_deref().unwrap(),
                payload.google_client_secret.as_deref().unwrap(),
            )
            .await?;
    }

    Ok(StatusCode::OK)
}

async fn delete_magic_link_config(
    State(app_state): State<AppState>,
    jar: CookieJar,
    Path(domain_id): Path<Uuid>,
) -> AppResult<impl IntoResponse> {
    let (_, user_id) = current_user(&jar, &app_state)?;

    app_state
        .domain_auth_use_cases
        .delete_magic_link_config(user_id, domain_id)
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

async fn delete_google_oauth_config(
    State(app_state): State<AppState>,
    jar: CookieJar,
    Path(domain_id): Path<Uuid>,
) -> AppResult<impl IntoResponse> {
    let (_, user_id) = current_user(&jar, &app_state)?;

    app_state
        .domain_auth_use_cases
        .delete_google_oauth_config(user_id, domain_id)
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

#[derive(Serialize)]
struct EndUserResponse {
    id: Uuid,
    email: String,
    roles: Vec<String>,
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
            roles: u.roles,
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
struct InviteEndUserPayload {
    email: String,
    pre_whitelist: Option<bool>,
}

async fn invite_end_user(
    State(app_state): State<AppState>,
    jar: CookieJar,
    Path(domain_id): Path<Uuid>,
    Json(payload): Json<InviteEndUserPayload>,
) -> AppResult<impl IntoResponse> {
    // Validate email format
    let email = payload.email.trim();
    if !is_valid_email(email) {
        return Err(AppError::InvalidInput("Invalid email format".into()));
    }

    let (_, owner_id) = current_user(&jar, &app_state)?;

    let user = app_state
        .domain_auth_use_cases
        .invite_end_user(
            owner_id,
            domain_id,
            email,
            payload.pre_whitelist.unwrap_or(false),
        )
        .await?;

    Ok((
        StatusCode::CREATED,
        Json(EndUserResponse {
            id: user.id,
            email: user.email,
            roles: user.roles,
            email_verified_at: user.email_verified_at,
            last_login_at: user.last_login_at,
            is_frozen: user.is_frozen,
            is_whitelisted: user.is_whitelisted,
            created_at: user.created_at,
        }),
    ))
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
        roles: user.roles,
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

// ============================================================================
// API Key Endpoints
// ============================================================================

#[derive(Serialize)]
struct ApiKeyResponse {
    id: Uuid,
    key_prefix: String,
    name: String,
    last_used_at: Option<chrono::NaiveDateTime>,
    revoked_at: Option<chrono::NaiveDateTime>,
    created_at: Option<chrono::NaiveDateTime>,
}

#[derive(Serialize)]
struct CreateApiKeyResponse {
    id: Uuid,
    key: String, // Full key, shown only once
    key_prefix: String,
    name: String,
    created_at: Option<chrono::NaiveDateTime>,
}

#[derive(Deserialize)]
struct CreateApiKeyPayload {
    name: Option<String>,
}

#[derive(Deserialize)]
struct ApiKeyPathParams {
    domain_id: Uuid,
    key_id: Uuid,
}

async fn list_api_keys(
    State(app_state): State<AppState>,
    jar: CookieJar,
    Path(domain_id): Path<Uuid>,
) -> AppResult<impl IntoResponse> {
    let (_, owner_id) = current_user(&jar, &app_state)?;

    let keys = app_state
        .api_key_use_cases
        .list_api_keys(owner_id, domain_id)
        .await?;

    let response: Vec<ApiKeyResponse> = keys
        .into_iter()
        .map(|k| ApiKeyResponse {
            id: k.id,
            key_prefix: k.key_prefix,
            name: k.name,
            last_used_at: k.last_used_at,
            revoked_at: k.revoked_at,
            created_at: k.created_at,
        })
        .collect();

    Ok(Json(response))
}

async fn create_api_key(
    State(app_state): State<AppState>,
    jar: CookieJar,
    Path(domain_id): Path<Uuid>,
    Json(payload): Json<CreateApiKeyPayload>,
) -> AppResult<impl IntoResponse> {
    let (_, owner_id) = current_user(&jar, &app_state)?;

    let name = payload.name.as_deref().unwrap_or("Default");

    let (profile, raw_key) = app_state
        .api_key_use_cases
        .create_api_key(owner_id, domain_id, name)
        .await?;

    Ok((
        StatusCode::CREATED,
        Json(CreateApiKeyResponse {
            id: profile.id,
            key: raw_key,
            key_prefix: profile.key_prefix,
            name: profile.name,
            created_at: profile.created_at,
        }),
    ))
}

async fn revoke_api_key(
    State(app_state): State<AppState>,
    jar: CookieJar,
    Path(params): Path<ApiKeyPathParams>,
) -> AppResult<impl IntoResponse> {
    let (_, owner_id) = current_user(&jar, &app_state)?;

    app_state
        .api_key_use_cases
        .revoke_api_key(owner_id, params.domain_id, params.key_id)
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

// ============================================================================
// Role Endpoints
// ============================================================================

#[derive(Serialize)]
struct RoleResponse {
    id: Uuid,
    name: String,
    user_count: i64,
    created_at: Option<chrono::NaiveDateTime>,
}

#[derive(Deserialize)]
struct CreateRolePayload {
    name: String,
}

#[derive(Deserialize)]
struct RolePathParams {
    domain_id: Uuid,
    role_name: String,
}

#[derive(Serialize)]
struct RoleUserCountResponse {
    user_count: i64,
}

#[derive(Deserialize)]
struct SetUserRolesPayload {
    roles: Vec<String>,
}

async fn list_roles(
    State(app_state): State<AppState>,
    jar: CookieJar,
    Path(domain_id): Path<Uuid>,
) -> AppResult<impl IntoResponse> {
    let (_, owner_id) = current_user(&jar, &app_state)?;

    let roles = app_state
        .domain_roles_use_cases
        .list_roles(owner_id, domain_id)
        .await?;

    let response: Vec<RoleResponse> = roles
        .into_iter()
        .map(|r| RoleResponse {
            id: r.id,
            name: r.name,
            user_count: r.user_count,
            created_at: r.created_at,
        })
        .collect();

    Ok(Json(response))
}

async fn create_role(
    State(app_state): State<AppState>,
    jar: CookieJar,
    Path(domain_id): Path<Uuid>,
    Json(payload): Json<CreateRolePayload>,
) -> AppResult<impl IntoResponse> {
    let (_, owner_id) = current_user(&jar, &app_state)?;

    let role = app_state
        .domain_roles_use_cases
        .create_role(owner_id, domain_id, &payload.name)
        .await?;

    Ok((
        StatusCode::CREATED,
        Json(RoleResponse {
            id: role.id,
            name: role.name,
            user_count: 0,
            created_at: role.created_at,
        }),
    ))
}

async fn delete_role(
    State(app_state): State<AppState>,
    jar: CookieJar,
    Path(params): Path<RolePathParams>,
) -> AppResult<impl IntoResponse> {
    let (_, owner_id) = current_user(&jar, &app_state)?;

    app_state
        .domain_roles_use_cases
        .delete_role(owner_id, params.domain_id, &params.role_name)
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

async fn get_role_user_count(
    State(app_state): State<AppState>,
    jar: CookieJar,
    Path(params): Path<RolePathParams>,
) -> AppResult<impl IntoResponse> {
    let (_, owner_id) = current_user(&jar, &app_state)?;

    let user_count = app_state
        .domain_roles_use_cases
        .count_users_with_role(owner_id, params.domain_id, &params.role_name)
        .await?;

    Ok(Json(RoleUserCountResponse { user_count }))
}

async fn set_user_roles(
    State(app_state): State<AppState>,
    jar: CookieJar,
    Path(params): Path<EndUserPathParams>,
    Json(payload): Json<SetUserRolesPayload>,
) -> AppResult<impl IntoResponse> {
    let (_, owner_id) = current_user(&jar, &app_state)?;

    app_state
        .domain_roles_use_cases
        .set_user_roles(owner_id, params.domain_id, params.user_id, payload.roles)
        .await?;

    Ok(StatusCode::OK)
}

// ============================================================================
// Billing Endpoints
// ============================================================================

/// Response for GET /billing/config - returns status of both modes
#[derive(Serialize)]
struct BillingConfigStatusResponse {
    active_mode: PaymentMode,
    test: Option<ModeConfigStatusResponse>,
    live: Option<ModeConfigStatusResponse>,
}

#[derive(Serialize)]
struct ModeConfigStatusResponse {
    publishable_key_last4: String,
    is_connected: bool,
}

async fn get_billing_config(
    State(app_state): State<AppState>,
    jar: CookieJar,
    Path(domain_id): Path<Uuid>,
) -> AppResult<impl IntoResponse> {
    let (_, owner_id) = current_user(&jar, &app_state)?;

    let config = app_state
        .billing_use_cases
        .get_stripe_config(owner_id, domain_id)
        .await?;

    Ok(Json(BillingConfigStatusResponse {
        active_mode: config.active_mode,
        test: config.test.map(|c| ModeConfigStatusResponse {
            publishable_key_last4: c.publishable_key_last4,
            is_connected: c.is_connected,
        }),
        live: config.live.map(|c| ModeConfigStatusResponse {
            publishable_key_last4: c.publishable_key_last4,
            is_connected: c.is_connected,
        }),
    }))
}

#[derive(Deserialize)]
struct UpdateBillingConfigPayload {
    mode: PaymentMode,
    secret_key: String,
    publishable_key: String,
    webhook_secret: String,
}

async fn update_billing_config(
    State(app_state): State<AppState>,
    jar: CookieJar,
    Path(domain_id): Path<Uuid>,
    Json(payload): Json<UpdateBillingConfigPayload>,
) -> AppResult<impl IntoResponse> {
    let (_, owner_id) = current_user(&jar, &app_state)?;

    let config = app_state
        .billing_use_cases
        .update_stripe_config(
            owner_id,
            domain_id,
            payload.mode,
            &payload.secret_key,
            &payload.publishable_key,
            &payload.webhook_secret,
        )
        .await?;

    Ok(Json(ModeConfigStatusResponse {
        publishable_key_last4: config.publishable_key_last4,
        is_connected: config.is_connected,
    }))
}

#[derive(Deserialize)]
struct DeleteBillingConfigPayload {
    mode: PaymentMode,
}

async fn delete_billing_config(
    State(app_state): State<AppState>,
    jar: CookieJar,
    Path(domain_id): Path<Uuid>,
    Json(payload): Json<DeleteBillingConfigPayload>,
) -> AppResult<impl IntoResponse> {
    let (_, owner_id) = current_user(&jar, &app_state)?;

    app_state
        .billing_use_cases
        .delete_stripe_config(owner_id, domain_id, payload.mode)
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

/// Set the active payment mode for a domain
#[derive(Deserialize)]
struct SetBillingModePayload {
    mode: PaymentMode,
}

#[derive(Serialize)]
struct SetBillingModeResponse {
    active_mode: PaymentMode,
}

async fn set_billing_mode(
    State(app_state): State<AppState>,
    jar: CookieJar,
    Path(domain_id): Path<Uuid>,
    Json(payload): Json<SetBillingModePayload>,
) -> AppResult<impl IntoResponse> {
    let (_, owner_id) = current_user(&jar, &app_state)?;

    let new_mode = app_state
        .billing_use_cases
        .set_active_mode(owner_id, domain_id, payload.mode)
        .await?;

    Ok(Json(SetBillingModeResponse {
        active_mode: new_mode,
    }))
}

#[derive(Serialize)]
struct BillingPlanResponse {
    id: Uuid,
    code: String,
    name: String,
    description: Option<String>,
    price_cents: i32,
    currency: String,
    interval: String,
    interval_count: i32,
    trial_days: i32,
    features: Vec<String>,
    is_public: bool,
    display_order: i32,
    stripe_product_id: Option<String>,
    stripe_price_id: Option<String>,
    is_archived: bool,
    created_at: Option<chrono::NaiveDateTime>,
}

#[derive(Deserialize)]
struct ListPlansQuery {
    include_archived: Option<bool>,
}

async fn list_billing_plans(
    State(app_state): State<AppState>,
    jar: CookieJar,
    Path(domain_id): Path<Uuid>,
    Query(query): Query<ListPlansQuery>,
) -> AppResult<impl IntoResponse> {
    let (_, owner_id) = current_user(&jar, &app_state)?;

    let plans = app_state
        .billing_use_cases
        .list_plans(owner_id, domain_id, query.include_archived.unwrap_or(false))
        .await?;

    let response: Vec<BillingPlanResponse> = plans
        .into_iter()
        .map(|p| BillingPlanResponse {
            id: p.id,
            code: p.code,
            name: p.name,
            description: p.description,
            price_cents: p.price_cents,
            currency: p.currency,
            interval: p.interval,
            interval_count: p.interval_count,
            trial_days: p.trial_days,
            features: p.features,
            is_public: p.is_public,
            display_order: p.display_order,
            stripe_product_id: p.stripe_product_id,
            stripe_price_id: p.stripe_price_id,
            is_archived: p.is_archived,
            created_at: p.created_at,
        })
        .collect();

    Ok(Json(response))
}

async fn create_billing_plan(
    State(app_state): State<AppState>,
    jar: CookieJar,
    Path(domain_id): Path<Uuid>,
    Json(input): Json<CreatePlanInput>,
) -> AppResult<impl IntoResponse> {
    let (_, owner_id) = current_user(&jar, &app_state)?;

    let plan = app_state
        .billing_use_cases
        .create_plan(owner_id, domain_id, input)
        .await?;

    Ok((
        StatusCode::CREATED,
        Json(BillingPlanResponse {
            id: plan.id,
            code: plan.code,
            name: plan.name,
            description: plan.description,
            price_cents: plan.price_cents,
            currency: plan.currency,
            interval: plan.interval,
            interval_count: plan.interval_count,
            trial_days: plan.trial_days,
            features: plan.features,
            is_public: plan.is_public,
            display_order: plan.display_order,
            stripe_product_id: plan.stripe_product_id,
            stripe_price_id: plan.stripe_price_id,
            is_archived: plan.is_archived,
            created_at: plan.created_at,
        }),
    ))
}

#[derive(Deserialize)]
struct PlanPathParams {
    domain_id: Uuid,
    plan_id: Uuid,
}

async fn update_billing_plan(
    State(app_state): State<AppState>,
    jar: CookieJar,
    Path(params): Path<PlanPathParams>,
    Json(input): Json<UpdatePlanInput>,
) -> AppResult<impl IntoResponse> {
    let (_, owner_id) = current_user(&jar, &app_state)?;

    let plan = app_state
        .billing_use_cases
        .update_plan(owner_id, params.domain_id, params.plan_id, input)
        .await?;

    Ok(Json(BillingPlanResponse {
        id: plan.id,
        code: plan.code,
        name: plan.name,
        description: plan.description,
        price_cents: plan.price_cents,
        currency: plan.currency,
        interval: plan.interval,
        interval_count: plan.interval_count,
        trial_days: plan.trial_days,
        features: plan.features,
        is_public: plan.is_public,
        display_order: plan.display_order,
        stripe_product_id: plan.stripe_product_id,
        stripe_price_id: plan.stripe_price_id,
        is_archived: plan.is_archived,
        created_at: plan.created_at,
    }))
}

async fn archive_billing_plan(
    State(app_state): State<AppState>,
    jar: CookieJar,
    Path(params): Path<PlanPathParams>,
) -> AppResult<impl IntoResponse> {
    let (_, owner_id) = current_user(&jar, &app_state)?;

    app_state
        .billing_use_cases
        .archive_plan(owner_id, params.domain_id, params.plan_id)
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct ReorderPlansPayload {
    plan_ids: Vec<Uuid>,
}

async fn reorder_billing_plans(
    State(app_state): State<AppState>,
    jar: CookieJar,
    Path(domain_id): Path<Uuid>,
    Json(payload): Json<ReorderPlansPayload>,
) -> AppResult<impl IntoResponse> {
    let (_, owner_id) = current_user(&jar, &app_state)?;

    app_state
        .billing_use_cases
        .reorder_plans(owner_id, domain_id, payload.plan_ids)
        .await?;

    Ok(StatusCode::OK)
}

#[derive(Serialize)]
struct SubscriberResponse {
    id: Uuid,
    user_id: Uuid,
    user_email: String,
    plan_id: Uuid,
    plan_name: String,
    plan_code: String,
    status: String,
    current_period_start: Option<chrono::NaiveDateTime>,
    current_period_end: Option<chrono::NaiveDateTime>,
    trial_start: Option<chrono::NaiveDateTime>,
    trial_end: Option<chrono::NaiveDateTime>,
    cancel_at_period_end: bool,
    manually_granted: bool,
    created_at: Option<chrono::NaiveDateTime>,
}

async fn list_billing_subscribers(
    State(app_state): State<AppState>,
    jar: CookieJar,
    Path(domain_id): Path<Uuid>,
) -> AppResult<impl IntoResponse> {
    let (_, owner_id) = current_user(&jar, &app_state)?;

    let subscribers = app_state
        .billing_use_cases
        .list_subscribers(owner_id, domain_id)
        .await?;

    let response: Vec<SubscriberResponse> = subscribers
        .into_iter()
        .map(|s| SubscriberResponse {
            id: s.subscription.id,
            user_id: s.subscription.end_user_id,
            user_email: s.user_email,
            plan_id: s.plan.id,
            plan_name: s.plan.name,
            plan_code: s.plan.code,
            status: s.subscription.status.as_str().to_string(),
            current_period_start: s.subscription.current_period_start,
            current_period_end: s.subscription.current_period_end,
            trial_start: s.subscription.trial_start,
            trial_end: s.subscription.trial_end,
            cancel_at_period_end: s.subscription.cancel_at_period_end,
            manually_granted: s.subscription.manually_granted,
            created_at: s.subscription.created_at,
        })
        .collect();

    Ok(Json(response))
}

#[derive(Deserialize)]
struct GrantSubscriptionPayload {
    plan_id: Uuid,
}

async fn grant_subscription(
    State(app_state): State<AppState>,
    jar: CookieJar,
    Path(params): Path<EndUserPathParams>,
    Json(payload): Json<GrantSubscriptionPayload>,
) -> AppResult<impl IntoResponse> {
    let (_, owner_id) = current_user(&jar, &app_state)?;

    // For manually granted subscriptions, we create a placeholder customer ID
    let stripe_customer_id = format!("manual_{}", params.user_id);

    let subscription = app_state
        .billing_use_cases
        .grant_subscription(
            owner_id,
            params.domain_id,
            params.user_id,
            payload.plan_id,
            &stripe_customer_id,
        )
        .await?;

    Ok((StatusCode::CREATED, Json(subscription)))
}

async fn revoke_subscription(
    State(app_state): State<AppState>,
    jar: CookieJar,
    Path(params): Path<EndUserPathParams>,
) -> AppResult<impl IntoResponse> {
    let (_, owner_id) = current_user(&jar, &app_state)?;

    app_state
        .billing_use_cases
        .revoke_subscription(owner_id, params.domain_id, params.user_id)
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

#[derive(Serialize)]
struct BillingAnalyticsResponse {
    mrr_cents: i64,
    active_subscribers: i64,
    trialing_subscribers: i64,
    past_due_subscribers: i64,
    plan_distribution: Vec<PlanDistributionResponse>,
}

#[derive(Serialize)]
struct PlanDistributionResponse {
    plan_id: Uuid,
    plan_name: String,
    subscriber_count: i64,
    revenue_cents: i64,
}

async fn get_billing_analytics(
    State(app_state): State<AppState>,
    jar: CookieJar,
    Path(domain_id): Path<Uuid>,
) -> AppResult<impl IntoResponse> {
    let (_, owner_id) = current_user(&jar, &app_state)?;

    let analytics = app_state
        .billing_use_cases
        .get_analytics(owner_id, domain_id)
        .await?;

    Ok(Json(BillingAnalyticsResponse {
        mrr_cents: analytics.mrr_cents,
        active_subscribers: analytics.active_subscribers,
        trialing_subscribers: analytics.trialing_subscribers,
        past_due_subscribers: analytics.past_due_subscribers,
        plan_distribution: analytics
            .plan_distribution
            .into_iter()
            .map(|p| PlanDistributionResponse {
                plan_id: p.plan_id,
                plan_name: p.plan_name,
                subscriber_count: p.subscriber_count,
                revenue_cents: p.revenue_cents,
            })
            .collect(),
    }))
}

// ============================================================================
// Payment History Endpoints
// ============================================================================

#[derive(Debug, Deserialize)]
struct PaymentListQuery {
    page: Option<i32>,
    per_page: Option<i32>,
    status: Option<String>,
    date_from: Option<i64>,
    date_to: Option<i64>,
    plan_code: Option<String>,
    user_email: Option<String>,
}

#[derive(Debug, Serialize)]
struct PaymentListResponse {
    payments: Vec<PaymentWithUserResponse>,
    total: i64,
    page: i32,
    per_page: i32,
    total_pages: i32,
    summary: PaymentSummaryResponse,
}

#[derive(Debug, Serialize)]
struct PaymentWithUserResponse {
    id: String,
    user_id: String,
    user_email: String,
    amount_cents: i32,
    amount_paid_cents: i32,
    amount_refunded_cents: i32,
    currency: String,
    status: String,
    plan_code: Option<String>,
    plan_name: Option<String>,
    invoice_url: Option<String>,
    invoice_pdf: Option<String>,
    invoice_number: Option<String>,
    billing_reason: Option<String>,
    failure_message: Option<String>,
    payment_date: Option<i64>,
    created_at: Option<i64>,
}

#[derive(Debug, Serialize)]
struct PaymentSummaryResponse {
    total_revenue_cents: i64,
    total_refunded_cents: i64,
    payment_count: i64,
    successful_payments: i64,
    failed_payments: i64,
}

/// GET /api/domains/{id}/billing/payments
/// Lists all payments for a domain with filtering and pagination
async fn list_billing_payments(
    State(app_state): State<AppState>,
    jar: CookieJar,
    Path(domain_id): Path<Uuid>,
    Query(query): Query<PaymentListQuery>,
) -> AppResult<impl IntoResponse> {
    use crate::application::use_cases::domain_billing::PaymentListFilters;
    use crate::domain::entities::payment_status::PaymentStatus;

    let (_, owner_id) = current_user(&jar, &app_state)?;

    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(20).clamp(1, 100);

    // Parse status filter
    let status = query
        .status
        .as_deref()
        .and_then(|s| s.parse::<PaymentStatus>().ok());

    // Parse date filters (timestamps to NaiveDateTime)
    let date_from = query
        .date_from
        .and_then(|ts| chrono::DateTime::from_timestamp(ts, 0).map(|dt| dt.naive_utc()));
    let date_to = query
        .date_to
        .and_then(|ts| chrono::DateTime::from_timestamp(ts, 0).map(|dt| dt.naive_utc()));

    let filters = PaymentListFilters {
        status,
        date_from,
        date_to,
        plan_code: query.plan_code,
        user_email: query.user_email,
    };

    // Get payments
    let paginated = app_state
        .billing_use_cases
        .list_domain_payments(owner_id, domain_id, &filters, page, per_page)
        .await?;

    // Get summary
    let summary = app_state
        .billing_use_cases
        .get_payment_summary(owner_id, domain_id, date_from, date_to)
        .await?;

    let payments: Vec<PaymentWithUserResponse> = paginated
        .payments
        .into_iter()
        .map(|p| PaymentWithUserResponse {
            id: p.payment.id.to_string(),
            user_id: p.payment.end_user_id.to_string(),
            user_email: p.user_email,
            amount_cents: p.payment.amount_cents,
            amount_paid_cents: p.payment.amount_paid_cents,
            amount_refunded_cents: p.payment.amount_refunded_cents,
            currency: p.payment.currency,
            status: p.payment.status.as_str().to_string(),
            plan_code: p.payment.plan_code,
            plan_name: p.payment.plan_name,
            invoice_url: p.payment.hosted_invoice_url,
            invoice_pdf: p.payment.invoice_pdf_url,
            invoice_number: p.payment.invoice_number,
            billing_reason: p.payment.billing_reason,
            failure_message: p.payment.failure_message,
            payment_date: p.payment.payment_date.map(|dt| dt.and_utc().timestamp()),
            created_at: p.payment.created_at.map(|dt| dt.and_utc().timestamp()),
        })
        .collect();

    Ok(Json(PaymentListResponse {
        payments,
        total: paginated.total,
        page: paginated.page,
        per_page: paginated.per_page,
        total_pages: paginated.total_pages,
        summary: PaymentSummaryResponse {
            total_revenue_cents: summary.total_revenue_cents,
            total_refunded_cents: summary.total_refunded_cents,
            payment_count: summary.payment_count,
            successful_payments: summary.successful_payments,
            failed_payments: summary.failed_payments,
        },
    }))
}

/// GET /api/domains/{id}/billing/payments/export
/// Exports payments as CSV file
async fn export_billing_payments(
    State(app_state): State<AppState>,
    jar: CookieJar,
    Path(domain_id): Path<Uuid>,
    Query(query): Query<PaymentListQuery>,
) -> AppResult<impl IntoResponse> {
    use crate::application::use_cases::domain_billing::PaymentListFilters;
    use crate::domain::entities::payment_status::PaymentStatus;

    let (_, owner_id) = current_user(&jar, &app_state)?;

    // Parse status filter
    let status = query
        .status
        .as_deref()
        .and_then(|s| s.parse::<PaymentStatus>().ok());

    // Parse date filters
    let date_from = query
        .date_from
        .and_then(|ts| chrono::DateTime::from_timestamp(ts, 0).map(|dt| dt.naive_utc()));
    let date_to = query
        .date_to
        .and_then(|ts| chrono::DateTime::from_timestamp(ts, 0).map(|dt| dt.naive_utc()));

    let filters = PaymentListFilters {
        status,
        date_from,
        date_to,
        plan_code: query.plan_code,
        user_email: query.user_email,
    };

    let csv = app_state
        .billing_use_cases
        .export_payments_csv(owner_id, domain_id, &filters)
        .await?;

    // Return CSV file with proper headers
    Ok((
        [
            (axum::http::header::CONTENT_TYPE, "text/csv"),
            (
                axum::http::header::CONTENT_DISPOSITION,
                "attachment; filename=\"payments.csv\"",
            ),
        ],
        csv,
    ))
}

// ============================================================================
// Payment Provider Handlers
// ============================================================================

/// GET /api/domains/{id}/billing/providers
/// List all enabled payment providers for this domain
async fn list_billing_providers(
    State(app_state): State<AppState>,
    jar: CookieJar,
    Path(domain_id): Path<Uuid>,
) -> AppResult<impl IntoResponse> {
    let (_, owner_id) = current_user(&jar, &app_state)?;

    let providers = app_state
        .billing_use_cases
        .list_enabled_providers(owner_id, domain_id)
        .await?;

    Ok(Json(providers))
}

#[derive(Deserialize)]
struct EnableProviderPayload {
    provider: PaymentProvider,
    mode: PaymentMode,
}

/// POST /api/domains/{id}/billing/providers
/// Enable a payment provider for this domain
async fn enable_billing_provider(
    State(app_state): State<AppState>,
    jar: CookieJar,
    Path(domain_id): Path<Uuid>,
    Json(payload): Json<EnableProviderPayload>,
) -> AppResult<impl IntoResponse> {
    let (_, owner_id) = current_user(&jar, &app_state)?;

    let provider = app_state
        .billing_use_cases
        .enable_provider(owner_id, domain_id, payload.provider, payload.mode)
        .await?;

    Ok((StatusCode::CREATED, Json(provider)))
}

#[derive(Deserialize)]
struct ProviderPath {
    domain_id: Uuid,
    provider: PaymentProvider,
    mode: PaymentMode,
}

/// DELETE /api/domains/{id}/billing/providers/{provider}/{mode}
/// Disable a payment provider for this domain
async fn disable_billing_provider(
    State(app_state): State<AppState>,
    jar: CookieJar,
    Path(path): Path<ProviderPath>,
) -> AppResult<impl IntoResponse> {
    let (_, owner_id) = current_user(&jar, &app_state)?;

    app_state
        .billing_use_cases
        .disable_provider(owner_id, path.domain_id, path.provider, path.mode)
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct SetProviderActivePayload {
    is_active: bool,
}

/// PATCH /api/domains/{id}/billing/providers/{provider}/{mode}/active
/// Toggle active status for a provider
async fn set_provider_active(
    State(app_state): State<AppState>,
    jar: CookieJar,
    Path(path): Path<ProviderPath>,
    Json(payload): Json<SetProviderActivePayload>,
) -> AppResult<impl IntoResponse> {
    let (_, owner_id) = current_user(&jar, &app_state)?;

    app_state
        .billing_use_cases
        .set_provider_active(
            owner_id,
            path.domain_id,
            path.provider,
            path.mode,
            payload.is_active,
        )
        .await?;

    Ok(StatusCode::NO_CONTENT)
}
