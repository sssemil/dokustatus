//! Billing routes: plans, subscription, checkout, portal, payments, plan changes.

use super::common::*;
use crate::domain::entities::{payment_mode::PaymentMode, payment_provider::PaymentProvider};

// ============================================================================
// Types
// ============================================================================

#[derive(Serialize)]
struct PublicPlanResponse {
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
    display_order: i32,
}

#[derive(Serialize)]
struct UserSubscriptionResponse {
    id: Option<Uuid>,
    plan_code: Option<String>,
    plan_name: Option<String>,
    status: String,
    current_period_end: Option<i64>,
    trial_end: Option<i64>,
    cancel_at_period_end: Option<bool>,
}

#[derive(Deserialize)]
struct CreateCheckoutPayload {
    plan_code: String,
    success_url: String,
    cancel_url: String,
}

#[derive(Serialize)]
struct CheckoutResponse {
    checkout_url: String,
}

#[derive(Deserialize)]
struct CreatePortalPayload {
    return_url: String,
}

#[derive(Serialize)]
struct PortalResponse {
    portal_url: String,
}

/// Query params for payment list
#[derive(Debug, Deserialize)]
struct PaymentListQuery {
    page: Option<i32>,
    per_page: Option<i32>,
}

/// Response for paginated payments
#[derive(Debug, Serialize)]
struct PaymentListResponse {
    payments: Vec<PaymentResponse>,
    total: i64,
    page: i32,
    per_page: i32,
    total_pages: i32,
}

#[derive(Debug, Serialize)]
struct PaymentResponse {
    id: String,
    amount_cents: i32,
    amount_paid_cents: i32,
    amount_refunded_cents: i32,
    currency: String,
    status: String,
    payment_provider: Option<PaymentProvider>,
    payment_mode: PaymentMode,
    plan_name: Option<String>,
    plan_code: Option<String>,
    invoice_url: Option<String>,
    invoice_pdf: Option<String>,
    invoice_number: Option<String>,
    payment_date: Option<i64>,
    created_at: Option<i64>,
}

// ============================================================================
// Plan Change (Upgrade/Downgrade) Types
// ============================================================================

/// Query params for plan change preview
#[derive(Debug, Deserialize)]
struct PlanChangePreviewQuery {
    plan_code: String,
}

/// Response for plan change preview
#[derive(Debug, Serialize)]
struct PlanChangePreviewResponse {
    prorated_amount_cents: i64,
    currency: String,
    period_end: i64,
    new_plan_name: String,
    new_plan_price_cents: i64,
    change_type: String,
    effective_at: i64,
}

/// Request body for plan change
#[derive(Debug, Deserialize)]
struct PlanChangeRequest {
    plan_code: String,
}

/// Response for plan change
#[derive(Debug, Serialize)]
struct PlanChangeResponse {
    success: bool,
    change_type: String,
    invoice_id: Option<String>,
    amount_charged_cents: Option<i64>,
    currency: Option<String>,
    client_secret: Option<String>,
    hosted_invoice_url: Option<String>,
    payment_intent_status: Option<String>,
    new_plan: PlanChangeNewPlanResponse,
    effective_at: i64,
    schedule_id: Option<String>,
}

#[derive(Debug, Serialize)]
struct PlanChangeNewPlanResponse {
    code: String,
    name: String,
    price_cents: i32,
    currency: String,
    interval: String,
    interval_count: i32,
    features: Vec<String>,
}

#[derive(Serialize)]
struct AvailableProvider {
    id: Uuid,
    domain_id: Uuid,
    provider: PaymentProvider,
    mode: PaymentMode,
    is_active: bool,
    display_order: i32,
    created_at: Option<chrono::NaiveDateTime>,
}

// ============================================================================
// Handlers
// ============================================================================

/// GET /api/public/domain/{domain}/billing/plans
/// Returns public subscription plans for a domain
async fn get_public_plans(
    State(app_state): State<AppState>,
    Path(hostname): Path<String>,
) -> AppResult<impl IntoResponse> {
    let root_domain = extract_root_from_reauth_hostname(&hostname);

    let domain = app_state
        .domain_auth_use_cases
        .get_domain_by_name(&root_domain)
        .await?
        .ok_or(AppError::NotFound)?;

    let plans = app_state
        .billing_use_cases
        .get_public_plans(domain.id)
        .await?;

    let response: Vec<PublicPlanResponse> = plans
        .into_iter()
        .map(|p| PublicPlanResponse {
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
            display_order: p.display_order,
        })
        .collect();

    Ok(Json(response))
}

