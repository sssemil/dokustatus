use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use std::collections::HashMap;
use uuid::Uuid;

use crate::{
    app_error::{AppError, AppResult},
    application::ports::payment_provider::{
        CheckoutResult, CheckoutUrls, CustomerInfo, CustomerId, InvoiceInfo, InvoicePdfResult,
        PaymentProviderPort, PlanChangePreview, PlanChangeResult, PlanChangeType, PlanInfo,
        SubscriptionId, SubscriptionInfo, SubscriptionResult,
    },
    domain::entities::{
        payment_mode::PaymentMode,
        payment_provider::PaymentProvider,
        payment_scenario::PaymentScenario,
        user_subscription::SubscriptionStatus,
    },
};

/// Dummy payment client for testing purposes.
///
/// This provider simulates all payment operations locally without making
/// any external API calls. It supports different payment scenarios to
/// test various outcomes (success, decline, 3DS, etc.).
#[derive(Clone)]
pub struct DummyPaymentClient {
    domain_id: Uuid,
}

impl DummyPaymentClient {
    pub fn new(domain_id: Uuid) -> Self {
        Self { domain_id }
    }

    /// Generate a deterministic customer ID
    fn generate_customer_id(&self, user_id: Uuid) -> CustomerId {
        CustomerId::new(format!("dummy_cus_{}", user_id))
    }

    /// Generate a unique subscription ID
    fn generate_subscription_id(&self) -> SubscriptionId {
        SubscriptionId::new(format!("dummy_sub_{}", Uuid::new_v4()))
    }

    /// Generate a unique invoice ID
    fn generate_invoice_id(&self) -> String {
        format!("dummy_inv_{}", Uuid::new_v4())
    }

    /// Calculate billing period based on plan interval
    fn calculate_period(
        &self,
        plan: &PlanInfo,
        start: DateTime<Utc>,
    ) -> (DateTime<Utc>, DateTime<Utc>) {
        let period_end = match plan.interval.as_str() {
            "month" => start + Duration::days(30 * plan.interval_count as i64),
            "year" => start + Duration::days(365 * plan.interval_count as i64),
            "week" => start + Duration::weeks(plan.interval_count as i64),
            "day" => start + Duration::days(plan.interval_count as i64),
            _ => start + Duration::days(30), // Default to monthly
        };
        (start, period_end)
    }

    /// Process payment scenario and return result
    fn process_scenario(&self, scenario: PaymentScenario) -> AppResult<PaymentResult> {
        match scenario {
            PaymentScenario::Success => Ok(PaymentResult::Succeeded),
            PaymentScenario::Decline => Err(AppError::PaymentDeclined(
                "Your card was declined.".to_string(),
            )),
            PaymentScenario::InsufficientFunds => Err(AppError::PaymentDeclined(
                "Your card has insufficient funds.".to_string(),
            )),
            PaymentScenario::ThreeDSecure => Ok(PaymentResult::RequiresConfirmation {
                token: format!("dummy_3ds_{}", Uuid::new_v4()),
            }),
            PaymentScenario::ExpiredCard => Err(AppError::PaymentDeclined(
                "Your card has expired.".to_string(),
            )),
            PaymentScenario::ProcessingError => Err(AppError::PaymentDeclined(
                "An error occurred while processing your card.".to_string(),
            )),
        }
    }

    /// Generate a simple invoice PDF
    fn generate_invoice_pdf_bytes(&self, invoice: &InvoiceInfo) -> AppResult<Vec<u8>> {
        // For now, generate a simple text-based PDF placeholder
        // In production, use genpdf or similar library for proper PDF generation
        let content = format!(
            r#"INVOICE

Invoice Number: {}
Date: {}

Amount: {:.2} {}
Status: {}

This is a test invoice from the Dummy Payment Provider.
For testing purposes only.
"#,
            invoice.invoice_number.as_deref().unwrap_or("N/A"),
            invoice
                .created_at
                .map(|d| d.format("%Y-%m-%d").to_string())
                .unwrap_or_else(|| "N/A".to_string()),
            invoice.amount_cents as f64 / 100.0,
            invoice.currency.to_uppercase(),
            invoice.status
        );

        // Return as UTF-8 bytes (text file for now - can be upgraded to PDF later)
        Ok(content.into_bytes())
    }
}

/// Internal payment result type
enum PaymentResult {
    Succeeded,
    RequiresConfirmation { token: String },
}

#[async_trait]
impl PaymentProviderPort for DummyPaymentClient {
    fn provider(&self) -> PaymentProvider {
        PaymentProvider::Dummy
    }

    fn mode(&self) -> PaymentMode {
        PaymentMode::Test
    }

