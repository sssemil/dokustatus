//! Stripe webhook handlers.

use super::common::*;
use crate::application::use_cases::domain_billing::{
    CreateSubscriptionInput, StripeSubscriptionUpdate,
};
use crate::domain::entities::payment_status::PaymentStatus;
use crate::domain::entities::user_subscription::SubscriptionStatus;
use crate::infra::stripe_client::StripeClient;
use chrono::{DateTime, NaiveDateTime, Utc};

// ============================================================================
// Helper Functions
// ============================================================================

/// Convert a Unix timestamp to NaiveDateTime
fn timestamp_to_naive(secs: i64) -> Option<NaiveDateTime> {
    DateTime::<Utc>::from_timestamp(secs, 0).map(|dt| dt.naive_utc())
}

/// Determines if a webhook processing error should trigger a Stripe retry.
///
/// Returns `true` if the error is retryable (transient), meaning we should
/// return 5xx to Stripe so they retry the webhook.
///
/// Returns `false` if the error is non-retryable (expected condition like
/// customer not found), meaning we should return 2xx and log.
fn is_retryable_error(error: &AppError) -> bool {
    match error {
        // Transient errors - retry may succeed
        AppError::Database(_) => true,
        AppError::Internal(_) => true,
        AppError::RateLimited => true,

        // Expected conditions - won't change with retry
        AppError::NotFound => false,
        AppError::InvalidInput(_) => false,
        AppError::ValidationError(_) => false,
        AppError::Forbidden => false,
        AppError::InvalidCredentials => false,
        AppError::InvalidApiKey => false,
        AppError::AccountSuspended => false,
        AppError::SessionMismatch => false,
        AppError::TooManyDocuments => false,
        AppError::PaymentDeclined(_) => false,
        AppError::ProviderNotConfigured => false,
        AppError::ProviderNotSupported => false,

        // Unknown/new variants - safer to retry
        #[allow(unreachable_patterns)]
        _ => true,
    }
}

/// Returns 500 Internal Server Error for Stripe to retry the webhook.
/// Logs the error with full context for debugging and future metrics extraction.
fn webhook_retryable_error(
    error: &AppError,
    event_type: &str,
    event_id: &str,
    context: &str,
) -> StatusCode {
    error!(
        error = %error,
        event_type,
        event_id,
        context,
        retryable = true,
        "Webhook processing failed, returning 500 for Stripe retry"
    );
    StatusCode::INTERNAL_SERVER_ERROR
}

// ============================================================================
// Handlers
// ============================================================================

/// POST /api/public/domain/{domain}/billing/webhook/test
/// Handles Stripe webhook events for test mode
async fn handle_webhook_test(
    state: State<AppState>,
    path: Path<String>,
    headers: HeaderMap,
    body: String,
) -> AppResult<impl IntoResponse> {
    handle_webhook_for_mode(state, path, headers, body, PaymentMode::Test).await
}

/// POST /api/public/domain/{domain}/billing/webhook/live
/// Handles Stripe webhook events for live mode
async fn handle_webhook_live(
    state: State<AppState>,
    path: Path<String>,
    headers: HeaderMap,
    body: String,
) -> AppResult<impl IntoResponse> {
    handle_webhook_for_mode(state, path, headers, body, PaymentMode::Live).await
}