/// GET /api/public/domain/{domain}/billing/subscription
/// Returns the current user's subscription status
async fn get_user_subscription(
    State(app_state): State<AppState>,
    Path(hostname): Path<String>,
    cookies: CookieJar,
) -> AppResult<impl IntoResponse> {
    let root_domain = extract_root_from_reauth_hostname(&hostname);

    // Get user from token
    let (user_id, domain_id) = get_current_user(&app_state, &cookies, &root_domain)?;

    let sub = app_state
        .billing_use_cases
        .get_user_subscription_with_plan(domain_id, user_id)
        .await?;

    match sub {
        Some((subscription, plan)) => Ok(Json(UserSubscriptionResponse {
            id: Some(subscription.id),
            plan_code: Some(plan.code),
            plan_name: Some(plan.name),
            status: subscription.status.as_str().to_string(),
            current_period_end: subscription
                .current_period_end
                .map(|dt| dt.and_utc().timestamp()),
            trial_end: subscription.trial_end.map(|dt| dt.and_utc().timestamp()),
            cancel_at_period_end: Some(subscription.cancel_at_period_end),
        })),
        None => Ok(Json(UserSubscriptionResponse {
            id: None,
            plan_code: None,
            plan_name: None,
            status: "none".to_string(),
            current_period_end: None,
            trial_end: None,
            cancel_at_period_end: None,
        })),
    }
}

/// POST /api/public/domain/{domain}/billing/checkout
/// Creates a Stripe checkout session for subscribing to a plan
async fn create_checkout(
    State(app_state): State<AppState>,
    Path(hostname): Path<String>,
    cookies: CookieJar,
    Json(payload): Json<CreateCheckoutPayload>,
) -> AppResult<impl IntoResponse> {
    use crate::infra::stripe_client::StripeClient;
    use std::collections::HashMap;

    let root_domain = extract_root_from_reauth_hostname(&hostname);

    // Get user from token
    let (user_id, domain_id) = get_current_user(&app_state, &cookies, &root_domain)?;

    // Get user details
    let user = app_state
        .domain_auth_use_cases
        .get_end_user_by_id(user_id)
        .await?
        .ok_or(AppError::NotFound)?;

    // Get the plan
    let mut plan = app_state
        .billing_use_cases
        .get_plan_by_code(domain_id, &payload.plan_code)
        .await?
        .ok_or(AppError::NotFound)?;

    // Verify plan is public (users can only subscribe to public plans)
    if !plan.is_public {
        return Err(AppError::NotFound);
    }

    // Get Stripe client
    let secret_key = app_state
        .billing_use_cases
        .get_stripe_secret_key(domain_id)
        .await?;
    let stripe = StripeClient::new(secret_key);

    // Lazily create Stripe product/price if not set
    if plan.stripe_product_id.is_none() || plan.stripe_price_id.is_none() {
        // Create Stripe product if needed
        let product_id = if let Some(ref id) = plan.stripe_product_id {
            id.clone()
        } else {
            let product = stripe
                .create_product(&plan.name, plan.description.as_deref())
                .await?;
            product.id
        };

        // Create Stripe price if needed
        let price_id = if let Some(ref id) = plan.stripe_price_id {
            id.clone()
        } else {
            // Convert interval to Stripe format (month/year)
            let stripe_interval = match plan.interval.as_str() {
                "monthly" => "month",
                "yearly" => "year",
                other => other, // Allow custom intervals
            };
            let price = stripe
                .create_price(
                    &product_id,
                    plan.price_cents as i64,
                    &plan.currency,
                    stripe_interval,
                    plan.interval_count,
                )
                .await?;
            price.id
        };

        // Update plan with Stripe IDs
        app_state
            .billing_use_cases
            .set_stripe_ids(plan.id, &product_id, &price_id)
            .await?;

        plan.stripe_product_id = Some(product_id);
        plan.stripe_price_id = Some(price_id.clone());
    }

    let price_id = plan.stripe_price_id.as_ref().unwrap();

    // Get or create customer
    let mut metadata = HashMap::new();
    metadata.insert("user_id".to_string(), user_id.to_string());
    metadata.insert("domain_id".to_string(), domain_id.to_string());
    let customer = stripe
        .get_or_create_customer(&user.email, Some(metadata))
        .await?;

    // Create checkout session
    let session = stripe
        .create_checkout_session(
            &customer.id,
            price_id,
            &payload.success_url,
            &payload.cancel_url,
            Some(&user_id.to_string()),
            Some(plan.trial_days),
        )
        .await?;

    let checkout_url = session.url.ok_or(AppError::Internal(
        "Stripe checkout session missing URL".into(),
    ))?;

    Ok(Json(CheckoutResponse { checkout_url }))
}

