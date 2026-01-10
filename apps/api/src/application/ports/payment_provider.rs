use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    app_error::AppResult,
    domain::entities::{
        payment_mode::PaymentMode, payment_provider::PaymentProvider,
        payment_scenario::PaymentScenario, user_subscription::SubscriptionStatus,
    },
};

// ============================================================================
// Port Types - Provider-agnostic domain types
// ============================================================================

/// Unique identifier for a customer in a payment provider
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CustomerId(pub String);

impl CustomerId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for CustomerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Unique identifier for a subscription in a payment provider
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SubscriptionId(pub String);

impl SubscriptionId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for SubscriptionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Basic plan information for checkout and subscription operations
#[derive(Debug, Clone, Serialize)]
pub struct PlanInfo {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub price_cents: i32,
    pub currency: String,
    pub interval: String,
    pub interval_count: i32,
    pub trial_days: i32,
    /// Provider-specific price ID (e.g., Stripe price ID)
    pub external_price_id: Option<String>,
    /// Provider-specific product ID (e.g., Stripe product ID)
    pub external_product_id: Option<String>,
}

/// URLs for checkout redirects
#[derive(Debug, Clone)]
pub struct CheckoutUrls {
    pub success_url: String,
    pub cancel_url: String,
}

/// Result of creating a checkout session
#[derive(Debug, Clone, Serialize)]
pub struct CheckoutResult {
    /// URL to redirect the user to for checkout (for external providers like Stripe)
    pub checkout_url: Option<String>,
    /// Whether the checkout is inline (no redirect needed - for dummy provider)
    pub is_inline: bool,
    /// Session/checkout ID for tracking
    pub session_id: Option<String>,
}

/// Result of starting a subscription
#[derive(Debug, Clone, Serialize)]
pub struct SubscriptionResult {
    /// Provider-specific subscription ID
    pub subscription_id: SubscriptionId,
    /// Provider-specific customer ID
    pub customer_id: CustomerId,
    /// Current status of the subscription
    pub status: SubscriptionStatus,
    /// Start of the current billing period
    pub current_period_start: Option<DateTime<Utc>>,
    /// End of the current billing period
    pub current_period_end: Option<DateTime<Utc>>,
    /// Trial start (if applicable)
    pub trial_start: Option<DateTime<Utc>>,
    /// Trial end (if applicable)
    pub trial_end: Option<DateTime<Utc>>,
    /// Whether payment requires additional confirmation (3D Secure)
    pub requires_confirmation: bool,
    /// Confirmation token for 3DS flow
    pub confirmation_token: Option<String>,
    /// Invoice ID for the first payment
    pub invoice_id: Option<String>,
}

/// Result of a plan change (upgrade or downgrade)
#[derive(Debug, Clone, Serialize)]
pub struct PlanChangeResult {
    /// Whether the change was successful
    pub success: bool,
    /// Type of change (upgrade or downgrade)
    pub change_type: PlanChangeType,
    /// Invoice ID for upgrade charges
    pub invoice_id: Option<String>,
    /// Amount charged for upgrade
    pub amount_charged_cents: Option<i64>,
    /// Currency of the charge
    pub currency: Option<String>,
    /// Client secret for 3DS confirmation (if needed)
    pub client_secret: Option<String>,
    /// URL for hosted invoice (fallback)
    pub hosted_invoice_url: Option<String>,
    /// Payment intent status
    pub payment_intent_status: Option<String>,
    /// When the change takes effect
    pub effective_at: DateTime<Utc>,
    /// Schedule ID for downgrades
    pub schedule_id: Option<String>,
}

/// Type of plan change
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PlanChangeType {
    /// Immediate change with proration
    Upgrade,
    /// Scheduled change at period end
    Downgrade,
}

impl PlanChangeType {
    pub fn as_str(&self) -> &'static str {
        match self {
            PlanChangeType::Upgrade => "upgrade",
            PlanChangeType::Downgrade => "downgrade",
        }
    }
}