/// Internal webhook handler that processes events for a specific mode
async fn handle_webhook_for_mode(
    State(app_state): State<AppState>,
    Path(hostname): Path<String>,
    headers: HeaderMap,
    body: String,
    payment_mode: PaymentMode,
) -> AppResult<impl IntoResponse> {
    let root_domain = extract_root_from_reauth_hostname(&hostname);

    // Get domain
    let domain = app_state
        .domain_auth_use_cases
        .get_domain_by_name(&root_domain)
        .await?
        .ok_or(AppError::NotFound)?;

    // Get webhook secret for the specific mode
    let webhook_secret = app_state
        .billing_use_cases
        .get_stripe_webhook_secret_for_mode(domain.id, payment_mode)
        .await?;

    // Get Stripe signature
    let signature = headers
        .get("stripe-signature")
        .and_then(|v| v.to_str().ok())
        .ok_or(AppError::InvalidInput("Missing Stripe signature".into()))?;

    // Verify signature
    StripeClient::verify_webhook_signature(&body, signature, &webhook_secret)?;

    // Parse event
    let event: serde_json::Value = serde_json::from_str(&body)
        .map_err(|e| AppError::InvalidInput(format!("Invalid webhook payload: {}", e)))?;

    let event_type = event["type"].as_str().unwrap_or("");
    let event_id = event["id"].as_str().unwrap_or("");

    // Check idempotency
    if app_state
        .billing_use_cases
        .is_event_processed(event_id)
        .await?
    {
        return Ok(StatusCode::OK);
    }

    // Handle event types
    match event_type {
        "checkout.session.completed" => {
            handle_checkout_session_completed(
                &app_state,
                &event,
                &domain,
                payment_mode,
                event_type,
                event_id,
            )
            .await?
        }
        "customer.subscription.updated" | "customer.subscription.deleted" => {
            handle_subscription_update(
                &app_state,
                &event,
                &domain,
                payment_mode,
                event_type,
                event_id,
            )
            .await?
        }
        // Invoice events for payment history tracking
        // Note: invoice.payment_succeeded is the newer event name (some Stripe configs use it)
        "invoice.created"
        | "invoice.paid"
        | "invoice.payment_succeeded"
        | "invoice.updated"
        | "invoice.finalized" => {
            handle_invoice_sync(
                &app_state,
                &event,
                &domain,
                payment_mode,
                event_type,
                event_id,
            )
            .await?
        }
        "invoice.payment_failed" => {
            handle_invoice_payment_failed(
                &app_state,
                &event,
                &domain,
                payment_mode,
                event_type,
                event_id,
            )
            .await?
        }
        "invoice.voided" => handle_invoice_voided(&app_state, &event, event_type, event_id).await?,
        "invoice.marked_uncollectible" => {
            handle_invoice_uncollectible(&app_state, &event, event_type, event_id).await?
        }
        "charge.refunded" => {
            handle_charge_refunded(&app_state, &event, event_type, event_id).await?
        }
        "charge.succeeded" => {
            // Backup confirmation of payment - sync invoice if we have one
            let charge = &event["data"]["object"];
            if let Some(invoice_id) = charge["invoice"].as_str() {
                tracing::debug!(
                    "Charge succeeded for invoice {}, invoice event should handle sync",
                    invoice_id
                );
            }
        }
        "charge.failed" => handle_charge_failed(&app_state, &event, event_type, event_id).await?,
        "charge.dispute.created" => {
            // Dispute opened - log for awareness (could add dispute tracking later)
            let dispute = &event["data"]["object"];
            let charge_id = dispute["charge"].as_str().unwrap_or("unknown");
            let amount = dispute["amount"].as_i64().unwrap_or(0);
            tracing::warn!(
                "Dispute opened for charge {} (amount: {} cents) on domain {}",
                charge_id,
                amount,
                domain.domain
            );
        }
        "charge.dispute.closed" => {
            let dispute = &event["data"]["object"];
            let status = dispute["status"].as_str().unwrap_or("unknown");
            let charge_id = dispute["charge"].as_str().unwrap_or("unknown");
            tracing::info!(
                "Dispute closed for charge {} with status: {}",
                charge_id,
                status
            );
        }
        "checkout.session.async_payment_failed" => {
            // Async payment (bank transfer, etc.) failed
            let session = &event["data"]["object"];
            let session_id = session["id"].as_str().unwrap_or("unknown");
            tracing::warn!("Async payment failed for checkout session {}", session_id);
        }
        "checkout.session.expired" => {
            // Checkout was abandoned
            let session = &event["data"]["object"];
            let session_id = session["id"].as_str().unwrap_or("unknown");
            tracing::debug!("Checkout session {} expired", session_id);
        }
        "customer.subscription.trial_will_end" => {
            // Trial ending soon - could trigger notification
            let subscription = &event["data"]["object"];
            let sub_id = subscription["id"].as_str().unwrap_or("unknown");
            let trial_end = subscription["trial_end"].as_i64();
            tracing::info!(
                "Trial will end for subscription {}: {:?}",
                sub_id,
                trial_end
            );
        }
        _ => {
            tracing::debug!("Unhandled webhook event type: {}", event_type);
        }
    }

    Ok(StatusCode::OK)
}