/// POST /api/public/domain/{domain}/billing/portal
/// Creates a Stripe customer portal session
async fn create_portal(
    State(app_state): State<AppState>,
    Path(hostname): Path<String>,
    cookies: CookieJar,
    Json(payload): Json<CreatePortalPayload>,
) -> AppResult<impl IntoResponse> {
    use crate::infra::stripe_client::StripeClient;

    let root_domain = extract_root_from_reauth_hostname(&hostname);

    // Get user from token
    let (user_id, domain_id) = get_current_user(&app_state, &cookies, &root_domain)?;

    // Get user's subscription to find Stripe customer ID
    let subscription = app_state
        .billing_use_cases
        .get_user_subscription(domain_id, user_id)
        .await?
        .ok_or(AppError::NotFound)?;

    // Get Stripe client
    let secret_key = app_state
        .billing_use_cases
        .get_stripe_secret_key(domain_id)
        .await?;
    let stripe = StripeClient::new(secret_key);

    // Create portal session
    let portal = stripe
        .create_portal_session(&subscription.stripe_customer_id, &payload.return_url)
        .await?;

    Ok(Json(PortalResponse {
        portal_url: portal.url,
    }))
}

/// POST /api/public/domain/{domain}/billing/cancel
/// Cancels the user's subscription at period end
async fn cancel_subscription(
    State(app_state): State<AppState>,
    Path(hostname): Path<String>,
    cookies: CookieJar,
) -> AppResult<impl IntoResponse> {
    use crate::infra::stripe_client::StripeClient;

    let root_domain = extract_root_from_reauth_hostname(&hostname);

    // Get user from token
    let (user_id, domain_id) = get_current_user(&app_state, &cookies, &root_domain)?;

    // Get user's subscription
    let subscription = app_state
        .billing_use_cases
        .get_user_subscription(domain_id, user_id)
        .await?
        .ok_or(AppError::NotFound)?;

    // Get Stripe subscription ID
    let stripe_subscription_id =
        subscription
            .stripe_subscription_id
            .ok_or(AppError::InvalidInput(
                "No active Stripe subscription".into(),
            ))?;

    // Get Stripe client
    let secret_key = app_state
        .billing_use_cases
        .get_stripe_secret_key(domain_id)
        .await?;
    let stripe = StripeClient::new(secret_key);

    // Cancel at period end
    stripe
        .cancel_subscription(&stripe_subscription_id, true)
        .await?;

    Ok(StatusCode::OK)
}

/// GET /api/public/domain/{domain}/billing/payments
/// Returns the user's payment history
async fn get_user_payments(
    State(app_state): State<AppState>,
    Path(hostname): Path<String>,
    Query(query): Query<PaymentListQuery>,
    cookies: CookieJar,
) -> AppResult<impl IntoResponse> {
    let root_domain = extract_root_from_reauth_hostname(&hostname);

    // Get user from token
    let (user_id, domain_id) = get_current_user(&app_state, &cookies, &root_domain)?;

    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(10).clamp(1, 100);

    let paginated = app_state
        .billing_use_cases
        .get_user_payments(domain_id, user_id, page, per_page)
        .await?;

    let payments: Vec<PaymentResponse> = paginated
        .payments
        .into_iter()
        .map(|p| PaymentResponse {
            id: p.payment.id.to_string(),
            amount_cents: p.payment.amount_cents,
            amount_paid_cents: p.payment.amount_paid_cents,
            amount_refunded_cents: p.payment.amount_refunded_cents,
            currency: p.payment.currency,
            status: p.payment.status.as_ref().to_string(),
            payment_provider: p.payment.payment_provider,
            payment_mode: p.payment.payment_mode,
            plan_name: p.payment.plan_name,
            plan_code: p.payment.plan_code,
            invoice_url: p.payment.hosted_invoice_url,
            invoice_pdf: p.payment.invoice_pdf_url,
            invoice_number: p.payment.invoice_number,
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
    }))
}