/// Preview of a plan change (proration info)
#[derive(Debug, Clone, Serialize)]
pub struct PlanChangePreview {
    /// Prorated amount to charge (positive for upgrade, negative for credit)
    pub prorated_amount_cents: i64,
    /// Currency of the proration
    pub currency: String,
    /// End of current billing period
    pub period_end: DateTime<Utc>,
    /// New plan name
    pub new_plan_name: String,
    /// New plan price
    pub new_plan_price_cents: i64,
    /// Type of change
    pub change_type: PlanChangeType,
    /// When the change takes effect
    pub effective_at: DateTime<Utc>,
}

/// Subscription status information
#[derive(Debug, Clone, Serialize)]
pub struct SubscriptionInfo {
    pub subscription_id: SubscriptionId,
    pub customer_id: CustomerId,
    pub status: SubscriptionStatus,
    pub current_period_start: Option<DateTime<Utc>>,
    pub current_period_end: Option<DateTime<Utc>>,
    pub trial_start: Option<DateTime<Utc>>,
    pub trial_end: Option<DateTime<Utc>>,
    pub cancel_at_period_end: bool,
    pub canceled_at: Option<DateTime<Utc>>,
    /// External price ID to identify the plan
    pub price_id: Option<String>,
    /// Subscription item ID (for upgrades)
    pub subscription_item_id: Option<String>,
}

/// Invoice information
#[derive(Debug, Clone, Serialize)]
pub struct InvoiceInfo {
    pub invoice_id: String,
    pub customer_id: CustomerId,
    pub amount_cents: i32,
    pub amount_paid_cents: i32,
    pub currency: String,
    pub status: String,
    pub hosted_url: Option<String>,
    pub pdf_url: Option<String>,
    pub invoice_number: Option<String>,
    pub billing_reason: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub paid_at: Option<DateTime<Utc>>,
}

// ============================================================================
// Payment Provider Port - Domain-action based interface
// ============================================================================

/// Payment provider port - abstracts payment provider operations.
///
/// This trait defines domain-level actions (not provider primitives).
/// Implementations should map these to provider-specific APIs.
#[async_trait]
pub trait PaymentProviderPort: Send + Sync {
    /// Get the provider type
    fn provider(&self) -> PaymentProvider;

    /// Get the payment mode (test/live)
    fn mode(&self) -> PaymentMode;

    // ========================================================================
    // Customer Management
    // ========================================================================

    /// Ensure a customer exists in the payment provider.
    /// Creates a new customer if one doesn't exist.
    async fn ensure_customer(
        &self,
        email: &str,
        user_id: Uuid,
        domain_id: Uuid,
    ) -> AppResult<CustomerId>;

    /// Get customer information by ID from the payment provider.
    ///
    /// # Provider Behavior
    /// - **Stripe**: Queries the Stripe API for customer data
    /// - **Dummy**: Returns `None` - customer data is in the local database only
    /// - **Coinbase**: Not yet implemented
    ///
    /// For dummy provider, customer data is only available in the local database.
    async fn get_customer(&self, customer_id: &CustomerId) -> AppResult<Option<CustomerInfo>>;

    // ========================================================================
    // Checkout & Subscription Creation
    // ========================================================================

    /// Create a checkout session for a new subscription.
    /// For Stripe/Coinbase: Returns a redirect URL.
    /// For Dummy: Returns inline checkout info.
    async fn create_checkout(
        &self,
        customer: &CustomerId,
        plan: &PlanInfo,
        urls: &CheckoutUrls,
        scenario: Option<PaymentScenario>,
    ) -> AppResult<CheckoutResult>;

    /// Start a subscription directly (for inline checkout flows).
    /// Primarily used by the dummy provider.
    async fn start_subscription(
        &self,
        customer: &CustomerId,
        plan: &PlanInfo,
        scenario: Option<PaymentScenario>,
    ) -> AppResult<SubscriptionResult>;