// ============================================================================
// Event Handlers
// ============================================================================

async fn handle_checkout_session_completed(
    app_state: &AppState,
    event: &serde_json::Value,
    domain: &crate::application::use_cases::domain::DomainProfile,
    payment_mode: PaymentMode,
    event_type: &str,
    event_id: &str,
) -> AppResult<()> {
    let session = &event["data"]["object"];
    let customer_id = session["customer"].as_str().unwrap_or("");
    let subscription_id = session["subscription"].as_str();
    let client_reference_id = session["client_reference_id"].as_str();

    // Both subscription_id and client_reference_id are required for processing
    let (sub_id, user_id_str) = match (subscription_id, client_reference_id) {
        (Some(s), Some(u)) => (s, u),
        _ => {
            // One-time payment or missing data - nothing to process
            tracing::debug!(
                event_id,
                "checkout.session.completed without subscription or client_reference_id"
            );
            return Ok(());
        }
    };

    let user_id = match Uuid::parse_str(user_id_str) {
        Ok(id) => id,
        Err(_) => {
            tracing::debug!(
                event_id,
                user_id_str,
                retryable = false,
                "Invalid user_id format in client_reference_id"
            );
            return Ok(());
        }
    };

    // Get the Stripe subscription to find the price ID
    let secret_key = app_state
        .billing_use_cases
        .get_stripe_secret_key(domain.id)
        .await?;
    let stripe = StripeClient::new(secret_key);

    let stripe_sub = match stripe.get_subscription(sub_id).await {
        Ok(s) => s,
        Err(e) if is_retryable_error(&e) => {
            return Err(AppError::Internal(
                webhook_retryable_error(&e, event_type, event_id, "fetch subscription").to_string(),
            ));
        }
        Err(e) => {
            tracing::debug!(
                error = %e,
                sub_id,
                event_id,
                retryable = false,
                "Non-retryable error fetching subscription, skipping"
            );
            return Ok(());
        }
    };

    // Find plan by Stripe price ID - search ALL plans (not just public)
    // because plan visibility can change after purchase
    let plan = match app_state
        .billing_use_cases
        .get_plan_by_stripe_price_id(domain.id, payment_mode, &stripe_sub.price_id())
        .await
    {
        Ok(Some(p)) => p,
        Ok(None) => {
            // Configuration error - plan exists in Stripe but not in our system
            error!(
                price_id = stripe_sub.price_id(),
                domain_id = %domain.id,
                event_id,
                "CONFIGURATION ERROR: No plan found for Stripe price_id. User subscription may be missing!"
            );
            return Ok(());
        }
        Err(e) if is_retryable_error(&e) => {
            return Err(AppError::Internal(
                webhook_retryable_error(&e, event_type, event_id, "lookup plan").to_string(),
            ));
        }
        Err(e) => {
            tracing::debug!(
                error = %e,
                event_id,
                retryable = false,
                "Non-retryable error looking up plan"
            );
            return Ok(());
        }
    };

    // Map Stripe status to our SubscriptionStatus - don't assume Active
    let status = match stripe_sub.status.as_str() {
        "active" => SubscriptionStatus::Active,
        "past_due" => SubscriptionStatus::PastDue,
        "canceled" => SubscriptionStatus::Canceled,
        "trialing" => SubscriptionStatus::Trialing,
        "incomplete" => SubscriptionStatus::Incomplete,
        "incomplete_expired" => SubscriptionStatus::IncompleteExpired,
        "unpaid" => SubscriptionStatus::Unpaid,
        "paused" => SubscriptionStatus::Paused,
        // Default to Incomplete - never grant access by default
        _ => SubscriptionStatus::Incomplete,
    };

    let input = CreateSubscriptionInput {
        domain_id: domain.id,
        payment_mode,
        end_user_id: user_id,
        plan_id: plan.id,
        stripe_customer_id: customer_id.to_string(),
        stripe_subscription_id: Some(sub_id.to_string()),
        status,
        current_period_start: timestamp_to_naive(stripe_sub.current_period_start),
        current_period_end: timestamp_to_naive(stripe_sub.current_period_end),
        trial_start: stripe_sub.trial_start.and_then(timestamp_to_naive),
        trial_end: stripe_sub.trial_end.and_then(timestamp_to_naive),
    };

    // Create subscription - MUST succeed for user access
    let created_sub = match app_state
        .billing_use_cases
        .create_or_update_subscription(&input)
        .await
    {
        Ok(s) => s,
        Err(e) if is_retryable_error(&e) => {
            return Err(AppError::Internal(
                webhook_retryable_error(&e, event_type, event_id, "create subscription")
                    .to_string(),
            ));
        }
        Err(e) => {
            // Non-retryable but critical - user won't have access!
            error!(
                error = %e,
                user_id = %user_id,
                event_id,
                retryable = false,
                "CRITICAL: Non-retryable subscription creation failure - user may lack access!"
            );
            return Ok(());
        }
    };

    // Event logging is non-critical, don't fail on logging errors
    if let Err(e) = app_state
        .billing_use_cases
        .log_webhook_event(
            created_sub.id,
            event_type,
            None,
            Some(status),
            event_id,
            serde_json::json!({"customer_id": customer_id, "stripe_status": &stripe_sub.status}),
        )
        .await
    {
        tracing::warn!(error = %e, event_id, "Failed to log webhook event (non-critical)");
    }

    Ok(())
}

