use async_trait::async_trait;
use chrono::{DateTime, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

/// Convert a Unix timestamp to NaiveDateTime
fn timestamp_to_naive(secs: i64) -> Option<NaiveDateTime> {
    DateTime::<Utc>::from_timestamp(secs, 0).map(|dt| dt.naive_utc())
}

use crate::{
    adapters::persistence::enabled_payment_providers::{
        EnabledPaymentProviderProfile, EnabledPaymentProvidersRepoTrait,
    },
    app_error::{AppError, AppResult},
    application::ports::payment_provider::{
        PaymentProviderPort, PlanChangeType as PortPlanChangeType, PlanInfo, SubscriptionId,
    },
    application::use_cases::payment_provider_factory::PaymentProviderFactory,
    domain::entities::{
        billing_state::BillingState, payment_mode::PaymentMode, payment_provider::PaymentProvider,
        payment_status::PaymentStatus, user_subscription::SubscriptionStatus,
    },
    infra::crypto::ProcessCipher,
};

use super::domain::DomainRepoTrait;
use crate::application::validators::is_valid_plan_code;

/// Maximum number of plan changes allowed per billing period
const MAX_PLAN_CHANGES_PER_PERIOD: i32 = 5;

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
        // Monthly or any unknown intervals are treated as monthly
        _ => interval_count,
    };

    price_cents as f64 / divisor as f64
}

/// Calculates the prorated amount for a plan change (upgrade or downgrade).
///
/// Standard proration formula:
/// - remaining_ratio = remaining_seconds / total_seconds
/// - credit = current_price * remaining_ratio (0 if on trial)
/// - charge = new_price * remaining_ratio
/// - proration = charge - credit
///
/// For upgrades, result is positive (user pays the difference).
/// For downgrades, result is negative (user would get credit, but we schedule instead).
/// For trials, no credit is given since nothing was paid yet.
fn calculate_proration(
    current_price_cents: i64,
    new_price_cents: i64,
    period_start: DateTime<Utc>,
    period_end: DateTime<Utc>,
    now: DateTime<Utc>,
    is_trial: bool,
) -> i64 {
    let total_seconds = (period_end - period_start).num_seconds() as f64;
    let remaining_seconds = (period_end - now).num_seconds() as f64;

    if total_seconds <= 0.0 || remaining_seconds <= 0.0 {
        return 0;
    }

    let ratio = remaining_seconds / total_seconds;

    let credit = if is_trial {
        0.0
    } else {
        current_price_cents as f64 * ratio
    };
    let charge = new_price_cents as f64 * ratio;

    (charge - credit).round() as i64
}

// ============================================================================
// Profile Types
// ============================================================================

#[derive(Debug, Clone)]
pub struct BillingStripeConfigProfile {
    pub id: Uuid,
    pub domain_id: Uuid,
    pub payment_mode: PaymentMode,
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
    pub payment_provider: Option<PaymentProvider>,
    pub payment_mode: PaymentMode,
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
    pub payment_provider: Option<PaymentProvider>,
    pub payment_mode: PaymentMode,
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
    // Rate limiting for plan changes
    pub changes_this_period: i32,
    pub period_changes_reset_at: Option<DateTime<Utc>>,
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
    pub payment_provider: Option<PaymentProvider>,
    pub payment_mode: PaymentMode,
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
    pub payment_mode: PaymentMode,
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
    pub payment_mode: PaymentMode,
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

use strum::{AsRefStr, Display, EnumString};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, AsRefStr, Display, EnumString)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase", ascii_case_insensitive)]
pub enum PlanChangeType {
    Upgrade,   // Immediate with proration
    Downgrade, // Scheduled for period end
    Lateral,   // Same price, scheduled for period end
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
    pub warnings: Vec<String>,
    /// Number of plan changes remaining in current billing period
    pub changes_remaining_this_period: i32,
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
pub trait BillingStripeConfigRepoTrait: Send + Sync {
    /// Get Stripe config for a specific mode
    async fn get_by_domain_and_mode(
        &self,
        domain_id: Uuid,
        mode: PaymentMode,
    ) -> AppResult<Option<BillingStripeConfigProfile>>;

    /// List all configs for a domain (both test and live)
    async fn list_by_domain(&self, domain_id: Uuid) -> AppResult<Vec<BillingStripeConfigProfile>>;

    /// Upsert config for a specific mode
    async fn upsert(
        &self,
        domain_id: Uuid,
        mode: PaymentMode,
        stripe_secret_key_encrypted: &str,
        stripe_publishable_key: &str,
        stripe_webhook_secret_encrypted: &str,
    ) -> AppResult<BillingStripeConfigProfile>;

    /// Delete config for a specific mode
    async fn delete(&self, domain_id: Uuid, mode: PaymentMode) -> AppResult<()>;

    /// Check if any config exists for this domain (either mode)
    async fn has_any_config(&self, domain_id: Uuid) -> AppResult<bool>;
}

#[async_trait]
pub trait SubscriptionPlanRepoTrait: Send + Sync {
    async fn get_by_id(&self, id: Uuid) -> AppResult<Option<SubscriptionPlanProfile>>;
    async fn get_by_domain_and_code(
        &self,
        domain_id: Uuid,
        mode: PaymentMode,
        code: &str,
    ) -> AppResult<Option<SubscriptionPlanProfile>>;
    async fn list_by_domain_and_mode(
        &self,
        domain_id: Uuid,
        mode: PaymentMode,
        include_archived: bool,
    ) -> AppResult<Vec<SubscriptionPlanProfile>>;
    async fn list_public_by_domain_and_mode(
        &self,
        domain_id: Uuid,
        mode: PaymentMode,
    ) -> AppResult<Vec<SubscriptionPlanProfile>>;
    async fn create(
        &self,
        domain_id: Uuid,
        mode: PaymentMode,
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
    async fn count_by_domain_and_mode(&self, domain_id: Uuid, mode: PaymentMode) -> AppResult<i64>;
    /// Find plan by Stripe price ID (searches all plans in the mode, including archived)
    async fn get_by_stripe_price_id(
        &self,
        domain_id: Uuid,
        mode: PaymentMode,
        stripe_price_id: &str,
    ) -> AppResult<Option<SubscriptionPlanProfile>>;
}

#[async_trait]
pub trait UserSubscriptionRepoTrait: Send + Sync {
    async fn get_by_id(&self, id: Uuid) -> AppResult<Option<UserSubscriptionProfile>>;
    async fn get_by_user_and_mode(
        &self,
        domain_id: Uuid,
        mode: PaymentMode,
        end_user_id: Uuid,
    ) -> AppResult<Option<UserSubscriptionProfile>>;
    async fn get_by_stripe_subscription_id(
        &self,
        stripe_subscription_id: &str,
    ) -> AppResult<Option<UserSubscriptionProfile>>;
    async fn get_by_stripe_customer_id(
        &self,
        domain_id: Uuid,
        mode: PaymentMode,
        stripe_customer_id: &str,
    ) -> AppResult<Option<UserSubscriptionProfile>>;
    async fn list_by_domain_and_mode(
        &self,
        domain_id: Uuid,
        mode: PaymentMode,
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
        mode: PaymentMode,
        end_user_id: Uuid,
        plan_id: Uuid,
        granted_by: Uuid,
        stripe_customer_id: &str,
    ) -> AppResult<UserSubscriptionProfile>;
    async fn revoke(&self, id: Uuid) -> AppResult<()>;
    async fn delete(&self, id: Uuid) -> AppResult<()>;
    /// Atomically increment the plan change counter if under the limit.
    /// Returns true if increment succeeded, false if rate limit exceeded.
    /// This prevents race conditions where concurrent requests could bypass the limit.
    async fn increment_changes_counter(
        &self,
        id: Uuid,
        period_end: DateTime<Utc>,
        max_changes: i32,
    ) -> AppResult<bool>;
    async fn count_active_by_domain_and_mode(
        &self,
        domain_id: Uuid,
        mode: PaymentMode,
    ) -> AppResult<i64>;
    async fn count_by_status_and_mode(
        &self,
        domain_id: Uuid,
        mode: PaymentMode,
        status: SubscriptionStatus,
    ) -> AppResult<i64>;
    /// Count subscriptions in a specific mode (for deletion validation)
    async fn count_by_domain_and_mode(&self, domain_id: Uuid, mode: PaymentMode) -> AppResult<i64>;
}

#[async_trait]
pub trait SubscriptionEventRepoTrait: Send + Sync {
    async fn create(&self, input: &CreateSubscriptionEventInput) -> AppResult<()>;
    async fn list_by_subscription(
        &self,
        subscription_id: Uuid,
    ) -> AppResult<Vec<SubscriptionEventProfile>>;
    async fn exists_by_stripe_event_id(&self, stripe_event_id: &str) -> AppResult<bool>;
}

#[async_trait]
pub trait BillingPaymentRepoTrait: Send + Sync {
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
        mode: PaymentMode,
        end_user_id: Uuid,
        page: i32,
        per_page: i32,
    ) -> AppResult<PaginatedPayments>;

    /// List payments for a domain with filters (dashboard)
    async fn list_by_domain(
        &self,
        domain_id: Uuid,
        mode: PaymentMode,
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
        mode: PaymentMode,
        date_from: Option<NaiveDateTime>,
        date_to: Option<NaiveDateTime>,
    ) -> AppResult<PaymentSummary>;

    /// List all payments for export (no pagination)
    async fn list_all_for_export(
        &self,
        domain_id: Uuid,
        mode: PaymentMode,
        filters: &PaymentListFilters,
    ) -> AppResult<Vec<BillingPaymentWithUser>>;
}

// ============================================================================
// Use Cases
// ============================================================================

#[derive(Clone)]
pub struct DomainBillingUseCases {
    domain_repo: Arc<dyn DomainRepoTrait>,
    stripe_config_repo: Arc<dyn BillingStripeConfigRepoTrait>,
    enabled_providers_repo: Arc<dyn EnabledPaymentProvidersRepoTrait>,
    plan_repo: Arc<dyn SubscriptionPlanRepoTrait>,
    subscription_repo: Arc<dyn UserSubscriptionRepoTrait>,
    event_repo: Arc<dyn SubscriptionEventRepoTrait>,
    payment_repo: Arc<dyn BillingPaymentRepoTrait>,
    cipher: ProcessCipher,
    provider_factory: Arc<PaymentProviderFactory>,
    // NOTE: No fallback Stripe credentials - we cannot accept payments on behalf of other developers.
    // Each domain must configure their own Stripe account.
}

impl DomainBillingUseCases {
    pub fn new(
        domain_repo: Arc<dyn DomainRepoTrait>,
        stripe_config_repo: Arc<dyn BillingStripeConfigRepoTrait>,
        enabled_providers_repo: Arc<dyn EnabledPaymentProvidersRepoTrait>,
        plan_repo: Arc<dyn SubscriptionPlanRepoTrait>,
        subscription_repo: Arc<dyn UserSubscriptionRepoTrait>,
        event_repo: Arc<dyn SubscriptionEventRepoTrait>,
        payment_repo: Arc<dyn BillingPaymentRepoTrait>,
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

    /// Get the active payment mode for a domain
    pub async fn get_active_mode(&self, domain_id: Uuid) -> AppResult<PaymentMode> {
        let domain = self
            .domain_repo
            .get_by_id(domain_id)
            .await?
            .ok_or(AppError::NotFound)?;
        Ok(domain.active_payment_mode)
    }

    /// Get the active payment provider for a domain.
    ///
    /// This method determines the correct provider based on:
    /// 1. The domain's active payment mode
    /// 2. Enabled providers for the domain (prefers Stripe if configured)
    async fn get_active_provider(
        &self,
        domain_id: Uuid,
    ) -> AppResult<Arc<dyn PaymentProviderPort>> {
        // Get the domain's active mode
        let payment_mode = self.get_active_mode(domain_id).await?;

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

    /// Ensure a plan has Stripe product and price IDs, creating them lazily if needed.
    /// This is used during checkout and plan changes to auto-create Stripe resources.
    /// Uses the active payment provider (Stripe or Dummy) via the provider factory.
    async fn ensure_stripe_ids(
        &self,
        domain_id: Uuid,
        plan: SubscriptionPlanProfile,
    ) -> AppResult<SubscriptionPlanProfile> {
        if plan.stripe_product_id.is_some() && plan.stripe_price_id.is_some() {
            return Ok(plan);
        }

        // Use the active provider (Stripe in production, Dummy in tests)
        let provider = self.get_active_provider(domain_id).await?;
        let plan_info = Self::plan_to_port_info(&plan);
        let (product_id, price_id) = provider.ensure_product_and_price(&plan_info).await?;

        // Persist to DB
        self.set_stripe_ids(plan.id, &product_id, &price_id).await?;

        // Return updated plan
        Ok(SubscriptionPlanProfile {
            stripe_product_id: Some(product_id),
            stripe_price_id: Some(price_id),
            ..plan
        })
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

        let test_config = configs.iter().find(|c| c.payment_mode == PaymentMode::Test);
        let live_config = configs.iter().find(|c| c.payment_mode == PaymentMode::Live);

        Ok(StripeConfigStatusResponse {
            active_mode: domain.active_payment_mode,
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
        mode: PaymentMode,
        secret_key: &str,
        publishable_key: &str,
        webhook_secret: &str,
    ) -> AppResult<ModeConfigStatus> {
        self.get_domain_verified(owner_id, domain_id).await?;

        // Validate key prefixes match the declared mode
        mode.validate_stripe_key_prefix(secret_key, "Secret key")
            .map_err(AppError::InvalidInput)?;
        mode.validate_stripe_key_prefix(publishable_key, "Publishable key")
            .map_err(AppError::InvalidInput)?;

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
        mode: PaymentMode,
    ) -> AppResult<()> {
        let domain = self.get_domain_verified(owner_id, domain_id).await?;

        // Cannot delete config for active mode if it has data
        if domain.active_payment_mode == mode {
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
                    mode.as_ref()
                )));
            }
        }

        self.stripe_config_repo.delete(domain_id, mode).await
    }

    /// Switch the active payment mode for a domain
    pub async fn set_active_mode(
        &self,
        owner_id: Uuid,
        domain_id: Uuid,
        mode: PaymentMode,
    ) -> AppResult<PaymentMode> {
        self.get_domain_verified(owner_id, domain_id).await?;

        // Verify the mode has a config before activating
        let config = self
            .stripe_config_repo
            .get_by_domain_and_mode(domain_id, mode)
            .await?;
        if config.is_none() {
            return Err(AppError::InvalidInput(format!(
                "Cannot switch to {} mode without configuring Stripe keys first",
                mode.as_ref()
            )));
        }

        self.domain_repo
            .set_active_payment_mode(domain_id, mode)
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
        mode: PaymentMode,
    ) -> AppResult<String> {
        let config = self
            .stripe_config_repo
            .get_by_domain_and_mode(domain_id, mode)
            .await?
            .ok_or(AppError::InvalidInput(format!(
                "Stripe {} mode not configured for this domain.",
                mode.as_ref()
            )))?;
        self.cipher.decrypt(&config.stripe_secret_key_encrypted)
    }

    /// Get webhook secret for a specific mode.
    /// Used by webhook handlers to verify signatures.
    pub async fn get_stripe_webhook_secret_for_mode(
        &self,
        domain_id: Uuid,
        mode: PaymentMode,
    ) -> AppResult<String> {
        let config = self
            .stripe_config_repo
            .get_by_domain_and_mode(domain_id, mode)
            .await?
            .ok_or(AppError::InvalidInput(format!(
                "Stripe {} mode not configured for this domain.",
                mode.as_ref()
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
        mode: PaymentMode,
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
                mode.as_ref()
            )));
        }

