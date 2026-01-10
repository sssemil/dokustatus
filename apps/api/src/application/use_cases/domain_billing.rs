use async_trait::async_trait;
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::{
    adapters::persistence::enabled_payment_providers::{
        EnabledPaymentProviderProfile, EnabledPaymentProvidersRepo,
    },
    app_error::{AppError, AppResult},
    application::ports::payment_provider::{
        PaymentProviderPort, PlanChangeType as PortPlanChangeType, PlanInfo, SubscriptionId,
    },
    application::use_cases::payment_provider_factory::PaymentProviderFactory,
    domain::entities::{
        billing_state::BillingState, payment_mode::PaymentMode, payment_provider::PaymentProvider,
        payment_status::PaymentStatus, stripe_mode::StripeMode,
        user_subscription::SubscriptionStatus,
    },
    infra::crypto::ProcessCipher,
};

use super::domain::DomainRepo;

/// Number of months in a year, used for MRR calculations
const MONTHS_PER_YEAR: i64 = 12;

/// Calculates the monthly equivalent amount in cents for a given price and billing interval.
/// Uses floating-point arithmetic to avoid integer division precision loss.
///
/// # Arguments
/// * `price_cents` - The price in cents for the billing interval
/// * `interval` - The interval type ("yearly", "year", "monthly", "month", or other)
/// * `interval_count` - Number of intervals (e.g., 2 for "every 2 months")
///
/// # Returns
/// Monthly amount as f64 (caller should accumulate and round at the end)
///
/// # Rounding
/// Uses standard `.round()` (round half away from zero). For financial calculations
/// this is appropriate as it treats positive and negative values symmetrically.
fn calculate_monthly_amount_cents(price_cents: i64, interval: &str, interval_count: i64) -> f64 {
    debug_assert!(price_cents >= 0, "price_cents should not be negative");

    // Protect against division by zero (should be prevented upstream, but be defensive)
    let interval_count = std::cmp::max(interval_count, 1);

    let divisor = match interval {
        "yearly" | "year" => interval_count * MONTHS_PER_YEAR,
        // Monthly or any unknown intervals are treated as monthly (legacy behavior preserved)
        _ => interval_count,
    };

    price_cents as f64 / divisor as f64
}

// ============================================================================
// Profile Types
// ============================================================================