async fn handle_subscription_update(
    app_state: &AppState,
    event: &serde_json::Value,
    domain: &crate::application::use_cases::domain::DomainProfile,
    payment_mode: PaymentMode,
    event_type: &str,
    event_id: &str,
) -> AppResult<()> {
    let subscription = &event["data"]["object"];
    let stripe_sub_id = subscription["id"].as_str().unwrap_or("");
    let status_str = subscription["status"].as_str().unwrap_or("");

    let new_status = match status_str {
        "active" => SubscriptionStatus::Active,
        "past_due" => SubscriptionStatus::PastDue,
        "canceled" => SubscriptionStatus::Canceled,
        "trialing" => SubscriptionStatus::Trialing,
        "incomplete" => SubscriptionStatus::Incomplete,
        "incomplete_expired" => SubscriptionStatus::IncompleteExpired,
        "unpaid" => SubscriptionStatus::Unpaid,
        "paused" => SubscriptionStatus::Paused,
        // Default to Incomplete for unknown statuses - never grant access by default
        _ => SubscriptionStatus::Incomplete,
    };

    // Extract price_id from subscription items to handle plan upgrades/downgrades
    let stripe_price_id = subscription["items"]["data"]
        .as_array()
        .and_then(|items| items.first())
        .and_then(|item| item["price"]["id"].as_str());

    // Look up plan by stripe_price_id to handle plan changes
    // Use the webhook mode for plan lookup
    let plan_id = if let Some(price_id) = stripe_price_id {
        app_state
            .billing_use_cases
            .get_plan_by_stripe_price_id(domain.id, payment_mode, price_id)
            .await
            .ok()
            .flatten()
            .map(|p| p.id)
    } else {
        None
    };

    let update = StripeSubscriptionUpdate {
        status: new_status,
        plan_id, // Update plan if it changed (upgrade/downgrade via Stripe portal)
        stripe_subscription_id: None, // Already set, don't overwrite
        current_period_start: subscription["current_period_start"]
            .as_i64()
            .and_then(timestamp_to_naive),
        current_period_end: subscription["current_period_end"]
            .as_i64()
            .and_then(timestamp_to_naive),
        cancel_at_period_end: subscription["cancel_at_period_end"]
            .as_bool()
            .unwrap_or(false),
        canceled_at: subscription["canceled_at"]
            .as_i64()
            .and_then(timestamp_to_naive),
        trial_start: subscription["trial_start"]
            .as_i64()
            .and_then(timestamp_to_naive),
        trial_end: subscription["trial_end"]
            .as_i64()
            .and_then(timestamp_to_naive),
    };

    match app_state
        .billing_use_cases
        .update_subscription_from_stripe(stripe_sub_id, &update)
        .await
    {
        Ok(updated_sub) => {
            // Log event - non-critical
            if let Err(e) = app_state
                .billing_use_cases
                .log_webhook_event(
                    updated_sub.id,
                    event_type,
                    None,
                    Some(new_status),
                    event_id,
                    serde_json::json!({"stripe_status": status_str}),
                )
                .await
            {
                tracing::warn!(error = %e, event_id, "Failed to log subscription update event");
            }
        }
        Err(e) if is_retryable_error(&e) => {
            return Err(AppError::Internal(
                webhook_retryable_error(&e, event_type, event_id, "update subscription")
                    .to_string(),
            ));
        }
        Err(e) => {
            // NotFound = subscription not in our system, expected for external customers
            tracing::debug!(
                error = %e,
                stripe_sub_id,
                event_id,
                retryable = false,
                "Subscription not found in our system, skipping"
            );
        }
    }

    Ok(())
}