        // For Stripe, ensure it's configured for the mode
        if provider == PaymentProvider::Stripe
            && !self.is_stripe_configured_for_mode(domain_id, mode).await?
        {
            return Err(AppError::InvalidInput(format!(
                "Stripe {} mode must be configured before enabling",
                mode.as_ref()
            )));
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
        mut input: CreatePlanInput,
    ) -> AppResult<SubscriptionPlanProfile> {
        let domain = self.get_domain_verified(owner_id, domain_id).await?;

        // Reject non-ASCII codes before normalization to prevent Unicode case-folding exploits
        // (e.g., Kelvin sign "K" normalizing to ASCII "k")
        if !input.code.is_ascii() {
            return Err(AppError::InvalidInput(
                "Plan code must contain only ASCII characters.".into(),
            ));
        }

        // Normalize code to lowercase
        input.code = input.code.to_lowercase();

        // Validate plan code (must be URL-friendly)
        if !is_valid_plan_code(&input.code) {
            return Err(AppError::InvalidInput(
                "Plan code must be 1-50 characters using only lowercase letters, numbers, hyphens, and underscores. Must start with a letter or number.".into(),
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
            .create(domain_id, domain.active_payment_mode, &input)
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
            .list_by_domain_and_mode(domain_id, domain.active_payment_mode, include_archived)
            .await
    }

    /// List plans for a specific mode (dashboard, for viewing other mode's plans)
    pub async fn list_plans_for_mode(
        &self,
        owner_id: Uuid,
        domain_id: Uuid,
        mode: PaymentMode,
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
        mode: PaymentMode,
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
            .list_by_domain_and_mode(domain_id, domain.active_payment_mode)
            .await
    }

    /// List subscribers for a specific mode (dashboard)
    pub async fn list_subscribers_for_mode(
        &self,
        owner_id: Uuid,
        domain_id: Uuid,
        mode: PaymentMode,
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
        if plan.payment_mode != domain.active_payment_mode {
            return Err(AppError::InvalidInput(format!(
                "Cannot grant subscription to a plan in {} mode when active mode is {}",
                plan.payment_mode.as_ref(),
                domain.active_payment_mode.as_ref()
            )));
        }

        let sub = self
            .subscription_repo
            .grant_manually(
                domain_id,
                domain.active_payment_mode,
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
            .get_by_user_and_mode(domain_id, domain.active_payment_mode, user_id)
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
    ///
    /// Calculates proration internally (provider-agnostic) instead of relying on
    /// provider-specific preview APIs which may fail for edge cases like
    /// subscriptions with `cancel_at_period_end=true`.
    pub async fn preview_plan_change(
        &self,
        domain_id: Uuid,
        user_id: Uuid,
        new_plan_code: &str,
    ) -> AppResult<PlanChangePreview> {
        let mode = self.get_active_mode(domain_id).await?;

        // Get user's current subscription from local DB
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

        // Get subscription ID for provider lookup
        let external_subscription_id =
            sub.stripe_subscription_id
                .as_ref()
                .ok_or(AppError::InvalidInput(
                    "Cannot preview change for manually granted subscription".into(),
                ))?;

        // Ensure new plan has external IDs (lazy creation like checkout)
        let new_plan = self.ensure_stripe_ids(domain_id, new_plan).await?;

        // Get subscription info from provider for period data
        let provider = self.get_active_provider(domain_id).await?;
        let subscription_id = SubscriptionId::new(external_subscription_id);
        let sub_info = provider
            .get_subscription(&subscription_id)
            .await?
            .ok_or(AppError::NotFound)?;

        // Extract period info
        let period_start = sub_info.current_period_start.ok_or(AppError::Internal(
            "Subscription missing period start".into(),
        ))?;
        let period_end = sub_info.current_period_end.ok_or(AppError::Internal(
            "Subscription missing period end".into(),
        ))?;
        let now = Utc::now();

        // Determine if subscription is on trial
        let is_trial = sub_info
            .trial_end
            .map(|te| te > now)
            .unwrap_or(false);

        // Calculate proration internally (provider-agnostic)
        let current_price_cents = current_plan.price_cents as i64;
        let new_price_cents = new_plan.price_cents as i64;
        let prorated_amount = calculate_proration(
            current_price_cents,
            new_price_cents,
            period_start,
            period_end,
            now,
            is_trial,
        );

        // Determine change type based on price
        // - Higher price = Upgrade (immediate)
        // - Lower price = Downgrade (scheduled)
        // - Same price = Lateral (scheduled)
        let change_type = if new_price_cents > current_price_cents {
            PlanChangeType::Upgrade
        } else if new_price_cents < current_price_cents {
            PlanChangeType::Downgrade
        } else {
            PlanChangeType::Lateral
        };

        // Effective date: upgrades are immediate, downgrades/laterals at period end
        let effective_at = if change_type == PlanChangeType::Upgrade {
            now
        } else {
            period_end
        };

        // Generate warnings based on subscription state
        let mut warnings = Vec::new();

        if sub_info.cancel_at_period_end {
            warnings.push(
                "If you proceed, your subscription will be reactivated (cancellation removed)."
                    .to_string(),
            );
        }

        if is_trial {
            warnings.push(
                "Your trial will end immediately and you will be charged.".to_string(),
            );
        }

        // Calculate changes remaining in this period
        let current_changes = if let Some(reset_at) = sub.period_changes_reset_at {
            if reset_at < now {
                0 // Period has passed, counter will reset
            } else {
                sub.changes_this_period
            }
        } else {
            0
        };
        let changes_remaining = (MAX_PLAN_CHANGES_PER_PERIOD - current_changes).max(0);

        // Add warning if near rate limit
        if changes_remaining <= 2 && changes_remaining > 0 {
            warnings.push(format!(
                "You have {} plan change(s) remaining this billing period.",
                changes_remaining
            ));
        }

        Ok(PlanChangePreview {
            prorated_amount_cents: prorated_amount,
            currency: current_plan.currency.clone(),
            period_end: period_end.timestamp(),
            new_plan_name: new_plan.name.clone(),
            new_plan_price_cents: new_price_cents,
            change_type,
            effective_at: effective_at.timestamp(),
            warnings,
            changes_remaining_this_period: changes_remaining,
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

        // Ensure new plan has Stripe IDs (lazy creation like checkout)
        let new_plan = self.ensure_stripe_ids(domain_id, new_plan).await?;

        // Atomically check and increment rate limit counter BEFORE calling payment provider.
        // This prevents race conditions where concurrent requests could bypass the limit.
        // Use the subscription's period end, or calculate based on current plan's interval as fallback.
        let period_end = sub
            .current_period_end
            .map(|dt| DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc))
            .unwrap_or_else(|| {
                let interval_days = match current_plan.interval.as_str() {
                    "yearly" | "year" => 365,
                    "weekly" | "week" => 7,
                    "daily" | "day" => 1,
                    _ => 30, // monthly default
                };
                Utc::now() + chrono::Duration::days(interval_days)
            });

        let rate_limit_ok = self
            .subscription_repo
            .increment_changes_counter(sub.id, period_end, MAX_PLAN_CHANGES_PER_PERIOD)
            .await?;

        if !rate_limit_ok {
            return Err(AppError::InvalidInput(format!(
                "Maximum plan changes ({}) reached for this billing period. Try again after your next billing date.",
                MAX_PLAN_CHANGES_PER_PERIOD
            )));
        }

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
            PortPlanChangeType::Lateral => PlanChangeType::Lateral,
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
                    "change_type": change_type.as_ref(),
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

        // Note: Rate limit counter was already atomically incremented before the
        // payment provider call. We don't need to do anything here.

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

        // Check rate limit - reset if period has passed
        let changes_count = if let Some(reset_at) = sub.period_changes_reset_at {
            if reset_at < Utc::now() {
                // Period has passed, counter will be reset on next change
                0
            } else {
                sub.changes_this_period
            }
        } else {
            0
        };

        if changes_count >= MAX_PLAN_CHANGES_PER_PERIOD {
            return Err(AppError::InvalidInput(format!(
                "Maximum plan changes ({}) reached for this billing period. Try again after your next billing date.",
                MAX_PLAN_CHANGES_PER_PERIOD
            )));
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

        // Interval changes are allowed:
        // - monthly -> yearly = upgrade (immediate with proration)
        // - yearly -> monthly = downgrade (scheduled for year end)
        // The change_plan logic handles the timing based on price comparison.

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
        let mode = domain.active_payment_mode;

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
            .get_by_user_and_mode(input.domain_id, input.payment_mode, input.end_user_id)
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
        mode: PaymentMode,
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
                .and_then(timestamp_to_naive)
        } else {
            None
        };

        let input = CreatePaymentInput {
            domain_id,
            payment_mode: mode,
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
            invoice_created_at: invoice["created"].as_i64().and_then(timestamp_to_naive),
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
            payment_mode: PaymentMode::Test,
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
                domain.active_payment_mode,
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
            .get_payment_summary(domain_id, domain.active_payment_mode, date_from, date_to)
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
            .list_all_for_export(domain_id, domain.active_payment_mode, filters)
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
                p.payment.status.as_ref(),
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
    pub active_mode: PaymentMode,
    pub test: Option<ModeConfigStatus>,
    pub live: Option<ModeConfigStatus>,
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
        // Unknown intervals default to monthly behavior
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

#[cfg(test)]
mod proration_calculation_tests {
    use super::*;
    use chrono::Duration;

    fn make_period(days: i64) -> (DateTime<Utc>, DateTime<Utc>) {
        let start = Utc::now() - Duration::days(days / 2);
        let end = start + Duration::days(days);
        (start, end)
    }

    #[test]
    fn test_upgrade_midway_through_period() {
        // Upgrade from $20 to $100, 5 days used out of 30
        let period_start = Utc::now() - Duration::days(5);
        let period_end = period_start + Duration::days(30);
        let now = Utc::now();

        let result = calculate_proration(
            2000, // $20 current
            10000, // $100 new
            period_start,
            period_end,
            now,
            false, // not trial
        );

        // remaining = 25/30 = 0.833
        // credit = 2000 * 0.833 = 1666.67
        // charge = 10000 * 0.833 = 8333.33
        // net = 8333.33 - 1666.67 = 6666.67 ≈ 6667
        assert!(result > 6600 && result < 6700, "Expected ~6667, got {}", result);
    }

    #[test]
    fn test_downgrade_midway_through_period() {
        // Downgrade from $100 to $20, 5 days used out of 30
        let period_start = Utc::now() - Duration::days(5);
        let period_end = period_start + Duration::days(30);
        let now = Utc::now();

        let result = calculate_proration(
            10000, // $100 current
            2000,  // $20 new
            period_start,
            period_end,
            now,
            false,
        );

        // remaining = 25/30 = 0.833
        // credit = 10000 * 0.833 = 8333.33
        // charge = 2000 * 0.833 = 1666.67
        // net = 1666.67 - 8333.33 = -6666.67 ≈ -6667 (negative = credit)
        assert!(result < -6600 && result > -6700, "Expected ~-6667, got {}", result);
    }

    #[test]
    fn test_upgrade_during_trial_no_credit() {
        // During trial, user hasn't paid, so no credit for current plan
        let period_start = Utc::now() - Duration::days(1);
        let period_end = period_start + Duration::days(30);
        let now = Utc::now();

        let result = calculate_proration(
            2000,  // $20 current (but on trial)
            10000, // $100 new
            period_start,
            period_end,
            now,
            true, // ON TRIAL
        );

        // remaining = 29/30 = 0.967
        // credit = 0 (trial)
        // charge = 10000 * 0.967 = 9667
        // net = 9667
        assert!(result > 9600 && result < 9700, "Expected ~9667, got {}", result);
    }

    #[test]
    fn test_same_price_no_proration() {
        // Same price = 0 proration
        let period_start = Utc::now() - Duration::days(5);
        let period_end = period_start + Duration::days(30);
        let now = Utc::now();

        let result = calculate_proration(
            2000, // $20
            2000, // $20
            period_start,
            period_end,
            now,
            false,
        );

        assert_eq!(result, 0);
    }

    #[test]
    fn test_period_already_ended_returns_zero() {
        // If we're past period end, no proration
        let period_start = Utc::now() - Duration::days(35);
        let period_end = period_start + Duration::days(30); // ended 5 days ago
        let now = Utc::now();

        let result = calculate_proration(2000, 10000, period_start, period_end, now, false);

        assert_eq!(result, 0);
    }

    #[test]
    fn test_period_just_started_full_proration() {
        // Right at start of period = full remaining time
        let period_start = Utc::now();
        let period_end = period_start + Duration::days(30);
        let now = period_start + Duration::seconds(1); // 1 second in

        let result = calculate_proration(
            2000,
            10000,
            period_start,
            period_end,
            now,
            false,
        );

        // Nearly full period remaining
        // charge ≈ 10000, credit ≈ 2000, net ≈ 8000
        assert!(result > 7900 && result < 8100, "Expected ~8000, got {}", result);
    }

    #[test]
    fn test_period_about_to_end_minimal_proration() {
        // Almost end of period = minimal proration
        let period_start = Utc::now() - Duration::days(29);
        let period_end = period_start + Duration::days(30);
        let now = Utc::now(); // 1 day remaining

        let result = calculate_proration(
            2000,
            10000,
            period_start,
            period_end,
            now,
            false,
        );

        // remaining = 1/30 = 0.033
        // charge = 10000 * 0.033 = 333
        // credit = 2000 * 0.033 = 67
        // net = 333 - 67 = 267
        assert!(result > 200 && result < 350, "Expected ~267, got {}", result);
    }

    #[test]
    fn test_free_to_paid_upgrade() {
        // Free plan ($0) to paid plan ($100)
        let period_start = Utc::now() - Duration::days(15);
        let period_end = period_start + Duration::days(30);
        let now = Utc::now();

        let result = calculate_proration(
            0,     // Free plan
            10000, // $100 new
            period_start,
            period_end,
            now,
            false,
        );

        // remaining = 15/30 = 0.5
        // credit = 0 (free plan)
        // charge = 10000 * 0.5 = 5000
        assert_eq!(result, 5000);
    }

    #[test]
    fn test_paid_to_free_downgrade() {
        // Paid plan ($100) to free plan ($0)
        let period_start = Utc::now() - Duration::days(15);
        let period_end = period_start + Duration::days(30);
        let now = Utc::now();

        let result = calculate_proration(
            10000, // $100 current
            0,     // Free plan
            period_start,
            period_end,
            now,
            false,
        );

        // remaining = 15/30 = 0.5
        // credit = 10000 * 0.5 = 5000
        // charge = 0
        // net = -5000 (credit)
        assert_eq!(result, -5000);
    }
}

#[cfg(test)]
mod plan_change_type_tests {
    use super::*;

    #[test]
    fn test_as_ref_all_variants() {
        assert_eq!(PlanChangeType::Upgrade.as_ref(), "upgrade");
        assert_eq!(PlanChangeType::Downgrade.as_ref(), "downgrade");
        assert_eq!(PlanChangeType::Lateral.as_ref(), "lateral");
    }
}

#[cfg(test)]
mod classification_tests {
    use super::*;

    /// Helper to determine change type based on prices (mirrors the logic in preview_plan_change)
    fn classify_change(current_price: i64, new_price: i64) -> PlanChangeType {
        if new_price > current_price {
            PlanChangeType::Upgrade
        } else if new_price < current_price {
            PlanChangeType::Downgrade
        } else {
            PlanChangeType::Lateral
        }
    }

    #[test]
    fn test_higher_price_is_upgrade() {
        assert_eq!(classify_change(2000, 10000), PlanChangeType::Upgrade);
    }

    #[test]
    fn test_lower_price_is_downgrade() {
        assert_eq!(classify_change(10000, 2000), PlanChangeType::Downgrade);
    }

    #[test]
    fn test_same_price_is_lateral() {
        assert_eq!(classify_change(5000, 5000), PlanChangeType::Lateral);
    }

    #[test]
    fn test_free_to_paid_is_upgrade() {
        assert_eq!(classify_change(0, 1000), PlanChangeType::Upgrade);
    }

    #[test]
    fn test_paid_to_free_is_downgrade() {
        assert_eq!(classify_change(1000, 0), PlanChangeType::Downgrade);
    }

    #[test]
    fn test_free_to_free_is_lateral() {
        // Two different free plans
        assert_eq!(classify_change(0, 0), PlanChangeType::Lateral);
    }
}

#[cfg(test)]
mod rate_limit_tests {
    use super::*;

    #[test]
    fn test_max_changes_constant() {
        assert_eq!(MAX_PLAN_CHANGES_PER_PERIOD, 5);
    }

    #[test]
    fn test_changes_remaining_calculation() {
        // Test the calculation logic used in preview
        let now = Utc::now();
        let future_reset = now + chrono::Duration::days(15);
        let past_reset = now - chrono::Duration::days(1);

        // When reset is in future, use current count
        let changes_count = 3;
        let reset_at = Some(future_reset);
        let current = if let Some(r) = reset_at {
            if r < now { 0 } else { changes_count }
        } else {
            0
        };
        assert_eq!(current, 3);
        assert_eq!(MAX_PLAN_CHANGES_PER_PERIOD - current, 2);

        // When reset is in past, counter resets to 0
        let reset_at = Some(past_reset);
        let current = if let Some(r) = reset_at {
            if r < now { 0 } else { changes_count }
        } else {
            0
        };
        assert_eq!(current, 0);
        assert_eq!(MAX_PLAN_CHANGES_PER_PERIOD - current, 5);

        // When no reset_at, treat as 0
        let reset_at: Option<DateTime<Utc>> = None;
        let current = if let Some(r) = reset_at {
            if r < now { 0 } else { changes_count }
        } else {
            0
        };
        assert_eq!(current, 0);
    }

    #[test]
    fn test_rate_limit_threshold() {
        // Test that we correctly identify when limit is reached
        let at_limit = MAX_PLAN_CHANGES_PER_PERIOD;
        let below_limit = MAX_PLAN_CHANGES_PER_PERIOD - 1;

        assert!(at_limit >= MAX_PLAN_CHANGES_PER_PERIOD);
        assert!(below_limit < MAX_PLAN_CHANGES_PER_PERIOD);
    }
}

#[cfg(test)]
mod billing_integration_tests {
    use std::sync::Arc;

    use super::*;
    use crate::{
        infra::crypto::ProcessCipher,
        test_utils::{
            InMemoryBillingPaymentRepo, InMemoryBillingStripeConfigRepo, InMemoryDomainRepo,
            InMemoryEnabledPaymentProvidersRepo, InMemorySubscriptionEventRepo,
            InMemorySubscriptionPlanRepo, InMemoryUserSubscriptionRepo, create_test_domain,
            create_test_payment, create_test_plan, create_test_subscription,
        },
    };

    // Test key: 32 bytes of zeros, base64 encoded
    const TEST_KEY_B64: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";

    /// Helper to create a test billing use cases instance with in-memory repos.
    fn create_test_use_cases() -> (
        DomainBillingUseCases,
        Arc<InMemoryDomainRepo>,
        Arc<InMemorySubscriptionPlanRepo>,
        Arc<InMemoryUserSubscriptionRepo>,
        Arc<InMemorySubscriptionEventRepo>,
    ) {
        let domain_repo = Arc::new(InMemoryDomainRepo::new());
        let stripe_config_repo = Arc::new(InMemoryBillingStripeConfigRepo::new());
        let enabled_providers_repo = Arc::new(InMemoryEnabledPaymentProvidersRepo::new());
        let plan_repo = Arc::new(InMemorySubscriptionPlanRepo::new());
        // Link subscription repo to plan repo so list_by_domain_and_mode can retrieve plan data
        let subscription_repo =
            Arc::new(InMemoryUserSubscriptionRepo::new().with_plan_repo(plan_repo.clone()));
        let event_repo = Arc::new(InMemorySubscriptionEventRepo::new());
        let payment_repo = Arc::new(InMemoryBillingPaymentRepo::new());

        // Use a test cipher (32 bytes of zeros, base64 encoded)
        let cipher = ProcessCipher::new_from_base64(TEST_KEY_B64).unwrap();

        // Create a simple provider factory that returns DummyPaymentClient
        // For testing, we create a minimal mock that just returns the dummy client
        let factory = Arc::new(PaymentProviderFactory::new(
            cipher.clone(),
            stripe_config_repo.clone() as Arc<dyn BillingStripeConfigRepoTrait>,
        ));

        let use_cases = DomainBillingUseCases::new(
            domain_repo.clone() as Arc<dyn super::super::domain::DomainRepoTrait>,
            stripe_config_repo as Arc<dyn BillingStripeConfigRepoTrait>,
            enabled_providers_repo as Arc<dyn EnabledPaymentProvidersRepoTrait>,
            plan_repo.clone() as Arc<dyn SubscriptionPlanRepoTrait>,
            subscription_repo.clone() as Arc<dyn UserSubscriptionRepoTrait>,
            event_repo.clone() as Arc<dyn SubscriptionEventRepoTrait>,
            payment_repo as Arc<dyn BillingPaymentRepoTrait>,
            cipher,
            factory,
        );

        (
            use_cases,
            domain_repo,
            plan_repo,
            subscription_repo,
            event_repo,
        )
    }

    // ========================================================================
    // Plan CRUD Tests
    // ========================================================================

    #[tokio::test]
    async fn test_create_plan_success() {
        let (use_cases, domain_repo, plan_repo, _, _) = create_test_use_cases();

        // Create a test domain
        let owner_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        // Create a plan
        let input = CreatePlanInput {
            code: "basic".to_string(),
            name: "Basic Plan".to_string(),
            description: Some("A basic plan".to_string()),
            price_cents: 999,
            currency: "usd".to_string(),
            interval: "monthly".to_string(),
            interval_count: 1,
            trial_days: 7,
            features: vec!["Feature A".to_string()],
            is_public: true,
        };

        let plan = use_cases
            .create_plan(owner_id, domain.id, input)
            .await
            .unwrap();

        assert_eq!(plan.code, "basic");
        assert_eq!(plan.name, "Basic Plan");
        assert_eq!(plan.price_cents, 999);
        assert_eq!(plan.payment_mode, PaymentMode::Test);
        assert_eq!(plan.trial_days, 7);

        // Verify plan is in the repo
        let stored = plan_repo.get_by_id(plan.id).await.unwrap();
        assert!(stored.is_some());
    }

    #[tokio::test]
    async fn test_create_plan_rejects_invalid_code() {
        let (use_cases, domain_repo, _, _, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        // Empty code should fail
        let input = CreatePlanInput {
            code: "".to_string(),
            name: "Test Plan".to_string(),
            description: None,
            price_cents: 999,
            currency: "usd".to_string(),
            interval: "monthly".to_string(),
            interval_count: 1,
            trial_days: 0,
            features: vec![],
            is_public: true,
        };

        let result = use_cases.create_plan(owner_id, domain.id, input).await;
        assert!(matches!(result, Err(AppError::InvalidInput(_))));
    }

    #[tokio::test]
    async fn test_create_plan_rejects_negative_price() {
        let (use_cases, domain_repo, _, _, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let input = CreatePlanInput {
            code: "test".to_string(),
            name: "Test Plan".to_string(),
            description: None,
            price_cents: -100,
            currency: "usd".to_string(),
            interval: "monthly".to_string(),
            interval_count: 1,
            trial_days: 0,
            features: vec![],
            is_public: true,
        };

        let result = use_cases.create_plan(owner_id, domain.id, input).await;
        assert!(matches!(result, Err(AppError::InvalidInput(_))));
    }

    #[tokio::test]
    async fn test_create_plan_forbidden_for_non_owner() {
        let (use_cases, domain_repo, _, _, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let other_user_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let input = CreatePlanInput {
            code: "test".to_string(),
            name: "Test Plan".to_string(),
            description: None,
            price_cents: 999,
            currency: "usd".to_string(),
            interval: "monthly".to_string(),
            interval_count: 1,
            trial_days: 0,
            features: vec![],
            is_public: true,
        };

        // Non-owner should get Forbidden
        let result = use_cases.create_plan(other_user_id, domain.id, input).await;
        assert!(matches!(result, Err(AppError::Forbidden)));
    }

    #[tokio::test]
    async fn test_create_plan_rejects_special_characters() {
        let (use_cases, domain_repo, _, _, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let invalid_codes = vec![
            "plan@code",
            "plan.code",
            "plan/code",
            "plan!",
            "plan$code",
            "plan#tag",
        ];

        for invalid_code in invalid_codes {
            let input = CreatePlanInput {
                code: invalid_code.to_string(),
                name: "Test Plan".to_string(),
                description: None,
                price_cents: 999,
                currency: "usd".to_string(),
                interval: "monthly".to_string(),
                interval_count: 1,
                trial_days: 0,
                features: vec![],
                is_public: true,
            };

            let result = use_cases.create_plan(owner_id, domain.id, input).await;
            assert!(
                matches!(result, Err(AppError::InvalidInput(_))),
                "Expected InvalidInput for code: {}",
                invalid_code
            );
        }
    }

    #[tokio::test]
    async fn test_create_plan_rejects_leading_hyphen() {
        let (use_cases, domain_repo, _, _, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let input = CreatePlanInput {
            code: "-basic".to_string(),
            name: "Test Plan".to_string(),
            description: None,
            price_cents: 999,
            currency: "usd".to_string(),
            interval: "monthly".to_string(),
            interval_count: 1,
            trial_days: 0,
            features: vec![],
            is_public: true,
        };

        let result = use_cases.create_plan(owner_id, domain.id, input).await;
        assert!(matches!(result, Err(AppError::InvalidInput(_))));
    }

    #[tokio::test]
    async fn test_create_plan_rejects_leading_underscore() {
        let (use_cases, domain_repo, _, _, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let input = CreatePlanInput {
            code: "_basic".to_string(),
            name: "Test Plan".to_string(),
            description: None,
            price_cents: 999,
            currency: "usd".to_string(),
            interval: "monthly".to_string(),
            interval_count: 1,
            trial_days: 0,
            features: vec![],
            is_public: true,
        };

        let result = use_cases.create_plan(owner_id, domain.id, input).await;
        assert!(matches!(result, Err(AppError::InvalidInput(_))));
    }

    #[tokio::test]
    async fn test_create_plan_rejects_whitespace() {
        let (use_cases, domain_repo, _, _, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let whitespace_codes = vec!["basic plan", " basic", "basic ", "basic\t", "basic\n"];

        for code in whitespace_codes {
            let input = CreatePlanInput {
                code: code.to_string(),
                name: "Test Plan".to_string(),
                description: None,
                price_cents: 999,
                currency: "usd".to_string(),
                interval: "monthly".to_string(),
                interval_count: 1,
                trial_days: 0,
                features: vec![],
                is_public: true,
            };

            let result = use_cases.create_plan(owner_id, domain.id, input).await;
            assert!(
                matches!(result, Err(AppError::InvalidInput(_))),
                "Expected InvalidInput for code with whitespace: {:?}",
                code
            );
        }
    }

    #[tokio::test]
    async fn test_create_plan_normalizes_to_lowercase() {
        let (use_cases, domain_repo, plan_repo, _, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let input = CreatePlanInput {
            code: "BASIC-PLAN".to_string(),
            name: "Test Plan".to_string(),
            description: None,
            price_cents: 999,
            currency: "usd".to_string(),
            interval: "monthly".to_string(),
            interval_count: 1,
            trial_days: 0,
            features: vec![],
            is_public: true,
        };

        let result = use_cases.create_plan(owner_id, domain.id, input).await;
        assert!(result.is_ok());

        // Verify the plan was stored with lowercase code
        let plans = plan_repo.plans.lock().unwrap();
        let created_plan = plans.values().next().expect("Plan should exist");
        assert_eq!(created_plan.code, "basic-plan");
    }

    #[tokio::test]
    async fn test_create_plan_accepts_valid_codes_with_separators() {
        let (use_cases, domain_repo, plan_repo, _, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let valid_codes = vec![
            "pro-plan",
            "tier_1",
            "plan-with-dashes",
            "plan_with_underscores",
            "123plan",
            "plan123",
        ];

        for code in valid_codes {
            // Clear plan repo for each iteration
            plan_repo.plans.lock().unwrap().clear();

            let input = CreatePlanInput {
                code: code.to_string(),
                name: "Test Plan".to_string(),
                description: None,
                price_cents: 999,
                currency: "usd".to_string(),
                interval: "monthly".to_string(),
                interval_count: 1,
                trial_days: 0,
                features: vec![],
                is_public: true,
            };

            let result = use_cases.create_plan(owner_id, domain.id, input).await;
            assert!(result.is_ok(), "Expected success for code: {}", code);
        }
    }

    #[tokio::test]
    async fn test_create_plan_rejects_non_ascii_characters() {
        let (use_cases, domain_repo, _, _, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        // Test various non-ASCII codes that could potentially normalize to ASCII
        let non_ascii_codes = vec![
            "plän",     // German umlaut
            "план",     // Cyrillic
            "计划",     // Chinese
            "プラン",   // Japanese
            "\u{212A}", // Kelvin sign (normalizes to 'k')
        ];

        for code in non_ascii_codes {
            let input = CreatePlanInput {
                code: code.to_string(),
                name: "Test Plan".to_string(),
                description: None,
                price_cents: 999,
                currency: "usd".to_string(),
                interval: "monthly".to_string(),
                interval_count: 1,
                trial_days: 0,
                features: vec![],
                is_public: true,
            };

            let result = use_cases.create_plan(owner_id, domain.id, input).await;
            assert!(
                matches!(result, Err(AppError::InvalidInput(_))),
                "Expected InvalidInput for non-ASCII code: {:?}",
                code
            );
        }
    }

    #[tokio::test]
    async fn test_list_plans_excludes_archived_by_default() {
        let (use_cases, domain_repo, plan_repo, _, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        // Add active and archived plans
        let active_plan = create_test_plan(domain.id, |p| {
            p.is_archived = false;
            p.payment_mode = PaymentMode::Test;
        });
        let archived_plan = create_test_plan(domain.id, |p| {
            p.is_archived = true;
            p.payment_mode = PaymentMode::Test;
        });

        {
            let mut plans = plan_repo.plans.lock().unwrap();
            plans.insert(active_plan.id, active_plan.clone());
            plans.insert(archived_plan.id, archived_plan.clone());
        }

        // List without archived
        let plans = use_cases
            .list_plans(owner_id, domain.id, false)
            .await
            .unwrap();
        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].id, active_plan.id);

        // List with archived
        let plans = use_cases
            .list_plans(owner_id, domain.id, true)
            .await
            .unwrap();
        assert_eq!(plans.len(), 2);
    }

    #[tokio::test]
    async fn test_delete_plan_fails_with_subscribers() {
        let (use_cases, domain_repo, plan_repo, _, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let plan = create_test_plan(domain.id, |_| {});
        plan_repo
            .plans
            .lock()
            .unwrap()
            .insert(plan.id, plan.clone());

        // Set subscriber count > 0
        plan_repo.set_subscriber_count(plan.id, 5);

        let result = use_cases.delete_plan(owner_id, domain.id, plan.id).await;
        assert!(matches!(result, Err(AppError::InvalidInput(_))));
    }

    #[tokio::test]
    async fn test_delete_plan_succeeds_with_no_subscribers() {
        let (use_cases, domain_repo, plan_repo, _, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let plan = create_test_plan(domain.id, |_| {});
        plan_repo
            .plans
            .lock()
            .unwrap()
            .insert(plan.id, plan.clone());

        // No subscribers, should succeed
        let result = use_cases.delete_plan(owner_id, domain.id, plan.id).await;
        assert!(result.is_ok());

        // Plan should be gone
        let stored = plan_repo.get_by_id(plan.id).await.unwrap();
        assert!(stored.is_none());
    }

    // ========================================================================
    // MRR Calculation Tests (Use Case Level)
    // ========================================================================

    #[tokio::test]
    async fn test_calculate_mrr_aggregates_correctly() {
        let (use_cases, domain_repo, plan_repo, subscription_repo, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        // Create two plans: monthly and yearly
        let monthly_plan = create_test_plan(domain.id, |p| {
            p.code = "monthly".to_string();
            p.price_cents = 999; // $9.99/month
            p.interval = "monthly".to_string();
            p.payment_mode = PaymentMode::Test;
        });
        let yearly_plan = create_test_plan(domain.id, |p| {
            p.code = "yearly".to_string();
            p.price_cents = 9999; // $99.99/year
            p.interval = "yearly".to_string();
            p.payment_mode = PaymentMode::Test;
        });

        {
            let mut plans = plan_repo.plans.lock().unwrap();
            plans.insert(monthly_plan.id, monthly_plan.clone());
            plans.insert(yearly_plan.id, yearly_plan.clone());
        }

        // Create subscriptions
        let user1 = Uuid::new_v4();
        let user2 = Uuid::new_v4();

        let sub1 = create_test_subscription(domain.id, user1, monthly_plan.id, |s| {
            s.status = SubscriptionStatus::Active;
            s.payment_mode = PaymentMode::Test;
        });
        let sub2 = create_test_subscription(domain.id, user2, yearly_plan.id, |s| {
            s.status = SubscriptionStatus::Active;
            s.payment_mode = PaymentMode::Test;
        });

        {
            let mut subs = subscription_repo.subscriptions.lock().unwrap();
            subs.insert(sub1.id, sub1);
            subs.insert(sub2.id, sub2);
        }

        // Calculate MRR via get_analytics
        let analytics = use_cases.get_analytics(owner_id, domain.id).await.unwrap();

        // Expected: $9.99 + ($99.99/12) = $9.99 + $8.33 = $18.32 = 1832 cents
        // More precisely: 999 + 833 (rounded from 9999/12 = 833.25) = 1832
        assert!(
            analytics.mrr_cents >= 1831 && analytics.mrr_cents <= 1833,
            "MRR should be around 1832 cents, got {}",
            analytics.mrr_cents
        );
    }

    // ========================================================================
    // Subscription Claims Tests
    // ========================================================================

    #[tokio::test]
    async fn test_get_subscription_claims_active() {
        let (use_cases, domain_repo, plan_repo, subscription_repo, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let end_user_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let plan = create_test_plan(domain.id, |p| {
            p.code = "premium".to_string();
            p.name = "Premium Plan".to_string();
            p.payment_mode = PaymentMode::Test;
        });
        plan_repo
            .plans
            .lock()
            .unwrap()
            .insert(plan.id, plan.clone());

        let sub = create_test_subscription(domain.id, end_user_id, plan.id, |s| {
            s.status = SubscriptionStatus::Active;
            s.payment_mode = PaymentMode::Test;
        });
        subscription_repo
            .subscriptions
            .lock()
            .unwrap()
            .insert(sub.id, sub.clone());

        // Get subscription claims for user
        let claims = use_cases
            .get_subscription_claims(domain.id, end_user_id)
            .await
            .unwrap();

        assert_eq!(claims.status, "active");
        assert_eq!(claims.plan_code, Some("premium".to_string()));
        assert_eq!(claims.plan_name, Some("Premium Plan".to_string()));
    }

    #[tokio::test]
    async fn test_get_subscription_claims_returns_none_status_for_no_subscription() {
        let (use_cases, domain_repo, _, _, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        // Non-existent user should return "none" status
        let claims = use_cases
            .get_subscription_claims(domain.id, Uuid::new_v4())
            .await
            .unwrap();

        assert_eq!(claims.status, "none");
        assert!(claims.plan_code.is_none());
    }

    // ========================================================================
    // Provider Enablement Tests
    // ========================================================================

    #[tokio::test]
    async fn test_enable_dummy_provider() {
        let (use_cases, domain_repo, _, _, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        // Enable Dummy provider for test mode (doesn't require Stripe config)
        let result = use_cases
            .enable_provider(
                owner_id,
                domain.id,
                PaymentProvider::Dummy,
                PaymentMode::Test,
            )
            .await;

        assert!(result.is_ok());
        let provider = result.unwrap();
        assert_eq!(provider.provider, PaymentProvider::Dummy);
        assert!(provider.is_active);
    }

    #[tokio::test]
    async fn test_enable_stripe_requires_config() {
        let (use_cases, domain_repo, _, _, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        // Enabling Stripe without config should fail
        let result = use_cases
            .enable_provider(
                owner_id,
                domain.id,
                PaymentProvider::Stripe,
                PaymentMode::Test,
            )
            .await;

        assert!(matches!(result, Err(AppError::InvalidInput(_))));
    }

    // ========================================================================
    // Stripe Config Tests
    // ========================================================================

    #[tokio::test]
    async fn test_update_stripe_config_success() {
        let (use_cases, domain_repo, _, _, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let result = use_cases
            .update_stripe_config(
                owner_id,
                domain.id,
                PaymentMode::Test,
                "sk_test_abc123",
                "pk_test_abc123",
                "whsec_abc123",
            )
            .await;

        assert!(result.is_ok());
        let status = result.unwrap();
        assert!(status.is_connected);
    }

    #[tokio::test]
    async fn test_update_stripe_config_rejects_mismatched_key_mode() {
        let (use_cases, domain_repo, _, _, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        // Live key for test mode should fail
        let result = use_cases
            .update_stripe_config(
                owner_id,
                domain.id,
                PaymentMode::Test,
                "sk_live_abc123", // Wrong prefix for test mode
                "pk_test_abc123",
                "whsec_abc123",
            )
            .await;

        assert!(matches!(result, Err(AppError::InvalidInput(_))));
    }

    #[tokio::test]
    async fn test_get_stripe_config_returns_configured_mode_only() {
        let (use_cases, domain_repo, _, _, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        // Configure test mode only
        use_cases
            .update_stripe_config(
                owner_id,
                domain.id,
                PaymentMode::Test,
                "sk_test_abc123",
                "pk_test_abc123",
                "whsec_abc123",
            )
            .await
            .unwrap();

        let config = use_cases
            .get_stripe_config(owner_id, domain.id)
            .await
            .unwrap();

        // Only test mode should be configured
        assert!(config.test.is_some());
        assert!(config.live.is_none());
        assert_eq!(config.active_mode, PaymentMode::Test);
    }

    #[tokio::test]
    async fn test_set_active_mode_requires_config() {
        let (use_cases, domain_repo, _, _, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        // Switching to live without config should fail
        let result = use_cases
            .set_active_mode(owner_id, domain.id, PaymentMode::Live)
            .await;

        assert!(matches!(result, Err(AppError::InvalidInput(_))));
    }

    #[tokio::test]
    async fn test_set_active_mode_succeeds_with_config() {
        let (use_cases, domain_repo, _, _, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        // Configure live mode first
        use_cases
            .update_stripe_config(
                owner_id,
                domain.id,
                PaymentMode::Live,
                "sk_live_abc123",
                "pk_live_abc123",
                "whsec_abc123",
            )
            .await
            .unwrap();

        // Now switching should succeed
        let result = use_cases
            .set_active_mode(owner_id, domain.id, PaymentMode::Live)
            .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), PaymentMode::Live);
    }

    // ========================================================================
    // Subscription Management Tests
    // ========================================================================

    #[tokio::test]
    async fn test_grant_subscription_creates_new_subscription() {
        let (use_cases, domain_repo, plan_repo, subscription_repo, event_repo) =
            create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let end_user_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let plan = create_test_plan(domain.id, |p| {
            p.payment_mode = PaymentMode::Test;
        });
        plan_repo
            .plans
            .lock()
            .unwrap()
            .insert(plan.id, plan.clone());

        let result = use_cases
            .grant_subscription(owner_id, domain.id, end_user_id, plan.id, "cus_test123")
            .await;

        assert!(result.is_ok());
        let sub = result.unwrap();
        assert_eq!(sub.end_user_id, end_user_id);
        assert_eq!(sub.plan_id, plan.id);
        assert!(sub.manually_granted);
        assert_eq!(sub.status, SubscriptionStatus::Active);

        // Should be stored in repo
        let stored = subscription_repo.get_by_id(sub.id).await.unwrap();
        assert!(stored.is_some());

        // Should have logged an event
        let events = event_repo.get_all();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, "granted");
    }

    #[tokio::test]
    async fn test_grant_subscription_fails_for_wrong_mode_plan() {
        let (use_cases, domain_repo, plan_repo, _, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let end_user_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        // Plan is for live mode, but domain is in test mode
        let plan = create_test_plan(domain.id, |p| {
            p.payment_mode = PaymentMode::Live;
        });
        plan_repo
            .plans
            .lock()
            .unwrap()
            .insert(plan.id, plan.clone());

        let result = use_cases
            .grant_subscription(owner_id, domain.id, end_user_id, plan.id, "cus_test123")
            .await;

        assert!(matches!(result, Err(AppError::InvalidInput(_))));
    }

    #[tokio::test]
    async fn test_revoke_subscription_cancels_subscription() {
        let (use_cases, domain_repo, plan_repo, subscription_repo, event_repo) =
            create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let end_user_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let plan = create_test_plan(domain.id, |p| {
            p.payment_mode = PaymentMode::Test;
        });
        plan_repo
            .plans
            .lock()
            .unwrap()
            .insert(plan.id, plan.clone());

        let sub = create_test_subscription(domain.id, end_user_id, plan.id, |s| {
            s.status = SubscriptionStatus::Active;
            s.payment_mode = PaymentMode::Test;
        });
        subscription_repo
            .subscriptions
            .lock()
            .unwrap()
            .insert(sub.id, sub.clone());

        // revoke_subscription takes user_id, not subscription_id
        let result = use_cases
            .revoke_subscription(owner_id, domain.id, end_user_id)
            .await;

        assert!(result.is_ok());

        // Subscription should be canceled
        let stored = subscription_repo.get_by_id(sub.id).await.unwrap().unwrap();
        assert_eq!(stored.status, SubscriptionStatus::Canceled);

        // Should have logged an event
        let events = event_repo.get_all();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, "revoked");
    }

    #[tokio::test]
    async fn test_get_user_subscription_returns_subscription() {
        let (use_cases, domain_repo, plan_repo, subscription_repo, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let end_user_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let plan = create_test_plan(domain.id, |p| {
            p.payment_mode = PaymentMode::Test;
        });
        plan_repo
            .plans
            .lock()
            .unwrap()
            .insert(plan.id, plan.clone());

        let sub = create_test_subscription(domain.id, end_user_id, plan.id, |s| {
            s.status = SubscriptionStatus::Active;
            s.payment_mode = PaymentMode::Test;
        });
        subscription_repo
            .subscriptions
            .lock()
            .unwrap()
            .insert(sub.id, sub.clone());

        let result = use_cases
            .get_user_subscription(domain.id, end_user_id)
            .await
            .unwrap();

        assert!(result.is_some());
        let retrieved = result.unwrap();
        assert_eq!(retrieved.id, sub.id);
    }

    #[tokio::test]
    async fn test_list_subscribers_returns_all_subscriptions() {
        let (use_cases, domain_repo, plan_repo, subscription_repo, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let plan = create_test_plan(domain.id, |p| {
            p.payment_mode = PaymentMode::Test;
        });
        plan_repo
            .plans
            .lock()
            .unwrap()
            .insert(plan.id, plan.clone());

        // Create multiple subscriptions
        for i in 0..3 {
            let user_id = Uuid::new_v4();
            let sub = create_test_subscription(domain.id, user_id, plan.id, |s| {
                s.status = SubscriptionStatus::Active;
                s.payment_mode = PaymentMode::Test;
            });
            subscription_repo
                .subscriptions
                .lock()
                .unwrap()
                .insert(sub.id, sub);
            subscription_repo.set_user_email(user_id, &format!("user{}@test.com", i));
        }

        let subscribers = use_cases
            .list_subscribers(owner_id, domain.id)
            .await
            .unwrap();

        assert_eq!(subscribers.len(), 3);
    }

    // ========================================================================
    // Plan Update Tests
    // ========================================================================

    #[tokio::test]
    async fn test_update_plan_modifies_fields() {
        let (use_cases, domain_repo, plan_repo, _, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let plan = create_test_plan(domain.id, |p| {
            p.name = "Old Name".to_string();
            p.price_cents = 999;
            p.payment_mode = PaymentMode::Test;
        });
        plan_repo
            .plans
            .lock()
            .unwrap()
            .insert(plan.id, plan.clone());

        let input = UpdatePlanInput {
            name: Some("New Name".to_string()),
            price_cents: Some(1999),
            description: None,
            interval: None,
            interval_count: None,
            trial_days: None,
            features: None,
            is_public: None,
        };

        let updated = use_cases
            .update_plan(owner_id, domain.id, plan.id, input)
            .await
            .unwrap();

        assert_eq!(updated.name, "New Name");
        assert_eq!(updated.price_cents, 1999);
    }

    #[tokio::test]
    async fn test_archive_plan_marks_as_archived() {
        let (use_cases, domain_repo, plan_repo, _, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let plan = create_test_plan(domain.id, |p| {
            p.is_archived = false;
            p.payment_mode = PaymentMode::Test;
        });
        plan_repo
            .plans
            .lock()
            .unwrap()
            .insert(plan.id, plan.clone());

        use_cases
            .archive_plan(owner_id, domain.id, plan.id)
            .await
            .unwrap();

        let stored = plan_repo.get_by_id(plan.id).await.unwrap().unwrap();
        assert!(stored.is_archived);
        assert!(stored.archived_at.is_some());
    }

    #[tokio::test]
    async fn test_get_public_plans_excludes_private() {
        let (use_cases, domain_repo, plan_repo, _, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let public_plan = create_test_plan(domain.id, |p| {
            p.is_public = true;
            p.is_archived = false;
            p.payment_mode = PaymentMode::Test;
        });
        let private_plan = create_test_plan(domain.id, |p| {
            p.is_public = false;
            p.is_archived = false;
            p.payment_mode = PaymentMode::Test;
        });

        {
            let mut plans = plan_repo.plans.lock().unwrap();
            plans.insert(public_plan.id, public_plan.clone());
            plans.insert(private_plan.id, private_plan.clone());
        }

        let public_plans = use_cases.get_public_plans(domain.id).await.unwrap();

        assert_eq!(public_plans.len(), 1);
        assert_eq!(public_plans[0].id, public_plan.id);
    }

    // ========================================================================
    // Payment Tests
    // ========================================================================

    #[tokio::test]
    async fn test_get_payment_summary_calculates_correctly() {
        let (use_cases, domain_repo, _, _, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        // For this test we need to access the payment repo directly
        // since payments are typically created via webhooks
        let summary = use_cases
            .get_payment_summary(owner_id, domain.id, None, None)
            .await
            .unwrap();

        // With no payments, all should be zero
        assert_eq!(summary.total_revenue_cents, 0);
        assert_eq!(summary.payment_count, 0);
    }

    // ========================================================================
    // Analytics Tests
    // ========================================================================

    #[tokio::test]
    async fn test_get_analytics_includes_plan_distribution() {
        let (use_cases, domain_repo, plan_repo, subscription_repo, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let basic = create_test_plan(domain.id, |p| {
            p.code = "basic".to_string();
            p.name = "Basic".to_string();
            p.price_cents = 999;
            p.payment_mode = PaymentMode::Test;
        });
        let premium = create_test_plan(domain.id, |p| {
            p.code = "premium".to_string();
            p.name = "Premium".to_string();
            p.price_cents = 2999;
            p.payment_mode = PaymentMode::Test;
        });

        {
            let mut plans = plan_repo.plans.lock().unwrap();
            plans.insert(basic.id, basic.clone());
            plans.insert(premium.id, premium.clone());
        }

        // 2 basic subscribers, 1 premium
        for _ in 0..2 {
            let sub = create_test_subscription(domain.id, Uuid::new_v4(), basic.id, |s| {
                s.status = SubscriptionStatus::Active;
                s.payment_mode = PaymentMode::Test;
            });
            subscription_repo
                .subscriptions
                .lock()
                .unwrap()
                .insert(sub.id, sub);
        }
        let premium_sub = create_test_subscription(domain.id, Uuid::new_v4(), premium.id, |s| {
            s.status = SubscriptionStatus::Active;
            s.payment_mode = PaymentMode::Test;
        });
        subscription_repo
            .subscriptions
            .lock()
            .unwrap()
            .insert(premium_sub.id, premium_sub);

        let analytics = use_cases.get_analytics(owner_id, domain.id).await.unwrap();

        assert_eq!(analytics.active_subscribers, 3);
        assert_eq!(analytics.plan_distribution.len(), 2);

        // Find basic plan distribution
        let basic_dist = analytics
            .plan_distribution
            .iter()
            .find(|d| d.plan_name == "Basic")
            .unwrap();
        assert_eq!(basic_dist.subscriber_count, 2);
    }

    #[tokio::test]
    async fn test_get_analytics_counts_trialing_subscribers() {
        let (use_cases, domain_repo, plan_repo, subscription_repo, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let plan = create_test_plan(domain.id, |p| {
            p.payment_mode = PaymentMode::Test;
        });
        plan_repo
            .plans
            .lock()
            .unwrap()
            .insert(plan.id, plan.clone());

        // One active, one trialing
        let active_sub = create_test_subscription(domain.id, Uuid::new_v4(), plan.id, |s| {
            s.status = SubscriptionStatus::Active;
            s.payment_mode = PaymentMode::Test;
        });
        let trialing_sub = create_test_subscription(domain.id, Uuid::new_v4(), plan.id, |s| {
            s.status = SubscriptionStatus::Trialing;
            s.payment_mode = PaymentMode::Test;
        });

        {
            let mut subs = subscription_repo.subscriptions.lock().unwrap();
            subs.insert(active_sub.id, active_sub);
            subs.insert(trialing_sub.id, trialing_sub);
        }

        let analytics = use_cases.get_analytics(owner_id, domain.id).await.unwrap();

        assert_eq!(analytics.active_subscribers, 1);
        assert_eq!(analytics.trialing_subscribers, 1);
    }

    // ========================================================================
    // Webhook Event Idempotency Tests
    // ========================================================================

    #[tokio::test]
    async fn test_is_event_processed_returns_false_for_new_event() {
        let (use_cases, _, _, _, _) = create_test_use_cases();

        let result = use_cases
            .is_event_processed("evt_new_event_123")
            .await
            .unwrap();

        assert!(!result);
    }

    #[tokio::test]
    async fn test_is_event_processed_returns_true_for_existing_event() {
        let (use_cases, _, _, _, event_repo) = create_test_use_cases();

        // Manually add an event with a stripe_event_id
        let input = CreateSubscriptionEventInput {
            subscription_id: Uuid::new_v4(),
            event_type: "test".to_string(),
            previous_status: None,
            new_status: None,
            stripe_event_id: Some("evt_existing_123".to_string()),
            metadata: serde_json::json!({}),
            created_by: None,
        };
        event_repo.create(&input).await.unwrap();

        let result = use_cases
            .is_event_processed("evt_existing_123")
            .await
            .unwrap();

        assert!(result);
    }

    // ========================================================================
    // Payment Status Tests (Step 1)
    // ========================================================================

    /// Helper that also returns payment repo for payment-focused tests.
    fn create_test_use_cases_with_payments() -> (
        DomainBillingUseCases,
        Arc<InMemoryDomainRepo>,
        Arc<InMemorySubscriptionPlanRepo>,
        Arc<InMemoryUserSubscriptionRepo>,
        Arc<InMemorySubscriptionEventRepo>,
        Arc<InMemoryBillingPaymentRepo>,
    ) {
        let domain_repo = Arc::new(InMemoryDomainRepo::new());
        let stripe_config_repo = Arc::new(InMemoryBillingStripeConfigRepo::new());
        let enabled_providers_repo = Arc::new(InMemoryEnabledPaymentProvidersRepo::new());
        let plan_repo = Arc::new(InMemorySubscriptionPlanRepo::new());
        let subscription_repo =
            Arc::new(InMemoryUserSubscriptionRepo::new().with_plan_repo(plan_repo.clone()));
        let event_repo = Arc::new(InMemorySubscriptionEventRepo::new());
        let payment_repo = Arc::new(InMemoryBillingPaymentRepo::new());

        let cipher = ProcessCipher::new_from_base64(TEST_KEY_B64).unwrap();

        let factory = Arc::new(PaymentProviderFactory::new(
            cipher.clone(),
            stripe_config_repo.clone() as Arc<dyn BillingStripeConfigRepoTrait>,
        ));

        let use_cases = DomainBillingUseCases::new(
            domain_repo.clone() as Arc<dyn super::super::domain::DomainRepoTrait>,
            stripe_config_repo as Arc<dyn BillingStripeConfigRepoTrait>,
            enabled_providers_repo as Arc<dyn EnabledPaymentProvidersRepoTrait>,
            plan_repo.clone() as Arc<dyn SubscriptionPlanRepoTrait>,
            subscription_repo.clone() as Arc<dyn UserSubscriptionRepoTrait>,
            event_repo.clone() as Arc<dyn SubscriptionEventRepoTrait>,
            payment_repo.clone() as Arc<dyn BillingPaymentRepoTrait>,
            cipher,
            factory,
        );

        (
            use_cases,
            domain_repo,
            plan_repo,
            subscription_repo,
            event_repo,
            payment_repo,
        )
    }

    #[tokio::test]
    async fn test_create_dummy_payment_success() {
        let (use_cases, domain_repo, plan_repo, _, _, payment_repo) =
            create_test_use_cases_with_payments();

        let owner_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let subscription_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let plan = create_test_plan(domain.id, |p| {
            p.price_cents = 1999;
            p.currency = "eur".to_string();
            p.name = "Premium Plan".to_string();
            p.code = "premium".to_string();
            p.payment_mode = PaymentMode::Test;
        });
        plan_repo
            .plans
            .lock()
            .unwrap()
            .insert(plan.id, plan.clone());

        let payment = use_cases
            .create_dummy_payment(domain.id, user_id, subscription_id, &plan)
            .await
            .unwrap();

        assert_eq!(payment.domain_id, domain.id);
        assert_eq!(payment.end_user_id, user_id);
        assert_eq!(payment.subscription_id, Some(subscription_id));
        assert_eq!(payment.amount_cents, 1999);
        assert_eq!(payment.amount_paid_cents, 1999);
        assert_eq!(payment.currency, "EUR");
        assert_eq!(payment.status, PaymentStatus::Paid);
        assert_eq!(payment.plan_code, Some("premium".to_string()));
        assert_eq!(payment.plan_name, Some("Premium Plan".to_string()));
        assert!(payment.stripe_invoice_id.starts_with("dummy_inv_"));
        assert!(
            payment
                .stripe_payment_intent_id
                .unwrap()
                .starts_with("dummy_pi_")
        );

        let stored = payment_repo
            .get_by_stripe_invoice_id(&payment.stripe_invoice_id)
            .await
            .unwrap();
        assert!(stored.is_some());
    }

    #[tokio::test]
    async fn test_update_payment_status_to_refunded() {
        let (use_cases, _, _, _, _, payment_repo) = create_test_use_cases_with_payments();

        let domain_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let payment = create_test_payment(domain_id, user_id, |p| {
            p.status = PaymentStatus::Paid;
            p.amount_cents = 1000;
            p.amount_paid_cents = 1000;
        });
        let invoice_id = payment.stripe_invoice_id.clone();

        {
            let key = (domain_id, payment.payment_mode, invoice_id.clone());
            payment_repo.payments.lock().unwrap().insert(key, payment);
        }

        use_cases
            .update_payment_status(&invoice_id, PaymentStatus::Refunded, Some(1000), None)
            .await
            .unwrap();

        let updated = payment_repo
            .get_by_stripe_invoice_id(&invoice_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(updated.status, PaymentStatus::Refunded);
        assert_eq!(updated.amount_refunded_cents, 1000);
        assert!(updated.refunded_at.is_some());
    }

    #[tokio::test]
    async fn test_update_payment_status_partial_refund() {
        let (use_cases, _, _, _, _, payment_repo) = create_test_use_cases_with_payments();

        let domain_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let payment = create_test_payment(domain_id, user_id, |p| {
            p.status = PaymentStatus::Paid;
            p.amount_cents = 1000;
            p.amount_paid_cents = 1000;
        });
        let invoice_id = payment.stripe_invoice_id.clone();

        {
            let key = (domain_id, payment.payment_mode, invoice_id.clone());
            payment_repo.payments.lock().unwrap().insert(key, payment);
        }

        use_cases
            .update_payment_status(&invoice_id, PaymentStatus::PartialRefund, Some(500), None)
            .await
            .unwrap();

        let updated = payment_repo
            .get_by_stripe_invoice_id(&invoice_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(updated.status, PaymentStatus::PartialRefund);
        assert_eq!(updated.amount_refunded_cents, 500);
    }

    #[tokio::test]
    async fn test_update_payment_status_terminal_state_preserved() {
        let (use_cases, _, _, _, _, payment_repo) = create_test_use_cases_with_payments();

        let domain_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let payment = create_test_payment(domain_id, user_id, |p| {
            p.status = PaymentStatus::Refunded;
            p.amount_refunded_cents = 1000;
        });
        let invoice_id = payment.stripe_invoice_id.clone();

        {
            let key = (domain_id, payment.payment_mode, invoice_id.clone());
            payment_repo.payments.lock().unwrap().insert(key, payment);
        }

        use_cases
            .update_payment_status(&invoice_id, PaymentStatus::Paid, None, None)
            .await
            .unwrap();

        let stored = payment_repo
            .get_by_stripe_invoice_id(&invoice_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(stored.status, PaymentStatus::Refunded);
    }

    #[tokio::test]
    async fn test_update_payment_status_with_failure_message() {
        let (use_cases, _, _, _, _, payment_repo) = create_test_use_cases_with_payments();

        let domain_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let payment = create_test_payment(domain_id, user_id, |p| {
            p.status = PaymentStatus::Pending;
        });
        let invoice_id = payment.stripe_invoice_id.clone();

        {
            let key = (domain_id, payment.payment_mode, invoice_id.clone());
            payment_repo.payments.lock().unwrap().insert(key, payment);
        }

        use_cases
            .update_payment_status(
                &invoice_id,
                PaymentStatus::Failed,
                None,
                Some("Card declined".to_string()),
            )
            .await
            .unwrap();

        let updated = payment_repo
            .get_by_stripe_invoice_id(&invoice_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(updated.status, PaymentStatus::Failed);
        assert_eq!(updated.failure_message, Some("Card declined".to_string()));
    }

    #[tokio::test]
    async fn test_update_payment_status_missing_invoice_ok() {
        let (use_cases, _, _, _, _, _) = create_test_use_cases_with_payments();

        let result = use_cases
            .update_payment_status("nonexistent_invoice", PaymentStatus::Refunded, None, None)
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_get_user_payments_returns_user_payments() {
        let (use_cases, domain_repo, _, _, _, payment_repo) = create_test_use_cases_with_payments();

        let owner_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        for i in 0..3 {
            let payment = create_test_payment(domain.id, user_id, |p| {
                p.stripe_invoice_id = format!("inv_user_{}", i);
                p.payment_mode = PaymentMode::Test;
            });
            let key = (
                domain.id,
                PaymentMode::Test,
                payment.stripe_invoice_id.clone(),
            );
            payment_repo.payments.lock().unwrap().insert(key, payment);
        }

        payment_repo.set_user_email(user_id, "user@test.com");

        let result = use_cases
            .get_user_payments(domain.id, user_id, 1, 10)
            .await
            .unwrap();

        assert_eq!(result.payments.len(), 3);
        assert_eq!(result.total, 3);
        assert!(
            result
                .payments
                .iter()
                .all(|p| p.user_email == "user@test.com")
        );
    }

    #[tokio::test]
    async fn test_get_user_payments_pagination() {
        let (use_cases, domain_repo, _, _, _, payment_repo) = create_test_use_cases_with_payments();

        let owner_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        for i in 0..5 {
            let payment = create_test_payment(domain.id, user_id, |p| {
                p.stripe_invoice_id = format!("inv_page_{}", i);
                p.payment_mode = PaymentMode::Test;
            });
            let key = (
                domain.id,
                PaymentMode::Test,
                payment.stripe_invoice_id.clone(),
            );
            payment_repo.payments.lock().unwrap().insert(key, payment);
        }

        let page1 = use_cases
            .get_user_payments(domain.id, user_id, 1, 2)
            .await
            .unwrap();
        let page2 = use_cases
            .get_user_payments(domain.id, user_id, 2, 2)
            .await
            .unwrap();

        assert_eq!(page1.payments.len(), 2);
        assert_eq!(page1.page, 1);
        assert_eq!(page1.total, 5);
        assert_eq!(page1.total_pages, 3);

        assert_eq!(page2.payments.len(), 2);
        assert_eq!(page2.page, 2);
    }

    #[tokio::test]
    async fn test_get_user_payments_empty_list() {
        let (use_cases, domain_repo, _, _, _, _) = create_test_use_cases_with_payments();

        let owner_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let result = use_cases
            .get_user_payments(domain.id, user_id, 1, 10)
            .await
            .unwrap();

        assert_eq!(result.payments.len(), 0);
        assert_eq!(result.total, 0);
    }

    #[tokio::test]
    async fn test_get_user_payments_respects_mode() {
        let (use_cases, domain_repo, _, _, _, payment_repo) = create_test_use_cases_with_payments();

        let owner_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let test_payment = create_test_payment(domain.id, user_id, |p| {
            p.stripe_invoice_id = "inv_test_mode".to_string();
            p.payment_mode = PaymentMode::Test;
        });
        let live_payment = create_test_payment(domain.id, user_id, |p| {
            p.stripe_invoice_id = "inv_live_mode".to_string();
            p.payment_mode = PaymentMode::Live;
        });

        {
            let mut payments = payment_repo.payments.lock().unwrap();
            payments.insert(
                (domain.id, PaymentMode::Test, "inv_test_mode".to_string()),
                test_payment,
            );
            payments.insert(
                (domain.id, PaymentMode::Live, "inv_live_mode".to_string()),
                live_payment,
            );
        }

        let result = use_cases
            .get_user_payments(domain.id, user_id, 1, 10)
            .await
            .unwrap();

        assert_eq!(result.payments.len(), 1);
        assert_eq!(
            result.payments[0].payment.stripe_invoice_id,
            "inv_test_mode"
        );
    }

    #[tokio::test]
    async fn test_list_domain_payments_all() {
        let (use_cases, domain_repo, _, _, _, payment_repo) = create_test_use_cases_with_payments();

        let owner_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        for i in 0..3 {
            let user_id = Uuid::new_v4();
            let payment = create_test_payment(domain.id, user_id, |p| {
                p.stripe_invoice_id = format!("inv_domain_{}", i);
                p.payment_mode = PaymentMode::Test;
            });
            payment_repo.set_user_email(user_id, &format!("user{}@test.com", i));
            let key = (
                domain.id,
                PaymentMode::Test,
                payment.stripe_invoice_id.clone(),
            );
            payment_repo.payments.lock().unwrap().insert(key, payment);
        }

        let filters = PaymentListFilters {
            status: None,
            date_from: None,
            date_to: None,
            plan_code: None,
            user_email: None,
        };

        let result = use_cases
            .list_domain_payments(owner_id, domain.id, &filters, 1, 10)
            .await
            .unwrap();

        assert_eq!(result.payments.len(), 3);
        assert_eq!(result.total, 3);
    }

    #[tokio::test]
    async fn test_list_domain_payments_filter_by_status() {
        let (use_cases, domain_repo, _, _, _, payment_repo) = create_test_use_cases_with_payments();

        let owner_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let paid = create_test_payment(domain.id, user_id, |p| {
            p.stripe_invoice_id = "inv_paid".to_string();
            p.status = PaymentStatus::Paid;
            p.payment_mode = PaymentMode::Test;
        });
        let failed = create_test_payment(domain.id, user_id, |p| {
            p.stripe_invoice_id = "inv_failed".to_string();
            p.status = PaymentStatus::Failed;
            p.payment_mode = PaymentMode::Test;
        });

        {
            let mut payments = payment_repo.payments.lock().unwrap();
            payments.insert((domain.id, PaymentMode::Test, "inv_paid".to_string()), paid);
            payments.insert(
                (domain.id, PaymentMode::Test, "inv_failed".to_string()),
                failed,
            );
        }

        let filters = PaymentListFilters {
            status: Some(PaymentStatus::Paid),
            date_from: None,
            date_to: None,
            plan_code: None,
            user_email: None,
        };

        let result = use_cases
            .list_domain_payments(owner_id, domain.id, &filters, 1, 10)
            .await
            .unwrap();

        assert_eq!(result.payments.len(), 1);
        assert_eq!(result.payments[0].payment.status, PaymentStatus::Paid);
    }

    #[tokio::test]
    async fn test_list_domain_payments_filter_by_plan_code() {
        let (use_cases, domain_repo, _, _, _, payment_repo) = create_test_use_cases_with_payments();

        let owner_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let basic = create_test_payment(domain.id, user_id, |p| {
            p.stripe_invoice_id = "inv_basic".to_string();
            p.plan_code = Some("basic".to_string());
            p.payment_mode = PaymentMode::Test;
        });
        let premium = create_test_payment(domain.id, user_id, |p| {
            p.stripe_invoice_id = "inv_premium".to_string();
            p.plan_code = Some("premium".to_string());
            p.payment_mode = PaymentMode::Test;
        });

        {
            let mut payments = payment_repo.payments.lock().unwrap();
            payments.insert(
                (domain.id, PaymentMode::Test, "inv_basic".to_string()),
                basic,
            );
            payments.insert(
                (domain.id, PaymentMode::Test, "inv_premium".to_string()),
                premium,
            );
        }

        let filters = PaymentListFilters {
            status: None,
            date_from: None,
            date_to: None,
            plan_code: Some("premium".to_string()),
            user_email: None,
        };

        let result = use_cases
            .list_domain_payments(owner_id, domain.id, &filters, 1, 10)
            .await
            .unwrap();

        assert_eq!(result.payments.len(), 1);
        assert_eq!(
            result.payments[0].payment.plan_code,
            Some("premium".to_string())
        );
    }

    #[tokio::test]
    async fn test_list_domain_payments_filter_by_user_email() {
        let (use_cases, domain_repo, _, _, _, payment_repo) = create_test_use_cases_with_payments();

        let owner_id = Uuid::new_v4();
        let user1 = Uuid::new_v4();
        let user2 = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let p1 = create_test_payment(domain.id, user1, |p| {
            p.stripe_invoice_id = "inv_alice".to_string();
            p.payment_mode = PaymentMode::Test;
        });
        let p2 = create_test_payment(domain.id, user2, |p| {
            p.stripe_invoice_id = "inv_bob".to_string();
            p.payment_mode = PaymentMode::Test;
        });

        payment_repo.set_user_email(user1, "alice@example.com");
        payment_repo.set_user_email(user2, "bob@example.com");

        {
            let mut payments = payment_repo.payments.lock().unwrap();
            payments.insert((domain.id, PaymentMode::Test, "inv_alice".to_string()), p1);
            payments.insert((domain.id, PaymentMode::Test, "inv_bob".to_string()), p2);
        }

        let filters = PaymentListFilters {
            status: None,
            date_from: None,
            date_to: None,
            plan_code: None,
            user_email: Some("alice".to_string()),
        };

        let result = use_cases
            .list_domain_payments(owner_id, domain.id, &filters, 1, 10)
            .await
            .unwrap();

        assert_eq!(result.payments.len(), 1);
        assert_eq!(result.payments[0].user_email, "alice@example.com");
    }

    #[tokio::test]
    async fn test_list_domain_payments_forbidden_non_owner() {
        let (use_cases, domain_repo, _, _, _, _) = create_test_use_cases_with_payments();

        let owner_id = Uuid::new_v4();
        let other_user = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let filters = PaymentListFilters {
            status: None,
            date_from: None,
            date_to: None,
            plan_code: None,
            user_email: None,
        };

        let result = use_cases
            .list_domain_payments(other_user, domain.id, &filters, 1, 10)
            .await;

        assert!(matches!(result, Err(AppError::Forbidden)));
    }

    #[tokio::test]
    async fn test_export_payments_csv_format() {
        let (use_cases, domain_repo, _, _, _, payment_repo) = create_test_use_cases_with_payments();

        let owner_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let payment = create_test_payment(domain.id, user_id, |p| {
            p.stripe_invoice_id = "inv_csv_test".to_string();
            p.plan_name = Some("Basic Plan".to_string());
            p.amount_cents = 999;
            p.status = PaymentStatus::Paid;
            p.invoice_number = Some("INV-001".to_string());
            p.billing_reason = Some("subscription_create".to_string());
            p.payment_mode = PaymentMode::Test;
        });

        payment_repo.set_user_email(user_id, "test@example.com");

        {
            let key = (domain.id, PaymentMode::Test, "inv_csv_test".to_string());
            payment_repo.payments.lock().unwrap().insert(key, payment);
        }

        let filters = PaymentListFilters {
            status: None,
            date_from: None,
            date_to: None,
            plan_code: None,
            user_email: None,
        };

        let csv = use_cases
            .export_payments_csv(owner_id, domain.id, &filters)
            .await
            .unwrap();

        assert!(
            csv.starts_with("Date,User Email,Plan,Amount,Status,Invoice Number,Billing Reason\n")
        );
        assert!(csv.contains("test@example.com"));
        assert!(csv.contains("Basic Plan"));
        assert!(csv.contains("9.99"));
        assert!(csv.contains("paid"));
        assert!(csv.contains("INV-001"));
    }

    #[tokio::test]
    async fn test_export_payments_csv_escapes_commas() {
        let (use_cases, domain_repo, _, _, _, payment_repo) = create_test_use_cases_with_payments();

        let owner_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let payment = create_test_payment(domain.id, user_id, |p| {
            p.stripe_invoice_id = "inv_comma".to_string();
            p.plan_name = Some("Pro Plan, Annual".to_string());
            p.payment_mode = PaymentMode::Test;
        });

        payment_repo.set_user_email(user_id, "user@test.com");

        {
            let key = (domain.id, PaymentMode::Test, "inv_comma".to_string());
            payment_repo.payments.lock().unwrap().insert(key, payment);
        }

        let filters = PaymentListFilters::default();

        let csv = use_cases
            .export_payments_csv(owner_id, domain.id, &filters)
            .await
            .unwrap();

        assert!(csv.contains("\"Pro Plan, Annual\""));
    }

    #[tokio::test]
    async fn test_export_payments_csv_escapes_quotes() {
        let (use_cases, domain_repo, _, _, _, payment_repo) = create_test_use_cases_with_payments();

        let owner_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let payment = create_test_payment(domain.id, user_id, |p| {
            p.stripe_invoice_id = "inv_quote".to_string();
            p.plan_name = Some("The \"Best\" Plan".to_string());
            p.payment_mode = PaymentMode::Test;
        });

        payment_repo.set_user_email(user_id, "user@test.com");

        {
            let key = (domain.id, PaymentMode::Test, "inv_quote".to_string());
            payment_repo.payments.lock().unwrap().insert(key, payment);
        }

        let filters = PaymentListFilters::default();

        let csv = use_cases
            .export_payments_csv(owner_id, domain.id, &filters)
            .await
            .unwrap();

        assert!(csv.contains("\"The \"\"Best\"\" Plan\""));
    }

    #[tokio::test]
    async fn test_export_payments_csv_prevents_formula_injection() {
        let (use_cases, domain_repo, _, _, _, payment_repo) = create_test_use_cases_with_payments();

        let owner_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let payment = create_test_payment(domain.id, user_id, |p| {
            p.stripe_invoice_id = "inv_formula".to_string();
            p.plan_name = Some("=SUM(A1:A10)".to_string());
            p.payment_mode = PaymentMode::Test;
        });

        payment_repo.set_user_email(user_id, "user@test.com");

        {
            let key = (domain.id, PaymentMode::Test, "inv_formula".to_string());
            payment_repo.payments.lock().unwrap().insert(key, payment);
        }

        let filters = PaymentListFilters::default();

        let csv = use_cases
            .export_payments_csv(owner_id, domain.id, &filters)
            .await
            .unwrap();

        assert!(csv.contains("\"'=SUM(A1:A10)\""));
    }

    #[tokio::test]
    async fn test_export_payments_csv_handles_null_fields() {
        let (use_cases, domain_repo, _, _, _, payment_repo) = create_test_use_cases_with_payments();

        let owner_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let payment = create_test_payment(domain.id, user_id, |p| {
            p.stripe_invoice_id = "inv_nulls".to_string();
            p.plan_name = None;
            p.invoice_number = None;
            p.billing_reason = None;
            p.payment_mode = PaymentMode::Test;
        });

        payment_repo.set_user_email(user_id, "user@test.com");

        {
            let key = (domain.id, PaymentMode::Test, "inv_nulls".to_string());
            payment_repo.payments.lock().unwrap().insert(key, payment);
        }

        let filters = PaymentListFilters::default();

        let result = use_cases
            .export_payments_csv(owner_id, domain.id, &filters)
            .await;

        assert!(result.is_ok());
        let csv = result.unwrap();
        assert!(csv.lines().count() >= 2);
    }

    #[tokio::test]
    async fn test_export_payments_csv_forbidden_non_owner() {
        let (use_cases, domain_repo, _, _, _, _) = create_test_use_cases_with_payments();

        let owner_id = Uuid::new_v4();
        let other_user = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let filters = PaymentListFilters::default();

        let result = use_cases
            .export_payments_csv(other_user, domain.id, &filters)
            .await;

        assert!(matches!(result, Err(AppError::Forbidden)));
    }

    // ========================================================================
    // Stripe Config Tests (Step 2)
    // ========================================================================

    #[tokio::test]
    async fn test_delete_stripe_config_inactive_mode_success() {
        let (use_cases, domain_repo, _, _, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        use_cases
            .update_stripe_config(
                owner_id,
                domain.id,
                PaymentMode::Live,
                "sk_live_abc123",
                "pk_live_abc123",
                "whsec_abc123",
            )
            .await
            .unwrap();

        let result = use_cases
            .delete_stripe_config(owner_id, domain.id, PaymentMode::Live)
            .await;

        assert!(result.is_ok());

        let is_configured = use_cases
            .is_stripe_configured_for_mode(domain.id, PaymentMode::Live)
            .await
            .unwrap();
        assert!(!is_configured);
    }

    #[tokio::test]
    async fn test_delete_stripe_config_active_mode_no_data_success() {
        let (use_cases, domain_repo, _, _, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        use_cases
            .update_stripe_config(
                owner_id,
                domain.id,
                PaymentMode::Test,
                "sk_test_abc123",
                "pk_test_abc123",
                "whsec_abc123",
            )
            .await
            .unwrap();

        let result = use_cases
            .delete_stripe_config(owner_id, domain.id, PaymentMode::Test)
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_delete_stripe_config_active_mode_with_plans_fails() {
        let (use_cases, domain_repo, plan_repo, _, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        use_cases
            .update_stripe_config(
                owner_id,
                domain.id,
                PaymentMode::Test,
                "sk_test_abc123",
                "pk_test_abc123",
                "whsec_abc123",
            )
            .await
            .unwrap();

        let plan = create_test_plan(domain.id, |p| {
            p.payment_mode = PaymentMode::Test;
        });
        plan_repo.plans.lock().unwrap().insert(plan.id, plan);

        let result = use_cases
            .delete_stripe_config(owner_id, domain.id, PaymentMode::Test)
            .await;

        assert!(matches!(result, Err(AppError::InvalidInput(_))));
    }

    #[tokio::test]
    async fn test_delete_stripe_config_active_mode_with_subscriptions_fails() {
        let (use_cases, domain_repo, plan_repo, subscription_repo, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        use_cases
            .update_stripe_config(
                owner_id,
                domain.id,
                PaymentMode::Test,
                "sk_test_abc123",
                "pk_test_abc123",
                "whsec_abc123",
            )
            .await
            .unwrap();

        let plan = create_test_plan(domain.id, |p| {
            p.payment_mode = PaymentMode::Test;
        });
        plan_repo
            .plans
            .lock()
            .unwrap()
            .insert(plan.id, plan.clone());

        let sub = create_test_subscription(domain.id, Uuid::new_v4(), plan.id, |s| {
            s.payment_mode = PaymentMode::Test;
        });
        subscription_repo
            .subscriptions
            .lock()
            .unwrap()
            .insert(sub.id, sub);

        let result = use_cases
            .delete_stripe_config(owner_id, domain.id, PaymentMode::Test)
            .await;

        assert!(matches!(result, Err(AppError::InvalidInput(_))));
    }

    #[tokio::test]
    async fn test_delete_stripe_config_forbidden_non_owner() {
        let (use_cases, domain_repo, _, _, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let other_user = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let result = use_cases
            .delete_stripe_config(other_user, domain.id, PaymentMode::Test)
            .await;

        assert!(matches!(result, Err(AppError::Forbidden)));
    }

    #[tokio::test]
    async fn test_get_stripe_secret_key_success() {
        let (use_cases, domain_repo, _, _, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        use_cases
            .update_stripe_config(
                owner_id,
                domain.id,
                PaymentMode::Test,
                "sk_test_secret123",
                "pk_test_abc123",
                "whsec_abc123",
            )
            .await
            .unwrap();

        let key = use_cases.get_stripe_secret_key(domain.id).await.unwrap();
        assert_eq!(key, "sk_test_secret123");
    }

    #[tokio::test]
    async fn test_get_stripe_secret_key_not_configured_fails() {
        let (use_cases, domain_repo, _, _, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let result = use_cases.get_stripe_secret_key(domain.id).await;

        assert!(matches!(result, Err(AppError::InvalidInput(_))));
    }

    #[tokio::test]
    async fn test_get_stripe_secret_key_for_mode_success() {
        let (use_cases, domain_repo, _, _, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        use_cases
            .update_stripe_config(
                owner_id,
                domain.id,
                PaymentMode::Live,
                "sk_live_modesecret",
                "pk_live_abc123",
                "whsec_abc123",
            )
            .await
            .unwrap();

        let key = use_cases
            .get_stripe_secret_key_for_mode(domain.id, PaymentMode::Live)
            .await
            .unwrap();
        assert_eq!(key, "sk_live_modesecret");
    }

    #[tokio::test]
    async fn test_get_stripe_secret_key_for_mode_not_configured_fails() {
        let (use_cases, domain_repo, _, _, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let result = use_cases
            .get_stripe_secret_key_for_mode(domain.id, PaymentMode::Live)
            .await;

        assert!(matches!(result, Err(AppError::InvalidInput(_))));
    }

    #[tokio::test]
    async fn test_get_stripe_webhook_secret_for_mode_success() {
        let (use_cases, domain_repo, _, _, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        use_cases
            .update_stripe_config(
                owner_id,
                domain.id,
                PaymentMode::Test,
                "sk_test_abc123",
                "pk_test_abc123",
                "whsec_webhooksecret",
            )
            .await
            .unwrap();

        let secret = use_cases
            .get_stripe_webhook_secret_for_mode(domain.id, PaymentMode::Test)
            .await
            .unwrap();
        assert_eq!(secret, "whsec_webhooksecret");
    }

    #[tokio::test]
    async fn test_get_stripe_webhook_secret_for_mode_not_configured_fails() {
        let (use_cases, domain_repo, _, _, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let result = use_cases
            .get_stripe_webhook_secret_for_mode(domain.id, PaymentMode::Live)
            .await;

        assert!(matches!(result, Err(AppError::InvalidInput(_))));
    }

    #[tokio::test]
    async fn test_is_stripe_configured_true() {
        let (use_cases, domain_repo, _, _, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        use_cases
            .update_stripe_config(
                owner_id,
                domain.id,
                PaymentMode::Test,
                "sk_test_abc123",
                "pk_test_abc123",
                "whsec_abc123",
            )
            .await
            .unwrap();

        let result = use_cases.is_stripe_configured(domain.id).await.unwrap();
        assert!(result);
    }

    #[tokio::test]
    async fn test_is_stripe_configured_false() {
        let (use_cases, domain_repo, _, _, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let result = use_cases.is_stripe_configured(domain.id).await.unwrap();
        assert!(!result);
    }

    #[tokio::test]
    async fn test_is_stripe_configured_for_mode_true() {
        let (use_cases, domain_repo, _, _, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        use_cases
            .update_stripe_config(
                owner_id,
                domain.id,
                PaymentMode::Live,
                "sk_live_abc123",
                "pk_live_abc123",
                "whsec_abc123",
            )
            .await
            .unwrap();

        let live_configured = use_cases
            .is_stripe_configured_for_mode(domain.id, PaymentMode::Live)
            .await
            .unwrap();
        let test_configured = use_cases
            .is_stripe_configured_for_mode(domain.id, PaymentMode::Test)
            .await
            .unwrap();

        assert!(live_configured);
        assert!(!test_configured);
    }

    #[tokio::test]
    async fn test_is_stripe_configured_for_mode_false() {
        let (use_cases, domain_repo, _, _, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let result = use_cases
            .is_stripe_configured_for_mode(domain.id, PaymentMode::Test)
            .await
            .unwrap();
        assert!(!result);
    }

    // ========================================================================
    // Provider Management Tests (Step 3)
    // ========================================================================

    #[tokio::test]
    async fn test_disable_provider_success_multiple_active() {
        let (use_cases, domain_repo, _, _, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        use_cases
            .update_stripe_config(
                owner_id,
                domain.id,
                PaymentMode::Test,
                "sk_test_abc123",
                "pk_test_abc123",
                "whsec_abc123",
            )
            .await
            .unwrap();

        use_cases
            .enable_provider(
                owner_id,
                domain.id,
                PaymentProvider::Dummy,
                PaymentMode::Test,
            )
            .await
            .unwrap();
        use_cases
            .enable_provider(
                owner_id,
                domain.id,
                PaymentProvider::Stripe,
                PaymentMode::Test,
            )
            .await
            .unwrap();

        let result = use_cases
            .disable_provider(
                owner_id,
                domain.id,
                PaymentProvider::Dummy,
                PaymentMode::Test,
            )
            .await;

        assert!(result.is_ok());

        let is_enabled = use_cases
            .is_provider_enabled(domain.id, PaymentProvider::Dummy, PaymentMode::Test)
            .await
            .unwrap();
        assert!(!is_enabled);
    }

    #[tokio::test]
    async fn test_disable_provider_last_active_fails() {
        let (use_cases, domain_repo, _, _, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        use_cases
            .enable_provider(
                owner_id,
                domain.id,
                PaymentProvider::Dummy,
                PaymentMode::Test,
            )
            .await
            .unwrap();

        let result = use_cases
            .disable_provider(
                owner_id,
                domain.id,
                PaymentProvider::Dummy,
                PaymentMode::Test,
            )
            .await;

        assert!(matches!(result, Err(AppError::InvalidInput(_))));
    }

    #[tokio::test]
    async fn test_disable_provider_forbidden_non_owner() {
        let (use_cases, domain_repo, _, _, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let other_user = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let result = use_cases
            .disable_provider(
                other_user,
                domain.id,
                PaymentProvider::Dummy,
                PaymentMode::Test,
            )
            .await;

        assert!(matches!(result, Err(AppError::Forbidden)));
    }

    #[tokio::test]
    async fn test_set_provider_active_enable() {
        let (use_cases, domain_repo, _, _, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        use_cases
            .enable_provider(
                owner_id,
                domain.id,
                PaymentProvider::Dummy,
                PaymentMode::Test,
            )
            .await
            .unwrap();

        let result = use_cases
            .set_provider_active(
                owner_id,
                domain.id,
                PaymentProvider::Dummy,
                PaymentMode::Test,
                true,
            )
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_set_provider_active_disable_not_last() {
        let (use_cases, domain_repo, _, _, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        // Add Stripe config so we can enable Stripe provider
        use_cases
            .update_stripe_config(
                owner_id,
                domain.id,
                PaymentMode::Test,
                "sk_test_abc123",
                "pk_test_abc123",
                "whsec_abc123",
            )
            .await
            .unwrap();

        // Enable both Dummy and Stripe in Test mode
        use_cases
            .enable_provider(
                owner_id,
                domain.id,
                PaymentProvider::Dummy,
                PaymentMode::Test,
            )
            .await
            .unwrap();
        use_cases
            .enable_provider(
                owner_id,
                domain.id,
                PaymentProvider::Stripe,
                PaymentMode::Test,
            )
            .await
            .unwrap();

        // Disable Dummy - should succeed since Stripe is still active
        let result = use_cases
            .set_provider_active(
                owner_id,
                domain.id,
                PaymentProvider::Dummy,
                PaymentMode::Test,
                false,
            )
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_set_provider_active_disable_last_fails() {
        let (use_cases, domain_repo, _, _, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        use_cases
            .enable_provider(
                owner_id,
                domain.id,
                PaymentProvider::Dummy,
                PaymentMode::Test,
            )
            .await
            .unwrap();

        let result = use_cases
            .set_provider_active(
                owner_id,
                domain.id,
                PaymentProvider::Dummy,
                PaymentMode::Test,
                false,
            )
            .await;

        assert!(matches!(result, Err(AppError::InvalidInput(_))));
    }

    #[tokio::test]
    async fn test_list_enabled_providers_returns_all() {
        let (use_cases, domain_repo, _, _, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        // Add Stripe config so we can enable Stripe provider
        use_cases
            .update_stripe_config(
                owner_id,
                domain.id,
                PaymentMode::Test,
                "sk_test_abc123",
                "pk_test_abc123",
                "whsec_abc123",
            )
            .await
            .unwrap();

        // Enable both Dummy and Stripe in Test mode
        use_cases
            .enable_provider(
                owner_id,
                domain.id,
                PaymentProvider::Dummy,
                PaymentMode::Test,
            )
            .await
            .unwrap();
        use_cases
            .enable_provider(
                owner_id,
                domain.id,
                PaymentProvider::Stripe,
                PaymentMode::Test,
            )
            .await
            .unwrap();

        let providers = use_cases
            .list_enabled_providers(owner_id, domain.id)
            .await
            .unwrap();

        assert_eq!(providers.len(), 2);
    }

    #[tokio::test]
    async fn test_list_enabled_providers_empty() {
        let (use_cases, domain_repo, _, _, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let providers = use_cases
            .list_enabled_providers(owner_id, domain.id)
            .await
            .unwrap();

        assert!(providers.is_empty());
    }

    #[tokio::test]
    async fn test_list_active_providers_filters_inactive() {
        let (use_cases, domain_repo, _, _, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        // Add Stripe config so we can enable Stripe provider
        use_cases
            .update_stripe_config(
                owner_id,
                domain.id,
                PaymentMode::Test,
                "sk_test_abc123",
                "pk_test_abc123",
                "whsec_abc123",
            )
            .await
            .unwrap();

        // Enable both Dummy and Stripe in Test mode
        use_cases
            .enable_provider(
                owner_id,
                domain.id,
                PaymentProvider::Dummy,
                PaymentMode::Test,
            )
            .await
            .unwrap();
        use_cases
            .enable_provider(
                owner_id,
                domain.id,
                PaymentProvider::Stripe,
                PaymentMode::Test,
            )
            .await
            .unwrap();

        // Disable Dummy - Stripe is still active
        use_cases
            .set_provider_active(
                owner_id,
                domain.id,
                PaymentProvider::Dummy,
                PaymentMode::Test,
                false,
            )
            .await
            .unwrap();

        let active_providers = use_cases.list_active_providers(domain.id).await.unwrap();

        assert_eq!(active_providers.len(), 1);
        assert_eq!(active_providers[0].provider, PaymentProvider::Stripe);
    }

    #[tokio::test]
    async fn test_set_provider_display_order_success() {
        let (use_cases, domain_repo, _, _, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        use_cases
            .enable_provider(
                owner_id,
                domain.id,
                PaymentProvider::Dummy,
                PaymentMode::Test,
            )
            .await
            .unwrap();

        let result = use_cases
            .set_provider_display_order(
                owner_id,
                domain.id,
                PaymentProvider::Dummy,
                PaymentMode::Test,
                5,
            )
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_is_provider_enabled_true() {
        let (use_cases, domain_repo, _, _, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        use_cases
            .enable_provider(
                owner_id,
                domain.id,
                PaymentProvider::Dummy,
                PaymentMode::Test,
            )
            .await
            .unwrap();

        let result = use_cases
            .is_provider_enabled(domain.id, PaymentProvider::Dummy, PaymentMode::Test)
            .await
            .unwrap();

        assert!(result);
    }

    #[tokio::test]
    async fn test_is_provider_enabled_false() {
        let (use_cases, domain_repo, _, _, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let result = use_cases
            .is_provider_enabled(domain.id, PaymentProvider::Dummy, PaymentMode::Test)
            .await
            .unwrap();

        assert!(!result);
    }

    // ========================================================================
    // Step 4: Plan Management Tests
    // ========================================================================

    #[tokio::test]
    async fn test_list_plans_for_mode_returns_test_plans() {
        let (use_cases, domain_repo, plan_repo, _, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let test_plan = create_test_plan(domain.id, |p| {
            p.payment_mode = PaymentMode::Test;
            p.code = "test_plan".to_string();
        });
        let live_plan = create_test_plan(domain.id, |p| {
            p.payment_mode = PaymentMode::Live;
            p.code = "live_plan".to_string();
        });
        plan_repo
            .plans
            .lock()
            .unwrap()
            .insert(test_plan.id, test_plan.clone());
        plan_repo
            .plans
            .lock()
            .unwrap()
            .insert(live_plan.id, live_plan.clone());

        let plans = use_cases
            .list_plans_for_mode(owner_id, domain.id, PaymentMode::Test, false)
            .await
            .unwrap();

        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].code, "test_plan");
    }

    #[tokio::test]
    async fn test_list_plans_for_mode_returns_live_plans() {
        let (use_cases, domain_repo, plan_repo, _, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Live;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let test_plan = create_test_plan(domain.id, |p| {
            p.payment_mode = PaymentMode::Test;
            p.code = "test_plan".to_string();
        });
        let live_plan = create_test_plan(domain.id, |p| {
            p.payment_mode = PaymentMode::Live;
            p.code = "live_plan".to_string();
        });
        plan_repo
            .plans
            .lock()
            .unwrap()
            .insert(test_plan.id, test_plan.clone());
        plan_repo
            .plans
            .lock()
            .unwrap()
            .insert(live_plan.id, live_plan.clone());

        let plans = use_cases
            .list_plans_for_mode(owner_id, domain.id, PaymentMode::Live, false)
            .await
            .unwrap();

        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].code, "live_plan");
    }

    #[tokio::test]
    async fn test_reorder_plans_success() {
        let (use_cases, domain_repo, plan_repo, _, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let plan_a = create_test_plan(domain.id, |p| {
            p.code = "plan_a".to_string();
            p.display_order = 0;
        });
        let plan_b = create_test_plan(domain.id, |p| {
            p.code = "plan_b".to_string();
            p.display_order = 1;
        });
        plan_repo
            .plans
            .lock()
            .unwrap()
            .insert(plan_a.id, plan_a.clone());
        plan_repo
            .plans
            .lock()
            .unwrap()
            .insert(plan_b.id, plan_b.clone());

        // Reverse the order
        let result = use_cases
            .reorder_plans(owner_id, domain.id, vec![plan_b.id, plan_a.id])
            .await;

        assert!(result.is_ok());

        let plans = plan_repo.plans.lock().unwrap();
        assert_eq!(plans.get(&plan_b.id).unwrap().display_order, 0);
        assert_eq!(plans.get(&plan_a.id).unwrap().display_order, 1);
    }

    #[tokio::test]
    async fn test_reorder_plans_wrong_domain_fails() {
        let (use_cases, domain_repo, plan_repo, _, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let other_domain_id = Uuid::new_v4();
        let plan = create_test_plan(other_domain_id, |p| {
            p.code = "other_domain_plan".to_string();
        });
        plan_repo
            .plans
            .lock()
            .unwrap()
            .insert(plan.id, plan.clone());

        let result = use_cases
            .reorder_plans(owner_id, domain.id, vec![plan.id])
            .await;

        assert!(matches!(result, Err(AppError::Forbidden)));
    }

    #[tokio::test]
    async fn test_reorder_plans_not_found_fails() {
        let (use_cases, domain_repo, _, _, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let nonexistent_plan_id = Uuid::new_v4();

        let result = use_cases
            .reorder_plans(owner_id, domain.id, vec![nonexistent_plan_id])
            .await;

        assert!(matches!(result, Err(AppError::NotFound)));
    }

    #[tokio::test]
    async fn test_reorder_plans_forbidden_non_owner() {
        let (use_cases, domain_repo, plan_repo, _, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let other_user = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let plan = create_test_plan(domain.id, |_| {});
        plan_repo
            .plans
            .lock()
            .unwrap()
            .insert(plan.id, plan.clone());

        let result = use_cases
            .reorder_plans(other_user, domain.id, vec![plan.id])
            .await;

        assert!(matches!(result, Err(AppError::Forbidden)));
    }

    #[tokio::test]
    async fn test_get_plan_by_code_found() {
        let (use_cases, domain_repo, plan_repo, _, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let plan = create_test_plan(domain.id, |p| {
            p.payment_mode = PaymentMode::Test;
            p.code = "premium".to_string();
        });
        plan_repo
            .plans
            .lock()
            .unwrap()
            .insert(plan.id, plan.clone());

        let result = use_cases
            .get_plan_by_code(domain.id, "premium")
            .await
            .unwrap();

        assert!(result.is_some());
        assert_eq!(result.unwrap().code, "premium");
    }

    #[tokio::test]
    async fn test_get_plan_by_code_not_found() {
        let (use_cases, domain_repo, _, _, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let result = use_cases
            .get_plan_by_code(domain.id, "nonexistent")
            .await
            .unwrap();

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_get_plan_by_code_wrong_mode_not_found() {
        let (use_cases, domain_repo, plan_repo, _, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test; // Active mode is Test
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        // Create plan in Live mode
        let plan = create_test_plan(domain.id, |p| {
            p.payment_mode = PaymentMode::Live;
            p.code = "premium".to_string();
        });
        plan_repo
            .plans
            .lock()
            .unwrap()
            .insert(plan.id, plan.clone());

        // get_plan_by_code uses active mode (Test), so Live plan not found
        let result = use_cases
            .get_plan_by_code(domain.id, "premium")
            .await
            .unwrap();

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_set_stripe_ids_success() {
        let (use_cases, domain_repo, plan_repo, _, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let plan = create_test_plan(domain.id, |p| {
            p.stripe_product_id = None;
            p.stripe_price_id = None;
        });
        plan_repo
            .plans
            .lock()
            .unwrap()
            .insert(plan.id, plan.clone());

        let result = use_cases
            .set_stripe_ids(plan.id, "prod_abc123", "price_xyz789")
            .await;

        assert!(result.is_ok());

        let updated = plan_repo.plans.lock().unwrap();
        let updated_plan = updated.get(&plan.id).unwrap();
        assert_eq!(
            updated_plan.stripe_product_id,
            Some("prod_abc123".to_string())
        );
        assert_eq!(
            updated_plan.stripe_price_id,
            Some("price_xyz789".to_string())
        );
    }

    #[tokio::test]
    async fn test_get_plan_by_stripe_price_id_found() {
        let (use_cases, domain_repo, plan_repo, _, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let plan = create_test_plan(domain.id, |p| {
            p.payment_mode = PaymentMode::Test;
            p.stripe_price_id = Some("price_unique123".to_string());
        });
        plan_repo
            .plans
            .lock()
            .unwrap()
            .insert(plan.id, plan.clone());

        let result = use_cases
            .get_plan_by_stripe_price_id(domain.id, PaymentMode::Test, "price_unique123")
            .await
            .unwrap();

        assert!(result.is_some());
        assert_eq!(
            result.unwrap().stripe_price_id,
            Some("price_unique123".to_string())
        );
    }

    #[tokio::test]
    async fn test_get_plan_by_stripe_price_id_not_found() {
        let (use_cases, domain_repo, _, _, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let result = use_cases
            .get_plan_by_stripe_price_id(domain.id, PaymentMode::Test, "nonexistent_price")
            .await
            .unwrap();

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_get_user_subscription_with_plan_found() {
        let (use_cases, domain_repo, plan_repo, subscription_repo, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let end_user_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let plan = create_test_plan(domain.id, |p| {
            p.payment_mode = PaymentMode::Test;
            p.code = "basic".to_string();
        });
        plan_repo
            .plans
            .lock()
            .unwrap()
            .insert(plan.id, plan.clone());

        let subscription = create_test_subscription(domain.id, end_user_id, plan.id, |s| {
            s.payment_mode = PaymentMode::Test;
        });
        subscription_repo
            .subscriptions
            .lock()
            .unwrap()
            .insert(subscription.id, subscription.clone());

        let result = use_cases
            .get_user_subscription_with_plan(domain.id, end_user_id)
            .await
            .unwrap();

        assert!(result.is_some());
        let (sub, retrieved_plan) = result.unwrap();
        assert_eq!(sub.end_user_id, end_user_id);
        assert_eq!(retrieved_plan.code, "basic");
    }

    #[tokio::test]
    async fn test_get_user_subscription_with_plan_not_found() {
        let (use_cases, domain_repo, _, _, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let end_user_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let result = use_cases
            .get_user_subscription_with_plan(domain.id, end_user_id)
            .await
            .unwrap();

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_list_subscribers_for_mode_test() {
        let (use_cases, domain_repo, plan_repo, subscription_repo, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let end_user_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let plan = create_test_plan(domain.id, |p| {
            p.payment_mode = PaymentMode::Test;
        });
        plan_repo
            .plans
            .lock()
            .unwrap()
            .insert(plan.id, plan.clone());

        let subscription = create_test_subscription(domain.id, end_user_id, plan.id, |s| {
            s.payment_mode = PaymentMode::Test;
        });
        subscription_repo
            .subscriptions
            .lock()
            .unwrap()
            .insert(subscription.id, subscription.clone());

        // Add a Live mode subscription that shouldn't be returned
        let live_plan = create_test_plan(domain.id, |p| {
            p.payment_mode = PaymentMode::Live;
        });
        plan_repo
            .plans
            .lock()
            .unwrap()
            .insert(live_plan.id, live_plan.clone());

        let live_subscription =
            create_test_subscription(domain.id, Uuid::new_v4(), live_plan.id, |s| {
                s.payment_mode = PaymentMode::Live;
            });
        subscription_repo
            .subscriptions
            .lock()
            .unwrap()
            .insert(live_subscription.id, live_subscription.clone());

        let result = use_cases
            .list_subscribers_for_mode(owner_id, domain.id, PaymentMode::Test)
            .await
            .unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].subscription.payment_mode, PaymentMode::Test);
    }

    #[tokio::test]
    async fn test_list_subscribers_for_mode_live() {
        let (use_cases, domain_repo, plan_repo, subscription_repo, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let end_user_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Live;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let plan = create_test_plan(domain.id, |p| {
            p.payment_mode = PaymentMode::Live;
        });
        plan_repo
            .plans
            .lock()
            .unwrap()
            .insert(plan.id, plan.clone());

        let subscription = create_test_subscription(domain.id, end_user_id, plan.id, |s| {
            s.payment_mode = PaymentMode::Live;
        });
        subscription_repo
            .subscriptions
            .lock()
            .unwrap()
            .insert(subscription.id, subscription.clone());

        let result = use_cases
            .list_subscribers_for_mode(owner_id, domain.id, PaymentMode::Live)
            .await
            .unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].subscription.payment_mode, PaymentMode::Live);
    }

    // ========================================================================
    // Step 5: Webhook/Sync Tests
    // ========================================================================

    #[tokio::test]
    async fn test_create_or_update_subscription_creates_new() {
        let (use_cases, domain_repo, plan_repo, subscription_repo, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let end_user_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let plan = create_test_plan(domain.id, |p| {
            p.payment_mode = PaymentMode::Test;
        });
        plan_repo
            .plans
            .lock()
            .unwrap()
            .insert(plan.id, plan.clone());

        let input = CreateSubscriptionInput {
            domain_id: domain.id,
            payment_mode: PaymentMode::Test,
            end_user_id,
            plan_id: plan.id,
            status: SubscriptionStatus::Active,
            stripe_customer_id: "cus_test123".to_string(),
            stripe_subscription_id: Some("sub_test123".to_string()),
            current_period_start: None,
            current_period_end: None,
            trial_start: None,
            trial_end: None,
        };

        let result = use_cases.create_or_update_subscription(&input).await;

        assert!(result.is_ok());
        let sub = result.unwrap();
        assert_eq!(sub.end_user_id, end_user_id);
        assert_eq!(sub.status, SubscriptionStatus::Active);

        // Verify it was stored
        let stored = subscription_repo.subscriptions.lock().unwrap();
        assert_eq!(stored.len(), 1);
    }

    #[tokio::test]
    async fn test_create_or_update_subscription_updates_existing() {
        let (use_cases, domain_repo, plan_repo, subscription_repo, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let end_user_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let plan = create_test_plan(domain.id, |p| {
            p.payment_mode = PaymentMode::Test;
            p.code = "basic".to_string();
        });
        plan_repo
            .plans
            .lock()
            .unwrap()
            .insert(plan.id, plan.clone());

        let new_plan = create_test_plan(domain.id, |p| {
            p.payment_mode = PaymentMode::Test;
            p.code = "premium".to_string();
        });
        plan_repo
            .plans
            .lock()
            .unwrap()
            .insert(new_plan.id, new_plan.clone());

        // Create existing subscription
        let existing = create_test_subscription(domain.id, end_user_id, plan.id, |s| {
            s.payment_mode = PaymentMode::Test;
            s.status = SubscriptionStatus::Trialing;
        });
        subscription_repo
            .subscriptions
            .lock()
            .unwrap()
            .insert(existing.id, existing.clone());

        // Now "update" it with new data via create_or_update
        let input = CreateSubscriptionInput {
            domain_id: domain.id,
            payment_mode: PaymentMode::Test,
            end_user_id,
            plan_id: new_plan.id,
            status: SubscriptionStatus::Active,
            stripe_customer_id: "cus_new_test123".to_string(),
            stripe_subscription_id: Some("sub_new_test123".to_string()),
            current_period_start: None,
            current_period_end: None,
            trial_start: None,
            trial_end: None,
        };

        let result = use_cases.create_or_update_subscription(&input).await;

        assert!(result.is_ok());
        let sub = result.unwrap();
        assert_eq!(sub.status, SubscriptionStatus::Active);
        assert_eq!(sub.plan_id, new_plan.id);

        // Still only one subscription (updated, not duplicated)
        let stored = subscription_repo.subscriptions.lock().unwrap();
        assert_eq!(stored.len(), 1);
    }

    #[tokio::test]
    async fn test_update_subscription_from_stripe_success() {
        let (use_cases, domain_repo, plan_repo, subscription_repo, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let end_user_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let plan = create_test_plan(domain.id, |p| {
            p.payment_mode = PaymentMode::Test;
        });
        plan_repo
            .plans
            .lock()
            .unwrap()
            .insert(plan.id, plan.clone());

        let subscription = create_test_subscription(domain.id, end_user_id, plan.id, |s| {
            s.payment_mode = PaymentMode::Test;
            s.status = SubscriptionStatus::Active;
            s.stripe_subscription_id = Some("sub_stripe123".to_string());
        });
        subscription_repo
            .subscriptions
            .lock()
            .unwrap()
            .insert(subscription.id, subscription.clone());

        let update = StripeSubscriptionUpdate {
            status: SubscriptionStatus::Canceled,
            plan_id: None,
            stripe_subscription_id: None,
            current_period_start: None,
            current_period_end: None,
            cancel_at_period_end: true,
            canceled_at: Some(chrono::Utc::now().naive_utc()),
            trial_start: None,
            trial_end: None,
        };

        let result = use_cases
            .update_subscription_from_stripe("sub_stripe123", &update)
            .await;

        assert!(result.is_ok());
        let updated = result.unwrap();
        assert_eq!(updated.status, SubscriptionStatus::Canceled);
        assert!(updated.cancel_at_period_end);
    }

    #[tokio::test]
    async fn test_update_subscription_from_stripe_not_found() {
        let (use_cases, _, _, _, _) = create_test_use_cases();

        let update = StripeSubscriptionUpdate {
            status: SubscriptionStatus::Active,
            plan_id: None,
            stripe_subscription_id: None,
            current_period_start: None,
            current_period_end: None,
            cancel_at_period_end: false,
            canceled_at: None,
            trial_start: None,
            trial_end: None,
        };

        let result = use_cases
            .update_subscription_from_stripe("sub_nonexistent", &update)
            .await;

        assert!(matches!(result, Err(AppError::NotFound)));
    }

    #[tokio::test]
    async fn test_log_webhook_event_creates_record() {
        let (use_cases, domain_repo, plan_repo, subscription_repo, event_repo) =
            create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let end_user_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let plan = create_test_plan(domain.id, |_| {});
        plan_repo
            .plans
            .lock()
            .unwrap()
            .insert(plan.id, plan.clone());

        let subscription = create_test_subscription(domain.id, end_user_id, plan.id, |_| {});
        subscription_repo
            .subscriptions
            .lock()
            .unwrap()
            .insert(subscription.id, subscription.clone());

        let result = use_cases
            .log_webhook_event(
                subscription.id,
                "customer.subscription.updated",
                Some(SubscriptionStatus::Trialing),
                Some(SubscriptionStatus::Active),
                "evt_stripe123",
                serde_json::json!({"trigger": "trial_end"}),
            )
            .await;

        assert!(result.is_ok());

        let events = event_repo.events.lock().unwrap();
        assert_eq!(events.len(), 1);
        let event = events.first().unwrap();
        assert_eq!(event.event_type, "customer.subscription.updated");
        assert_eq!(event.previous_status, Some(SubscriptionStatus::Trialing));
        assert_eq!(event.new_status, Some(SubscriptionStatus::Active));
    }

    #[tokio::test]
    async fn test_sync_invoice_from_webhook_creates_payment() {
        let (use_cases, domain_repo, plan_repo, subscription_repo, _, payment_repo) =
            create_test_use_cases_with_payments();

        let owner_id = Uuid::new_v4();
        let end_user_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let plan = create_test_plan(domain.id, |p| {
            p.payment_mode = PaymentMode::Test;
            p.code = "basic".to_string();
            p.name = "Basic Plan".to_string();
        });
        plan_repo
            .plans
            .lock()
            .unwrap()
            .insert(plan.id, plan.clone());

        let subscription = create_test_subscription(domain.id, end_user_id, plan.id, |s| {
            s.payment_mode = PaymentMode::Test;
            s.stripe_customer_id = "cus_webhook123".to_string();
        });
        subscription_repo
            .subscriptions
            .lock()
            .unwrap()
            .insert(subscription.id, subscription.clone());

        let invoice_json = serde_json::json!({
            "id": "in_webhook123",
            "customer": "cus_webhook123",
            "amount_due": 1999,
            "amount_paid": 1999,
            "currency": "usd",
            "status": "paid",
            "hosted_invoice_url": "https://invoice.stripe.com/test",
            "invoice_pdf": "https://invoice.stripe.com/test.pdf",
            "number": "INV-001",
            "billing_reason": "subscription_create",
            "created": 1704067200,
            "status_transitions": {
                "paid_at": 1704067300
            }
        });

        let result = use_cases
            .sync_invoice_from_webhook(domain.id, PaymentMode::Test, &invoice_json)
            .await;

        assert!(result.is_ok());
        let payment = result.unwrap();
        assert_eq!(payment.stripe_invoice_id, "in_webhook123");
        assert_eq!(payment.amount_cents, 1999);
        assert_eq!(payment.status, PaymentStatus::Paid);
        assert_eq!(payment.plan_code, Some("basic".to_string()));

        // Verify it was stored
        let payments = payment_repo.payments.lock().unwrap();
        assert_eq!(payments.len(), 1);
    }

    #[tokio::test]
    async fn test_sync_invoice_from_webhook_no_subscription_fails() {
        let (use_cases, domain_repo, _, _, _, _) = create_test_use_cases_with_payments();

        let owner_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let invoice_json = serde_json::json!({
            "id": "in_orphan123",
            "customer": "cus_nonexistent",
            "amount_due": 1000,
            "amount_paid": 0,
            "currency": "usd",
            "status": "open"
        });

        let result = use_cases
            .sync_invoice_from_webhook(domain.id, PaymentMode::Test, &invoice_json)
            .await;

        assert!(matches!(result, Err(AppError::NotFound)));
    }

    #[tokio::test]
    async fn test_sync_invoice_from_webhook_updates_existing() {
        let (use_cases, domain_repo, plan_repo, subscription_repo, _, payment_repo) =
            create_test_use_cases_with_payments();

        let owner_id = Uuid::new_v4();
        let end_user_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let plan = create_test_plan(domain.id, |p| {
            p.payment_mode = PaymentMode::Test;
        });
        plan_repo
            .plans
            .lock()
            .unwrap()
            .insert(plan.id, plan.clone());

        let subscription = create_test_subscription(domain.id, end_user_id, plan.id, |s| {
            s.payment_mode = PaymentMode::Test;
            s.stripe_customer_id = "cus_upsert123".to_string();
        });
        subscription_repo
            .subscriptions
            .lock()
            .unwrap()
            .insert(subscription.id, subscription.clone());

        // Create initial payment with "open" status
        let initial_payment = create_test_payment(domain.id, end_user_id, |p| {
            p.payment_mode = PaymentMode::Test;
            p.stripe_invoice_id = "in_upsert123".to_string();
            p.status = PaymentStatus::Pending;
            p.amount_paid_cents = 0;
        });
        payment_repo.payments.lock().unwrap().insert(
            (
                initial_payment.domain_id,
                initial_payment.payment_mode,
                initial_payment.stripe_invoice_id.clone(),
            ),
            initial_payment,
        );

        // Now sync with updated (paid) status
        let invoice_json = serde_json::json!({
            "id": "in_upsert123",
            "customer": "cus_upsert123",
            "amount_due": 2999,
            "amount_paid": 2999,
            "currency": "usd",
            "status": "paid",
            "status_transitions": {
                "paid_at": 1704067300
            }
        });

        let result = use_cases
            .sync_invoice_from_webhook(domain.id, PaymentMode::Test, &invoice_json)
            .await;

        assert!(result.is_ok());
        let payment = result.unwrap();
        assert_eq!(payment.status, PaymentStatus::Paid);
        assert_eq!(payment.amount_paid_cents, 2999);

        // Still only one payment (updated via upsert)
        let payments = payment_repo.payments.lock().unwrap();
        assert_eq!(payments.len(), 1);
    }

    #[tokio::test]
    async fn test_sync_invoice_extracts_all_fields_correctly() {
        let (use_cases, domain_repo, plan_repo, subscription_repo, _, _) =
            create_test_use_cases_with_payments();

        let owner_id = Uuid::new_v4();
        let end_user_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let plan = create_test_plan(domain.id, |p| {
            p.payment_mode = PaymentMode::Test;
            p.code = "premium".to_string();
            p.name = "Premium Plan".to_string();
        });
        plan_repo
            .plans
            .lock()
            .unwrap()
            .insert(plan.id, plan.clone());

        let subscription = create_test_subscription(domain.id, end_user_id, plan.id, |s| {
            s.payment_mode = PaymentMode::Test;
            s.stripe_customer_id = "cus_fields123".to_string();
        });
        subscription_repo
            .subscriptions
            .lock()
            .unwrap()
            .insert(subscription.id, subscription.clone());

        let invoice_json = serde_json::json!({
            "id": "in_fields123",
            "customer": "cus_fields123",
            "payment_intent": "pi_intent123",
            "amount_due": 4999,
            "amount_paid": 4999,
            "currency": "eur",
            "status": "paid",
            "hosted_invoice_url": "https://invoice.stripe.com/hosted",
            "invoice_pdf": "https://invoice.stripe.com/pdf",
            "number": "INV-2024-001",
            "billing_reason": "subscription_cycle",
            "created": 1704067200,
            "status_transitions": {
                "paid_at": 1704067400
            }
        });

        let result = use_cases
            .sync_invoice_from_webhook(domain.id, PaymentMode::Test, &invoice_json)
            .await;

        assert!(result.is_ok());
        let payment = result.unwrap();

        assert_eq!(payment.stripe_invoice_id, "in_fields123");
        assert_eq!(
            payment.stripe_payment_intent_id,
            Some("pi_intent123".to_string())
        );
        assert_eq!(payment.stripe_customer_id, "cus_fields123");
        assert_eq!(payment.amount_cents, 4999);
        assert_eq!(payment.amount_paid_cents, 4999);
        assert_eq!(payment.currency, "EUR");
        assert_eq!(payment.status, PaymentStatus::Paid);
        assert_eq!(
            payment.hosted_invoice_url,
            Some("https://invoice.stripe.com/hosted".to_string())
        );
        assert_eq!(
            payment.invoice_pdf_url,
            Some("https://invoice.stripe.com/pdf".to_string())
        );
        assert_eq!(payment.invoice_number, Some("INV-2024-001".to_string()));
        assert_eq!(
            payment.billing_reason,
            Some("subscription_cycle".to_string())
        );
        assert_eq!(payment.plan_code, Some("premium".to_string()));
        assert_eq!(payment.plan_name, Some("Premium Plan".to_string()));
        assert!(payment.payment_date.is_some()); // paid status should have payment_date
    }

    #[tokio::test]
    async fn test_sync_invoice_pending_status_no_payment_date() {
        let (use_cases, domain_repo, plan_repo, subscription_repo, _, _) =
            create_test_use_cases_with_payments();

        let owner_id = Uuid::new_v4();
        let end_user_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let plan = create_test_plan(domain.id, |p| {
            p.payment_mode = PaymentMode::Test;
        });
        plan_repo
            .plans
            .lock()
            .unwrap()
            .insert(plan.id, plan.clone());

        let subscription = create_test_subscription(domain.id, end_user_id, plan.id, |s| {
            s.payment_mode = PaymentMode::Test;
            s.stripe_customer_id = "cus_pending123".to_string();
        });
        subscription_repo
            .subscriptions
            .lock()
            .unwrap()
            .insert(subscription.id, subscription.clone());

        let invoice_json = serde_json::json!({
            "id": "in_pending123",
            "customer": "cus_pending123",
            "amount_due": 1999,
            "amount_paid": 0,
            "currency": "usd",
            "status": "open",
            "created": 1704067200
        });

        let result = use_cases
            .sync_invoice_from_webhook(domain.id, PaymentMode::Test, &invoice_json)
            .await;

        assert!(result.is_ok());
        let payment = result.unwrap();
        assert_eq!(payment.status, PaymentStatus::Pending);
        assert!(payment.payment_date.is_none()); // pending status should not have payment_date
    }

    // ========================================================================
    // Step 6: Plan Change Validation Tests
    // ========================================================================

    #[tokio::test]
    async fn test_preview_plan_change_no_subscription_fails() {
        let (use_cases, domain_repo, _, _, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let result = use_cases
            .preview_plan_change(domain.id, user_id, "premium")
            .await;

        assert!(
            matches!(result, Err(AppError::InvalidInput(msg)) if msg.contains("No active subscription"))
        );
    }

    #[tokio::test]
    async fn test_preview_plan_change_manually_granted_fails() {
        let (use_cases, domain_repo, plan_repo, subscription_repo, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let plan = create_test_plan(domain.id, |p| {
            p.payment_mode = PaymentMode::Test;
            p.code = "basic".to_string();
        });
        plan_repo
            .plans
            .lock()
            .unwrap()
            .insert(plan.id, plan.clone());

        let subscription = create_test_subscription(domain.id, user_id, plan.id, |s| {
            s.payment_mode = PaymentMode::Test;
            s.manually_granted = true;
            s.stripe_subscription_id = None;
        });
        subscription_repo
            .subscriptions
            .lock()
            .unwrap()
            .insert(subscription.id, subscription.clone());

        let result = use_cases
            .preview_plan_change(domain.id, user_id, "premium")
            .await;

        assert!(
            matches!(result, Err(AppError::InvalidInput(msg)) if msg.contains("manually granted"))
        );
    }

    #[tokio::test]
    async fn test_preview_plan_change_missing_stripe_subscription_id_fails() {
        let (use_cases, domain_repo, plan_repo, subscription_repo, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let plan = create_test_plan(domain.id, |p| {
            p.payment_mode = PaymentMode::Test;
            p.code = "basic".to_string();
        });
        plan_repo
            .plans
            .lock()
            .unwrap()
            .insert(plan.id, plan.clone());

        let subscription = create_test_subscription(domain.id, user_id, plan.id, |s| {
            s.payment_mode = PaymentMode::Test;
            s.manually_granted = false;
            s.stripe_subscription_id = None; // No stripe subscription
        });
        subscription_repo
            .subscriptions
            .lock()
            .unwrap()
            .insert(subscription.id, subscription.clone());

        let result = use_cases
            .preview_plan_change(domain.id, user_id, "premium")
            .await;

        assert!(
            matches!(result, Err(AppError::InvalidInput(msg)) if msg.contains("not linked to Stripe"))
        );
    }

    #[tokio::test]
    async fn test_preview_plan_change_past_due_status_fails() {
        let (use_cases, domain_repo, plan_repo, subscription_repo, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let plan = create_test_plan(domain.id, |p| {
            p.payment_mode = PaymentMode::Test;
            p.code = "basic".to_string();
        });
        plan_repo
            .plans
            .lock()
            .unwrap()
            .insert(plan.id, plan.clone());

        let subscription = create_test_subscription(domain.id, user_id, plan.id, |s| {
            s.payment_mode = PaymentMode::Test;
            s.status = SubscriptionStatus::PastDue;
            s.stripe_subscription_id = Some("sub_test123".to_string());
        });
        subscription_repo
            .subscriptions
            .lock()
            .unwrap()
            .insert(subscription.id, subscription.clone());

        let result = use_cases
            .preview_plan_change(domain.id, user_id, "premium")
            .await;

        assert!(matches!(result, Err(AppError::InvalidInput(msg)) if msg.contains("past due")));
    }

    #[tokio::test]
    async fn test_preview_plan_change_incomplete_status_fails() {
        let (use_cases, domain_repo, plan_repo, subscription_repo, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let plan = create_test_plan(domain.id, |p| {
            p.payment_mode = PaymentMode::Test;
            p.code = "basic".to_string();
        });
        plan_repo
            .plans
            .lock()
            .unwrap()
            .insert(plan.id, plan.clone());

        let subscription = create_test_subscription(domain.id, user_id, plan.id, |s| {
            s.payment_mode = PaymentMode::Test;
            s.status = SubscriptionStatus::Incomplete;
            s.stripe_subscription_id = Some("sub_test123".to_string());
        });
        subscription_repo
            .subscriptions
            .lock()
            .unwrap()
            .insert(subscription.id, subscription.clone());

        let result = use_cases
            .preview_plan_change(domain.id, user_id, "premium")
            .await;

        assert!(
            matches!(result, Err(AppError::InvalidInput(msg)) if msg.contains("complete the current payment"))
        );
    }

    #[tokio::test]
    async fn test_preview_plan_change_paused_status_fails() {
        let (use_cases, domain_repo, plan_repo, subscription_repo, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let plan = create_test_plan(domain.id, |p| {
            p.payment_mode = PaymentMode::Test;
            p.code = "basic".to_string();
        });
        plan_repo
            .plans
            .lock()
            .unwrap()
            .insert(plan.id, plan.clone());

        let subscription = create_test_subscription(domain.id, user_id, plan.id, |s| {
            s.payment_mode = PaymentMode::Test;
            s.status = SubscriptionStatus::Paused;
            s.stripe_subscription_id = Some("sub_test123".to_string());
        });
        subscription_repo
            .subscriptions
            .lock()
            .unwrap()
            .insert(subscription.id, subscription.clone());

        let result = use_cases
            .preview_plan_change(domain.id, user_id, "premium")
            .await;

        assert!(matches!(result, Err(AppError::InvalidInput(msg)) if msg.contains("paused")));
    }

    #[tokio::test]
    async fn test_preview_plan_change_canceled_status_fails() {
        let (use_cases, domain_repo, plan_repo, subscription_repo, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let plan = create_test_plan(domain.id, |p| {
            p.payment_mode = PaymentMode::Test;
            p.code = "basic".to_string();
        });
        plan_repo
            .plans
            .lock()
            .unwrap()
            .insert(plan.id, plan.clone());

        let subscription = create_test_subscription(domain.id, user_id, plan.id, |s| {
            s.payment_mode = PaymentMode::Test;
            s.status = SubscriptionStatus::Canceled;
            s.cancel_at_period_end = false; // Fully canceled, not just scheduled to cancel
            s.stripe_subscription_id = Some("sub_test123".to_string());
        });
        subscription_repo
            .subscriptions
            .lock()
            .unwrap()
            .insert(subscription.id, subscription.clone());

        let result = use_cases
            .preview_plan_change(domain.id, user_id, "premium")
            .await;

        assert!(
            matches!(result, Err(AppError::InvalidInput(msg)) if msg.contains("canceled subscription"))
        );
    }

    #[tokio::test]
    async fn test_preview_plan_change_same_plan_fails() {
        let (use_cases, domain_repo, plan_repo, subscription_repo, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let plan = create_test_plan(domain.id, |p| {
            p.payment_mode = PaymentMode::Test;
            p.code = "basic".to_string();
            p.is_public = true;
        });
        plan_repo
            .plans
            .lock()
            .unwrap()
            .insert(plan.id, plan.clone());

        let subscription = create_test_subscription(domain.id, user_id, plan.id, |s| {
            s.payment_mode = PaymentMode::Test;
            s.status = SubscriptionStatus::Active;
            s.stripe_subscription_id = Some("sub_test123".to_string());
        });
        subscription_repo
            .subscriptions
            .lock()
            .unwrap()
            .insert(subscription.id, subscription.clone());

        // Try to "change" to the same plan
        let result = use_cases
            .preview_plan_change(domain.id, user_id, "basic")
            .await;

        assert!(
            matches!(result, Err(AppError::InvalidInput(msg)) if msg.contains("Already subscribed"))
        );
    }

    #[tokio::test]
    async fn test_preview_plan_change_archived_plan_fails() {
        let (use_cases, domain_repo, plan_repo, subscription_repo, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let current_plan = create_test_plan(domain.id, |p| {
            p.payment_mode = PaymentMode::Test;
            p.code = "basic".to_string();
            p.is_public = true;
        });
        plan_repo
            .plans
            .lock()
            .unwrap()
            .insert(current_plan.id, current_plan.clone());

        let archived_plan = create_test_plan(domain.id, |p| {
            p.payment_mode = PaymentMode::Test;
            p.code = "premium".to_string();
            p.is_public = true;
            p.is_archived = true; // Archived!
        });
        plan_repo
            .plans
            .lock()
            .unwrap()
            .insert(archived_plan.id, archived_plan.clone());

        let subscription = create_test_subscription(domain.id, user_id, current_plan.id, |s| {
            s.payment_mode = PaymentMode::Test;
            s.status = SubscriptionStatus::Active;
            s.stripe_subscription_id = Some("sub_test123".to_string());
        });
        subscription_repo
            .subscriptions
            .lock()
            .unwrap()
            .insert(subscription.id, subscription.clone());

        let result = use_cases
            .preview_plan_change(domain.id, user_id, "premium")
            .await;

        assert!(
            matches!(result, Err(AppError::InvalidInput(msg)) if msg.contains("no longer available"))
        );
    }

    #[tokio::test]
    async fn test_preview_plan_change_private_plan_fails() {
        let (use_cases, domain_repo, plan_repo, subscription_repo, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let current_plan = create_test_plan(domain.id, |p| {
            p.payment_mode = PaymentMode::Test;
            p.code = "basic".to_string();
            p.is_public = true;
        });
        plan_repo
            .plans
            .lock()
            .unwrap()
            .insert(current_plan.id, current_plan.clone());

        let private_plan = create_test_plan(domain.id, |p| {
            p.payment_mode = PaymentMode::Test;
            p.code = "enterprise".to_string();
            p.is_public = false; // Private!
        });
        plan_repo
            .plans
            .lock()
            .unwrap()
            .insert(private_plan.id, private_plan.clone());

        let subscription = create_test_subscription(domain.id, user_id, current_plan.id, |s| {
            s.payment_mode = PaymentMode::Test;
            s.status = SubscriptionStatus::Active;
            s.stripe_subscription_id = Some("sub_test123".to_string());
        });
        subscription_repo
            .subscriptions
            .lock()
            .unwrap()
            .insert(subscription.id, subscription.clone());

        let result = use_cases
            .preview_plan_change(domain.id, user_id, "enterprise")
            .await;

        assert!(
            matches!(result, Err(AppError::InvalidInput(msg)) if msg.contains("not available"))
        );
    }

    #[tokio::test]
    async fn test_preview_plan_change_interval_change_allowed() {
        // Interval changes are now allowed (monthly -> yearly is an upgrade, yearly -> monthly is downgrade)
        let (use_cases, domain_repo, plan_repo, subscription_repo, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let monthly_plan = create_test_plan(domain.id, |p| {
            p.payment_mode = PaymentMode::Test;
            p.code = "basic_monthly".to_string();
            p.interval = "month".to_string();
            p.price_cents = 2000; // $20/month
            p.is_public = true;
        });
        plan_repo
            .plans
            .lock()
            .unwrap()
            .insert(monthly_plan.id, monthly_plan.clone());

        let yearly_plan = create_test_plan(domain.id, |p| {
            p.payment_mode = PaymentMode::Test;
            p.code = "basic_yearly".to_string();
            p.interval = "year".to_string();
            p.price_cents = 20000; // $200/year (discounted from $240)
            p.is_public = true;
        });
        plan_repo
            .plans
            .lock()
            .unwrap()
            .insert(yearly_plan.id, yearly_plan.clone());

        let subscription = create_test_subscription(domain.id, user_id, monthly_plan.id, |s| {
            s.payment_mode = PaymentMode::Test;
            s.status = SubscriptionStatus::Active;
            s.stripe_subscription_id = Some("sub_test123".to_string());
        });
        subscription_repo
            .subscriptions
            .lock()
            .unwrap()
            .insert(subscription.id, subscription.clone());

        let result = use_cases
            .preview_plan_change(domain.id, user_id, "basic_yearly")
            .await;

        // Should succeed - interval changes are now allowed
        assert!(result.is_ok(), "Expected Ok, got {:?}", result);
        let preview = result.unwrap();
        // Monthly to yearly with higher price = upgrade
        assert_eq!(preview.change_type, PlanChangeType::Upgrade);
    }

    #[tokio::test]
    async fn test_preview_plan_change_plan_not_found_fails() {
        let (use_cases, domain_repo, plan_repo, subscription_repo, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let plan = create_test_plan(domain.id, |p| {
            p.payment_mode = PaymentMode::Test;
            p.code = "basic".to_string();
        });
        plan_repo
            .plans
            .lock()
            .unwrap()
            .insert(plan.id, plan.clone());

        let subscription = create_test_subscription(domain.id, user_id, plan.id, |s| {
            s.payment_mode = PaymentMode::Test;
            s.status = SubscriptionStatus::Active;
            s.stripe_subscription_id = Some("sub_test123".to_string());
        });
        subscription_repo
            .subscriptions
            .lock()
            .unwrap()
            .insert(subscription.id, subscription.clone());

        let result = use_cases
            .preview_plan_change(domain.id, user_id, "nonexistent_plan")
            .await;

        assert!(matches!(result, Err(AppError::InvalidInput(msg)) if msg.contains("not found")));
    }

    // change_plan validation tests - these should have the same validation as preview_plan_change

    #[tokio::test]
    async fn test_change_plan_no_subscription_fails() {
        let (use_cases, domain_repo, _, _, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let result = use_cases.change_plan(domain.id, user_id, "premium").await;

        assert!(
            matches!(result, Err(AppError::InvalidInput(msg)) if msg.contains("No active subscription"))
        );
    }

    #[tokio::test]
    async fn test_change_plan_manually_granted_fails() {
        let (use_cases, domain_repo, plan_repo, subscription_repo, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let plan = create_test_plan(domain.id, |p| {
            p.payment_mode = PaymentMode::Test;
            p.code = "basic".to_string();
        });
        plan_repo
            .plans
            .lock()
            .unwrap()
            .insert(plan.id, plan.clone());

        let subscription = create_test_subscription(domain.id, user_id, plan.id, |s| {
            s.payment_mode = PaymentMode::Test;
            s.manually_granted = true;
            s.stripe_subscription_id = None;
        });
        subscription_repo
            .subscriptions
            .lock()
            .unwrap()
            .insert(subscription.id, subscription.clone());

        let result = use_cases.change_plan(domain.id, user_id, "premium").await;

        assert!(
            matches!(result, Err(AppError::InvalidInput(msg)) if msg.contains("manually granted"))
        );
    }

    #[tokio::test]
    async fn test_change_plan_same_plan_fails() {
        let (use_cases, domain_repo, plan_repo, subscription_repo, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let plan = create_test_plan(domain.id, |p| {
            p.payment_mode = PaymentMode::Test;
            p.code = "basic".to_string();
            p.is_public = true;
        });
        plan_repo
            .plans
            .lock()
            .unwrap()
            .insert(plan.id, plan.clone());

        let subscription = create_test_subscription(domain.id, user_id, plan.id, |s| {
            s.payment_mode = PaymentMode::Test;
            s.status = SubscriptionStatus::Active;
            s.stripe_subscription_id = Some("sub_test123".to_string());
        });
        subscription_repo
            .subscriptions
            .lock()
            .unwrap()
            .insert(subscription.id, subscription.clone());

        let result = use_cases.change_plan(domain.id, user_id, "basic").await;

        assert!(
            matches!(result, Err(AppError::InvalidInput(msg)) if msg.contains("Already subscribed"))
        );
    }

    #[tokio::test]
    async fn test_change_plan_past_due_status_fails() {
        let (use_cases, domain_repo, plan_repo, subscription_repo, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let current_plan = create_test_plan(domain.id, |p| {
            p.payment_mode = PaymentMode::Test;
            p.code = "basic".to_string();
        });
        plan_repo
            .plans
            .lock()
            .unwrap()
            .insert(current_plan.id, current_plan.clone());

        let new_plan = create_test_plan(domain.id, |p| {
            p.payment_mode = PaymentMode::Test;
            p.code = "premium".to_string();
            p.is_public = true;
        });
        plan_repo
            .plans
            .lock()
            .unwrap()
            .insert(new_plan.id, new_plan.clone());

        let subscription = create_test_subscription(domain.id, user_id, current_plan.id, |s| {
            s.payment_mode = PaymentMode::Test;
            s.status = SubscriptionStatus::PastDue;
            s.stripe_subscription_id = Some("sub_test123".to_string());
        });
        subscription_repo
            .subscriptions
            .lock()
            .unwrap()
            .insert(subscription.id, subscription.clone());

        let result = use_cases.change_plan(domain.id, user_id, "premium").await;

        assert!(matches!(result, Err(AppError::InvalidInput(msg)) if msg.contains("past due")));
    }

    #[tokio::test]
    async fn test_change_plan_archived_plan_fails() {
        let (use_cases, domain_repo, plan_repo, subscription_repo, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let current_plan = create_test_plan(domain.id, |p| {
            p.payment_mode = PaymentMode::Test;
            p.code = "basic".to_string();
        });
        plan_repo
            .plans
            .lock()
            .unwrap()
            .insert(current_plan.id, current_plan.clone());

        let archived_plan = create_test_plan(domain.id, |p| {
            p.payment_mode = PaymentMode::Test;
            p.code = "legacy".to_string();
            p.is_public = true;
            p.is_archived = true;
        });
        plan_repo
            .plans
            .lock()
            .unwrap()
            .insert(archived_plan.id, archived_plan.clone());

        let subscription = create_test_subscription(domain.id, user_id, current_plan.id, |s| {
            s.payment_mode = PaymentMode::Test;
            s.status = SubscriptionStatus::Active;
            s.stripe_subscription_id = Some("sub_test123".to_string());
        });
        subscription_repo
            .subscriptions
            .lock()
            .unwrap()
            .insert(subscription.id, subscription.clone());

        let result = use_cases.change_plan(domain.id, user_id, "legacy").await;

        assert!(
            matches!(result, Err(AppError::InvalidInput(msg)) if msg.contains("no longer available"))
        );
    }

    #[tokio::test]
    async fn test_change_plan_interval_change_allowed() {
        // Interval changes are now allowed (monthly -> yearly, yearly -> monthly)
        let (use_cases, domain_repo, plan_repo, subscription_repo, _) = create_test_use_cases();

        let owner_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        let monthly_plan = create_test_plan(domain.id, |p| {
            p.payment_mode = PaymentMode::Test;
            p.code = "monthly".to_string();
            p.interval = "month".to_string();
            p.price_cents = 2000; // $20/month
        });
        plan_repo
            .plans
            .lock()
            .unwrap()
            .insert(monthly_plan.id, monthly_plan.clone());

        let yearly_plan = create_test_plan(domain.id, |p| {
            p.payment_mode = PaymentMode::Test;
            p.code = "yearly".to_string();
            p.interval = "year".to_string();
            p.price_cents = 20000; // $200/year
            p.is_public = true;
        });
        plan_repo
            .plans
            .lock()
            .unwrap()
            .insert(yearly_plan.id, yearly_plan.clone());

        let subscription = create_test_subscription(domain.id, user_id, monthly_plan.id, |s| {
            s.payment_mode = PaymentMode::Test;
            s.status = SubscriptionStatus::Active;
            s.stripe_subscription_id = Some("sub_test123".to_string());
        });
        subscription_repo
            .subscriptions
            .lock()
            .unwrap()
            .insert(subscription.id, subscription.clone());

        let result = use_cases.change_plan(domain.id, user_id, "yearly").await;

        // Interval changes are allowed - should succeed (or fail for other reasons like provider not configured)
        // The validation for interval mismatch should NOT happen
        match &result {
            Err(AppError::InvalidInput(msg)) => {
                assert!(
                    !msg.contains("Cannot switch between"),
                    "Interval changes should be allowed, but got: {}",
                    msg
                );
            }
            _ => {} // Other errors (like provider not configured) or success are fine
        }
    }

    // ========================================================================
    // ensure_stripe_ids Tests (uses provider abstraction)
    // ========================================================================

    /// Helper to create test use cases with access to enabled_providers_repo for provider setup.
    fn create_test_use_cases_with_providers() -> (
        DomainBillingUseCases,
        Arc<InMemoryDomainRepo>,
        Arc<InMemorySubscriptionPlanRepo>,
        Arc<InMemoryUserSubscriptionRepo>,
        Arc<InMemorySubscriptionEventRepo>,
        Arc<InMemoryEnabledPaymentProvidersRepo>,
    ) {
        let domain_repo = Arc::new(InMemoryDomainRepo::new());
        let stripe_config_repo = Arc::new(InMemoryBillingStripeConfigRepo::new());
        let enabled_providers_repo = Arc::new(InMemoryEnabledPaymentProvidersRepo::new());
        let plan_repo = Arc::new(InMemorySubscriptionPlanRepo::new());
        let subscription_repo =
            Arc::new(InMemoryUserSubscriptionRepo::new().with_plan_repo(plan_repo.clone()));
        let event_repo = Arc::new(InMemorySubscriptionEventRepo::new());
        let payment_repo = Arc::new(InMemoryBillingPaymentRepo::new());

        let cipher = ProcessCipher::new_from_base64(TEST_KEY_B64).unwrap();
        let factory = Arc::new(PaymentProviderFactory::new(
            cipher.clone(),
            stripe_config_repo.clone() as Arc<dyn BillingStripeConfigRepoTrait>,
        ));

        let use_cases = DomainBillingUseCases::new(
            domain_repo.clone() as Arc<dyn super::super::domain::DomainRepoTrait>,
            stripe_config_repo as Arc<dyn BillingStripeConfigRepoTrait>,
            enabled_providers_repo.clone() as Arc<dyn EnabledPaymentProvidersRepoTrait>,
            plan_repo.clone() as Arc<dyn SubscriptionPlanRepoTrait>,
            subscription_repo.clone() as Arc<dyn UserSubscriptionRepoTrait>,
            event_repo.clone() as Arc<dyn SubscriptionEventRepoTrait>,
            payment_repo as Arc<dyn BillingPaymentRepoTrait>,
            cipher,
            factory,
        );

        (
            use_cases,
            domain_repo,
            plan_repo,
            subscription_repo,
            event_repo,
            enabled_providers_repo,
        )
    }

    #[tokio::test]
    async fn test_ensure_stripe_ids_skips_when_ids_present() {
        let (use_cases, domain_repo, plan_repo, _, _, enabled_providers_repo) =
            create_test_use_cases_with_providers();

        let owner_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        // Enable Dummy provider for this domain
        enabled_providers_repo
            .enable(domain.id, PaymentProvider::Dummy, PaymentMode::Test, 0)
            .await
            .unwrap();

        // Create a plan WITH existing Stripe IDs
        let plan = create_test_plan(domain.id, |p| {
            p.payment_mode = PaymentMode::Test;
            p.stripe_product_id = Some("prod_existing".to_string());
            p.stripe_price_id = Some("price_existing".to_string());
        });
        plan_repo
            .plans
            .lock()
            .unwrap()
            .insert(plan.id, plan.clone());

        // Call ensure_stripe_ids - should return early without changing anything
        let result = use_cases.ensure_stripe_ids(domain.id, plan.clone()).await;

        assert!(result.is_ok());
        let returned_plan = result.unwrap();

        // IDs should remain unchanged (the originals)
        assert_eq!(
            returned_plan.stripe_product_id,
            Some("prod_existing".to_string())
        );
        assert_eq!(
            returned_plan.stripe_price_id,
            Some("price_existing".to_string())
        );
    }

    #[tokio::test]
    async fn test_ensure_stripe_ids_creates_ids_when_missing() {
        let (use_cases, domain_repo, plan_repo, _, _, enabled_providers_repo) =
            create_test_use_cases_with_providers();

        let owner_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        // Enable Dummy provider for this domain
        enabled_providers_repo
            .enable(domain.id, PaymentProvider::Dummy, PaymentMode::Test, 0)
            .await
            .unwrap();

        // Create a plan WITHOUT Stripe IDs
        let plan = create_test_plan(domain.id, |p| {
            p.payment_mode = PaymentMode::Test;
            p.stripe_product_id = None;
            p.stripe_price_id = None;
        });
        plan_repo
            .plans
            .lock()
            .unwrap()
            .insert(plan.id, plan.clone());

        // Call ensure_stripe_ids - should create IDs via DummyPaymentClient
        let result = use_cases.ensure_stripe_ids(domain.id, plan.clone()).await;

        assert!(result.is_ok());
        let returned_plan = result.unwrap();

        // IDs should now be set (DummyPaymentClient returns "dummy_prod_{id}" and "dummy_price_{id}")
        assert!(returned_plan.stripe_product_id.is_some());
        assert!(returned_plan.stripe_price_id.is_some());

        // Verify IDs were persisted to the repository
        let stored_plan = plan_repo.plans.lock().unwrap().get(&plan.id).cloned();
        assert!(stored_plan.is_some());
        let stored_plan = stored_plan.unwrap();
        assert!(stored_plan.stripe_product_id.is_some());
        assert!(stored_plan.stripe_price_id.is_some());
    }

    #[tokio::test]
    async fn test_preview_plan_change_creates_stripe_ids_when_missing() {
        let (use_cases, domain_repo, plan_repo, subscription_repo, _, enabled_providers_repo) =
            create_test_use_cases_with_providers();

        let owner_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        // Enable Dummy provider for this domain
        enabled_providers_repo
            .enable(domain.id, PaymentProvider::Dummy, PaymentMode::Test, 0)
            .await
            .unwrap();

        // Create current plan WITH Stripe IDs (user is subscribed to this)
        let current_plan = create_test_plan(domain.id, |p| {
            p.payment_mode = PaymentMode::Test;
            p.code = "basic".to_string();
            p.price_cents = 999;
            p.stripe_product_id = Some("prod_existing".to_string());
            p.stripe_price_id = Some("price_existing".to_string());
        });
        plan_repo
            .plans
            .lock()
            .unwrap()
            .insert(current_plan.id, current_plan.clone());

        // Create target plan WITHOUT Stripe IDs
        let new_plan = create_test_plan(domain.id, |p| {
            p.payment_mode = PaymentMode::Test;
            p.code = "premium".to_string();
            p.price_cents = 1999;
            p.stripe_product_id = None;
            p.stripe_price_id = None;
        });
        plan_repo
            .plans
            .lock()
            .unwrap()
            .insert(new_plan.id, new_plan.clone());

        // Create subscription for user on current plan
        let subscription = create_test_subscription(domain.id, user_id, current_plan.id, |s| {
            s.payment_mode = PaymentMode::Test;
            s.status = SubscriptionStatus::Active;
            s.stripe_subscription_id = Some("sub_test123".to_string());
        });
        subscription_repo
            .subscriptions
            .lock()
            .unwrap()
            .insert(subscription.id, subscription.clone());

        // Act: preview plan change to the plan without Stripe IDs
        let result = use_cases
            .preview_plan_change(domain.id, user_id, "premium")
            .await;

        // Assert: Should succeed (Stripe IDs auto-created via DummyPaymentClient)
        assert!(result.is_ok());

        // Verify the target plan now has Stripe IDs in repository
        let updated_plan = plan_repo.plans.lock().unwrap().get(&new_plan.id).cloned();
        assert!(updated_plan.is_some());
        let updated_plan = updated_plan.unwrap();
        assert!(
            updated_plan.stripe_product_id.is_some(),
            "stripe_product_id should be set after preview_plan_change"
        );
        assert!(
            updated_plan.stripe_price_id.is_some(),
            "stripe_price_id should be set after preview_plan_change"
        );
    }

    #[tokio::test]
    async fn test_change_plan_creates_stripe_ids_when_missing() {
        let (use_cases, domain_repo, plan_repo, subscription_repo, _, enabled_providers_repo) =
            create_test_use_cases_with_providers();

        let owner_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let domain = create_test_domain(|d| {
            d.owner_end_user_id = Some(owner_id);
            d.active_payment_mode = PaymentMode::Test;
        });
        domain_repo
            .domains
            .lock()
            .unwrap()
            .insert(domain.id, domain.clone());

        // Enable Dummy provider for this domain
        enabled_providers_repo
            .enable(domain.id, PaymentProvider::Dummy, PaymentMode::Test, 0)
            .await
            .unwrap();

        // Create current plan WITH Stripe IDs (user is subscribed to this)
        let current_plan = create_test_plan(domain.id, |p| {
            p.payment_mode = PaymentMode::Test;
            p.code = "basic".to_string();
            p.price_cents = 999;
            p.stripe_product_id = Some("prod_existing".to_string());
            p.stripe_price_id = Some("price_existing".to_string());
        });
        plan_repo
            .plans
            .lock()
            .unwrap()
            .insert(current_plan.id, current_plan.clone());

        // Create target plan WITHOUT Stripe IDs
        let new_plan = create_test_plan(domain.id, |p| {
            p.payment_mode = PaymentMode::Test;
            p.code = "premium".to_string();
            p.price_cents = 1999;
            p.stripe_product_id = None;
            p.stripe_price_id = None;
        });
        plan_repo
            .plans
            .lock()
            .unwrap()
            .insert(new_plan.id, new_plan.clone());

        // Create subscription for user on current plan
        let subscription = create_test_subscription(domain.id, user_id, current_plan.id, |s| {
            s.payment_mode = PaymentMode::Test;
            s.status = SubscriptionStatus::Active;
            s.stripe_subscription_id = Some("sub_test123".to_string());
        });
        subscription_repo
            .subscriptions
            .lock()
            .unwrap()
            .insert(subscription.id, subscription.clone());

        // Act: change plan to the plan without Stripe IDs
        let result = use_cases.change_plan(domain.id, user_id, "premium").await;

        // Assert: Should succeed (Stripe IDs auto-created via DummyPaymentClient)
        assert!(result.is_ok());

        // Verify the target plan now has Stripe IDs in repository
        let updated_plan = plan_repo.plans.lock().unwrap().get(&new_plan.id).cloned();
        assert!(updated_plan.is_some());
        let updated_plan = updated_plan.unwrap();
        assert!(
            updated_plan.stripe_product_id.is_some(),
            "stripe_product_id should be set after change_plan"
        );
        assert!(
            updated_plan.stripe_price_id.is_some(),
            "stripe_price_id should be set after change_plan"
        );
    }
}
