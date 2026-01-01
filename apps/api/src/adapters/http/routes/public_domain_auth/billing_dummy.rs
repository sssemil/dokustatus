//! Dummy payment provider routes for testing.

use super::common::*;
use crate::application::use_cases::domain_billing::CreateSubscriptionInput;
use crate::domain::entities::payment_mode::PaymentMode;
use crate::domain::entities::payment_provider::PaymentProvider;
use crate::domain::entities::payment_scenario::PaymentScenario;
use crate::domain::entities::user_subscription::SubscriptionStatus;
use chrono::Duration as ChronoDuration;

// ============================================================================
// Types
// ============================================================================

#[derive(Serialize)]
struct DummyScenarioInfo {
    scenario: PaymentScenario,
    display_name: String,
    description: String,
    test_card: String,
}

#[derive(Deserialize)]
struct DummyCheckoutPayload {
    plan_code: String,
    scenario: PaymentScenario,
}

#[derive(Serialize)]
struct DummyCheckoutResponse {
    success: bool,
    requires_confirmation: bool,
    confirmation_token: Option<String>,
    error_message: Option<String>,
    subscription_id: Option<String>,
}

#[derive(Deserialize)]
struct DummyConfirmPayload {
    confirmation_token: String,
}

// ============================================================================
// Handlers
// ============================================================================

/// GET /api/public/domain/{domain}/billing/dummy/scenarios
/// Returns available test scenarios for the dummy payment provider
async fn get_dummy_scenarios(
    State(_app_state): State<AppState>,
    Path(_hostname): Path<String>,
) -> AppResult<impl IntoResponse> {
    let scenarios = vec![
        DummyScenarioInfo {
            scenario: PaymentScenario::Success,
            display_name: "Success".to_string(),
            description: "Payment completes successfully".to_string(),
            test_card: "4242 4242 4242 4242".to_string(),
        },
        DummyScenarioInfo {
            scenario: PaymentScenario::Decline,
            display_name: "Card Declined".to_string(),
            description: "Card is declined by the issuer".to_string(),
            test_card: "4000 0000 0000 0002".to_string(),
        },
        DummyScenarioInfo {
            scenario: PaymentScenario::InsufficientFunds,
            display_name: "Insufficient Funds".to_string(),
            description: "Card has insufficient funds".to_string(),
            test_card: "4000 0000 0000 9995".to_string(),
        },
        DummyScenarioInfo {
            scenario: PaymentScenario::ThreeDSecure,
            display_name: "3D Secure Required".to_string(),
            description: "Requires additional authentication".to_string(),
            test_card: "4000 0000 0000 3220".to_string(),
        },
        DummyScenarioInfo {
            scenario: PaymentScenario::ExpiredCard,
            display_name: "Expired Card".to_string(),
            description: "Card has expired".to_string(),
            test_card: "4000 0000 0000 0069".to_string(),
        },
        DummyScenarioInfo {
            scenario: PaymentScenario::ProcessingError,
            display_name: "Processing Error".to_string(),
            description: "A processing error occurred".to_string(),
            test_card: "4000 0000 0000 0119".to_string(),
        },
    ];

    Ok(Json(scenarios))
}

/// POST /api/public/domain/{domain}/billing/checkout/dummy
/// Creates a test subscription using the dummy payment provider
async fn create_dummy_checkout(
    State(app_state): State<AppState>,
    Path(hostname): Path<String>,
    cookies: CookieJar,
    Json(payload): Json<DummyCheckoutPayload>,
) -> AppResult<impl IntoResponse> {
    let root_domain = extract_root_from_reauth_hostname(&hostname);

    // Get user from token
    let (user_id, domain_id) = get_current_user(&app_state, &cookies, &root_domain)?;

    // Verify dummy provider is enabled
    let is_enabled = app_state
        .billing_use_cases
        .is_provider_enabled(domain_id, PaymentProvider::Dummy, PaymentMode::Test)
        .await?;

    if !is_enabled {
        return Err(AppError::InvalidInput(
            "Dummy payment provider is not enabled for this domain".into(),
        ));
    }

    // Get the plan
    let plan = app_state
        .billing_use_cases
        .get_plan_by_code(domain_id, &payload.plan_code)
        .await?
        .ok_or(AppError::NotFound)?;

    // Build response based on scenario
    let response = match payload.scenario {
        PaymentScenario::Success => {
            // Generate subscription ID
            let subscription_id_str = format!("dummy_sub_{}", Uuid::new_v4());

            let now = chrono::Utc::now().naive_utc();
            let period_end = now
                + ChronoDuration::days(match plan.interval.as_str() {
                    "yearly" => 365 * plan.interval_count as i64,
                    _ => 30 * plan.interval_count as i64, // monthly default
                });

            let subscription = app_state
                .billing_use_cases
                .create_or_update_subscription(&CreateSubscriptionInput {
                    domain_id,
                    stripe_mode: StripeMode::Test,
                    end_user_id: user_id,
                    plan_id: plan.id,
                    stripe_customer_id: format!("dummy_cus_{}", user_id),
                    stripe_subscription_id: Some(subscription_id_str.clone()),
                    status: SubscriptionStatus::Active,
                    current_period_start: Some(now),
                    current_period_end: Some(period_end),
                    trial_start: None,
                    trial_end: None,
                })
                .await?;

            // Create payment record
            app_state
                .billing_use_cases
                .create_dummy_payment(domain_id, user_id, subscription.id, &plan)
                .await?;

            DummyCheckoutResponse {
                success: true,
                requires_confirmation: false,
                confirmation_token: None,
                error_message: None,
                subscription_id: Some(subscription_id_str),
            }
        }
        PaymentScenario::ThreeDSecure => {
            // Encode plan_code in the token so confirm endpoint can use it
            DummyCheckoutResponse {
                success: false,
                requires_confirmation: true,
                confirmation_token: Some(format!(
                    "3ds_token_{}_{}",
                    payload.plan_code,
                    Uuid::new_v4()
                )),
                error_message: None,
                subscription_id: None,
            }
        }
        PaymentScenario::Decline => DummyCheckoutResponse {
            success: false,
            requires_confirmation: false,
            confirmation_token: None,
            error_message: Some("Your card was declined".into()),
            subscription_id: None,
        },
        PaymentScenario::InsufficientFunds => DummyCheckoutResponse {
            success: false,
            requires_confirmation: false,
            confirmation_token: None,
            error_message: Some("Your card has insufficient funds".into()),
            subscription_id: None,
        },
        PaymentScenario::ExpiredCard => DummyCheckoutResponse {
            success: false,
            requires_confirmation: false,
            confirmation_token: None,
            error_message: Some("Your card has expired".into()),
            subscription_id: None,
        },
        PaymentScenario::ProcessingError => DummyCheckoutResponse {
            success: false,
            requires_confirmation: false,
            confirmation_token: None,
            error_message: Some("A processing error occurred. Please try again.".into()),
            subscription_id: None,
        },
    };

    // Log for debugging
    tracing::info!(
        domain_id = %domain_id,
        user_id = %user_id,
        scenario = ?payload.scenario,
        success = response.success,
        "Dummy checkout processed"
    );

    Ok(Json(response))
}