async fn handle_invoice_sync(
    app_state: &AppState,
    event: &serde_json::Value,
    domain: &crate::application::use_cases::domain::DomainProfile,
    payment_mode: PaymentMode,
    event_type: &str,
    event_id: &str,
) -> AppResult<()> {
    let invoice = &event["data"]["object"];

    // Try to sync the invoice to our payments table
    match app_state
        .billing_use_cases
        .sync_invoice_from_webhook(domain.id, payment_mode, invoice)
        .await
    {
        Ok(_payment) => {
            tracing::info!(event_type, event_id, "Synced payment from webhook");
        }
        Err(e) if is_retryable_error(&e) => {
            // DB error - retry to prevent data loss
            return Err(AppError::Internal(
                webhook_retryable_error(&e, event_type, event_id, "sync invoice").to_string(),
            ));
        }
        Err(e) => {
            // NotFound = customer not in our system, expected
            tracing::debug!(
                error = %e,
                event_type,
                event_id,
                retryable = false,
                "Could not sync invoice (non-retryable), skipping"
            );
        }
    }

    Ok(())
}

async fn handle_invoice_payment_failed(
    app_state: &AppState,
    event: &serde_json::Value,
    domain: &crate::application::use_cases::domain::DomainProfile,
    payment_mode: PaymentMode,
    event_type: &str,
    event_id: &str,
) -> AppResult<()> {
    let invoice = &event["data"]["object"];
    let invoice_id = invoice["id"].as_str().unwrap_or("");

    // First try to sync/create the invoice
    let _ = app_state
        .billing_use_cases
        .sync_invoice_from_webhook(domain.id, payment_mode, invoice)
        .await;

    // Extract failure message from the invoice
    let failure_message = invoice["last_finalization_error"]["message"]
        .as_str()
        .or_else(|| invoice["last_payment_error"]["message"].as_str())
        .map(|s| s.to_string());

    // Update status to failed
    if let Err(e) = app_state
        .billing_use_cases
        .update_payment_status(invoice_id, PaymentStatus::Failed, None, failure_message)
        .await
    {
        if is_retryable_error(&e) {
            return Err(AppError::Internal(
                webhook_retryable_error(&e, event_type, event_id, "update payment status")
                    .to_string(),
            ));
        } else {
            // Non-retryable: record might not exist (customer not in our system)
            tracing::debug!(
                error = %e,
                invoice_id,
                event_id,
                retryable = false,
                "Could not update payment status - record may not exist"
            );
        }
    }

    Ok(())
}

async fn handle_invoice_voided(
    app_state: &AppState,
    event: &serde_json::Value,
    event_type: &str,
    event_id: &str,
) -> AppResult<()> {
    let invoice = &event["data"]["object"];
    let invoice_id = invoice["id"].as_str().unwrap_or("");

    if let Err(e) = app_state
        .billing_use_cases
        .update_payment_status(invoice_id, PaymentStatus::Void, None, None)
        .await
    {
        if is_retryable_error(&e) {
            return Err(AppError::Internal(
                webhook_retryable_error(&e, event_type, event_id, "update payment status")
                    .to_string(),
            ));
        } else {
            tracing::debug!(
                error = %e,
                invoice_id,
                event_id,
                retryable = false,
                "Could not update payment status - record may not exist"
            );
        }
    }

    Ok(())
}