    /// Confirm a subscription requiring additional authentication (3DS).
    async fn confirm_subscription(&self, confirmation_token: &str)
    -> AppResult<SubscriptionResult>;

    // ========================================================================
    // Subscription Lifecycle
    // ========================================================================

    /// Get subscription information from the payment provider.
    ///
    /// # Provider Behavior
    /// - **Stripe**: Queries the Stripe API for current subscription state
    /// - **Dummy**: Returns `None` - subscription state is in the local database only
    /// - **Coinbase**: Not yet implemented (returns `ProviderNotSupported` from factory)
    ///
    /// # Return Value
    /// - `Some(info)` - Subscription found in external provider
    /// - `None` - Subscription not found or provider doesn't support external lookup
    ///
    /// For dummy provider, callers should use `UserSubscriptionRepo` to read subscription state.
    async fn get_subscription(
        &self,
        subscription_id: &SubscriptionId,
    ) -> AppResult<Option<SubscriptionInfo>>;

    /// Cancel a subscription
    async fn cancel_subscription(
        &self,
        subscription_id: &SubscriptionId,
        at_period_end: bool,
    ) -> AppResult<()>;

    /// Preview a plan change (proration calculation)
    async fn preview_plan_change(
        &self,
        subscription_id: &SubscriptionId,
        new_plan: &PlanInfo,
    ) -> AppResult<PlanChangePreview>;

    /// Execute a plan change (upgrade or downgrade)
    async fn change_plan(
        &self,
        subscription_id: &SubscriptionId,
        subscription_item_id: Option<&str>,
        new_plan: &PlanInfo,
    ) -> AppResult<PlanChangeResult>;

    // ========================================================================
    // Invoicing & PDF Generation
    // ========================================================================

    /// Generate an invoice PDF.
    /// For Stripe: Returns the PDF URL from Stripe.
    /// For Dummy: Generates a PDF on-the-fly.
    async fn get_invoice_pdf(&self, invoice_id: &str) -> AppResult<InvoicePdfResult>;

    /// List invoices for a customer
    async fn list_invoices(
        &self,
        customer_id: &CustomerId,
        limit: i32,
    ) -> AppResult<Vec<InvoiceInfo>>;

    // ========================================================================
    // Portal & Self-Service
    // ========================================================================

    /// Create a portal session for customer self-service.
    /// Returns None if the provider doesn't support a hosted portal.
    async fn create_portal_session(
        &self,
        customer_id: &CustomerId,
        return_url: &str,
    ) -> AppResult<Option<String>>;

    // ========================================================================
    // Payment Method Management (for Dummy provider)
    // ========================================================================

    /// Update the payment scenario for a dummy subscription.
    /// Only applicable for the dummy provider.
    async fn update_payment_scenario(
        &self,
        subscription_id: &SubscriptionId,
        scenario: PaymentScenario,
    ) -> AppResult<()>;

    // ========================================================================
    // Provider-specific setup
    // ========================================================================

    /// Ensure product and price exist in the provider.
    /// For Stripe: Creates product/price if missing.
    /// For Dummy: No-op (returns provided IDs).
    async fn ensure_product_and_price(&self, plan: &PlanInfo) -> AppResult<(String, String)>; // (product_id, price_id)
}

/// Customer information
#[derive(Debug, Clone, Serialize)]
pub struct CustomerInfo {
    pub customer_id: CustomerId,
    pub email: Option<String>,
    pub metadata: std::collections::HashMap<String, String>,
}

/// Invoice PDF result
#[derive(Debug, Clone)]
pub enum InvoicePdfResult {
    /// URL to download the PDF (Stripe)
    Url(String),
    /// PDF bytes generated on-the-fly (Dummy)
    Bytes(Vec<u8>),
    /// No PDF available
    NotAvailable,
}
