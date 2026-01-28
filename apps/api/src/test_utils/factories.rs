//! Test data factories for creating valid test fixtures.
//!
//! Each factory function creates a complete, valid object with sensible defaults.
//! Use the closure parameter to override specific fields as needed.

use chrono::NaiveDateTime;
use uuid::Uuid;

use crate::{
    adapters::persistence::enabled_payment_providers::EnabledPaymentProviderProfile,
    application::use_cases::{
        domain::DomainProfile,
        domain_billing::{
            BillingPaymentProfile, BillingStripeConfigProfile, SubscriptionEventProfile,
            SubscriptionPlanProfile, UserSubscriptionProfile,
        },
    },
    domain::entities::{
        billing_state::BillingState, domain::DomainStatus, payment_mode::PaymentMode,
        payment_provider::PaymentProvider, payment_status::PaymentStatus,
        user_subscription::SubscriptionStatus,
    },
};

/// Create a test domain with sensible defaults.
pub fn create_test_domain(overrides: impl FnOnce(&mut DomainProfile)) -> DomainProfile {
    let mut domain = DomainProfile {
        id: Uuid::new_v4(),
        owner_end_user_id: Some(Uuid::new_v4()),
        domain: "example.com".to_string(),
        status: DomainStatus::Verified,
        active_payment_mode: PaymentMode::Test,
        verification_started_at: None,
        verified_at: Some(test_datetime()),
        created_at: Some(test_datetime()),
        updated_at: Some(test_datetime()),
    };
    overrides(&mut domain);
    domain
}

/// Create a test subscription plan with sensible defaults.
pub fn create_test_plan(
    domain_id: Uuid,
    overrides: impl FnOnce(&mut SubscriptionPlanProfile),
) -> SubscriptionPlanProfile {
    let mut plan = SubscriptionPlanProfile {
        id: Uuid::new_v4(),
        domain_id,
        payment_provider: Some(PaymentProvider::Dummy),
        payment_mode: PaymentMode::Test,
        code: "basic".to_string(),
        name: "Basic Plan".to_string(),
        description: Some("A basic subscription plan".to_string()),
        price_cents: 999,
        currency: "usd".to_string(),
        interval: "monthly".to_string(),
        interval_count: 1,
        trial_days: 0,
        features: vec!["Feature 1".to_string(), "Feature 2".to_string()],
        is_public: true,
        display_order: 0,
        stripe_product_id: Some("prod_test123".to_string()),
        stripe_price_id: Some("price_test123".to_string()),
        is_archived: false,
        archived_at: None,
        created_at: Some(test_datetime()),
        updated_at: Some(test_datetime()),
    };
    overrides(&mut plan);
    plan
}

/// Create a test user subscription with sensible defaults.
pub fn create_test_subscription(
    domain_id: Uuid,
    end_user_id: Uuid,
    plan_id: Uuid,
    overrides: impl FnOnce(&mut UserSubscriptionProfile),
) -> UserSubscriptionProfile {
    let now = test_datetime();
    let period_end = test_datetime_offset_days(30);

    let mut subscription = UserSubscriptionProfile {
        id: Uuid::new_v4(),
        domain_id,
        payment_provider: Some(PaymentProvider::Dummy),
        payment_mode: PaymentMode::Test,
        billing_state: Some(BillingState::Active),
        end_user_id,
        plan_id,
        status: SubscriptionStatus::Active,
        stripe_customer_id: format!("cus_test{}", Uuid::new_v4().simple()),
        stripe_subscription_id: Some(format!("sub_test{}", Uuid::new_v4().simple())),
        current_period_start: Some(now),
        current_period_end: Some(period_end),
        trial_start: None,
        trial_end: None,
        cancel_at_period_end: false,
        canceled_at: None,
        manually_granted: false,
        granted_by: None,
        granted_at: None,
        created_at: Some(now),
        updated_at: Some(now),
    };
    overrides(&mut subscription);
    subscription
}

/// Create a test billing payment with sensible defaults.
pub fn create_test_payment(
    domain_id: Uuid,
    end_user_id: Uuid,
    overrides: impl FnOnce(&mut BillingPaymentProfile),
) -> BillingPaymentProfile {
    let now = test_datetime();

    let mut payment = BillingPaymentProfile {
        id: Uuid::new_v4(),
        domain_id,
        payment_provider: Some(PaymentProvider::Dummy),
        payment_mode: PaymentMode::Test,
        end_user_id,
        subscription_id: None,
        stripe_invoice_id: format!("in_test{}", Uuid::new_v4().simple()),
        stripe_payment_intent_id: Some(format!("pi_test{}", Uuid::new_v4().simple())),
        stripe_customer_id: format!("cus_test{}", Uuid::new_v4().simple()),
        amount_cents: 999,
        amount_paid_cents: 999,
        amount_refunded_cents: 0,
        currency: "usd".to_string(),
        status: PaymentStatus::Paid,
        plan_id: None,
        plan_code: Some("basic".to_string()),
        plan_name: Some("Basic Plan".to_string()),
        hosted_invoice_url: Some("https://invoice.stripe.com/test".to_string()),
        invoice_pdf_url: Some("https://invoice.stripe.com/test.pdf".to_string()),
        invoice_number: Some("INV-0001".to_string()),
        billing_reason: Some("subscription_create".to_string()),
        failure_message: None,
        invoice_created_at: Some(now),
        payment_date: Some(now),
        refunded_at: None,
        created_at: Some(now),
        updated_at: Some(now),
    };
    overrides(&mut payment);
    payment
}