async fn handle_invoice_uncollectible(
    app_state: &AppState,
    event: &serde_json::Value,
    event_type: &str,
    event_id: &str,
) -> AppResult<()> {
    let invoice = &event["data"]["object"];
    let invoice_id = invoice["id"].as_str().unwrap_or("");

    if let Err(e) = app_state
        .billing_use_cases
        .update_payment_status(invoice_id, PaymentStatus::Uncollectible, None, None)
        .await
    {
        if is_retryable_error(&e) {
            return Err(AppError::Internal(
                webhook_retryable_error(&e, event_type, event_id, "update payment status")
                    .to_string(),
            ));
        } else {
            tracing::debug!(
                error = %e,
                invoice_id,
                event_id,
                retryable = false,
                "Could not update payment status - record may not exist"
            );
        }
    }

    Ok(())
}

async fn handle_charge_refunded(
    app_state: &AppState,
    event: &serde_json::Value,
    event_type: &str,
    event_id: &str,
) -> AppResult<()> {
    // Handle refunds - need to find the associated invoice
    let charge = &event["data"]["object"];
    let invoice_id = charge["invoice"].as_str();
    let amount_refunded = charge["amount_refunded"].as_i64().unwrap_or(0) as i32;
    let amount = charge["amount"].as_i64().unwrap_or(0) as i32;

    if let Some(invoice_id) = invoice_id {
        // Determine if it's a full or partial refund
        let status = if amount_refunded >= amount {
            PaymentStatus::Refunded
        } else {
            PaymentStatus::PartialRefund
        };

        if let Err(e) = app_state
            .billing_use_cases
            .update_payment_status(invoice_id, status, Some(amount_refunded), None)
            .await
        {
            if is_retryable_error(&e) {
                return Err(AppError::Internal(
                    webhook_retryable_error(&e, event_type, event_id, "update payment status")
                        .to_string(),
                ));
            } else {
                tracing::debug!(
                    error = %e,
                    invoice_id,
                    event_id,
                    retryable = false,
                    "Could not update payment status - record may not exist"
                );
            }
        }
    }

    Ok(())
}

async fn handle_charge_failed(
    app_state: &AppState,
    event: &serde_json::Value,
    event_type: &str,
    event_id: &str,
) -> AppResult<()> {
    // Payment failed - update invoice status if we have one
    let charge = &event["data"]["object"];
    if let Some(invoice_id) = charge["invoice"].as_str() {
        let failure_message = charge["failure_message"].as_str().map(|s| s.to_string());
        if let Err(e) = app_state
            .billing_use_cases
            .update_payment_status(invoice_id, PaymentStatus::Failed, None, failure_message)
            .await
        {
            if is_retryable_error(&e) {
                return Err(AppError::Internal(
                    webhook_retryable_error(&e, event_type, event_id, "update payment status")
                        .to_string(),
                ));
            } else {
                tracing::debug!(
                    error = %e,
                    invoice_id,
                    event_id,
                    retryable = false,
                    "Could not update payment status - record may not exist"
                );
            }
        }
    }

    Ok(())
}

