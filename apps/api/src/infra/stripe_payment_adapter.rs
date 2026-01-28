use async_trait::async_trait;
use chrono::{DateTime, TimeZone, Utc};
use std::collections::HashMap;
use uuid::Uuid;

use crate::{
    app_error::{AppError, AppResult},
    application::ports::payment_provider::{
        CheckoutResult, CheckoutUrls, CustomerId, CustomerInfo, InvoiceInfo, InvoicePdfResult,
        PaymentProviderPort, PlanChangeResult, PlanChangeType, PlanInfo, SubscriptionId,
        SubscriptionInfo, SubscriptionResult,
    },
    domain::entities::{
        payment_mode::PaymentMode, payment_provider::PaymentProvider,
        payment_scenario::PaymentScenario, user_subscription::SubscriptionStatus,
    },
    infra::stripe_client::StripeClient,
};

/// Adapter that wraps StripeClient to implement PaymentProviderPort.
///
/// This adapter translates domain-action-based calls to Stripe API calls.
#[derive(Clone)]
pub struct StripePaymentAdapter {
    client: StripeClient,
    mode: PaymentMode,
}

impl StripePaymentAdapter {
    pub fn new(secret_key: String, mode: PaymentMode) -> Self {
        Self {
            client: StripeClient::new(secret_key),
            mode,
        }
    }

    /// Get the underlying StripeClient for operations not covered by the trait
    pub fn client(&self) -> &StripeClient {
        &self.client
    }

    /// Convert Stripe subscription status to domain status
    fn map_subscription_status(status: &str) -> SubscriptionStatus {
        match status {
            "active" => SubscriptionStatus::Active,
            "trialing" => SubscriptionStatus::Trialing,
            "past_due" => SubscriptionStatus::PastDue,
            "canceled" => SubscriptionStatus::Canceled,
            "incomplete" => SubscriptionStatus::Incomplete,
            "incomplete_expired" => SubscriptionStatus::IncompleteExpired,
            "unpaid" => SubscriptionStatus::Unpaid,
            "paused" => SubscriptionStatus::Paused,
            _ => SubscriptionStatus::Incomplete,
        }
    }

    /// Convert timestamp to DateTime<Utc>
    fn timestamp_to_datetime(ts: i64) -> DateTime<Utc> {
        Utc.timestamp_opt(ts, 0).single().unwrap_or_else(Utc::now)
    }

    /// Convert optional timestamp to Option<DateTime<Utc>>
    fn opt_timestamp_to_datetime(ts: Option<i64>) -> Option<DateTime<Utc>> {
        ts.map(Self::timestamp_to_datetime)
    }
}

#[async_trait]
impl PaymentProviderPort for StripePaymentAdapter {
    fn provider(&self) -> PaymentProvider {
        PaymentProvider::Stripe
    }

    fn mode(&self) -> PaymentMode {
        self.mode
    }

    // ========================================================================
    // Customer Management
    // ========================================================================

    async fn ensure_customer(
        &self,
        email: &str,
        user_id: Uuid,
        domain_id: Uuid,
    ) -> AppResult<CustomerId> {
        let metadata = HashMap::from([
            ("user_id".to_string(), user_id.to_string()),
            ("domain_id".to_string(), domain_id.to_string()),
        ]);

        let customer = self
            .client
            .get_or_create_customer(email, Some(metadata))
            .await?;
        Ok(CustomerId::new(customer.id))
    }

    async fn get_customer(&self, customer_id: &CustomerId) -> AppResult<Option<CustomerInfo>> {
        match self.client.get_customer(customer_id.as_str()).await {
            Ok(customer) => Ok(Some(CustomerInfo {
                customer_id: customer_id.clone(),
                email: customer.email,
                metadata: HashMap::new(), // Stripe's get_customer doesn't return metadata in our current impl
            })),
            Err(AppError::NotFound) => Ok(None),
            Err(e) => Err(e),
        }
    }

    // ========================================================================
    // Checkout & Subscription Creation
    // ========================================================================