/// POST /api/public/domain/{domain}/billing/dummy/confirm
/// Confirms a 3DS payment for the dummy provider
async fn confirm_dummy_checkout(
    State(app_state): State<AppState>,
    Path(hostname): Path<String>,
    cookies: CookieJar,
    Json(payload): Json<DummyConfirmPayload>,
) -> AppResult<impl IntoResponse> {
    let root_domain = extract_root_from_reauth_hostname(&hostname);

    // Get user from token
    let (user_id, domain_id) = get_current_user(&app_state, &cookies, &root_domain)?;

    // Verify and parse the token (format: 3ds_token_{plan_code}_{uuid})
    if !payload.confirmation_token.starts_with("3ds_token_") {
        return Err(AppError::InvalidInput("Invalid confirmation token".into()));
    }

    // Extract plan_code from token: 3ds_token_{plan_code}_{uuid}
    let token_parts: Vec<&str> = payload.confirmation_token.splitn(4, '_').collect();
    if token_parts.len() < 4 {
        return Err(AppError::InvalidInput(
            "Invalid confirmation token format".into(),
        ));
    }
    let plan_code = token_parts[2]; // 3ds, token, {plan_code}, {uuid}

    // Get the plan
    let plan = app_state
        .billing_use_cases
        .get_plan_by_code(domain_id, plan_code)
        .await?
        .ok_or(AppError::NotFound)?;

    // Create subscription and payment records
    let subscription_id_str = format!("dummy_sub_{}", Uuid::new_v4());
    let now = chrono::Utc::now().naive_utc();
    let period_end = now
        + ChronoDuration::days(match plan.interval.as_str() {
            "yearly" => 365 * plan.interval_count as i64,
            _ => 30 * plan.interval_count as i64,
        });

    let subscription = app_state
        .billing_use_cases
        .create_or_update_subscription(&CreateSubscriptionInput {
            domain_id,
            stripe_mode: StripeMode::Test,
            end_user_id: user_id,
            plan_id: plan.id,
            stripe_customer_id: format!("dummy_cus_{}", user_id),
            stripe_subscription_id: Some(subscription_id_str.clone()),
            status: SubscriptionStatus::Active,
            current_period_start: Some(now),
            current_period_end: Some(period_end),
            trial_start: None,
            trial_end: None,
        })
        .await?;

    // Create payment record
    app_state
        .billing_use_cases
        .create_dummy_payment(domain_id, user_id, subscription.id, &plan)
        .await?;

    let response = DummyCheckoutResponse {
        success: true,
        requires_confirmation: false,
        confirmation_token: None,
        error_message: None,
        subscription_id: Some(subscription_id_str),
    };

    tracing::info!(
        domain_id = %domain_id,
        user_id = %user_id,
        plan_code = %plan_code,
        "Dummy 3DS confirmation processed"
    );

    Ok(Json(response))
}

// ============================================================================
// Router
// ============================================================================

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/{domain}/billing/checkout/dummy",
            post(create_dummy_checkout),
        )
        .route(
            "/{domain}/billing/dummy/confirm",
            post(confirm_dummy_checkout),
        )
        .route(
            "/{domain}/billing/dummy/scenarios",
            get(get_dummy_scenarios),
        )
}