    // ========================================================================
    // Customer Management
    // ========================================================================

    async fn ensure_customer(
        &self,
        email: &str,
        user_id: Uuid,
        _domain_id: Uuid,
    ) -> AppResult<CustomerId> {
        // Dummy provider just generates a deterministic customer ID
        let customer_id = self.generate_customer_id(user_id);
        tracing::debug!(
            customer_id = %customer_id,
            email = %email,
            "Dummy: Created/retrieved customer"
        );
        Ok(customer_id)
    }

    async fn get_customer(&self, customer_id: &CustomerId) -> AppResult<Option<CustomerInfo>> {
        // For dummy provider, we can't look up stored customers
        // Just return basic info based on the customer ID
        if customer_id.as_str().starts_with("dummy_cus_") {
            Ok(Some(CustomerInfo {
                customer_id: customer_id.clone(),
                email: None,
                metadata: HashMap::new(),
            }))
        } else {
            Ok(None)
        }
    }

    // ========================================================================
    // Checkout & Subscription Creation
    // ========================================================================

    async fn create_checkout(
        &self,
        customer: &CustomerId,
        plan: &PlanInfo,
        _urls: &CheckoutUrls,
        _scenario: Option<PaymentScenario>,
    ) -> AppResult<CheckoutResult> {
        // Dummy provider uses inline checkout, no redirect needed
        tracing::debug!(
            customer_id = %customer,
            plan_code = %plan.code,
            "Dummy: Creating inline checkout"
        );

        Ok(CheckoutResult {
            checkout_url: None,
            is_inline: true,
            session_id: Some(format!("dummy_session_{}", Uuid::new_v4())),
        })
    }

    async fn start_subscription(
        &self,
        customer: &CustomerId,
        plan: &PlanInfo,
        scenario: Option<PaymentScenario>,
    ) -> AppResult<SubscriptionResult> {
        let scenario = scenario.unwrap_or_default();
        tracing::debug!(
            customer_id = %customer,
            plan_code = %plan.code,
            scenario = %scenario,
            "Dummy: Starting subscription"
        );

        // Process the payment scenario
        match self.process_scenario(scenario)? {
            PaymentResult::Succeeded => {
                let subscription_id = self.generate_subscription_id();
                let now = Utc::now();

                // Calculate trial and billing periods
                let (trial_start, trial_end) = if plan.trial_days > 0 {
                    let trial_end = now + Duration::days(plan.trial_days as i64);
                    (Some(now), Some(trial_end))
                } else {
                    (None, None)
                };

                let period_start = trial_end.unwrap_or(now);
                let (_, period_end) = self.calculate_period(plan, period_start);

                let status = if plan.trial_days > 0 {
                    SubscriptionStatus::Trialing
                } else {
                    SubscriptionStatus::Active
                };

                Ok(SubscriptionResult {
                    subscription_id,
                    customer_id: customer.clone(),
                    status,
                    current_period_start: Some(period_start),
                    current_period_end: Some(period_end),
                    trial_start,
                    trial_end,
                    requires_confirmation: false,
                    confirmation_token: None,
                    invoice_id: Some(self.generate_invoice_id()),
                })
            }
            PaymentResult::RequiresConfirmation { token } => {
                let subscription_id = self.generate_subscription_id();

                Ok(SubscriptionResult {
                    subscription_id,
                    customer_id: customer.clone(),
                    status: SubscriptionStatus::Incomplete,
                    current_period_start: None,
                    current_period_end: None,
                    trial_start: None,
                    trial_end: None,
                    requires_confirmation: true,
                    confirmation_token: Some(token),
                    invoice_id: None,
                })
            }
        }
    }

    async fn confirm_subscription(
        &self,
        confirmation_token: &str,
    ) -> AppResult<SubscriptionResult> {
        // Validate the token format
        if !confirmation_token.starts_with("dummy_3ds_") {
            return Err(AppError::ValidationError(
                "Invalid confirmation token".to_string(),
            ));
        }

        tracing::debug!(
            token = %confirmation_token,
            "Dummy: Confirming 3DS subscription"
        );

        // 3DS confirmation succeeds
        let subscription_id = self.generate_subscription_id();
        let now = Utc::now();
        let period_end = now + Duration::days(30);

        Ok(SubscriptionResult {
            subscription_id,
            customer_id: CustomerId::new("dummy_cus_confirmed"),
            status: SubscriptionStatus::Active,
            current_period_start: Some(now),
            current_period_end: Some(period_end),
            trial_start: None,
            trial_end: None,
            requires_confirmation: false,
            confirmation_token: None,
            invoice_id: Some(self.generate_invoice_id()),
        })
    }