    async fn create_checkout(
        &self,
        customer: &CustomerId,
        plan: &PlanInfo,
        urls: &CheckoutUrls,
        _scenario: Option<PaymentScenario>, // Ignored for Stripe
    ) -> AppResult<CheckoutResult> {
        let price_id = plan
            .external_price_id
            .as_ref()
            .ok_or_else(|| AppError::InvalidInput("Plan missing Stripe price ID".to_string()))?;

        let trial_days = if plan.trial_days > 0 {
            Some(plan.trial_days)
        } else {
            None
        };

        let session = self
            .client
            .create_checkout_session(
                customer.as_str(),
                price_id,
                &urls.success_url,
                &urls.cancel_url,
                None, // client_reference_id - could be user_id
                trial_days,
            )
            .await?;

        Ok(CheckoutResult {
            checkout_url: session.url,
            is_inline: false,
            session_id: Some(session.id),
        })
    }

    async fn start_subscription(
        &self,
        _customer: &CustomerId,
        _plan: &PlanInfo,
        _scenario: Option<PaymentScenario>,
    ) -> AppResult<SubscriptionResult> {
        // Stripe doesn't support direct subscription creation without checkout
        // This would require using the Subscriptions API with a payment method
        Err(AppError::InvalidInput(
            "Stripe requires checkout flow for subscription creation".to_string(),
        ))
    }

    async fn confirm_subscription(
        &self,
        _confirmation_token: &str,
    ) -> AppResult<SubscriptionResult> {
        // 3DS confirmation is handled by Stripe.js on the frontend
        // This endpoint is for Stripe webhooks to update subscription status
        Err(AppError::InvalidInput(
            "Stripe 3DS confirmation is handled by frontend".to_string(),
        ))
    }

    // ========================================================================
    // Subscription Lifecycle
    // ========================================================================

    async fn get_subscription(
        &self,
        subscription_id: &SubscriptionId,
    ) -> AppResult<Option<SubscriptionInfo>> {
        match self.client.get_subscription(subscription_id.as_str()).await {
            Ok(sub) => Ok(Some(SubscriptionInfo {
                subscription_id: SubscriptionId::new(&sub.id),
                customer_id: CustomerId::new(&sub.customer),
                status: Self::map_subscription_status(&sub.status),
                current_period_start: Some(Self::timestamp_to_datetime(sub.current_period_start)),
                current_period_end: Some(Self::timestamp_to_datetime(sub.current_period_end)),
                trial_start: Self::opt_timestamp_to_datetime(sub.trial_start),
                trial_end: Self::opt_timestamp_to_datetime(sub.trial_end),
                cancel_at_period_end: sub.cancel_at_period_end,
                canceled_at: Self::opt_timestamp_to_datetime(sub.canceled_at),
                price_id: Some(sub.price_id()),
                subscription_item_id: sub.subscription_item_id(),
            })),
            Err(AppError::NotFound) => Ok(None),
            Err(AppError::InvalidInput(msg)) if msg.contains("No such subscription") => Ok(None),
            Err(e) => Err(e),
        }
    }

    async fn cancel_subscription(
        &self,
        subscription_id: &SubscriptionId,
        at_period_end: bool,
    ) -> AppResult<()> {
        self.client
            .cancel_subscription(subscription_id.as_str(), at_period_end)
            .await?;
        Ok(())
    }