/// Create a test Stripe config with sensible defaults.
pub fn create_test_stripe_config(
    domain_id: Uuid,
    overrides: impl FnOnce(&mut BillingStripeConfigProfile),
) -> BillingStripeConfigProfile {
    let now = test_datetime();

    let mut config = BillingStripeConfigProfile {
        id: Uuid::new_v4(),
        domain_id,
        payment_mode: PaymentMode::Test,
        stripe_secret_key_encrypted: "encrypted_sk_test_xxx".to_string(),
        stripe_publishable_key: "pk_test_xxx".to_string(),
        stripe_webhook_secret_encrypted: "encrypted_whsec_xxx".to_string(),
        created_at: Some(now),
        updated_at: Some(now),
    };
    overrides(&mut config);
    config
}

/// Create a test subscription event with sensible defaults.
pub fn create_test_subscription_event(
    subscription_id: Uuid,
    overrides: impl FnOnce(&mut SubscriptionEventProfile),
) -> SubscriptionEventProfile {
    let now = test_datetime();

    let mut event = SubscriptionEventProfile {
        id: Uuid::new_v4(),
        subscription_id,
        event_type: "subscription.created".to_string(),
        previous_status: None,
        new_status: Some(SubscriptionStatus::Active),
        stripe_event_id: Some(format!("evt_test{}", Uuid::new_v4().simple())),
        metadata: serde_json::json!({}),
        created_by: None,
        created_at: Some(now),
    };
    overrides(&mut event);
    event
}

/// Create a test enabled payment provider with sensible defaults.
pub fn create_test_enabled_provider(
    domain_id: Uuid,
    overrides: impl FnOnce(&mut EnabledPaymentProviderProfile),
) -> EnabledPaymentProviderProfile {
    let now = test_datetime();

    let mut provider = EnabledPaymentProviderProfile {
        id: Uuid::new_v4(),
        domain_id,
        provider: PaymentProvider::Dummy,
        mode: PaymentMode::Test,
        is_active: true,
        display_order: 0,
        created_at: Some(now),
        updated_at: Some(now),
    };
    overrides(&mut provider);
    provider
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Returns a consistent test datetime (2024-01-15 12:00:00 UTC).
fn test_datetime() -> NaiveDateTime {
    NaiveDateTime::parse_from_str("2024-01-15 12:00:00", "%Y-%m-%d %H:%M:%S").unwrap()
}

/// Returns a test datetime offset by the given number of days.
fn test_datetime_offset_days(days: i64) -> NaiveDateTime {
    test_datetime() + chrono::Duration::days(days)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_domain_with_defaults() {
        let domain = create_test_domain(|_| {});
        assert_eq!(domain.domain, "example.com");
        assert_eq!(domain.status, DomainStatus::Verified);
        assert!(domain.owner_end_user_id.is_some());
    }

    #[test]
    fn test_create_domain_with_overrides() {
        let domain = create_test_domain(|d| {
            d.domain = "custom.com".to_string();
            d.status = DomainStatus::PendingDns;
        });
        assert_eq!(domain.domain, "custom.com");
        assert_eq!(domain.status, DomainStatus::PendingDns);
    }

    #[test]
    fn test_create_plan_with_defaults() {
        let domain_id = Uuid::new_v4();
        let plan = create_test_plan(domain_id, |_| {});
        assert_eq!(plan.domain_id, domain_id);
        assert_eq!(plan.code, "basic");
        assert_eq!(plan.price_cents, 999);
    }

    #[test]
    fn test_create_plan_with_overrides() {
        let domain_id = Uuid::new_v4();
        let plan = create_test_plan(domain_id, |p| {
            p.code = "premium".to_string();
            p.price_cents = 2999;
        });
        assert_eq!(plan.code, "premium");
        assert_eq!(plan.price_cents, 2999);
    }

    #[test]
    fn test_create_subscription_with_defaults() {
        let domain_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let plan_id = Uuid::new_v4();
        let sub = create_test_subscription(domain_id, user_id, plan_id, |_| {});
        assert_eq!(sub.domain_id, domain_id);
        assert_eq!(sub.end_user_id, user_id);
        assert_eq!(sub.plan_id, plan_id);
        assert_eq!(sub.status, SubscriptionStatus::Active);
    }

    #[test]
    fn test_create_payment_with_defaults() {
        let domain_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let payment = create_test_payment(domain_id, user_id, |_| {});
        assert_eq!(payment.domain_id, domain_id);
        assert_eq!(payment.end_user_id, user_id);
        assert_eq!(payment.status, PaymentStatus::Paid);
    }
}