    // ========================================================================
    // Subscription Lifecycle
    // ========================================================================

    async fn get_subscription(
        &self,
        subscription_id: &SubscriptionId,
    ) -> AppResult<Option<SubscriptionInfo>> {
        // Dummy provider can't look up stored subscriptions
        // This would need to be handled by the database
        if subscription_id.as_str().starts_with("dummy_sub_") {
            let now = Utc::now();
            Ok(Some(SubscriptionInfo {
                subscription_id: subscription_id.clone(),
                customer_id: CustomerId::new("dummy_cus_unknown"),
                status: SubscriptionStatus::Active,
                current_period_start: Some(now),
                current_period_end: Some(now + Duration::days(30)),
                trial_start: None,
                trial_end: None,
                cancel_at_period_end: false,
                canceled_at: None,
                price_id: None,
                subscription_item_id: None,
            }))
        } else {
            Ok(None)
        }
    }

    async fn cancel_subscription(
        &self,
        subscription_id: &SubscriptionId,
        at_period_end: bool,
    ) -> AppResult<()> {
        tracing::debug!(
            subscription_id = %subscription_id,
            at_period_end = %at_period_end,
            "Dummy: Canceling subscription"
        );

        // Dummy provider just logs the cancellation
        // Actual state update happens in the database
        Ok(())
    }

    async fn preview_plan_change(
        &self,
        _subscription_id: &SubscriptionId,
        new_plan: &PlanInfo,
    ) -> AppResult<PlanChangePreview> {
        let now = Utc::now();
        let period_end = now + Duration::days(30);

        // For dummy provider, just return a simple preview
        // Assume current plan is cheaper (upgrade scenario)
        let prorated_amount = (new_plan.price_cents / 2) as i64; // Simplified proration

        Ok(PlanChangePreview {
            prorated_amount_cents: prorated_amount,
            currency: new_plan.currency.clone(),
            period_end,
            new_plan_name: new_plan.name.clone(),
            new_plan_price_cents: new_plan.price_cents as i64,
            change_type: if prorated_amount > 0 {
                PlanChangeType::Upgrade
            } else {
                PlanChangeType::Downgrade
            },
            effective_at: now,
        })
    }

    async fn change_plan(
        &self,
        subscription_id: &SubscriptionId,
        _subscription_item_id: Option<&str>,
        new_plan: &PlanInfo,
    ) -> AppResult<PlanChangeResult> {
        tracing::debug!(
            subscription_id = %subscription_id,
            new_plan_code = %new_plan.code,
            "Dummy: Changing plan"
        );

        let now = Utc::now();

        Ok(PlanChangeResult {
            success: true,
            change_type: PlanChangeType::Upgrade, // Assume upgrade for simplicity
            invoice_id: Some(self.generate_invoice_id()),
            amount_charged_cents: Some((new_plan.price_cents / 2) as i64),
            currency: Some(new_plan.currency.clone()),
            client_secret: None,
            hosted_invoice_url: None,
            payment_intent_status: Some("succeeded".to_string()),
            effective_at: now,
            schedule_id: None,
        })
    }

    // ========================================================================
    // Invoicing & PDF Generation
    // ========================================================================

    async fn get_invoice_pdf(&self, invoice_id: &str) -> AppResult<InvoicePdfResult> {
        if !invoice_id.starts_with("dummy_inv_") {
            return Ok(InvoicePdfResult::NotAvailable);
        }

        // Generate a simple invoice
        let invoice = InvoiceInfo {
            invoice_id: invoice_id.to_string(),
            customer_id: CustomerId::new("dummy_cus_unknown"),
            amount_cents: 1000,
            amount_paid_cents: 1000,
            currency: "usd".to_string(),
            status: "paid".to_string(),
            hosted_url: None,
            pdf_url: None,
            invoice_number: Some(format!("INV-{}", &invoice_id[11..19])),
            billing_reason: Some("subscription_create".to_string()),
            created_at: Some(Utc::now()),
            paid_at: Some(Utc::now()),
        };

        let pdf_bytes = self.generate_invoice_pdf_bytes(&invoice)?;
        Ok(InvoicePdfResult::Bytes(pdf_bytes))
    }

    async fn list_invoices(
        &self,
        _customer_id: &CustomerId,
        limit: i32,
    ) -> AppResult<Vec<InvoiceInfo>> {
        // Dummy provider returns empty list - invoices are stored in DB
        tracing::debug!(limit = %limit, "Dummy: Listing invoices (returns empty)");
        Ok(vec![])
    }

    // ========================================================================
    // Portal & Self-Service
    // ========================================================================