    async fn change_plan(
        &self,
        subscription_id: &SubscriptionId,
        subscription_item_id: Option<&str>,
        new_plan: &PlanInfo,
        is_trial: bool,
    ) -> AppResult<PlanChangeResult> {
        let sub = self
            .client
            .get_subscription(subscription_id.as_str())
            .await?;

        let item_id = subscription_item_id
            .map(|s| s.to_string())
            .or_else(|| sub.subscription_item_id())
            .ok_or_else(|| AppError::InvalidInput("No subscription item found".to_string()))?;

        let new_price_id = new_plan.external_price_id.as_ref().ok_or_else(|| {
            AppError::InvalidInput("New plan missing Stripe price ID".to_string())
        })?;

        // Determine if upgrade, downgrade, or lateral (same price)
        // - Upgrade (new > current): immediate with proration
        // - Downgrade/Lateral (new <= current): scheduled for period end
        // - During trial: ALL changes are immediate (trial ends)
        let current_amount = sub
            .items
            .data
            .first()
            .and_then(|item| item.price.unit_amount)
            .unwrap_or(0);
        let new_amount = new_plan.price_cents as i64;

        let is_upgrade = new_amount > current_amount;
        let immediate = is_upgrade || is_trial;

        if immediate {
            // Immediate swap: upgrade, or any change during trial
            let change_type = if is_upgrade {
                PlanChangeType::Upgrade
            } else if new_amount == current_amount {
                PlanChangeType::Lateral
            } else {
                PlanChangeType::Downgrade
            };

            let idempotency_key = format!(
                "change_{}_{}_{}",
                subscription_id,
                new_plan.id,
                Utc::now().timestamp()
            );
            let upgraded = self
                .client
                .upgrade_subscription(
                    subscription_id.as_str(),
                    &item_id,
                    new_price_id,
                    1,
                    false,
                    &idempotency_key,
                )
                .await?;

            Ok(PlanChangeResult {
                success: true,
                change_type,
                invoice_id: upgraded.latest_invoice_id(),
                amount_charged_cents: upgraded.latest_invoice_amount(),
                currency: upgraded.latest_invoice_currency(),
                client_secret: upgraded.client_secret(),
                hosted_invoice_url: upgraded.hosted_invoice_url(),
                payment_intent_status: upgraded.payment_intent_status(),
                effective_at: Utc::now(),
                schedule_id: None,
            })
        } else {
            // Downgrade or Lateral: scheduled for period end
            let is_lateral = new_amount == current_amount;
            let change_type = if is_lateral {
                PlanChangeType::Lateral
            } else {
                PlanChangeType::Downgrade
            };

            let current_price_id = sub.price_id();
            let idempotency_key = format!(
                "{}_{}_{}_{}",
                if is_lateral { "lateral" } else { "downgrade" },
                subscription_id,
                new_plan.id,
                Utc::now().timestamp()
            );

            let schedule = self
                .client
                .schedule_downgrade(
                    subscription_id.as_str(),
                    &current_price_id,
                    new_price_id,
                    sub.current_period_end,
                    &idempotency_key,
                )
                .await?;

            Ok(PlanChangeResult {
                success: true,
                change_type,
                invoice_id: None,
                amount_charged_cents: None,
                currency: None,
                client_secret: None,
                hosted_invoice_url: None,
                payment_intent_status: None,
                effective_at: Self::timestamp_to_datetime(sub.current_period_end),
                schedule_id: Some(schedule.id),
            })
        }
    }

    // ========================================================================
    // Invoicing & PDF Generation
    // ========================================================================

    async fn get_invoice_pdf(&self, invoice_id: &str) -> AppResult<InvoicePdfResult> {
        // For Stripe, the invoice PDF URL is available from the invoice object
        // We would need to fetch the invoice to get the URL
        // For now, return that we'd need to look it up
        Ok(InvoicePdfResult::Url(format!(
            "https://invoice.stripe.com/{}/pdf",
            invoice_id
        )))
    }

    async fn list_invoices(
        &self,
        customer_id: &CustomerId,
        limit: i32,
    ) -> AppResult<Vec<InvoiceInfo>> {
        let invoices = self
            .client
            .list_invoices(customer_id.as_str(), Some(limit))
            .await?;

        Ok(invoices
            .into_iter()
            .map(|inv| InvoiceInfo {
                invoice_id: inv.id,
                customer_id: CustomerId::new(&inv.customer),
                amount_cents: inv.amount_due as i32,
                amount_paid_cents: inv.amount_paid as i32,
                currency: inv.currency,
                status: inv.status.unwrap_or_else(|| "unknown".to_string()),
                hosted_url: inv.hosted_invoice_url,
                pdf_url: inv.invoice_pdf,
                invoice_number: inv.number,
                billing_reason: inv.billing_reason,
                created_at: Self::opt_timestamp_to_datetime(inv.created),
                paid_at: None, // Would need status_transitions.paid_at from Stripe
            })
            .collect())
    }

    // ========================================================================
    // Portal & Self-Service
    // ========================================================================