#[derive(Debug, Clone)]
pub struct BillingStripeConfigProfile {
    pub id: Uuid,
    pub domain_id: Uuid,
    pub stripe_mode: StripeMode,
    pub stripe_secret_key_encrypted: String,
    pub stripe_publishable_key: String,
    pub stripe_webhook_secret_encrypted: String,
    pub created_at: Option<NaiveDateTime>,
    pub updated_at: Option<NaiveDateTime>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SubscriptionPlanProfile {
    pub id: Uuid,
    pub domain_id: Uuid,
    pub stripe_mode: StripeMode,
    /// New: payment provider (nullable during migration)
    pub payment_provider: Option<PaymentProvider>,
    /// New: payment mode (nullable during migration)
    pub payment_mode: Option<PaymentMode>,
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    pub price_cents: i32,
    pub currency: String,
    pub interval: String,
    pub interval_count: i32,
    pub trial_days: i32,
    pub features: Vec<String>,
    pub is_public: bool,
    pub display_order: i32,
    pub stripe_product_id: Option<String>,
    pub stripe_price_id: Option<String>,
    pub is_archived: bool,
    pub archived_at: Option<NaiveDateTime>,
    pub created_at: Option<NaiveDateTime>,
    pub updated_at: Option<NaiveDateTime>,
}

#[derive(Debug, Clone, Serialize)]
pub struct UserSubscriptionProfile {
    pub id: Uuid,
    pub domain_id: Uuid,
    pub stripe_mode: StripeMode,
    /// New: payment provider (nullable during migration)
    pub payment_provider: Option<PaymentProvider>,
    /// New: payment mode (nullable during migration)
    pub payment_mode: Option<PaymentMode>,
    /// New: billing state for provider switching
    pub billing_state: Option<BillingState>,
    pub end_user_id: Uuid,
    pub plan_id: Uuid,
    pub status: SubscriptionStatus,
    pub stripe_customer_id: String,
    pub stripe_subscription_id: Option<String>,
    pub current_period_start: Option<NaiveDateTime>,
    pub current_period_end: Option<NaiveDateTime>,
    pub trial_start: Option<NaiveDateTime>,
    pub trial_end: Option<NaiveDateTime>,
    pub cancel_at_period_end: bool,
    pub canceled_at: Option<NaiveDateTime>,
    pub manually_granted: bool,
    pub granted_by: Option<Uuid>,
    pub granted_at: Option<NaiveDateTime>,
    pub created_at: Option<NaiveDateTime>,
    pub updated_at: Option<NaiveDateTime>,
}

#[derive(Debug, Clone, Serialize)]
pub struct UserSubscriptionWithPlan {
    pub subscription: UserSubscriptionProfile,
    pub plan: SubscriptionPlanProfile,
    pub user_email: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SubscriptionEventProfile {
    pub id: Uuid,
    pub subscription_id: Uuid,
    pub event_type: String,
    pub previous_status: Option<SubscriptionStatus>,
    pub new_status: Option<SubscriptionStatus>,
    pub stripe_event_id: Option<String>,
    pub metadata: serde_json::Value,
    pub created_by: Option<Uuid>,
    pub created_at: Option<NaiveDateTime>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BillingPaymentProfile {
    pub id: Uuid,
    pub domain_id: Uuid,
    pub stripe_mode: StripeMode,
    /// New: payment provider (nullable during migration)
    pub payment_provider: Option<PaymentProvider>,
    /// New: payment mode (nullable during migration)
    pub payment_mode: Option<PaymentMode>,
    pub end_user_id: Uuid,
    pub subscription_id: Option<Uuid>,
    pub stripe_invoice_id: String,
    pub stripe_payment_intent_id: Option<String>,
    pub stripe_customer_id: String,
    pub amount_cents: i32,
    pub amount_paid_cents: i32,
    pub amount_refunded_cents: i32,
    pub currency: String,
    pub status: PaymentStatus,
    pub plan_id: Option<Uuid>,
    pub plan_code: Option<String>,
    pub plan_name: Option<String>,
    pub hosted_invoice_url: Option<String>,
    pub invoice_pdf_url: Option<String>,
    pub invoice_number: Option<String>,
    pub billing_reason: Option<String>,
    pub failure_message: Option<String>,
    pub invoice_created_at: Option<NaiveDateTime>,
    pub payment_date: Option<NaiveDateTime>,
    pub refunded_at: Option<NaiveDateTime>,
    pub created_at: Option<NaiveDateTime>,
    pub updated_at: Option<NaiveDateTime>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BillingPaymentWithUser {
    pub payment: BillingPaymentProfile,
    pub user_email: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PaginatedPayments {
    pub payments: Vec<BillingPaymentWithUser>,
    pub total: i64,
    pub page: i32,
    pub per_page: i32,
    pub total_pages: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct PaymentSummary {
    pub total_revenue_cents: i64,
    pub total_refunded_cents: i64,
    pub payment_count: i64,
    pub successful_payments: i64,
    pub failed_payments: i64,
}

// ============================================================================
// Input Types
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
pub struct CreatePlanInput {
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    pub price_cents: i32,
    pub currency: String,
    pub interval: String,
    pub interval_count: i32,
    pub trial_days: i32,
    pub features: Vec<String>,
    pub is_public: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdatePlanInput {
    pub name: Option<String>,
    pub description: Option<String>,
    pub price_cents: Option<i32>,
    pub interval: Option<String>,
    pub interval_count: Option<i32>,
    pub trial_days: Option<i32>,
    pub features: Option<Vec<String>>,
    pub is_public: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct CreateSubscriptionInput {
    pub domain_id: Uuid,
    pub stripe_mode: StripeMode,
    pub end_user_id: Uuid,
    pub plan_id: Uuid,
    pub stripe_customer_id: String,
    pub stripe_subscription_id: Option<String>,
    pub status: SubscriptionStatus,
    pub current_period_start: Option<NaiveDateTime>,
    pub current_period_end: Option<NaiveDateTime>,
    pub trial_start: Option<NaiveDateTime>,
    pub trial_end: Option<NaiveDateTime>,
}

#[derive(Debug, Clone)]
pub struct StripeSubscriptionUpdate {
    pub status: SubscriptionStatus,
    pub plan_id: Option<Uuid>, // To update plan on upgrade/downgrade
    pub stripe_subscription_id: Option<String>, // To set/update the Stripe subscription ID
    pub current_period_start: Option<NaiveDateTime>,
    pub current_period_end: Option<NaiveDateTime>,
    pub cancel_at_period_end: bool,
    pub canceled_at: Option<NaiveDateTime>,
    pub trial_start: Option<NaiveDateTime>,
    pub trial_end: Option<NaiveDateTime>,
}

#[derive(Debug, Clone)]
pub struct CreateSubscriptionEventInput {
    pub subscription_id: Uuid,
    pub event_type: String,
    pub previous_status: Option<SubscriptionStatus>,
    pub new_status: Option<SubscriptionStatus>,
    pub stripe_event_id: Option<String>,
    pub metadata: serde_json::Value,
    pub created_by: Option<Uuid>,
}

#[derive(Debug, Clone)]
pub struct CreatePaymentInput {
    pub domain_id: Uuid,
    pub stripe_mode: StripeMode,
    pub end_user_id: Uuid,
    pub subscription_id: Option<Uuid>,
    pub stripe_invoice_id: String,
    pub stripe_payment_intent_id: Option<String>,
    pub stripe_customer_id: String,
    pub amount_cents: i32,
    pub amount_paid_cents: i32,
    pub currency: String,
    pub status: PaymentStatus,
    pub plan_id: Option<Uuid>,
    pub plan_code: Option<String>,
    pub plan_name: Option<String>,
    pub hosted_invoice_url: Option<String>,
    pub invoice_pdf_url: Option<String>,
    pub invoice_number: Option<String>,
    pub billing_reason: Option<String>,
    pub failure_message: Option<String>,
    pub invoice_created_at: Option<NaiveDateTime>,
    pub payment_date: Option<NaiveDateTime>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PaymentListFilters {
    pub status: Option<PaymentStatus>,
    pub date_from: Option<NaiveDateTime>,
    pub date_to: Option<NaiveDateTime>,
    pub plan_code: Option<String>,
    pub user_email: Option<String>,
}

// ============================================================================
// Analytics Types
// ============================================================================

#[derive(Debug, Clone, Serialize)]
pub struct BillingAnalytics {
    pub mrr_cents: i64,
    pub active_subscribers: i64,
    pub trialing_subscribers: i64,
    pub past_due_subscribers: i64,
    pub plan_distribution: Vec<PlanDistribution>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlanDistribution {
    pub plan_id: Uuid,
    pub plan_name: String,
    pub subscriber_count: i64,
    pub revenue_cents: i64,
}

// ============================================================================
// Plan Change Types (Upgrade/Downgrade)
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PlanChangeType {
    Upgrade,   // Immediate with proration
    Downgrade, // Scheduled for period end
}

impl PlanChangeType {
    pub fn as_str(&self) -> &'static str {
        match self {
            PlanChangeType::Upgrade => "upgrade",
            PlanChangeType::Downgrade => "downgrade",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PlanChangePreview {
    pub prorated_amount_cents: i64,
    pub currency: String,
    pub period_end: i64,
    pub new_plan_name: String,
    pub new_plan_price_cents: i64,
    pub change_type: PlanChangeType,
    pub effective_at: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlanChangeResult {
    pub success: bool,
    pub change_type: PlanChangeType,
    pub invoice_id: Option<String>,
    pub amount_charged_cents: Option<i64>,
    pub currency: Option<String>,
    pub client_secret: Option<String>,
    pub hosted_invoice_url: Option<String>,
    pub payment_intent_status: Option<String>,
    pub new_plan: SubscriptionPlanProfile,
    pub effective_at: i64,
    pub schedule_id: Option<String>,
}

// ============================================================================
// Repository Traits
// ============================================================================

#[async_trait]
pub trait BillingStripeConfigRepo: Send + Sync {
    /// Get Stripe config for a specific mode
    async fn get_by_domain_and_mode(
        &self,
        domain_id: Uuid,
        mode: StripeMode,
    ) -> AppResult<Option<BillingStripeConfigProfile>>;

    /// List all configs for a domain (both test and live)
    async fn list_by_domain(&self, domain_id: Uuid) -> AppResult<Vec<BillingStripeConfigProfile>>;

    /// Upsert config for a specific mode
    async fn upsert(
        &self,
        domain_id: Uuid,
        mode: StripeMode,
        stripe_secret_key_encrypted: &str,
        stripe_publishable_key: &str,
        stripe_webhook_secret_encrypted: &str,
    ) -> AppResult<BillingStripeConfigProfile>;

    /// Delete config for a specific mode
    async fn delete(&self, domain_id: Uuid, mode: StripeMode) -> AppResult<()>;

    /// Check if any config exists for this domain (either mode)
    async fn has_any_config(&self, domain_id: Uuid) -> AppResult<bool>;
}

#[async_trait]
pub trait SubscriptionPlanRepo: Send + Sync {
    async fn get_by_id(&self, id: Uuid) -> AppResult<Option<SubscriptionPlanProfile>>;
    async fn get_by_domain_and_code(
        &self,
        domain_id: Uuid,
        mode: StripeMode,
        code: &str,
    ) -> AppResult<Option<SubscriptionPlanProfile>>;
    async fn list_by_domain_and_mode(
        &self,
        domain_id: Uuid,
        mode: StripeMode,
        include_archived: bool,
    ) -> AppResult<Vec<SubscriptionPlanProfile>>;
    async fn list_public_by_domain_and_mode(
        &self,
        domain_id: Uuid,
        mode: StripeMode,
    ) -> AppResult<Vec<SubscriptionPlanProfile>>;
    async fn create(
        &self,
        domain_id: Uuid,
        mode: StripeMode,
        input: &CreatePlanInput,
    ) -> AppResult<SubscriptionPlanProfile>;
    async fn update(&self, id: Uuid, input: &UpdatePlanInput)
    -> AppResult<SubscriptionPlanProfile>;
    async fn set_stripe_ids(&self, id: Uuid, product_id: &str, price_id: &str) -> AppResult<()>;
    async fn set_display_order(&self, id: Uuid, order: i32) -> AppResult<()>;
    async fn archive(&self, id: Uuid) -> AppResult<()>;
    async fn delete(&self, id: Uuid) -> AppResult<()>;
    async fn count_subscribers(&self, plan_id: Uuid) -> AppResult<i64>;
    /// Count plans in a specific mode (for deletion validation)
    async fn count_by_domain_and_mode(&self, domain_id: Uuid, mode: StripeMode) -> AppResult<i64>;
    /// Find plan by Stripe price ID (searches all plans in the mode, including archived)
    async fn get_by_stripe_price_id(
        &self,
        domain_id: Uuid,
        mode: StripeMode,
        stripe_price_id: &str,
    ) -> AppResult<Option<SubscriptionPlanProfile>>;
}

#[async_trait]
pub trait UserSubscriptionRepo: Send + Sync {
    async fn get_by_id(&self, id: Uuid) -> AppResult<Option<UserSubscriptionProfile>>;
    async fn get_by_user_and_mode(
        &self,
        domain_id: Uuid,
        mode: StripeMode,
        end_user_id: Uuid,
    ) -> AppResult<Option<UserSubscriptionProfile>>;
    async fn get_by_stripe_subscription_id(
        &self,
        stripe_subscription_id: &str,
    ) -> AppResult<Option<UserSubscriptionProfile>>;
    async fn get_by_stripe_customer_id(
        &self,
        domain_id: Uuid,
        mode: StripeMode,
        stripe_customer_id: &str,
    ) -> AppResult<Option<UserSubscriptionProfile>>;
    async fn list_by_domain_and_mode(
        &self,
        domain_id: Uuid,
        mode: StripeMode,
    ) -> AppResult<Vec<UserSubscriptionWithPlan>>;
    async fn list_by_plan(&self, plan_id: Uuid) -> AppResult<Vec<UserSubscriptionProfile>>;
    async fn create(&self, input: &CreateSubscriptionInput) -> AppResult<UserSubscriptionProfile>;
    async fn update_from_stripe(
        &self,
        id: Uuid,
        update: &StripeSubscriptionUpdate,
    ) -> AppResult<UserSubscriptionProfile>;
    async fn update_plan(&self, id: Uuid, plan_id: Uuid) -> AppResult<()>;
    async fn grant_manually(
        &self,
        domain_id: Uuid,
        mode: StripeMode,
        end_user_id: Uuid,
        plan_id: Uuid,
        granted_by: Uuid,
        stripe_customer_id: &str,
    ) -> AppResult<UserSubscriptionProfile>;
    async fn revoke(&self, id: Uuid) -> AppResult<()>;
    async fn delete(&self, id: Uuid) -> AppResult<()>;
    async fn count_active_by_domain_and_mode(
        &self,
        domain_id: Uuid,
        mode: StripeMode,
    ) -> AppResult<i64>;
    async fn count_by_status_and_mode(
        &self,
        domain_id: Uuid,
        mode: StripeMode,
        status: SubscriptionStatus,
    ) -> AppResult<i64>;
    /// Count subscriptions in a specific mode (for deletion validation)
    async fn count_by_domain_and_mode(&self, domain_id: Uuid, mode: StripeMode) -> AppResult<i64>;
}

#[async_trait]
pub trait SubscriptionEventRepo: Send + Sync {
    async fn create(&self, input: &CreateSubscriptionEventInput) -> AppResult<()>;
    async fn list_by_subscription(
        &self,
        subscription_id: Uuid,
    ) -> AppResult<Vec<SubscriptionEventProfile>>;
    async fn exists_by_stripe_event_id(&self, stripe_event_id: &str) -> AppResult<bool>;
}

#[async_trait]
pub trait BillingPaymentRepo: Send + Sync {
    /// Create or update a payment from Stripe webhook data
    async fn upsert_from_stripe(
        &self,
        input: &CreatePaymentInput,
    ) -> AppResult<BillingPaymentProfile>;

    /// Get payment by Stripe invoice ID (for idempotency checks)
    async fn get_by_stripe_invoice_id(
        &self,
        stripe_invoice_id: &str,
    ) -> AppResult<Option<BillingPaymentProfile>>;

    /// List payments for an end-user (their own payments)
    async fn list_by_user(
        &self,
        domain_id: Uuid,
        mode: StripeMode,
        end_user_id: Uuid,
        page: i32,
        per_page: i32,
    ) -> AppResult<PaginatedPayments>;

    /// List payments for a domain with filters (dashboard)
    async fn list_by_domain(
        &self,
        domain_id: Uuid,
        mode: StripeMode,
        filters: &PaymentListFilters,
        page: i32,
        per_page: i32,
    ) -> AppResult<PaginatedPayments>;

    /// Update payment status (for refunds, failures, etc.)
    async fn update_status(
        &self,
        stripe_invoice_id: &str,
        status: PaymentStatus,
        amount_refunded_cents: Option<i32>,
        failure_message: Option<String>,
    ) -> AppResult<()>;

    /// Get payment summary for analytics
    async fn get_payment_summary(
        &self,
        domain_id: Uuid,
        mode: StripeMode,
        date_from: Option<NaiveDateTime>,
        date_to: Option<NaiveDateTime>,
    ) -> AppResult<PaymentSummary>;

    /// List all payments for export (no pagination)
    async fn list_all_for_export(
        &self,
        domain_id: Uuid,
        mode: StripeMode,
        filters: &PaymentListFilters,
    ) -> AppResult<Vec<BillingPaymentWithUser>>;
}

// ============================================================================
// Use Cases
// ============================================================================

#[derive(Clone)]
pub struct DomainBillingUseCases {
    domain_repo: Arc<dyn DomainRepo>,
    stripe_config_repo: Arc<dyn BillingStripeConfigRepo>,
    enabled_providers_repo: Arc<dyn EnabledPaymentProvidersRepo>,
    plan_repo: Arc<dyn SubscriptionPlanRepo>,
    subscription_repo: Arc<dyn UserSubscriptionRepo>,
    event_repo: Arc<dyn SubscriptionEventRepo>,
    payment_repo: Arc<dyn BillingPaymentRepo>,
    cipher: ProcessCipher,
    provider_factory: Arc<PaymentProviderFactory>,
    // NOTE: No fallback Stripe credentials - we cannot accept payments on behalf of other developers.
    // Each domain must configure their own Stripe account.
}

impl DomainBillingUseCases {
    pub fn new(
        domain_repo: Arc<dyn DomainRepo>,
        stripe_config_repo: Arc<dyn BillingStripeConfigRepo>,
        enabled_providers_repo: Arc<dyn EnabledPaymentProvidersRepo>,
        plan_repo: Arc<dyn SubscriptionPlanRepo>,
        subscription_repo: Arc<dyn UserSubscriptionRepo>,
        event_repo: Arc<dyn SubscriptionEventRepo>,
        payment_repo: Arc<dyn BillingPaymentRepo>,
        cipher: ProcessCipher,
        provider_factory: Arc<PaymentProviderFactory>,
    ) -> Self {
        Self {
            domain_repo,
            stripe_config_repo,
            enabled_providers_repo,
            plan_repo,
            subscription_repo,
            event_repo,
            payment_repo,
            cipher,
            provider_factory,
        }
    }

    /// Helper to get domain and verify ownership
    async fn get_domain_verified(
        &self,
        owner_id: Uuid,
        domain_id: Uuid,
    ) -> AppResult<super::domain::DomainProfile> {
        let domain = self
            .domain_repo
            .get_by_id(domain_id)
            .await?
            .ok_or(AppError::NotFound)?;
        if domain.owner_end_user_id != Some(owner_id) {
            return Err(AppError::Forbidden);
        }
        Ok(domain)
    }

    /// Get the active Stripe mode for a domain
    pub async fn get_active_mode(&self, domain_id: Uuid) -> AppResult<StripeMode> {
        let domain = self
            .domain_repo
            .get_by_id(domain_id)
            .await?
            .ok_or(AppError::NotFound)?;
        Ok(domain.billing_stripe_mode)
    }

    /// Get the active payment provider for a domain.
    ///
    /// This method determines the correct provider based on:
    /// 1. The domain's active mode (StripeMode → PaymentMode)
    /// 2. Enabled providers for the domain (prefers Stripe if configured)
    async fn get_active_provider(
        &self,
        domain_id: Uuid,
    ) -> AppResult<Arc<dyn PaymentProviderPort>> {
        // Get the domain's active mode
        let stripe_mode = self.get_active_mode(domain_id).await?;
        let payment_mode: PaymentMode = stripe_mode.into();

        // Check enabled providers
        let enabled = self
            .enabled_providers_repo
            .list_active_by_domain(domain_id)
            .await?;

        // Find a provider that matches the mode, preferring Stripe
        let provider_type = enabled
            .iter()
            .filter(|p| p.mode == payment_mode)
            .find(|p| p.provider == PaymentProvider::Stripe)
            .or_else(|| enabled.iter().find(|p| p.mode == payment_mode))
            .map(|p| p.provider)
            .unwrap_or_else(|| {
                // Default: Dummy for test mode, Stripe for live
                if payment_mode == PaymentMode::Test {
                    PaymentProvider::Dummy
                } else {
                    PaymentProvider::Stripe
                }
            });

        tracing::debug!(
            domain_id = %domain_id,
            provider = ?provider_type,
            mode = ?payment_mode,
            "Selected payment provider for plan change"
        );

        self.provider_factory
            .get(domain_id, provider_type, payment_mode)
            .await
    }

    /// Convert a SubscriptionPlanProfile to PlanInfo for the provider port.
    fn plan_to_port_info(plan: &SubscriptionPlanProfile) -> PlanInfo {
        PlanInfo {
            id: plan.id,
            code: plan.code.clone(),
            name: plan.name.clone(),
            price_cents: plan.price_cents,
            currency: plan.currency.clone(),
            interval: plan.interval.clone(),
            interval_count: plan.interval_count,
            trial_days: plan.trial_days,
            external_price_id: plan.stripe_price_id.clone(),
            external_product_id: plan.stripe_product_id.clone(),
        }
    }

    // ========================================================================
    // Stripe Config Methods
    // ========================================================================

    /// Get Stripe config status for both modes
    pub async fn get_stripe_config(
        &self,
        owner_id: Uuid,
        domain_id: Uuid,
    ) -> AppResult<StripeConfigStatusResponse> {
        let domain = self.get_domain_verified(owner_id, domain_id).await?;

        let configs = self.stripe_config_repo.list_by_domain(domain_id).await?;

        let test_config = configs.iter().find(|c| c.stripe_mode == StripeMode::Test);
        let live_config = configs.iter().find(|c| c.stripe_mode == StripeMode::Live);

        Ok(StripeConfigStatusResponse {
            active_mode: domain.billing_stripe_mode,
            test: test_config.map(|c| ModeConfigStatus {
                publishable_key_last4: mask_key(&c.stripe_publishable_key),
                is_connected: true,
            }),
            live: live_config.map(|c| ModeConfigStatus {
                publishable_key_last4: mask_key(&c.stripe_publishable_key),
                is_connected: true,
            }),
        })
    }

    /// Update Stripe config for a specific mode
    pub async fn update_stripe_config(
        &self,
        owner_id: Uuid,
        domain_id: Uuid,
        mode: StripeMode,
        secret_key: &str,
        publishable_key: &str,
        webhook_secret: &str,
    ) -> AppResult<ModeConfigStatus> {
        self.get_domain_verified(owner_id, domain_id).await?;

        // Validate key prefixes match the declared mode
        mode.validate_key_prefix(secret_key, "Secret key")
            .map_err(|e| AppError::InvalidInput(e))?;
        mode.validate_key_prefix(publishable_key, "Publishable key")
            .map_err(|e| AppError::InvalidInput(e))?;

        // Encrypt secrets
        let secret_key_encrypted = self.cipher.encrypt(secret_key)?;
        let webhook_secret_encrypted = self.cipher.encrypt(webhook_secret)?;

        self.stripe_config_repo
            .upsert(
                domain_id,
                mode,
                &secret_key_encrypted,
                publishable_key,
                &webhook_secret_encrypted,
            )
            .await?;

        Ok(ModeConfigStatus {
            publishable_key_last4: mask_key(publishable_key),
            is_connected: true,
        })
    }

    /// Delete Stripe config for a specific mode
    pub async fn delete_stripe_config(
        &self,
        owner_id: Uuid,
        domain_id: Uuid,
        mode: StripeMode,
    ) -> AppResult<()> {
        let domain = self.get_domain_verified(owner_id, domain_id).await?;

        // Cannot delete config for active mode if it has data
        if domain.billing_stripe_mode == mode {
            let plan_count = self
                .plan_repo
                .count_by_domain_and_mode(domain_id, mode)
                .await?;
            let sub_count = self
                .subscription_repo
                .count_by_domain_and_mode(domain_id, mode)
                .await?;

            if plan_count > 0 || sub_count > 0 {
                return Err(AppError::InvalidInput(format!(
                    "Cannot delete {} mode config while plans or subscriptions exist. Delete or migrate them first.",
                    mode.as_str()
                )));
            }
        }

        self.stripe_config_repo.delete(domain_id, mode).await
    }

    /// Switch the active Stripe mode for a domain
    pub async fn set_active_mode(
        &self,
        owner_id: Uuid,
        domain_id: Uuid,
        mode: StripeMode,
    ) -> AppResult<StripeMode> {
        self.get_domain_verified(owner_id, domain_id).await?;

        // Verify the mode has a config before activating
        let config = self
            .stripe_config_repo
            .get_by_domain_and_mode(domain_id, mode)
            .await?;
        if config.is_none() {
            return Err(AppError::InvalidInput(format!(
                "Cannot switch to {} mode without configuring Stripe keys first",
                mode.as_str()
            )));
        }

        self.domain_repo
            .set_billing_stripe_mode(domain_id, mode)
            .await?;
        Ok(mode)
    }

    /// Get decrypted Stripe secret key for a domain's active mode.
    /// Returns error if Stripe is not configured - there is no fallback.
    pub async fn get_stripe_secret_key(&self, domain_id: Uuid) -> AppResult<String> {
        let mode = self.get_active_mode(domain_id).await?;
        let config = self
            .stripe_config_repo
            .get_by_domain_and_mode(domain_id, mode)
            .await?
            .ok_or(AppError::InvalidInput(
                "Billing not configured for this domain. Please configure Stripe in the dashboard."
                    .into(),
            ))?;
        self.cipher.decrypt(&config.stripe_secret_key_encrypted)
    }

    /// Get decrypted Stripe secret key for a specific mode.
    pub async fn get_stripe_secret_key_for_mode(
        &self,
        domain_id: Uuid,
        mode: StripeMode,
    ) -> AppResult<String> {
        let config = self
            .stripe_config_repo
            .get_by_domain_and_mode(domain_id, mode)
            .await?
            .ok_or(AppError::InvalidInput(format!(
                "Stripe {} mode not configured for this domain.",
                mode.as_str()
            )))?;
        self.cipher.decrypt(&config.stripe_secret_key_encrypted)
    }

    /// Get webhook secret for a specific mode.
    /// Used by webhook handlers to verify signatures.
    pub async fn get_stripe_webhook_secret_for_mode(
        &self,
        domain_id: Uuid,
        mode: StripeMode,
    ) -> AppResult<String> {
        let config = self
            .stripe_config_repo
            .get_by_domain_and_mode(domain_id, mode)
            .await?
            .ok_or(AppError::InvalidInput(format!(
                "Stripe {} mode not configured for this domain.",
                mode.as_str()
            )))?;
        self.cipher.decrypt(&config.stripe_webhook_secret_encrypted)
    }

    /// Check if Stripe is configured for a domain (any mode).
    pub async fn is_stripe_configured(&self, domain_id: Uuid) -> AppResult<bool> {
        self.stripe_config_repo.has_any_config(domain_id).await
    }

    /// Check if Stripe is configured for a specific mode.
    pub async fn is_stripe_configured_for_mode(
        &self,
        domain_id: Uuid,
        mode: StripeMode,
    ) -> AppResult<bool> {
        Ok(self
            .stripe_config_repo
            .get_by_domain_and_mode(domain_id, mode)
            .await?
            .is_some())
    }

    // ========================================================================
    // Payment Provider Methods
    // ========================================================================

    /// List all enabled payment providers for a domain
    pub async fn list_enabled_providers(
        &self,
        owner_id: Uuid,
        domain_id: Uuid,
    ) -> AppResult<Vec<EnabledPaymentProviderProfile>> {
        self.get_domain_verified(owner_id, domain_id).await?;
        self.enabled_providers_repo.list_by_domain(domain_id).await
    }

    /// List only active payment providers for a domain (for checkout display)
    pub async fn list_active_providers(
        &self,
        domain_id: Uuid,
    ) -> AppResult<Vec<EnabledPaymentProviderProfile>> {
        self.enabled_providers_repo
            .list_active_by_domain(domain_id)
            .await
    }

    /// Enable a payment provider for a domain
    pub async fn enable_provider(
        &self,
        owner_id: Uuid,
        domain_id: Uuid,
        provider: PaymentProvider,
        mode: PaymentMode,
    ) -> AppResult<EnabledPaymentProviderProfile> {
        self.get_domain_verified(owner_id, domain_id).await?;

        // Validate provider supports this mode
        if !provider.supports_mode(mode) {
            return Err(AppError::InvalidInput(format!(
                "{} does not support {} mode",
                provider.display_name(),
                mode.as_str()
            )));
        }

        // For Stripe, ensure it's configured for the mode
        if provider == PaymentProvider::Stripe {
            let stripe_mode = match mode {
                PaymentMode::Test => StripeMode::Test,
                PaymentMode::Live => StripeMode::Live,
            };
            if !self
                .is_stripe_configured_for_mode(domain_id, stripe_mode)
                .await?
            {
                return Err(AppError::InvalidInput(format!(
                    "Stripe {} mode must be configured before enabling",
                    mode.as_str()
                )));
            }
        }

        // Coinbase is not yet implemented
        if provider == PaymentProvider::Coinbase {
            return Err(AppError::ProviderNotSupported);
        }

        // Get current max display_order
        let existing = self
            .enabled_providers_repo
            .list_by_domain(domain_id)
            .await?;
        let display_order = existing.iter().map(|p| p.display_order).max().unwrap_or(-1) + 1;

        self.enabled_providers_repo
            .enable(domain_id, provider, mode, display_order)
            .await
    }

    /// Disable a payment provider for a domain
    pub async fn disable_provider(
        &self,
        owner_id: Uuid,
        domain_id: Uuid,
        provider: PaymentProvider,
        mode: PaymentMode,
    ) -> AppResult<()> {
        self.get_domain_verified(owner_id, domain_id).await?;

        // Check if this is the only active provider
        let active = self
            .enabled_providers_repo
            .list_active_by_domain(domain_id)
            .await?;
        let is_target =
            |p: &EnabledPaymentProviderProfile| p.provider == provider && p.mode == mode;

        if active.len() == 1 && active.iter().any(is_target) {
            return Err(AppError::InvalidInput(
                "Cannot disable the only active payment provider. Enable another provider first."
                    .into(),
            ));
        }

        self.enabled_providers_repo
            .disable(domain_id, provider, mode)
            .await
    }

    /// Set active status for a provider (toggle on/off without removing)
    pub async fn set_provider_active(
        &self,
        owner_id: Uuid,
        domain_id: Uuid,
        provider: PaymentProvider,
        mode: PaymentMode,
        is_active: bool,
    ) -> AppResult<()> {
        self.get_domain_verified(owner_id, domain_id).await?;

        // If deactivating, check if this would leave no active providers
        if !is_active {
            let active = self
                .enabled_providers_repo
                .list_active_by_domain(domain_id)
                .await?;
            let is_target =
                |p: &EnabledPaymentProviderProfile| p.provider == provider && p.mode == mode;

            if active.len() == 1 && active.iter().any(is_target) {
                return Err(AppError::InvalidInput(
                    "Cannot deactivate the only active payment provider".into(),
                ));
            }
        }

        self.enabled_providers_repo
            .set_active(domain_id, provider, mode, is_active)
            .await
    }

    /// Update display order for a provider
    pub async fn set_provider_display_order(
        &self,
        owner_id: Uuid,
        domain_id: Uuid,
        provider: PaymentProvider,
        mode: PaymentMode,
        display_order: i32,
    ) -> AppResult<()> {
        self.get_domain_verified(owner_id, domain_id).await?;
        self.enabled_providers_repo
            .set_display_order(domain_id, provider, mode, display_order)
            .await
    }

    /// Check if a provider is enabled for a domain
    pub async fn is_provider_enabled(
        &self,
        domain_id: Uuid,
        provider: PaymentProvider,
        mode: PaymentMode,
    ) -> AppResult<bool> {
        self.enabled_providers_repo
            .is_enabled(domain_id, provider, mode)
            .await
    }

    // ========================================================================
    // Plan Methods
    // ========================================================================

    pub async fn create_plan(
        &self,
        owner_id: Uuid,
        domain_id: Uuid,
        input: CreatePlanInput,
    ) -> AppResult<SubscriptionPlanProfile> {
        let domain = self.get_domain_verified(owner_id, domain_id).await?;

        // Validate input
        if input.code.is_empty() || input.code.len() > 50 {
            return Err(AppError::InvalidInput(
                "Plan code must be 1-50 characters".into(),
            ));
        }
        if input.name.is_empty() || input.name.len() > 100 {
            return Err(AppError::InvalidInput(
                "Plan name must be 1-100 characters".into(),
            ));
        }
        if input.price_cents < 0 {
            return Err(AppError::InvalidInput("Price cannot be negative".into()));
        }

        // Create plan in the domain's active mode
        self.plan_repo
            .create(domain_id, domain.billing_stripe_mode, &input)
            .await
    }

    pub async fn update_plan(
        &self,
        owner_id: Uuid,
        domain_id: Uuid,
        plan_id: Uuid,
        input: UpdatePlanInput,
    ) -> AppResult<SubscriptionPlanProfile> {
        self.get_domain_verified(owner_id, domain_id).await?;

        // Verify plan belongs to domain
        let plan = self
            .plan_repo
            .get_by_id(plan_id)
            .await?
            .ok_or(AppError::NotFound)?;
        if plan.domain_id != domain_id {
            return Err(AppError::NotFound);
        }

        self.plan_repo.update(plan_id, &input).await
    }

    pub async fn archive_plan(
        &self,
        owner_id: Uuid,
        domain_id: Uuid,
        plan_id: Uuid,
    ) -> AppResult<()> {
        self.get_domain_verified(owner_id, domain_id).await?;

        // Verify plan belongs to domain
        let plan = self
            .plan_repo
            .get_by_id(plan_id)
            .await?
            .ok_or(AppError::NotFound)?;
        if plan.domain_id != domain_id {
            return Err(AppError::NotFound);
        }

        self.plan_repo.archive(plan_id).await
    }

    pub async fn delete_plan(
        &self,
        owner_id: Uuid,
        domain_id: Uuid,
        plan_id: Uuid,
    ) -> AppResult<()> {
        self.get_domain_verified(owner_id, domain_id).await?;

        // Verify plan belongs to domain
        let plan = self
            .plan_repo
            .get_by_id(plan_id)
            .await?
            .ok_or(AppError::NotFound)?;
        if plan.domain_id != domain_id {
            return Err(AppError::NotFound);
        }

        // Check if plan has subscribers
        let subscriber_count = self.plan_repo.count_subscribers(plan_id).await?;
        if subscriber_count > 0 {
            return Err(AppError::InvalidInput(
                "Cannot delete plan with active subscribers. Archive it instead.".into(),
            ));
        }

        self.plan_repo.delete(plan_id).await
    }

    /// List plans for a domain's active mode (dashboard)
    pub async fn list_plans(
        &self,
        owner_id: Uuid,
        domain_id: Uuid,
        include_archived: bool,
    ) -> AppResult<Vec<SubscriptionPlanProfile>> {
        let domain = self.get_domain_verified(owner_id, domain_id).await?;
        self.plan_repo
            .list_by_domain_and_mode(domain_id, domain.billing_stripe_mode, include_archived)
            .await
    }

    /// List plans for a specific mode (dashboard, for viewing other mode's plans)
    pub async fn list_plans_for_mode(
        &self,
        owner_id: Uuid,
        domain_id: Uuid,
        mode: StripeMode,
        include_archived: bool,
    ) -> AppResult<Vec<SubscriptionPlanProfile>> {
        self.get_domain_verified(owner_id, domain_id).await?;
        self.plan_repo
            .list_by_domain_and_mode(domain_id, mode, include_archived)
            .await
    }

    pub async fn reorder_plans(
        &self,
        owner_id: Uuid,
        domain_id: Uuid,
        plan_ids: Vec<Uuid>,
    ) -> AppResult<()> {
        self.get_domain_verified(owner_id, domain_id).await?;

        // Verify each plan belongs to this domain before reordering
        for plan_id in &plan_ids {
            let plan = self
                .plan_repo
                .get_by_id(*plan_id)
                .await?
                .ok_or(AppError::NotFound)?;
            if plan.domain_id != domain_id {
                return Err(AppError::Forbidden);
            }
        }

        for (order, plan_id) in plan_ids.iter().enumerate() {
            self.plan_repo
                .set_display_order(*plan_id, order as i32)
                .await?;
        }
        Ok(())
    }

    // ========================================================================
    // Public Plan Methods (for ingress)
    // ========================================================================

    /// Get public plans for a domain's active mode (ingress billing page)
    pub async fn get_public_plans(
        &self,
        domain_id: Uuid,
    ) -> AppResult<Vec<SubscriptionPlanProfile>> {
        let mode = self.get_active_mode(domain_id).await?;
        self.plan_repo
            .list_public_by_domain_and_mode(domain_id, mode)
            .await
    }

    /// Get a plan by code in the domain's active mode
    pub async fn get_plan_by_code(
        &self,
        domain_id: Uuid,
        code: &str,
    ) -> AppResult<Option<SubscriptionPlanProfile>> {
        let mode = self.get_active_mode(domain_id).await?;
        self.plan_repo
            .get_by_domain_and_code(domain_id, mode, code)
            .await
    }

    /// Update a plan with Stripe product/price IDs (called during lazy Stripe setup)
    pub async fn set_stripe_ids(
        &self,
        plan_id: Uuid,
        product_id: &str,
        price_id: &str,
    ) -> AppResult<()> {
        self.plan_repo
            .set_stripe_ids(plan_id, product_id, price_id)
            .await
    }

    /// Find plan by Stripe price ID in a specific mode (used by webhook handlers)
    pub async fn get_plan_by_stripe_price_id(
        &self,
        domain_id: Uuid,
        mode: StripeMode,
        stripe_price_id: &str,
    ) -> AppResult<Option<SubscriptionPlanProfile>> {
        self.plan_repo
            .get_by_stripe_price_id(domain_id, mode, stripe_price_id)
            .await
    }

    // ========================================================================
    // Subscription Methods
    // ========================================================================

    /// Get user's subscription in the domain's active mode
    pub async fn get_user_subscription(
        &self,
        domain_id: Uuid,
        user_id: Uuid,
    ) -> AppResult<Option<UserSubscriptionProfile>> {
        let mode = self.get_active_mode(domain_id).await?;
        self.subscription_repo
            .get_by_user_and_mode(domain_id, mode, user_id)
            .await
    }

    /// Get user's subscription with plan info in the domain's active mode
    pub async fn get_user_subscription_with_plan(
        &self,
        domain_id: Uuid,
        user_id: Uuid,
    ) -> AppResult<Option<(UserSubscriptionProfile, SubscriptionPlanProfile)>> {
        let mode = self.get_active_mode(domain_id).await?;
        let sub = self
            .subscription_repo
            .get_by_user_and_mode(domain_id, mode, user_id)
            .await?;
        if let Some(sub) = sub {
            let plan = self
                .plan_repo
                .get_by_id(sub.plan_id)
                .await?
                .ok_or(AppError::Internal("Plan not found".into()))?;
            Ok(Some((sub, plan)))
        } else {
            Ok(None)
        }
    }

    /// List subscribers in the domain's active mode (dashboard)
    pub async fn list_subscribers(
        &self,
        owner_id: Uuid,
        domain_id: Uuid,
    ) -> AppResult<Vec<UserSubscriptionWithPlan>> {
        let domain = self.get_domain_verified(owner_id, domain_id).await?;
        self.subscription_repo
            .list_by_domain_and_mode(domain_id, domain.billing_stripe_mode)
            .await
    }

    /// List subscribers for a specific mode (dashboard)
    pub async fn list_subscribers_for_mode(
        &self,
        owner_id: Uuid,
        domain_id: Uuid,
        mode: StripeMode,
    ) -> AppResult<Vec<UserSubscriptionWithPlan>> {
        self.get_domain_verified(owner_id, domain_id).await?;
        self.subscription_repo
            .list_by_domain_and_mode(domain_id, mode)
            .await
    }

    pub async fn grant_subscription(
        &self,
        owner_id: Uuid,
        domain_id: Uuid,
        user_id: Uuid,
        plan_id: Uuid,
        stripe_customer_id: &str,
    ) -> AppResult<UserSubscriptionProfile> {
        let domain = self.get_domain_verified(owner_id, domain_id).await?;

        // Verify plan belongs to domain and is in active mode
        let plan = self
            .plan_repo
            .get_by_id(plan_id)
            .await?
            .ok_or(AppError::NotFound)?;
        if plan.domain_id != domain_id {
            return Err(AppError::NotFound);
        }
        if plan.stripe_mode != domain.billing_stripe_mode {
            return Err(AppError::InvalidInput(format!(
                "Cannot grant subscription to a plan in {} mode when active mode is {}",
                plan.stripe_mode.as_str(),
                domain.billing_stripe_mode.as_str()
            )));
        }

        let sub = self
            .subscription_repo
            .grant_manually(
                domain_id,
                domain.billing_stripe_mode,
                user_id,
                plan_id,
                owner_id,
                stripe_customer_id,
            )
            .await?;

        // Log event
        self.event_repo
            .create(&CreateSubscriptionEventInput {
                subscription_id: sub.id,
                event_type: "granted".to_string(),
                previous_status: None,
                new_status: Some(SubscriptionStatus::Active),
                stripe_event_id: None,
                metadata: serde_json::json!({"granted_by": owner_id.to_string()}),
                created_by: Some(owner_id),
            })
            .await?;

        Ok(sub)
    }

    pub async fn revoke_subscription(
        &self,
        owner_id: Uuid,
        domain_id: Uuid,
        user_id: Uuid,
    ) -> AppResult<()> {
        let domain = self.get_domain_verified(owner_id, domain_id).await?;

        let sub = self
            .subscription_repo
            .get_by_user_and_mode(domain_id, domain.billing_stripe_mode, user_id)
            .await?
            .ok_or(AppError::NotFound)?;

        // Log event before revoking
        self.event_repo
            .create(&CreateSubscriptionEventInput {
                subscription_id: sub.id,
                event_type: "revoked".to_string(),
                previous_status: Some(sub.status),
                new_status: Some(SubscriptionStatus::Canceled),
                stripe_event_id: None,
                metadata: serde_json::json!({"revoked_by": owner_id.to_string()}),
                created_by: Some(owner_id),
            })
            .await?;

        self.subscription_repo.revoke(sub.id).await
    }

    // ========================================================================
    // Plan Change (Upgrade/Downgrade)
    // ========================================================================

    /// Preview a plan change (proration calculation)
    pub async fn preview_plan_change(
        &self,
        domain_id: Uuid,
        user_id: Uuid,
        new_plan_code: &str,
    ) -> AppResult<PlanChangePreview> {
        let mode = self.get_active_mode(domain_id).await?;

        // Get user's current subscription
        let sub = self
            .subscription_repo
            .get_by_user_and_mode(domain_id, mode, user_id)
            .await?
            .ok_or(AppError::InvalidInput(
                "No active subscription found".into(),
            ))?;

        // Validate subscription state
        self.validate_subscription_for_plan_change(&sub)?;

        // Get current plan
        let current_plan = self
            .plan_repo
            .get_by_id(sub.plan_id)
            .await?
            .ok_or(AppError::Internal("Current plan not found".into()))?;

        // Get new plan
        let new_plan = self
            .plan_repo
            .get_by_domain_and_code(domain_id, mode, new_plan_code)
            .await?
            .ok_or(AppError::InvalidInput(format!(
                "Plan '{}' not found",
                new_plan_code
            )))?;

        // Validate new plan
        self.validate_new_plan(&current_plan, &new_plan)?;

        // Get subscription ID
        let stripe_subscription_id =
            sub.stripe_subscription_id
                .as_ref()
                .ok_or(AppError::InvalidInput(
                    "Cannot preview change for manually granted subscription".into(),
                ))?;

        // Get provider and call preview
        let provider = self.get_active_provider(domain_id).await?;
        let subscription_id = SubscriptionId::new(stripe_subscription_id);
        let plan_info = Self::plan_to_port_info(&new_plan);

        let preview = provider
            .preview_plan_change(&subscription_id, &plan_info)
            .await?;

        // Convert port type to domain type
        let change_type = match preview.change_type {
            PortPlanChangeType::Upgrade => PlanChangeType::Upgrade,
            PortPlanChangeType::Downgrade => PlanChangeType::Downgrade,
        };

        Ok(PlanChangePreview {
            prorated_amount_cents: preview.prorated_amount_cents,
            currency: preview.currency,
            period_end: preview.period_end.timestamp(),
            new_plan_name: preview.new_plan_name,
            new_plan_price_cents: preview.new_plan_price_cents,
            change_type,
            effective_at: preview.effective_at.timestamp(),
        })
    }

    /// Execute plan change (upgrade or downgrade)
    pub async fn change_plan(
        &self,
        domain_id: Uuid,
        user_id: Uuid,
        new_plan_code: &str,
    ) -> AppResult<PlanChangeResult> {
        let mode = self.get_active_mode(domain_id).await?;

        // Get user's current subscription
        let sub = self
            .subscription_repo
            .get_by_user_and_mode(domain_id, mode, user_id)
            .await?
            .ok_or(AppError::InvalidInput(
                "No active subscription found".into(),
            ))?;

        // Validate subscription state
        self.validate_subscription_for_plan_change(&sub)?;

        // Get current plan
        let current_plan = self
            .plan_repo
            .get_by_id(sub.plan_id)
            .await?
            .ok_or(AppError::Internal("Current plan not found".into()))?;

        // Get new plan
        let new_plan = self
            .plan_repo
            .get_by_domain_and_code(domain_id, mode, new_plan_code)
            .await?
            .ok_or(AppError::InvalidInput(format!(
                "Plan '{}' not found",
                new_plan_code
            )))?;

        // Validate new plan
        self.validate_new_plan(&current_plan, &new_plan)?;

        // Get subscription ID
        let stripe_subscription_id =
            sub.stripe_subscription_id
                .as_ref()
                .ok_or(AppError::InvalidInput(
                    "Cannot change plan for manually granted subscription".into(),
                ))?;

        // Get provider
        let provider = self.get_active_provider(domain_id).await?;
        let subscription_id = SubscriptionId::new(stripe_subscription_id);
        let plan_info = Self::plan_to_port_info(&new_plan);

        // Execute plan change via provider
        let result = provider
            .change_plan(&subscription_id, None, &plan_info)
            .await?;

        // Convert change type
        let change_type = match result.change_type {
            PortPlanChangeType::Upgrade => PlanChangeType::Upgrade,
            PortPlanChangeType::Downgrade => PlanChangeType::Downgrade,
        };

        // Log the plan change event
        self.event_repo
            .create(&CreateSubscriptionEventInput {
                subscription_id: sub.id,
                event_type: if result.schedule_id.is_some() {
                    "plan_change_scheduled".to_string()
                } else {
                    "plan_change".to_string()
                },
                previous_status: Some(sub.status),
                new_status: Some(sub.status),
                stripe_event_id: None,
                metadata: serde_json::json!({
                    "change_type": change_type.as_str(),
                    "from_plan": current_plan.code,
                    "to_plan": new_plan.code,
                    "amount_charged_cents": result.amount_charged_cents,
                    "payment_intent_status": result.payment_intent_status,
                    "schedule_id": result.schedule_id,
                }),
                created_by: Some(user_id),
            })
            .await?;

        // Update local plan if payment succeeded immediately
        if result.payment_intent_status.as_deref() == Some("succeeded") {
            self.subscription_repo
                .update_plan(sub.id, new_plan.id)
                .await?;
        }

        Ok(PlanChangeResult {
            success: result.success,
            change_type,
            invoice_id: result.invoice_id,
            amount_charged_cents: result.amount_charged_cents,
            currency: result.currency,
            client_secret: result.client_secret,
            hosted_invoice_url: result.hosted_invoice_url,
            payment_intent_status: result.payment_intent_status,
            new_plan,
            effective_at: result.effective_at.timestamp(),
            schedule_id: result.schedule_id,
        })
    }

    /// Validate that a subscription is in a state that allows plan changes
    fn validate_subscription_for_plan_change(
        &self,
        sub: &UserSubscriptionProfile,
    ) -> AppResult<()> {
        // Check if manually granted
        if sub.manually_granted {
            return Err(AppError::InvalidInput(
                "Cannot change plan for manually granted subscriptions".into(),
            ));
        }

        // Check if has Stripe subscription ID
        if sub.stripe_subscription_id.is_none() {
            return Err(AppError::InvalidInput(
                "Cannot change plan: subscription not linked to Stripe".into(),
            ));
        }

        // Validate status
        match sub.status {
            SubscriptionStatus::Active | SubscriptionStatus::Trialing => Ok(()),
            SubscriptionStatus::Canceled if sub.cancel_at_period_end => {
                // Allow re-subscribing if just cancel_at_period_end
                Ok(())
            }
            SubscriptionStatus::PastDue => {
                Err(AppError::InvalidInput(
                    "Cannot change plan while payment is past due. Please update your payment method first.".into()
                ))
            }
            SubscriptionStatus::Incomplete => {
                Err(AppError::InvalidInput(
                    "Cannot change plan: please complete the current payment first.".into()
                ))
            }
            SubscriptionStatus::IncompleteExpired => {
                Err(AppError::InvalidInput(
                    "Subscription has expired. Please subscribe again.".into()
                ))
            }
            SubscriptionStatus::Unpaid => {
                Err(AppError::InvalidInput(
                    "Cannot change plan while subscription is unpaid. Please update your payment method.".into()
                ))
            }
            SubscriptionStatus::Paused => {
                Err(AppError::InvalidInput(
                    "Cannot change plan while subscription is paused.".into()
                ))
            }
            SubscriptionStatus::Canceled => {
                Err(AppError::InvalidInput(
                    "Cannot change plan on a canceled subscription. Please subscribe again.".into()
                ))
            }
        }
    }

    /// Validate that the new plan is acceptable for a plan change
    fn validate_new_plan(
        &self,
        current_plan: &SubscriptionPlanProfile,
        new_plan: &SubscriptionPlanProfile,
    ) -> AppResult<()> {
        // Must be different plan
        if current_plan.id == new_plan.id {
            return Err(AppError::InvalidInput(
                "Already subscribed to this plan".into(),
            ));
        }

        // New plan must be public
        if !new_plan.is_public {
            return Err(AppError::InvalidInput(
                "This plan is not available for subscription".into(),
            ));
        }

        // New plan must not be archived
        if new_plan.is_archived {
            return Err(AppError::InvalidInput(
                "This plan is no longer available".into(),
            ));
        }

        // Must have same interval (can't switch between monthly and yearly)
        if current_plan.interval != new_plan.interval {
            return Err(AppError::InvalidInput(format!(
                "Cannot switch between {} and {} billing. Please cancel and resubscribe.",
                current_plan.interval, new_plan.interval
            )));
        }

        // Must have same interval count
        if current_plan.interval_count != new_plan.interval_count {
            return Err(AppError::InvalidInput(format!(
                "Cannot switch between different billing frequencies. Please cancel and resubscribe."
            )));
        }

        Ok(())
    }

    // ========================================================================
    // Analytics
    // ========================================================================

    pub async fn get_analytics(
        &self,
        owner_id: Uuid,
        domain_id: Uuid,
    ) -> AppResult<BillingAnalytics> {
        let domain = self.get_domain_verified(owner_id, domain_id).await?;
        let mode = domain.billing_stripe_mode;

        let active = self
            .subscription_repo
            .count_by_status_and_mode(domain_id, mode, SubscriptionStatus::Active)
            .await?;
        let trialing = self
            .subscription_repo
            .count_by_status_and_mode(domain_id, mode, SubscriptionStatus::Trialing)
            .await?;
        let past_due = self
            .subscription_repo
            .count_by_status_and_mode(domain_id, mode, SubscriptionStatus::PastDue)
            .await?;

        // Calculate MRR from active subscriptions
        let subscribers = self
            .subscription_repo
            .list_by_domain_and_mode(domain_id, mode)
            .await?;

        // Accumulator tuple: (plan_name, subscriber_count, revenue_cents_f64)
        type PlanAccumulator = (String, i64, f64);

        // Use f64 for accumulation to preserve precision; round at the end
        let mut mrr_cents_f64: f64 = 0.0;
        let mut plan_stats: std::collections::HashMap<Uuid, PlanAccumulator> =
            std::collections::HashMap::new();

        for sub in &subscribers {
            if sub.subscription.status.is_active() {
                let interval_count = sub.plan.interval_count as i64;
                let price = sub.plan.price_cents as i64;

                let monthly_amount = calculate_monthly_amount_cents(
                    price,
                    sub.plan.interval.as_str(),
                    interval_count,
                );
                mrr_cents_f64 += monthly_amount;

                let entry =
                    plan_stats
                        .entry(sub.plan.id)
                        .or_insert((sub.plan.name.clone(), 0, 0.0));
                entry.1 += 1;
                entry.2 += monthly_amount;
            }
        }

        // Convert to i64 with proper rounding
        let mrr_cents = mrr_cents_f64.round() as i64;

        let plan_distribution = plan_stats
            .into_iter()
            .map(|(id, (name, count, revenue_f64))| PlanDistribution {
                plan_id: id,
                plan_name: name,
                subscriber_count: count,
                revenue_cents: revenue_f64.round() as i64,
            })
            .collect();

        Ok(BillingAnalytics {
            mrr_cents,
            active_subscribers: active,
            trialing_subscribers: trialing,
            past_due_subscribers: past_due,
            plan_distribution,
        })
    }

    // ========================================================================
    // Subscription Claims for JWT
    // ========================================================================

    pub async fn get_subscription_claims(
        &self,
        domain_id: Uuid,
        user_id: Uuid,
    ) -> AppResult<SubscriptionClaims> {
        let mode = self.get_active_mode(domain_id).await?;
        let sub = self
            .subscription_repo
            .get_by_user_and_mode(domain_id, mode, user_id)
            .await?;

        if let Some(sub) = sub {
            let plan = self.plan_repo.get_by_id(sub.plan_id).await?;
            Ok(SubscriptionClaims {
                status: sub.status.as_str().to_string(),
                plan_code: plan.as_ref().map(|p| p.code.clone()),
                plan_name: plan.as_ref().map(|p| p.name.clone()),
                current_period_end: sub.current_period_end.map(|dt| dt.and_utc().timestamp()),
                cancel_at_period_end: Some(sub.cancel_at_period_end),
                trial_ends_at: sub.trial_end.map(|dt| dt.and_utc().timestamp()),
                subscription_id: Some(sub.id.to_string()),
            })
        } else {
            // Return "none" status for users without a subscription
            Ok(SubscriptionClaims::none())
        }
    }

    // ========================================================================
    // Webhook Handling
    // ========================================================================

    pub async fn is_event_processed(&self, stripe_event_id: &str) -> AppResult<bool> {
        self.event_repo
            .exists_by_stripe_event_id(stripe_event_id)
            .await
    }

    /// Create or update a subscription (used by webhooks)
    pub async fn create_or_update_subscription(
        &self,
        input: &CreateSubscriptionInput,
    ) -> AppResult<UserSubscriptionProfile> {
        // Check if subscription already exists for this user in this mode
        if let Some(existing) = self
            .subscription_repo
            .get_by_user_and_mode(input.domain_id, input.stripe_mode, input.end_user_id)
            .await?
        {
            // Update existing subscription with new plan and Stripe IDs
            let update = StripeSubscriptionUpdate {
                status: input.status,
                plan_id: Some(input.plan_id), // Update plan (handles upgrade/downgrade)
                stripe_subscription_id: input.stripe_subscription_id.clone(), // Update Stripe subscription ID
                current_period_start: input.current_period_start,
                current_period_end: input.current_period_end,
                cancel_at_period_end: false,
                canceled_at: None,
                trial_start: input.trial_start,
                trial_end: input.trial_end,
            };
            self.subscription_repo
                .update_from_stripe(existing.id, &update)
                .await
        } else {
            self.subscription_repo.create(input).await
        }
    }

    pub async fn update_subscription_from_stripe(
        &self,
        stripe_subscription_id: &str,
        update: &StripeSubscriptionUpdate,
    ) -> AppResult<UserSubscriptionProfile> {
        let sub = self
            .subscription_repo
            .get_by_stripe_subscription_id(stripe_subscription_id)
            .await?
            .ok_or(AppError::NotFound)?;
        self.subscription_repo
            .update_from_stripe(sub.id, update)
            .await
    }

    pub async fn log_webhook_event(
        &self,
        subscription_id: Uuid,
        event_type: &str,
        previous_status: Option<SubscriptionStatus>,
        new_status: Option<SubscriptionStatus>,
        stripe_event_id: &str,
        metadata: serde_json::Value,
    ) -> AppResult<()> {
        self.event_repo
            .create(&CreateSubscriptionEventInput {
                subscription_id,
                event_type: event_type.to_string(),
                previous_status,
                new_status,
                stripe_event_id: Some(stripe_event_id.to_string()),
                metadata,
                created_by: None,
            })
            .await
    }

    // ========================================================================
    // Payment History Methods
    // ========================================================================

    /// Sync an invoice from Stripe webhook data
    pub async fn sync_invoice_from_webhook(
        &self,
        domain_id: Uuid,
        mode: StripeMode,
        invoice: &serde_json::Value,
    ) -> AppResult<BillingPaymentProfile> {
        let stripe_invoice_id = invoice["id"].as_str().unwrap_or("");
        let customer_id = invoice["customer"].as_str().unwrap_or("");

        // Try to find the subscription and user from the customer ID
        let subscription = self
            .subscription_repo
            .get_by_stripe_customer_id(domain_id, mode, customer_id)
            .await?;

        let (end_user_id, subscription_id, plan_id, plan_code, plan_name) =
            if let Some(sub) = &subscription {
                let plan = self.plan_repo.get_by_id(sub.plan_id).await?;
                (
                    sub.end_user_id,
                    Some(sub.id),
                    Some(sub.plan_id),
                    plan.as_ref().map(|p| p.code.clone()),
                    plan.as_ref().map(|p| p.name.clone()),
                )
            } else {
                // If we can't find the subscription, we can't create the payment
                return Err(AppError::NotFound);
            };

        let status =
            PaymentStatus::from_stripe_invoice_status(invoice["status"].as_str().unwrap_or(""));

        let payment_date = if status == PaymentStatus::Paid {
            invoice["status_transitions"]["paid_at"]
                .as_i64()
                .and_then(|ts| NaiveDateTime::from_timestamp_opt(ts, 0))
        } else {
            None
        };

        let input = CreatePaymentInput {
            domain_id,
            stripe_mode: mode,
            end_user_id,
            subscription_id,
            stripe_invoice_id: stripe_invoice_id.to_string(),
            stripe_payment_intent_id: invoice["payment_intent"].as_str().map(|s| s.to_string()),
            stripe_customer_id: customer_id.to_string(),
            amount_cents: invoice["amount_due"].as_i64().unwrap_or(0) as i32,
            amount_paid_cents: invoice["amount_paid"].as_i64().unwrap_or(0) as i32,
            currency: invoice["currency"].as_str().unwrap_or("usd").to_uppercase(),
            status,
            plan_id,
            plan_code,
            plan_name,
            hosted_invoice_url: invoice["hosted_invoice_url"]
                .as_str()
                .map(|s| s.to_string()),
            invoice_pdf_url: invoice["invoice_pdf"].as_str().map(|s| s.to_string()),
            invoice_number: invoice["number"].as_str().map(|s| s.to_string()),
            billing_reason: invoice["billing_reason"].as_str().map(|s| s.to_string()),
            failure_message: None,
            invoice_created_at: invoice["created"]
                .as_i64()
                .and_then(|ts| NaiveDateTime::from_timestamp_opt(ts, 0)),
            payment_date,
        };

        self.payment_repo.upsert_from_stripe(&input).await
    }

    /// Create a payment record for dummy provider checkout
    pub async fn create_dummy_payment(
        &self,
        domain_id: Uuid,
        user_id: Uuid,
        subscription_id: Uuid,
        plan: &SubscriptionPlanProfile,
    ) -> AppResult<BillingPaymentProfile> {
        let now = chrono::Utc::now().naive_utc();
        let invoice_id = format!("dummy_inv_{}", Uuid::new_v4());

        let input = CreatePaymentInput {
            domain_id,
            stripe_mode: StripeMode::Test,
            end_user_id: user_id,
            subscription_id: Some(subscription_id),
            stripe_invoice_id: invoice_id,
            stripe_payment_intent_id: Some(format!("dummy_pi_{}", Uuid::new_v4())),
            stripe_customer_id: format!("dummy_cus_{}", user_id),
            amount_cents: plan.price_cents,
            amount_paid_cents: plan.price_cents,
            currency: plan.currency.to_uppercase(),
            status: PaymentStatus::Paid,
            plan_id: Some(plan.id),
            plan_code: Some(plan.code.clone()),
            plan_name: Some(plan.name.clone()),
            hosted_invoice_url: None,
            invoice_pdf_url: None,
            invoice_number: Some(format!("DUMMY-{}", now.format("%Y%m%d%H%M%S"))),
            billing_reason: Some("subscription_create".to_string()),
            failure_message: None,
            invoice_created_at: Some(now),
            payment_date: Some(now),
        };

        self.payment_repo.upsert_from_stripe(&input).await
    }

    /// Update payment status (for failures and refunds)
    pub async fn update_payment_status(
        &self,
        stripe_invoice_id: &str,
        status: PaymentStatus,
        amount_refunded_cents: Option<i32>,
        failure_message: Option<String>,
    ) -> AppResult<()> {
        self.payment_repo
            .update_status(
                stripe_invoice_id,
                status,
                amount_refunded_cents,
                failure_message,
            )
            .await
    }

    /// Get user's own payment history (for ingress billing page)
    pub async fn get_user_payments(
        &self,
        domain_id: Uuid,
        user_id: Uuid,
        page: i32,
        per_page: i32,
    ) -> AppResult<PaginatedPayments> {
        let mode = self.get_active_mode(domain_id).await?;
        self.payment_repo
            .list_by_user(domain_id, mode, user_id, page, per_page)
            .await
    }

    /// List payments for domain dashboard with filters
    pub async fn list_domain_payments(
        &self,
        owner_id: Uuid,
        domain_id: Uuid,
        filters: &PaymentListFilters,
        page: i32,
        per_page: i32,
    ) -> AppResult<PaginatedPayments> {
        let domain = self.get_domain_verified(owner_id, domain_id).await?;
        self.payment_repo
            .list_by_domain(
                domain_id,
                domain.billing_stripe_mode,
                filters,
                page,
                per_page,
            )
            .await
    }

    /// Get payment summary for dashboard
    pub async fn get_payment_summary(
        &self,
        owner_id: Uuid,
        domain_id: Uuid,
        date_from: Option<NaiveDateTime>,
        date_to: Option<NaiveDateTime>,
    ) -> AppResult<PaymentSummary> {
        let domain = self.get_domain_verified(owner_id, domain_id).await?;
        self.payment_repo
            .get_payment_summary(domain_id, domain.billing_stripe_mode, date_from, date_to)
            .await
    }

    /// Export payments as CSV
    pub async fn export_payments_csv(
        &self,
        owner_id: Uuid,
        domain_id: Uuid,
        filters: &PaymentListFilters,
    ) -> AppResult<String> {
        let domain = self.get_domain_verified(owner_id, domain_id).await?;
        let payments = self
            .payment_repo
            .list_all_for_export(domain_id, domain.billing_stripe_mode, filters)
            .await?;

        // Build CSV content
        let mut csv = String::new();
        csv.push_str("Date,User Email,Plan,Amount,Status,Invoice Number,Billing Reason\n");

        for p in payments {
            let date = p
                .payment
                .payment_date
                .or(p.payment.created_at)
                .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                .unwrap_or_default();

            let amount = format!("{:.2}", p.payment.amount_cents as f64 / 100.0);

            // Escape all user-provided fields for security (including formula injection prevention)
            let email = escape_csv_field(&p.user_email);
            let plan = escape_csv_field(p.payment.plan_name.as_deref().unwrap_or(""));
            let invoice = escape_csv_field(p.payment.invoice_number.as_deref().unwrap_or(""));
            let reason = escape_csv_field(p.payment.billing_reason.as_deref().unwrap_or(""));

            csv.push_str(&format!(
                "{},{},{},{},{},{},{}\n",
                date,
                email,
                plan,
                amount,
                p.payment.status.as_str(),
                invoice,
                reason
            ));
        }

        Ok(csv)
    }
}

/// Escape a field for CSV output, including formula injection prevention.
/// Spreadsheet applications (Excel, Google Sheets, etc.) will execute formulas
/// starting with =, +, -, @, tab, or carriage return. We prefix such values
/// with a single quote to prevent formula injection attacks.
fn escape_csv_field(field: &str) -> String {
    let needs_quoting =
        field.contains(',') || field.contains('"') || field.contains('\n') || field.contains('\r');

    // Check for formula injection characters at start
    let is_formula = field
        .chars()
        .next()
        .map(|c| matches!(c, '=' | '+' | '-' | '@' | '\t' | '\r'))
        .unwrap_or(false);

    let escaped = if is_formula {
        // Prefix with single quote to prevent formula execution
        format!("'{}", field)
    } else {
        field.to_string()
    };

    if needs_quoting || is_formula {
        format!("\"{}\"", escaped.replace('"', "\"\""))
    } else {
        escaped
    }
}

// ============================================================================
// Response Types
// ============================================================================

/// Mask a key to show only last 4 characters
fn mask_key(key: &str) -> String {
    if key.len() <= 4 {
        "*".repeat(key.len())
    } else {
        format!("...{}", &key[key.len() - 4..])
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ModeConfigStatus {
    pub publishable_key_last4: String,
    pub is_connected: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct StripeConfigStatusResponse {
    pub active_mode: StripeMode,
    pub test: Option<ModeConfigStatus>,
    pub live: Option<ModeConfigStatus>,
}

// Keep old response type for backwards compatibility if needed
#[derive(Debug, Clone, Serialize)]
pub struct StripeConfigResponse {
    pub publishable_key: String,
    pub has_secret_key: bool,
    pub is_connected: bool,
    // NOTE: No using_fallback field - each domain must configure their own Stripe account.
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SubscriptionClaims {
    pub status: String,
    pub plan_code: Option<String>,
    pub plan_name: Option<String>,
    pub current_period_end: Option<i64>,
    pub cancel_at_period_end: Option<bool>,
    pub trial_ends_at: Option<i64>,
    pub subscription_id: Option<String>,
}

impl SubscriptionClaims {
    pub fn none() -> Self {
        Self {
            status: "none".to_string(),
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod mrr_calculation_tests {
    use super::*;

    #[test]
    fn test_calculate_monthly_amount_yearly_plan() {
        // $119/year = 11900 cents / 12 months = 991.666... cents
        let result = calculate_monthly_amount_cents(11900, "yearly", 1);
        assert!((result - 991.6666666666666).abs() < 0.0001);
        assert_eq!(result.round() as i64, 992); // Rounds to 992, not truncates to 991
    }

    #[test]
    fn test_calculate_monthly_amount_year_alias() {
        // "year" should behave same as "yearly"
        let result = calculate_monthly_amount_cents(11900, "year", 1);
        assert_eq!(result.round() as i64, 992);
    }

    #[test]
    fn test_calculate_monthly_amount_monthly_plan() {
        // $9.99/month = 999 cents, interval_count=1
        let result = calculate_monthly_amount_cents(999, "monthly", 1);
        assert_eq!(result, 999.0);
    }

    #[test]
    fn test_calculate_monthly_amount_month_alias() {
        // "month" should behave same as "monthly"
        let result = calculate_monthly_amount_cents(999, "month", 1);
        assert_eq!(result, 999.0);
    }

    #[test]
    fn test_calculate_monthly_amount_biannual() {
        // $200 every 2 years = 20000 cents / 24 months = 833.333... cents
        let result = calculate_monthly_amount_cents(20000, "yearly", 2);
        assert!((result - 833.3333333333334).abs() < 0.0001);
        assert_eq!(result.round() as i64, 833);
    }

    #[test]
    fn test_calculate_monthly_amount_quarterly() {
        // $30 every 3 months = 3000 cents / 3 = 1000 cents
        let result = calculate_monthly_amount_cents(3000, "monthly", 3);
        assert_eq!(result, 1000.0);
    }

    #[test]
    fn test_calculate_monthly_amount_zero_price() {
        // Free plan
        let result = calculate_monthly_amount_cents(0, "yearly", 1);
        assert_eq!(result, 0.0);
    }

    #[test]
    fn test_calculate_monthly_amount_zero_interval_count() {
        // Edge case: interval_count=0 should be treated as 1 (defensive)
        let result = calculate_monthly_amount_cents(1200, "monthly", 0);
        assert_eq!(result, 1200.0);
    }

    #[test]
    fn test_calculate_monthly_amount_very_small_yearly() {
        // $1.99/year = 199 cents / 12 = 16.583... cents
        let result = calculate_monthly_amount_cents(199, "yearly", 1);
        assert_eq!(result.round() as i64, 17); // Rounds up, not truncates to 16
    }

    #[test]
    fn test_calculate_monthly_amount_unknown_interval() {
        // Unknown intervals default to monthly behavior (legacy)
        let result = calculate_monthly_amount_cents(1500, "weekly", 1);
        assert_eq!(result, 1500.0); // Treated as monthly
    }

    #[test]
    fn test_mrr_accumulation_precision() {
        // 10 subscribers at $119/year each
        // Each: 11900/12 = 991.666...
        // Sum: 9916.666...
        // Rounded: 9917 cents
        let mut total = 0.0;
        for _ in 0..10 {
            total += calculate_monthly_amount_cents(11900, "yearly", 1);
        }
        assert_eq!(total.round() as i64, 9917);

        // Old integer math would give: 10 * (11900 / 12) = 10 * 991 = 9910
        // We gain 7 cents of accuracy
    }

    #[test]
    fn test_mrr_accumulation_mixed_intervals() {
        // 5 yearly at $119 + 3 monthly at $9.99
        let mut total = 0.0;
        for _ in 0..5 {
            total += calculate_monthly_amount_cents(11900, "yearly", 1);
        }
        for _ in 0..3 {
            total += calculate_monthly_amount_cents(999, "monthly", 1);
        }
        // 5 * 991.666... + 3 * 999 = 4958.333... + 2997 = 7955.333...
        assert_eq!(total.round() as i64, 7955);
    }

    #[test]
    fn test_mrr_large_subscriber_count() {
        // 1000 subscribers at $119/year
        // Each: 991.666...
        // Total: 991666.666...
        // Rounded: 991667 cents = $9916.67 MRR
        let mut total = 0.0;
        for _ in 0..1000 {
            total += calculate_monthly_amount_cents(11900, "yearly", 1);
        }
        assert_eq!(total.round() as i64, 991667);

        // Old integer math: 1000 * 991 = 991000 (loses $6.67)
    }
}