    async fn create_portal_session(
        &self,
        _customer_id: &CustomerId,
        _return_url: &str,
    ) -> AppResult<Option<String>> {
        // Dummy provider doesn't have a hosted portal
        // All management is done through the main billing page
        Ok(None)
    }

    // ========================================================================
    // Payment Method Management
    // ========================================================================

    async fn update_payment_scenario(
        &self,
        subscription_id: &SubscriptionId,
        scenario: PaymentScenario,
    ) -> AppResult<()> {
        tracing::debug!(
            subscription_id = %subscription_id,
            scenario = %scenario,
            "Dummy: Updated payment scenario"
        );

        // Actual scenario is stored with the subscription in the database
        // This is just validation that the operation is supported
        Ok(())
    }

    // ========================================================================
    // Provider-specific setup
    // ========================================================================

    async fn ensure_product_and_price(&self, plan: &PlanInfo) -> AppResult<(String, String)> {
        // Dummy provider doesn't need actual products/prices
        // Generate deterministic IDs based on plan
        let product_id = format!("dummy_prod_{}", plan.id);
        let price_id = format!("dummy_price_{}", plan.id);

        tracing::debug!(
            plan_id = %plan.id,
            product_id = %product_id,
            price_id = %price_id,
            "Dummy: Ensured product and price"
        );

        Ok((product_id, price_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_ensure_customer() {
        let client = DummyPaymentClient::new(Uuid::new_v4());
        let user_id = Uuid::new_v4();
        let domain_id = Uuid::new_v4();

        let customer_id = client
            .ensure_customer("test@example.com", user_id, domain_id)
            .await
            .unwrap();

        assert!(customer_id.as_str().starts_with("dummy_cus_"));
    }

    #[tokio::test]
    async fn test_start_subscription_success() {
        let client = DummyPaymentClient::new(Uuid::new_v4());
        let customer = CustomerId::new("dummy_cus_test");
        let plan = PlanInfo {
            id: Uuid::new_v4(),
            code: "test".to_string(),
            name: "Test Plan".to_string(),
            price_cents: 1000,
            currency: "usd".to_string(),
            interval: "month".to_string(),
            interval_count: 1,
            trial_days: 0,
            external_price_id: None,
            external_product_id: None,
        };

        let result = client
            .start_subscription(&customer, &plan, Some(PaymentScenario::Success))
            .await
            .unwrap();

        assert_eq!(result.status, SubscriptionStatus::Active);
        assert!(!result.requires_confirmation);
    }

    #[tokio::test]
    async fn test_start_subscription_decline() {
        let client = DummyPaymentClient::new(Uuid::new_v4());
        let customer = CustomerId::new("dummy_cus_test");
        let plan = PlanInfo {
            id: Uuid::new_v4(),
            code: "test".to_string(),
            name: "Test Plan".to_string(),
            price_cents: 1000,
            currency: "usd".to_string(),
            interval: "month".to_string(),
            interval_count: 1,
            trial_days: 0,
            external_price_id: None,
            external_product_id: None,
        };

        let result = client
            .start_subscription(&customer, &plan, Some(PaymentScenario::Decline))
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_start_subscription_3ds() {
        let client = DummyPaymentClient::new(Uuid::new_v4());
        let customer = CustomerId::new("dummy_cus_test");
        let plan = PlanInfo {
            id: Uuid::new_v4(),
            code: "test".to_string(),
            name: "Test Plan".to_string(),
            price_cents: 1000,
            currency: "usd".to_string(),
            interval: "month".to_string(),
            interval_count: 1,
            trial_days: 0,
            external_price_id: None,
            external_product_id: None,
        };

        let result = client
            .start_subscription(&customer, &plan, Some(PaymentScenario::ThreeDSecure))
            .await
            .unwrap();

        assert_eq!(result.status, SubscriptionStatus::Incomplete);
        assert!(result.requires_confirmation);
        assert!(result.confirmation_token.is_some());
    }

    #[tokio::test]
    async fn test_start_subscription_with_trial() {
        let client = DummyPaymentClient::new(Uuid::new_v4());
        let customer = CustomerId::new("dummy_cus_test");
        let plan = PlanInfo {
            id: Uuid::new_v4(),
            code: "test".to_string(),
            name: "Test Plan".to_string(),
            price_cents: 1000,
            currency: "usd".to_string(),
            interval: "month".to_string(),
            interval_count: 1,
            trial_days: 14,
            external_price_id: None,
            external_product_id: None,
        };

        let result = client
            .start_subscription(&customer, &plan, Some(PaymentScenario::Success))
            .await
            .unwrap();

        assert_eq!(result.status, SubscriptionStatus::Trialing);
        assert!(result.trial_start.is_some());
        assert!(result.trial_end.is_some());
    }
}