    async fn create_portal_session(
        &self,
        customer_id: &CustomerId,
        return_url: &str,
    ) -> AppResult<Option<String>> {
        let session = self
            .client
            .create_portal_session(customer_id.as_str(), return_url)
            .await?;
        Ok(Some(session.url))
    }

    // ========================================================================
    // Payment Method Management
    // ========================================================================

    async fn update_payment_scenario(
        &self,
        _subscription_id: &SubscriptionId,
        _scenario: PaymentScenario,
    ) -> AppResult<()> {
        // Payment scenarios are only for the dummy provider
        Err(AppError::InvalidInput(
            "Payment scenarios are not supported for Stripe".to_string(),
        ))
    }

    // ========================================================================
    // Provider-specific setup
    // ========================================================================

    async fn ensure_product_and_price(&self, plan: &PlanInfo) -> AppResult<(String, String)> {
        // Check if we already have product/price IDs
        if let (Some(product_id), Some(price_id)) =
            (&plan.external_product_id, &plan.external_price_id)
        {
            return Ok((product_id.clone(), price_id.clone()));
        }

        // Create product
        let product = self
            .client
            .create_product(&plan.name, plan.code.as_str().into())
            .await?;

        // Convert interval from internal format to Stripe format
        let stripe_interval = convert_interval_to_stripe(&plan.interval);

        // Create price
        let price = self
            .client
            .create_price(
                &product.id,
                plan.price_cents as i64,
                &plan.currency,
                stripe_interval,
                plan.interval_count,
            )
            .await?;

        Ok((product.id, price.id))
    }
}

// Helper trait extensions for StripeSubscription
trait StripeSubscriptionExt {
    fn latest_invoice_id(&self) -> Option<String>;
    fn latest_invoice_amount(&self) -> Option<i64>;
    fn latest_invoice_currency(&self) -> Option<String>;
}

impl StripeSubscriptionExt for crate::infra::stripe_client::StripeSubscription {
    fn latest_invoice_id(&self) -> Option<String> {
        match &self.latest_invoice {
            Some(crate::infra::stripe_client::StripeLatestInvoice::Id(id)) => Some(id.clone()),
            Some(crate::infra::stripe_client::StripeLatestInvoice::Expanded(inv)) => {
                Some(inv.id.clone())
            }
            None => None,
        }
    }

    fn latest_invoice_amount(&self) -> Option<i64> {
        match &self.latest_invoice {
            Some(crate::infra::stripe_client::StripeLatestInvoice::Expanded(inv)) => {
                Some(inv.amount_due)
            }
            _ => None,
        }
    }

    fn latest_invoice_currency(&self) -> Option<String> {
        match &self.latest_invoice {
            Some(crate::infra::stripe_client::StripeLatestInvoice::Expanded(inv)) => {
                Some(inv.currency.clone())
            }
            _ => None,
        }
    }
}

/// Convert interval from internal format to Stripe format.
/// Stripe expects: month, year, week, day
/// Internal format may be: monthly, yearly, weekly, daily
fn convert_interval_to_stripe(interval: &str) -> &str {
    match interval {
        "monthly" => "month",
        "yearly" => "year",
        "weekly" => "week",
        "daily" => "day",
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_interval_monthly_to_month() {
        assert_eq!(convert_interval_to_stripe("monthly"), "month");
    }

    #[test]
    fn test_convert_interval_yearly_to_year() {
        assert_eq!(convert_interval_to_stripe("yearly"), "year");
    }

    #[test]
    fn test_convert_interval_weekly_to_week() {
        assert_eq!(convert_interval_to_stripe("weekly"), "week");
    }

    #[test]
    fn test_convert_interval_daily_to_day() {
        assert_eq!(convert_interval_to_stripe("daily"), "day");
    }

    #[test]
    fn test_convert_interval_passthrough_month() {
        assert_eq!(convert_interval_to_stripe("month"), "month");
    }

    #[test]
    fn test_convert_interval_passthrough_year() {
        assert_eq!(convert_interval_to_stripe("year"), "year");
    }

    #[test]
    fn test_convert_interval_passthrough_unknown() {
        assert_eq!(convert_interval_to_stripe("custom"), "custom");
    }
}