// ============================================================================
// Router
// ============================================================================

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/{domain}/billing/webhook/test", post(handle_webhook_test))
        .route("/{domain}/billing/webhook/live", post(handle_webhook_live))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod webhook_error_tests {
    use super::*;

    #[test]
    fn test_database_errors_are_retryable() {
        assert!(is_retryable_error(&AppError::Database(
            "connection lost".into()
        )));
    }

    #[test]
    fn test_internal_errors_are_retryable() {
        assert!(is_retryable_error(&AppError::Internal("unexpected".into())));
    }

    #[test]
    fn test_rate_limited_is_retryable() {
        assert!(is_retryable_error(&AppError::RateLimited));
    }

    #[test]
    fn test_not_found_is_not_retryable() {
        assert!(!is_retryable_error(&AppError::NotFound));
    }

    #[test]
    fn test_invalid_input_is_not_retryable() {
        assert!(!is_retryable_error(&AppError::InvalidInput(
            "bad data".into()
        )));
    }

    #[test]
    fn test_all_variants_explicitly_handled() {
        // Ensure all known variants have explicit handling
        let test_cases = vec![
            (AppError::Database("test".into()), true),
            (AppError::Internal("test".into()), true),
            (AppError::RateLimited, true),
            (AppError::NotFound, false),
            (AppError::InvalidInput("test".into()), false),
            (AppError::ValidationError("test".into()), false),
            (AppError::Forbidden, false),
            (AppError::InvalidCredentials, false),
            (AppError::InvalidApiKey, false),
            (AppError::AccountSuspended, false),
            (AppError::SessionMismatch, false),
            (AppError::TooManyDocuments, false),
            (AppError::PaymentDeclined("test".into()), false),
            (AppError::ProviderNotConfigured, false),
            (AppError::ProviderNotSupported, false),
        ];

        for (error, expected) in test_cases {
            assert_eq!(
                is_retryable_error(&error),
                expected,
                "Unexpected result for {:?}",
                error
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::Router;
    use axum_test::TestServer;
    use uuid::Uuid;

    use crate::domain::entities::domain::DomainStatus;
    use crate::test_utils::{TestAppStateBuilder, create_test_domain};

    fn build_test_router(app_state: AppState) -> Router<()> {
        router().with_state(app_state)
    }

    // =========================================================================
    // POST /{domain}/billing/webhook/test
    // =========================================================================

    #[tokio::test]
    async fn webhook_test_unknown_domain_returns_404() {
        let app_state = TestAppStateBuilder::new().build();

        let server = TestServer::new(build_test_router(app_state)).unwrap();

        let response = server
            .post("/reauth.unknown.com/billing/webhook/test")
            .text("{}")
            .await;

        response.assert_status(StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn webhook_test_unverified_domain_returns_400() {
        let domain_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.id = domain_id;
            d.domain = "example.com".to_string();
            d.status = DomainStatus::PendingDns;
        });

        let app_state = TestAppStateBuilder::new().with_domain(domain).build();

        let server = TestServer::new(build_test_router(app_state)).unwrap();

        let response = server
            .post("/reauth.example.com/billing/webhook/test")
            .text("{}")
            .await;

        // Unverified domain has no Stripe config
        response.assert_status(StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn webhook_test_no_stripe_config_returns_400() {
        let domain_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.id = domain_id;
            d.domain = "example.com".to_string();
            d.status = DomainStatus::Verified;
        });

        let app_state = TestAppStateBuilder::new().with_domain(domain).build();

        let server = TestServer::new(build_test_router(app_state)).unwrap();

        let response = server
            .post("/reauth.example.com/billing/webhook/test")
            .text("{}")
            .await;

        // No Stripe config means no webhook secret, should fail
        response.assert_status(StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn webhook_test_missing_signature_returns_400() {
        let domain_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.id = domain_id;
            d.domain = "example.com".to_string();
            d.status = DomainStatus::Verified;
        });

        let app_state = TestAppStateBuilder::new().with_domain(domain).build();

        let server = TestServer::new(build_test_router(app_state)).unwrap();

        // Even if Stripe config existed, missing signature should fail
        let response = server
            .post("/reauth.example.com/billing/webhook/test")
            .text("{}")
            .await;

        // Should fail due to no stripe config or missing signature
        response.assert_status(StatusCode::BAD_REQUEST);
    }

    // =========================================================================
    // POST /{domain}/billing/webhook/live
    // =========================================================================

    #[tokio::test]
    async fn webhook_live_unknown_domain_returns_404() {
        let app_state = TestAppStateBuilder::new().build();

        let server = TestServer::new(build_test_router(app_state)).unwrap();

        let response = server
            .post("/reauth.unknown.com/billing/webhook/live")
            .text("{}")
            .await;

        response.assert_status(StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn webhook_live_no_stripe_config_returns_400() {
        let domain_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.id = domain_id;
            d.domain = "example.com".to_string();
            d.status = DomainStatus::Verified;
        });

        let app_state = TestAppStateBuilder::new().with_domain(domain).build();

        let server = TestServer::new(build_test_router(app_state)).unwrap();

        let response = server
            .post("/reauth.example.com/billing/webhook/live")
            .text("{}")
            .await;

        // No Stripe config means no webhook secret
        response.assert_status(StatusCode::BAD_REQUEST);
    }
}