/// GET /api/public/domain/{domain}/billing/plan-change/preview
/// Preview the cost of upgrading or downgrading a subscription
async fn preview_plan_change(
    State(app_state): State<AppState>,
    Path(hostname): Path<String>,
    Query(query): Query<PlanChangePreviewQuery>,
    cookies: CookieJar,
) -> AppResult<impl IntoResponse> {
    let root_domain = extract_root_from_reauth_hostname(&hostname);

    // Get user from token
    let (user_id, domain_id) = get_current_user(&app_state, &cookies, &root_domain)?;

    // Get preview from use cases
    let preview = app_state
        .billing_use_cases
        .preview_plan_change(domain_id, user_id, &query.plan_code)
        .await?;

    Ok(Json(PlanChangePreviewResponse {
        prorated_amount_cents: preview.prorated_amount_cents,
        currency: preview.currency,
        period_end: preview.period_end,
        new_plan_name: preview.new_plan_name,
        new_plan_price_cents: preview.new_plan_price_cents,
        change_type: preview.change_type.as_ref().to_string(),
        effective_at: preview.effective_at,
    }))
}

/// POST /api/public/domain/{domain}/billing/plan-change
/// Execute a plan change (upgrade or downgrade)
async fn change_plan(
    State(app_state): State<AppState>,
    Path(hostname): Path<String>,
    _headers: HeaderMap,
    cookies: CookieJar,
    Json(payload): Json<PlanChangeRequest>,
) -> AppResult<impl IntoResponse> {
    let root_domain = extract_root_from_reauth_hostname(&hostname);

    // Get user from token
    let (user_id, domain_id) = get_current_user(&app_state, &cookies, &root_domain)?;

    // Execute plan change
    let result = app_state
        .billing_use_cases
        .change_plan(domain_id, user_id, &payload.plan_code)
        .await?;

    Ok(Json(PlanChangeResponse {
        success: result.success,
        change_type: result.change_type.as_ref().to_string(),
        invoice_id: result.invoice_id,
        amount_charged_cents: result.amount_charged_cents,
        currency: result.currency,
        client_secret: result.client_secret,
        hosted_invoice_url: result.hosted_invoice_url,
        payment_intent_status: result.payment_intent_status,
        new_plan: PlanChangeNewPlanResponse {
            code: result.new_plan.code,
            name: result.new_plan.name,
            price_cents: result.new_plan.price_cents,
            currency: result.new_plan.currency,
            interval: result.new_plan.interval,
            interval_count: result.new_plan.interval_count,
            features: result.new_plan.features,
        },
        effective_at: result.effective_at,
        schedule_id: result.schedule_id,
    }))
}

/// GET /api/public/domain/{domain}/billing/providers
/// Returns the list of active payment providers for this domain
async fn get_available_providers(
    State(app_state): State<AppState>,
    Path(hostname): Path<String>,
) -> AppResult<impl IntoResponse> {
    let root_domain = extract_root_from_reauth_hostname(&hostname);

    let domain = app_state
        .domain_auth_use_cases
        .get_domain_by_name(&root_domain)
        .await?
        .ok_or(AppError::NotFound)?;

    let active_providers = app_state
        .billing_use_cases
        .list_active_providers(domain.id)
        .await?;

    let response: Vec<AvailableProvider> = active_providers
        .into_iter()
        .map(|p| AvailableProvider {
            id: p.id,
            domain_id: p.domain_id,
            provider: p.provider,
            mode: p.mode,
            is_active: p.is_active,
            display_order: p.display_order,
            created_at: p.created_at,
        })
        .collect();

    Ok(Json(response))
}

// ============================================================================
// Router
// ============================================================================

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/{domain}/billing/plans", get(get_public_plans))
        .route("/{domain}/billing/subscription", get(get_user_subscription))
        .route("/{domain}/billing/checkout", post(create_checkout))
        .route("/{domain}/billing/portal", post(create_portal))
        .route("/{domain}/billing/cancel", post(cancel_subscription))
        .route("/{domain}/billing/payments", get(get_user_payments))
        .route(
            "/{domain}/billing/plan-change/preview",
            get(preview_plan_change),
        )
        .route("/{domain}/billing/plan-change", post(change_plan))
        .route("/{domain}/billing/providers", get(get_available_providers))
}
