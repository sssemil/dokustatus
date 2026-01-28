//! In-memory mock implementations for billing-related repository traits.

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Mutex;
use uuid::Uuid;

use crate::{
    adapters::persistence::enabled_payment_providers::{
        EnabledPaymentProviderProfile, EnabledPaymentProvidersRepoTrait,
    },
    app_error::{AppError, AppResult},
    application::use_cases::domain_billing::{
        BillingPaymentProfile, BillingPaymentRepoTrait, BillingPaymentWithUser,
        BillingStripeConfigProfile, BillingStripeConfigRepoTrait, CreatePaymentInput,
        CreatePlanInput, CreateSubscriptionEventInput, CreateSubscriptionInput, PaginatedPayments,
        PaymentListFilters, PaymentSummary, StripeSubscriptionUpdate, SubscriptionEventProfile,
        SubscriptionEventRepoTrait, SubscriptionPlanProfile, SubscriptionPlanRepoTrait,
        UpdatePlanInput, UserSubscriptionProfile, UserSubscriptionRepoTrait,
        UserSubscriptionWithPlan,
    },
    domain::entities::{
        payment_mode::PaymentMode, payment_provider::PaymentProvider,
        payment_status::PaymentStatus, user_subscription::SubscriptionStatus,
    },
};

use chrono::NaiveDateTime;

// ============================================================================
// InMemoryBillingStripeConfigRepo
// ============================================================================

#[derive(Default)]
pub struct InMemoryBillingStripeConfigRepo {
    pub configs: Mutex<HashMap<(Uuid, PaymentMode), BillingStripeConfigProfile>>,
}

impl InMemoryBillingStripeConfigRepo {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_configs(configs: Vec<BillingStripeConfigProfile>) -> Self {
        let map: HashMap<(Uuid, PaymentMode), BillingStripeConfigProfile> = configs
            .into_iter()
            .map(|c| ((c.domain_id, c.payment_mode), c))
            .collect();
        Self {
            configs: Mutex::new(map),
        }
    }
}

#[async_trait]
impl BillingStripeConfigRepoTrait for InMemoryBillingStripeConfigRepo {
    async fn get_by_domain_and_mode(
        &self,
        domain_id: Uuid,
        mode: PaymentMode,
    ) -> AppResult<Option<BillingStripeConfigProfile>> {
        Ok(self
            .configs
            .lock()
            .unwrap()
            .get(&(domain_id, mode))
            .cloned())
    }

    async fn list_by_domain(&self, domain_id: Uuid) -> AppResult<Vec<BillingStripeConfigProfile>> {
        Ok(self
            .configs
            .lock()
            .unwrap()
            .values()
            .filter(|c| c.domain_id == domain_id)
            .cloned()
            .collect())
    }

    async fn upsert(
        &self,
        domain_id: Uuid,
        mode: PaymentMode,
        stripe_secret_key_encrypted: &str,
        stripe_publishable_key: &str,
        stripe_webhook_secret_encrypted: &str,
    ) -> AppResult<BillingStripeConfigProfile> {
        let mut configs = self.configs.lock().unwrap();
        let now = chrono::Utc::now().naive_utc();

        let config = BillingStripeConfigProfile {
            id: configs
                .get(&(domain_id, mode))
                .map(|c| c.id)
                .unwrap_or_else(Uuid::new_v4),
            domain_id,
            payment_mode: mode,
            stripe_secret_key_encrypted: stripe_secret_key_encrypted.to_string(),
            stripe_publishable_key: stripe_publishable_key.to_string(),
            stripe_webhook_secret_encrypted: stripe_webhook_secret_encrypted.to_string(),
            created_at: configs
                .get(&(domain_id, mode))
                .and_then(|c| c.created_at)
                .or(Some(now)),
            updated_at: Some(now),
        };

        configs.insert((domain_id, mode), config.clone());
        Ok(config)
    }

    async fn delete(&self, domain_id: Uuid, mode: PaymentMode) -> AppResult<()> {
        self.configs.lock().unwrap().remove(&(domain_id, mode));
        Ok(())
    }

    async fn has_any_config(&self, domain_id: Uuid) -> AppResult<bool> {
        Ok(self
            .configs
            .lock()
            .unwrap()
            .keys()
            .any(|(d, _)| *d == domain_id))
    }
}

// ============================================================================
// InMemorySubscriptionPlanRepo
// ============================================================================

#[derive(Default)]
pub struct InMemorySubscriptionPlanRepo {
    pub plans: Mutex<HashMap<Uuid, SubscriptionPlanProfile>>,
    // Track subscribers per plan for count_subscribers
    pub subscriber_counts: Mutex<HashMap<Uuid, i64>>,
}

impl InMemorySubscriptionPlanRepo {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_plans(plans: Vec<SubscriptionPlanProfile>) -> Self {
        let map: HashMap<Uuid, SubscriptionPlanProfile> =
            plans.into_iter().map(|p| (p.id, p)).collect();
        Self {
            plans: Mutex::new(map),
            subscriber_counts: Mutex::new(HashMap::new()),
        }
    }

    /// Set the subscriber count for a plan (for testing).
    pub fn set_subscriber_count(&self, plan_id: Uuid, count: i64) {
        self.subscriber_counts
            .lock()
            .unwrap()
            .insert(plan_id, count);
    }
}

#[async_trait]
impl SubscriptionPlanRepoTrait for InMemorySubscriptionPlanRepo {
    async fn get_by_id(&self, id: Uuid) -> AppResult<Option<SubscriptionPlanProfile>> {
        Ok(self.plans.lock().unwrap().get(&id).cloned())
    }

    async fn get_by_domain_and_code(
        &self,
        domain_id: Uuid,
        mode: PaymentMode,
        code: &str,
    ) -> AppResult<Option<SubscriptionPlanProfile>> {
        Ok(self
            .plans
            .lock()
            .unwrap()
            .values()
            .find(|p| p.domain_id == domain_id && p.payment_mode == mode && p.code == code)
            .cloned())
    }

    async fn list_by_domain_and_mode(
        &self,
        domain_id: Uuid,
        mode: PaymentMode,
        include_archived: bool,
    ) -> AppResult<Vec<SubscriptionPlanProfile>> {
        let plans = self.plans.lock().unwrap();
        let mut result: Vec<_> = plans
            .values()
            .filter(|p| {
                p.domain_id == domain_id
                    && p.payment_mode == mode
                    && (include_archived || !p.is_archived)
            })
            .cloned()
            .collect();
        result.sort_by(|a, b| a.display_order.cmp(&b.display_order));
        Ok(result)
    }

    async fn list_public_by_domain_and_mode(
        &self,
        domain_id: Uuid,
        mode: PaymentMode,
    ) -> AppResult<Vec<SubscriptionPlanProfile>> {
        let plans = self.plans.lock().unwrap();
        let mut result: Vec<_> = plans
            .values()
            .filter(|p| {
                p.domain_id == domain_id && p.payment_mode == mode && p.is_public && !p.is_archived
            })
            .cloned()
            .collect();
        result.sort_by(|a, b| a.display_order.cmp(&b.display_order));
        Ok(result)
    }

    async fn create(
        &self,
        domain_id: Uuid,
        mode: PaymentMode,
        input: &CreatePlanInput,
    ) -> AppResult<SubscriptionPlanProfile> {
        let mut plans = self.plans.lock().unwrap();
        let now = chrono::Utc::now().naive_utc();

        // Calculate next display order
        let max_order = plans
            .values()
            .filter(|p| p.domain_id == domain_id && p.payment_mode == mode)
            .map(|p| p.display_order)
            .max()
            .unwrap_or(-1);

        let plan = SubscriptionPlanProfile {
            id: Uuid::new_v4(),
            domain_id,
            payment_provider: None, // Set when synced with payment provider
            payment_mode: mode,
            code: input.code.clone(),
            name: input.name.clone(),
            description: input.description.clone(),
            price_cents: input.price_cents,
            currency: input.currency.clone(),
            interval: input.interval.clone(),
            interval_count: input.interval_count,
            trial_days: input.trial_days,
            features: input.features.clone(),
            is_public: input.is_public,
            display_order: max_order + 1,
            stripe_product_id: None,
            stripe_price_id: None,
            is_archived: false,
            archived_at: None,
            created_at: Some(now),
            updated_at: Some(now),
        };

        plans.insert(plan.id, plan.clone());
        Ok(plan)
    }

    async fn update(
        &self,
        id: Uuid,
        input: &UpdatePlanInput,
    ) -> AppResult<SubscriptionPlanProfile> {
        let mut plans = self.plans.lock().unwrap();
        let plan = plans.get_mut(&id).ok_or(AppError::NotFound)?;

        if let Some(ref name) = input.name {
            plan.name = name.clone();
        }
        if let Some(ref description) = input.description {
            plan.description = Some(description.clone());
        }
        if let Some(price_cents) = input.price_cents {
            plan.price_cents = price_cents;
        }
        if let Some(ref interval) = input.interval {
            plan.interval = interval.clone();
        }
        if let Some(interval_count) = input.interval_count {
            plan.interval_count = interval_count;
        }
        if let Some(trial_days) = input.trial_days {
            plan.trial_days = trial_days;
        }
        if let Some(ref features) = input.features {
            plan.features = features.clone();
        }
        if let Some(is_public) = input.is_public {
            plan.is_public = is_public;
        }
        plan.updated_at = Some(chrono::Utc::now().naive_utc());

        Ok(plan.clone())
    }

    async fn set_stripe_ids(&self, id: Uuid, product_id: &str, price_id: &str) -> AppResult<()> {
        let mut plans = self.plans.lock().unwrap();
        let plan = plans.get_mut(&id).ok_or(AppError::NotFound)?;
        plan.stripe_product_id = Some(product_id.to_string());
        plan.stripe_price_id = Some(price_id.to_string());
        plan.updated_at = Some(chrono::Utc::now().naive_utc());
        Ok(())
    }

    async fn set_display_order(&self, id: Uuid, order: i32) -> AppResult<()> {
        let mut plans = self.plans.lock().unwrap();
        let plan = plans.get_mut(&id).ok_or(AppError::NotFound)?;
        plan.display_order = order;
        plan.updated_at = Some(chrono::Utc::now().naive_utc());
        Ok(())
    }

    async fn archive(&self, id: Uuid) -> AppResult<()> {
        let mut plans = self.plans.lock().unwrap();
        let plan = plans.get_mut(&id).ok_or(AppError::NotFound)?;
        plan.is_archived = true;
        plan.archived_at = Some(chrono::Utc::now().naive_utc());
        plan.updated_at = Some(chrono::Utc::now().naive_utc());
        Ok(())
    }

    async fn delete(&self, id: Uuid) -> AppResult<()> {
        self.plans.lock().unwrap().remove(&id);
        Ok(())
    }

    async fn count_subscribers(&self, plan_id: Uuid) -> AppResult<i64> {
        Ok(*self
            .subscriber_counts
            .lock()
            .unwrap()
            .get(&plan_id)
            .unwrap_or(&0))
    }

    async fn count_by_domain_and_mode(&self, domain_id: Uuid, mode: PaymentMode) -> AppResult<i64> {
        Ok(self
            .plans
            .lock()
            .unwrap()
            .values()
            .filter(|p| p.domain_id == domain_id && p.payment_mode == mode)
            .count() as i64)
    }

    async fn get_by_stripe_price_id(
        &self,
        domain_id: Uuid,
        mode: PaymentMode,
        stripe_price_id: &str,
    ) -> AppResult<Option<SubscriptionPlanProfile>> {
        Ok(self
            .plans
            .lock()
            .unwrap()
            .values()
            .find(|p| {
                p.domain_id == domain_id
                    && p.payment_mode == mode
                    && p.stripe_price_id.as_deref() == Some(stripe_price_id)
            })
            .cloned())
    }
}

// ============================================================================
// InMemoryUserSubscriptionRepo
// ============================================================================

#[derive(Default)]
pub struct InMemoryUserSubscriptionRepo {
    pub subscriptions: Mutex<HashMap<Uuid, UserSubscriptionProfile>>,
    // For list_by_domain_and_mode, we need plan data. Store plan references.
    pub plan_repo: Option<std::sync::Arc<InMemorySubscriptionPlanRepo>>,
    // User emails for UserSubscriptionWithPlan
    pub user_emails: Mutex<HashMap<Uuid, String>>,
}

impl InMemoryUserSubscriptionRepo {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_subscriptions(subscriptions: Vec<UserSubscriptionProfile>) -> Self {
        let map: HashMap<Uuid, UserSubscriptionProfile> =
            subscriptions.into_iter().map(|s| (s.id, s)).collect();
        Self {
            subscriptions: Mutex::new(map),
            plan_repo: None,
            user_emails: Mutex::new(HashMap::new()),
        }
    }

    /// Link to plan repo for list operations that need plan data.
    pub fn with_plan_repo(
        mut self,
        plan_repo: std::sync::Arc<InMemorySubscriptionPlanRepo>,
    ) -> Self {
        self.plan_repo = Some(plan_repo);
        self
    }

    /// Set user email for a user (used in list operations).
    pub fn set_user_email(&self, user_id: Uuid, email: &str) {
        self.user_emails
            .lock()
            .unwrap()
            .insert(user_id, email.to_string());
    }
}

#[async_trait]
impl UserSubscriptionRepoTrait for InMemoryUserSubscriptionRepo {
    async fn get_by_id(&self, id: Uuid) -> AppResult<Option<UserSubscriptionProfile>> {
        Ok(self.subscriptions.lock().unwrap().get(&id).cloned())
    }

    async fn get_by_user_and_mode(
        &self,
        domain_id: Uuid,
        mode: PaymentMode,
        end_user_id: Uuid,
    ) -> AppResult<Option<UserSubscriptionProfile>> {
        Ok(self
            .subscriptions
            .lock()
            .unwrap()
            .values()
            .find(|s| {
                s.domain_id == domain_id && s.payment_mode == mode && s.end_user_id == end_user_id
            })
            .cloned())
    }

    async fn get_by_stripe_subscription_id(
        &self,
        stripe_subscription_id: &str,
    ) -> AppResult<Option<UserSubscriptionProfile>> {
        Ok(self
            .subscriptions
            .lock()
            .unwrap()
            .values()
            .find(|s| s.stripe_subscription_id.as_deref() == Some(stripe_subscription_id))
            .cloned())
    }

    async fn get_by_stripe_customer_id(
        &self,
        domain_id: Uuid,
        mode: PaymentMode,
        stripe_customer_id: &str,
    ) -> AppResult<Option<UserSubscriptionProfile>> {
        Ok(self
            .subscriptions
            .lock()
            .unwrap()
            .values()
            .find(|s| {
                s.domain_id == domain_id
                    && s.payment_mode == mode
                    && s.stripe_customer_id == stripe_customer_id
            })
            .cloned())
    }

    async fn list_by_domain_and_mode(
        &self,
        domain_id: Uuid,
        mode: PaymentMode,
    ) -> AppResult<Vec<UserSubscriptionWithPlan>> {
        // Collect data from locks first (without holding across await)
        let (filtered_subs, emails_map) = {
            let subs = self.subscriptions.lock().unwrap();
            let emails = self.user_emails.lock().unwrap();

            let filtered: Vec<_> = subs
                .values()
                .filter(|s| s.domain_id == domain_id && s.payment_mode == mode)
                .cloned()
                .collect();

            let emails_map: HashMap<Uuid, String> = emails.clone();
            (filtered, emails_map)
        };

        // Now build result with async plan fetches
        let mut result = Vec::new();
        for sub in filtered_subs {
            // Create fallback plan that honors the subscription's payment_mode
            let fallback_plan = || {
                crate::test_utils::create_test_plan(domain_id, |p| {
                    p.id = sub.plan_id;
                    p.payment_mode = sub.payment_mode;
                })
            };

            let plan = if let Some(ref plan_repo) = self.plan_repo {
                plan_repo
                    .get_by_id(sub.plan_id)
                    .await?
                    .unwrap_or_else(fallback_plan)
            } else {
                fallback_plan()
            };

            let user_email = emails_map
                .get(&sub.end_user_id)
                .cloned()
                .unwrap_or_else(|| "test@example.com".to_string());

            result.push(UserSubscriptionWithPlan {
                subscription: sub,
                plan,
                user_email,
            });
        }

        result.sort_by(|a, b| b.subscription.created_at.cmp(&a.subscription.created_at));
        Ok(result)
    }

    async fn list_by_plan(&self, plan_id: Uuid) -> AppResult<Vec<UserSubscriptionProfile>> {
        Ok(self
            .subscriptions
            .lock()
            .unwrap()
            .values()
            .filter(|s| s.plan_id == plan_id)
            .cloned()
            .collect())
    }

    async fn create(&self, input: &CreateSubscriptionInput) -> AppResult<UserSubscriptionProfile> {
        let mut subs = self.subscriptions.lock().unwrap();
        let now = chrono::Utc::now().naive_utc();

        // Enforce: one active subscription per user per domain per mode
        let existing = subs.values().find(|s| {
            s.domain_id == input.domain_id
                && s.payment_mode == input.payment_mode
                && s.end_user_id == input.end_user_id
        });
        if existing.is_some() {
            return Err(AppError::InvalidInput(
                "Subscription already exists for this user".into(),
            ));
        }

        let sub = UserSubscriptionProfile {
            id: Uuid::new_v4(),
            domain_id: input.domain_id,
            payment_provider: None,
            payment_mode: input.payment_mode,
            billing_state: None,
            end_user_id: input.end_user_id,
            plan_id: input.plan_id,
            status: input.status,
            stripe_customer_id: input.stripe_customer_id.clone(),
            stripe_subscription_id: input.stripe_subscription_id.clone(),
            current_period_start: input.current_period_start,
            current_period_end: input.current_period_end,
            trial_start: input.trial_start,
            trial_end: input.trial_end,
            cancel_at_period_end: false,
            canceled_at: None,
            manually_granted: false,
            granted_by: None,
            granted_at: None,
            created_at: Some(now),
            updated_at: Some(now),
            changes_this_period: 0,
            period_changes_reset_at: None,
        };

        subs.insert(sub.id, sub.clone());
        Ok(sub)
    }

    async fn update_from_stripe(
        &self,
        id: Uuid,
        update: &StripeSubscriptionUpdate,
    ) -> AppResult<UserSubscriptionProfile> {
        let mut subs = self.subscriptions.lock().unwrap();
        let sub = subs.get_mut(&id).ok_or(AppError::NotFound)?;

        sub.status = update.status;
        if let Some(plan_id) = update.plan_id {
            sub.plan_id = plan_id;
        }
        if let Some(ref stripe_sub_id) = update.stripe_subscription_id {
            sub.stripe_subscription_id = Some(stripe_sub_id.clone());
        }
        sub.current_period_start = update.current_period_start;
        sub.current_period_end = update.current_period_end;
        sub.cancel_at_period_end = update.cancel_at_period_end;
        sub.canceled_at = update.canceled_at;
        sub.trial_start = update.trial_start;
        sub.trial_end = update.trial_end;
        sub.updated_at = Some(chrono::Utc::now().naive_utc());

        Ok(sub.clone())
    }

    async fn update_plan(&self, id: Uuid, plan_id: Uuid) -> AppResult<()> {
        let mut subs = self.subscriptions.lock().unwrap();
        let sub = subs.get_mut(&id).ok_or(AppError::NotFound)?;
        sub.plan_id = plan_id;
        sub.updated_at = Some(chrono::Utc::now().naive_utc());
        Ok(())
    }

    async fn grant_manually(
        &self,
        domain_id: Uuid,
        mode: PaymentMode,
        end_user_id: Uuid,
        plan_id: Uuid,
        granted_by: Uuid,
        stripe_customer_id: &str,
    ) -> AppResult<UserSubscriptionProfile> {
        let mut subs = self.subscriptions.lock().unwrap();
        let now = chrono::Utc::now().naive_utc();

        // Upsert behavior: update if exists, create if not
        let existing = subs.values_mut().find(|s| {
            s.domain_id == domain_id && s.payment_mode == mode && s.end_user_id == end_user_id
        });

        if let Some(sub) = existing {
            sub.plan_id = plan_id;
            sub.status = SubscriptionStatus::Active;
            sub.manually_granted = true;
            sub.granted_by = Some(granted_by);
            sub.granted_at = Some(now);
            sub.cancel_at_period_end = false;
            sub.canceled_at = None;
            sub.updated_at = Some(now);
            return Ok(sub.clone());
        }

        let sub = UserSubscriptionProfile {
            id: Uuid::new_v4(),
            domain_id,
            payment_provider: None,
            payment_mode: mode,
            billing_state: None,
            end_user_id,
            plan_id,
            status: SubscriptionStatus::Active,
            stripe_customer_id: stripe_customer_id.to_string(),
            stripe_subscription_id: None,
            current_period_start: None,
            current_period_end: None,
            trial_start: None,
            trial_end: None,
            cancel_at_period_end: false,
            canceled_at: None,
            manually_granted: true,
            granted_by: Some(granted_by),
            granted_at: Some(now),
            created_at: Some(now),
            updated_at: Some(now),
            changes_this_period: 0,
            period_changes_reset_at: None,
        };

        subs.insert(sub.id, sub.clone());
        Ok(sub)
    }

    async fn revoke(&self, id: Uuid) -> AppResult<()> {
        let mut subs = self.subscriptions.lock().unwrap();
        let sub = subs.get_mut(&id).ok_or(AppError::NotFound)?;
        sub.status = SubscriptionStatus::Canceled;
        sub.canceled_at = Some(chrono::Utc::now().naive_utc());
        sub.updated_at = Some(chrono::Utc::now().naive_utc());
        Ok(())
    }

    async fn delete(&self, id: Uuid) -> AppResult<()> {
        self.subscriptions.lock().unwrap().remove(&id);
        Ok(())
    }

    async fn increment_changes_counter(
        &self,
        id: Uuid,
        period_end: chrono::DateTime<chrono::Utc>,
        max_changes: i32,
    ) -> AppResult<bool> {
        let mut subs = self.subscriptions.lock().unwrap();
        let sub = subs.get_mut(&id).ok_or(AppError::NotFound)?;

        // Atomic check-and-increment matching DB behavior
        let now = chrono::Utc::now();
        let period_has_reset = sub.period_changes_reset_at.is_none()
            || sub.period_changes_reset_at.map(|r| r < now).unwrap_or(true);

        if period_has_reset {
            // Period reset - allow and set counter to 1
            sub.changes_this_period = 1;
            sub.period_changes_reset_at = Some(period_end);
            Ok(true)
        } else if sub.changes_this_period < max_changes {
            // Under limit - allow and increment
            sub.changes_this_period += 1;
            sub.period_changes_reset_at = Some(period_end);
            Ok(true)
        } else {
            // Rate limit exceeded
            Ok(false)
        }
    }

    async fn count_active_by_domain_and_mode(
        &self,
        domain_id: Uuid,
        mode: PaymentMode,
    ) -> AppResult<i64> {
        Ok(self
            .subscriptions
            .lock()
            .unwrap()
            .values()
            .filter(|s| {
                s.domain_id == domain_id
                    && s.payment_mode == mode
                    && (s.status == SubscriptionStatus::Active
                        || s.status == SubscriptionStatus::Trialing)
            })
            .count() as i64)
    }

    async fn count_by_status_and_mode(
        &self,
        domain_id: Uuid,
        mode: PaymentMode,
        status: SubscriptionStatus,
    ) -> AppResult<i64> {
        Ok(self
            .subscriptions
            .lock()
            .unwrap()
            .values()
            .filter(|s| s.domain_id == domain_id && s.payment_mode == mode && s.status == status)
            .count() as i64)
    }

    async fn count_by_domain_and_mode(&self, domain_id: Uuid, mode: PaymentMode) -> AppResult<i64> {
        Ok(self
            .subscriptions
            .lock()
            .unwrap()
            .values()
            .filter(|s| s.domain_id == domain_id && s.payment_mode == mode)
            .count() as i64)
    }
}

// ============================================================================
// InMemorySubscriptionEventRepo
// ============================================================================

#[derive(Default)]
pub struct InMemorySubscriptionEventRepo {
    pub events: Mutex<Vec<SubscriptionEventProfile>>,
}

impl InMemorySubscriptionEventRepo {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_events(events: Vec<SubscriptionEventProfile>) -> Self {
        Self {
            events: Mutex::new(events),
        }
    }

    /// Get all events (for test assertions).
    pub fn get_all(&self) -> Vec<SubscriptionEventProfile> {
        self.events.lock().unwrap().clone()
    }
}

#[async_trait]
impl SubscriptionEventRepoTrait for InMemorySubscriptionEventRepo {
    async fn create(&self, input: &CreateSubscriptionEventInput) -> AppResult<()> {
        let mut events = self.events.lock().unwrap();
        let now = chrono::Utc::now().naive_utc();

        let event = SubscriptionEventProfile {
            id: Uuid::new_v4(),
            subscription_id: input.subscription_id,
            event_type: input.event_type.clone(),
            previous_status: input.previous_status,
            new_status: input.new_status,
            stripe_event_id: input.stripe_event_id.clone(),
            metadata: input.metadata.clone(),
            created_by: input.created_by,
            created_at: Some(now),
        };

        events.push(event);
        Ok(())
    }

    async fn list_by_subscription(
        &self,
        subscription_id: Uuid,
    ) -> AppResult<Vec<SubscriptionEventProfile>> {
        Ok(self
            .events
            .lock()
            .unwrap()
            .iter()
            .filter(|e| e.subscription_id == subscription_id)
            .cloned()
            .collect())
    }

    async fn exists_by_stripe_event_id(&self, stripe_event_id: &str) -> AppResult<bool> {
        Ok(self
            .events
            .lock()
            .unwrap()
            .iter()
            .any(|e| e.stripe_event_id.as_deref() == Some(stripe_event_id)))
    }
}

// ============================================================================
// InMemoryBillingPaymentRepo
// ============================================================================

/// Composite key for billing payments matching Postgres unique constraint:
/// (domain_id, payment_mode, stripe_invoice_id)
type PaymentKey = (Uuid, PaymentMode, String);

#[derive(Default)]
pub struct InMemoryBillingPaymentRepo {
    pub payments: Mutex<HashMap<PaymentKey, BillingPaymentProfile>>,
    pub user_emails: Mutex<HashMap<Uuid, String>>,
}

impl InMemoryBillingPaymentRepo {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a payment key matching the Postgres unique constraint
    fn make_key(domain_id: Uuid, mode: PaymentMode, stripe_invoice_id: &str) -> PaymentKey {
        (domain_id, mode, stripe_invoice_id.to_string())
    }

    pub fn with_payments(payments: Vec<BillingPaymentProfile>) -> Self {
        let map: HashMap<PaymentKey, BillingPaymentProfile> = payments
            .into_iter()
            .map(|p| {
                let key = Self::make_key(p.domain_id, p.payment_mode, &p.stripe_invoice_id);
                (key, p)
            })
            .collect();
        Self {
            payments: Mutex::new(map),
            user_emails: Mutex::new(HashMap::new()),
        }
    }

    pub fn set_user_email(&self, user_id: Uuid, email: &str) {
        self.user_emails
            .lock()
            .unwrap()
            .insert(user_id, email.to_string());
    }
}

#[async_trait]
impl BillingPaymentRepoTrait for InMemoryBillingPaymentRepo {
    async fn upsert_from_stripe(
        &self,
        input: &CreatePaymentInput,
    ) -> AppResult<BillingPaymentProfile> {
        let mut payments = self.payments.lock().unwrap();
        let now = chrono::Utc::now().naive_utc();

        // Use composite key matching Postgres unique constraint
        let key = Self::make_key(
            input.domain_id,
            input.payment_mode,
            &input.stripe_invoice_id,
        );
        let existing = payments.get(&key);
        let id = existing.map(|p| p.id).unwrap_or_else(Uuid::new_v4);
        let created_at = existing.and_then(|p| p.created_at).or(Some(now));

        // Preserve terminal status if already set (matches Postgres ON CONFLICT behavior)
        // Terminal statuses: paid, refunded, partial_refund, void
        let status = if let Some(existing_payment) = existing {
            if existing_payment.status.is_terminal() {
                existing_payment.status
            } else {
                input.status
            }
        } else {
            input.status
        };

        // Preserve existing values using COALESCE-like logic (matches Postgres ON CONFLICT)
        // Fields with COALESCE in Postgres: prefer new value if Some, else keep existing
        let stripe_payment_intent_id = input
            .stripe_payment_intent_id
            .clone()
            .or_else(|| existing.and_then(|p| p.stripe_payment_intent_id.clone()));
        let hosted_invoice_url = input
            .hosted_invoice_url
            .clone()
            .or_else(|| existing.and_then(|p| p.hosted_invoice_url.clone()));
        let invoice_pdf_url = input
            .invoice_pdf_url
            .clone()
            .or_else(|| existing.and_then(|p| p.invoice_pdf_url.clone()));
        let invoice_number = input
            .invoice_number
            .clone()
            .or_else(|| existing.and_then(|p| p.invoice_number.clone()));
        let billing_reason = input
            .billing_reason
            .clone()
            .or_else(|| existing.and_then(|p| p.billing_reason.clone()));
        let payment_date = input
            .payment_date
            .or_else(|| existing.and_then(|p| p.payment_date));

        // Fields NOT in Postgres ON CONFLICT UPDATE clause - preserve existing on conflict
        // Important: If there's an existing record, keep its values even if NULL (don't fall back to input)
        // Only use input values for new records (when existing is None)
        // Note: end_user_id is also NOT in the UPDATE clause, so it should be preserved
        let (
            end_user_id,
            stripe_customer_id,
            subscription_id,
            plan_id,
            plan_code,
            plan_name,
            failure_message,
            invoice_created_at,
            currency,
        ) = if let Some(ex) = existing {
            // Existing record: preserve all these fields exactly as they are (even if NULL)
            (
                ex.end_user_id,
                ex.stripe_customer_id.clone(),
                ex.subscription_id,
                ex.plan_id,
                ex.plan_code.clone(),
                ex.plan_name.clone(),
                ex.failure_message.clone(),
                ex.invoice_created_at,
                ex.currency.clone(),
            )
        } else {
            // New record: use input values
            (
                input.end_user_id,
                input.stripe_customer_id.clone(),
                input.subscription_id,
                input.plan_id,
                input.plan_code.clone(),
                input.plan_name.clone(),
                input.failure_message.clone(),
                input.invoice_created_at,
                input.currency.clone(),
            )
        };

        // Refund fields are never set by upsert, only by update_status
        let amount_refunded_cents = existing.map(|p| p.amount_refunded_cents).unwrap_or(0);
        let refunded_at = existing.and_then(|p| p.refunded_at);

        let payment = BillingPaymentProfile {
            id,
            domain_id: input.domain_id,
            payment_provider: None,
            payment_mode: input.payment_mode,
            end_user_id,
            subscription_id,
            stripe_invoice_id: input.stripe_invoice_id.clone(),
            stripe_payment_intent_id,
            stripe_customer_id,
            amount_cents: input.amount_cents,
            amount_paid_cents: input.amount_paid_cents,
            amount_refunded_cents,
            currency,
            status,
            plan_id,
            plan_code,
            plan_name,
            hosted_invoice_url,
            invoice_pdf_url,
            invoice_number,
            billing_reason,
            failure_message,
            invoice_created_at,
            payment_date,
            refunded_at,
            created_at,
            updated_at: Some(now),
        };

        payments.insert(key, payment.clone());
        Ok(payment)
    }

    async fn get_by_stripe_invoice_id(
        &self,
        stripe_invoice_id: &str,
    ) -> AppResult<Option<BillingPaymentProfile>> {
        // Search by stripe_invoice_id across all domain/mode combinations
        // (This matches how Postgres would use an index on stripe_invoice_id)
        Ok(self
            .payments
            .lock()
            .unwrap()
            .values()
            .find(|p| p.stripe_invoice_id == stripe_invoice_id)
            .cloned())
    }

    async fn list_by_user(
        &self,
        domain_id: Uuid,
        mode: PaymentMode,
        end_user_id: Uuid,
        page: i32,
        per_page: i32,
    ) -> AppResult<PaginatedPayments> {
        // Validate pagination params to prevent underflow/overflow
        let page = std::cmp::max(1, page);
        let per_page = std::cmp::max(1, per_page);

        let payments = self.payments.lock().unwrap();
        let emails = self.user_emails.lock().unwrap();

        let mut filtered: Vec<_> = payments
            .values()
            .filter(|p| {
                p.domain_id == domain_id && p.payment_mode == mode && p.end_user_id == end_user_id
            })
            .cloned()
            .collect();

        // Sort by payment_date DESC NULLS LAST, then by created_at DESC (matches Postgres)
        filtered.sort_by(|a, b| match (b.payment_date, a.payment_date) {
            (Some(b_date), Some(a_date)) => b_date.cmp(&a_date),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => b.created_at.cmp(&a.created_at),
        });

        let total = filtered.len() as i64;
        let total_pages = ((total as f64) / (per_page as f64)).ceil() as i32;

        let start = ((page - 1) * per_page) as usize;
        let end = std::cmp::min(start + per_page as usize, filtered.len());

        let page_payments: Vec<BillingPaymentWithUser> = filtered
            .get(start..end)
            .unwrap_or(&[])
            .iter()
            .map(|p| BillingPaymentWithUser {
                payment: p.clone(),
                user_email: emails
                    .get(&p.end_user_id)
                    .cloned()
                    .unwrap_or_else(|| "test@example.com".to_string()),
            })
            .collect();

        Ok(PaginatedPayments {
            payments: page_payments,
            total,
            page,
            per_page,
            total_pages,
        })
    }

    async fn list_by_domain(
        &self,
        domain_id: Uuid,
        mode: PaymentMode,
        filters: &PaymentListFilters,
        page: i32,
        per_page: i32,
    ) -> AppResult<PaginatedPayments> {
        // Validate pagination params to prevent underflow/overflow
        let page = std::cmp::max(1, page);
        let per_page = std::cmp::max(1, per_page);

        let payments = self.payments.lock().unwrap();
        let emails = self.user_emails.lock().unwrap();

        // Apply filters (matches Postgres push_payment_filters behavior)
        let mut filtered: Vec<_> = payments
            .values()
            .filter(|p| {
                if p.domain_id != domain_id || p.payment_mode != mode {
                    return false;
                }
                // Status filter
                if let Some(status) = &filters.status {
                    if p.status != *status {
                        return false;
                    }
                }
                // Date range filters (payment_date OR created_at)
                if let Some(date_from) = &filters.date_from {
                    let in_range = p.payment_date.map(|d| d >= *date_from).unwrap_or(false)
                        || p.created_at.map(|d| d >= *date_from).unwrap_or(false);
                    if !in_range {
                        return false;
                    }
                }
                if let Some(date_to) = &filters.date_to {
                    let in_range = p.payment_date.map(|d| d <= *date_to).unwrap_or(false)
                        || p.created_at.map(|d| d <= *date_to).unwrap_or(false);
                    if !in_range {
                        return false;
                    }
                }
                // Plan code filter
                if let Some(plan_code) = &filters.plan_code {
                    if p.plan_code.as_ref() != Some(plan_code) {
                        return false;
                    }
                }
                // User email filter (ILIKE %email%)
                if let Some(user_email_filter) = &filters.user_email {
                    let user_email = emails.get(&p.end_user_id).cloned().unwrap_or_default();
                    if !user_email
                        .to_lowercase()
                        .contains(&user_email_filter.to_lowercase())
                    {
                        return false;
                    }
                }
                true
            })
            .cloned()
            .collect();

        // Sort by payment_date DESC NULLS LAST, then by created_at DESC (matches Postgres)
        filtered.sort_by(|a, b| match (b.payment_date, a.payment_date) {
            (Some(b_date), Some(a_date)) => b_date.cmp(&a_date),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => b.created_at.cmp(&a.created_at),
        });

        let total = filtered.len() as i64;
        let total_pages = ((total as f64) / (per_page as f64)).ceil() as i32;

        let start = ((page - 1) * per_page) as usize;
        let end = std::cmp::min(start + per_page as usize, filtered.len());

        let page_payments: Vec<BillingPaymentWithUser> = filtered
            .get(start..end)
            .unwrap_or(&[])
            .iter()
            .map(|p| BillingPaymentWithUser {
                payment: p.clone(),
                user_email: emails
                    .get(&p.end_user_id)
                    .cloned()
                    .unwrap_or_else(|| "test@example.com".to_string()),
            })
            .collect();

        Ok(PaginatedPayments {
            payments: page_payments,
            total,
            page,
            per_page,
            total_pages,
        })
    }

    async fn update_status(
        &self,
        stripe_invoice_id: &str,
        status: PaymentStatus,
        amount_refunded_cents: Option<i32>,
        failure_message: Option<String>,
    ) -> AppResult<()> {
        let mut payments = self.payments.lock().unwrap();

        // Find the payment by stripe_invoice_id (search across all domain/mode combinations)
        // Also find the key so we can update in place
        let key = payments
            .iter()
            .find(|(_, p)| p.stripe_invoice_id == stripe_invoice_id)
            .map(|(k, _)| k.clone());

        let Some(key) = key else {
            // Postgres returns Ok(()) and logs when invoice not found
            // (see billing_payment.rs:377-397)
            return Ok(());
        };

        let payment = payments.get_mut(&key).unwrap();

        // Terminal state transition rules (matches Postgres behavior):
        // - 'refunded' and 'void' are fully terminal, never update
        // - 'paid' can only transition to 'refunded' or 'partial_refund'
        // - 'partial_refund' can only transition to 'refunded'
        // - Other states (pending, failed, uncollectible) can transition to anything
        let can_transition = match payment.status {
            PaymentStatus::Refunded | PaymentStatus::Void => false,
            PaymentStatus::Paid => matches!(
                status,
                PaymentStatus::Refunded | PaymentStatus::PartialRefund
            ),
            PaymentStatus::PartialRefund => matches!(status, PaymentStatus::Refunded),
            _ => true, // pending, failed, uncollectible can transition to anything
        };

        if !can_transition {
            // Silently skip update (matches Postgres behavior - no error, just no rows affected)
            return Ok(());
        }

        payment.status = status;
        if let Some(refunded) = amount_refunded_cents {
            payment.amount_refunded_cents = refunded;
        }
        if status.is_refunded() {
            payment.refunded_at = Some(chrono::Utc::now().naive_utc());
        }
        if failure_message.is_some() {
            payment.failure_message = failure_message;
        }
        payment.updated_at = Some(chrono::Utc::now().naive_utc());

        Ok(())
    }

    async fn get_payment_summary(
        &self,
        domain_id: Uuid,
        mode: PaymentMode,
        date_from: Option<NaiveDateTime>,
        date_to: Option<NaiveDateTime>,
    ) -> AppResult<PaymentSummary> {
        let payments = self.payments.lock().unwrap();

        // Apply date filters (payment_date OR created_at) matching Postgres behavior
        let filtered: Vec<_> = payments
            .values()
            .filter(|p| {
                if p.domain_id != domain_id || p.payment_mode != mode {
                    return false;
                }
                // Date range filters (same as Postgres: payment_date OR created_at)
                if let Some(df) = &date_from {
                    let in_range = p.payment_date.map(|d| d >= *df).unwrap_or(false)
                        || p.created_at.map(|d| d >= *df).unwrap_or(false);
                    if !in_range {
                        return false;
                    }
                }
                if let Some(dt) = &date_to {
                    let in_range = p.payment_date.map(|d| d <= *dt).unwrap_or(false)
                        || p.created_at.map(|d| d <= *dt).unwrap_or(false);
                    if !in_range {
                        return false;
                    }
                }
                true
            })
            .collect();

        // Postgres sums amount_paid_cents only for 'paid' status (see Postgres adapter line 412)
        let total_revenue_cents: i64 = filtered
            .iter()
            .filter(|p| p.status == PaymentStatus::Paid)
            .map(|p| p.amount_paid_cents as i64)
            .sum();
        let total_refunded_cents: i64 = filtered
            .iter()
            .map(|p| p.amount_refunded_cents as i64)
            .sum();
        let payment_count = filtered.len() as i64;
        let successful_payments = filtered
            .iter()
            .filter(|p| p.status == PaymentStatus::Paid)
            .count() as i64;
        // Postgres also counts 'uncollectible' and 'void' as failed (see line 416)
        let failed_payments = filtered
            .iter()
            .filter(|p| {
                matches!(
                    p.status,
                    PaymentStatus::Failed | PaymentStatus::Uncollectible | PaymentStatus::Void
                )
            })
            .count() as i64;

        Ok(PaymentSummary {
            total_revenue_cents,
            total_refunded_cents,
            payment_count,
            successful_payments,
            failed_payments,
        })
    }

    async fn list_all_for_export(
        &self,
        domain_id: Uuid,
        mode: PaymentMode,
        filters: &PaymentListFilters,
    ) -> AppResult<Vec<BillingPaymentWithUser>> {
        let payments = self.payments.lock().unwrap();
        let emails = self.user_emails.lock().unwrap();

        // Apply filters (same as list_by_domain, matches Postgres push_payment_filters)
        let mut filtered: Vec<_> = payments
            .values()
            .filter(|p| {
                if p.domain_id != domain_id || p.payment_mode != mode {
                    return false;
                }
                if let Some(status) = &filters.status {
                    if p.status != *status {
                        return false;
                    }
                }
                if let Some(date_from) = &filters.date_from {
                    let in_range = p.payment_date.map(|d| d >= *date_from).unwrap_or(false)
                        || p.created_at.map(|d| d >= *date_from).unwrap_or(false);
                    if !in_range {
                        return false;
                    }
                }
                if let Some(date_to) = &filters.date_to {
                    let in_range = p.payment_date.map(|d| d <= *date_to).unwrap_or(false)
                        || p.created_at.map(|d| d <= *date_to).unwrap_or(false);
                    if !in_range {
                        return false;
                    }
                }
                if let Some(plan_code) = &filters.plan_code {
                    if p.plan_code.as_ref() != Some(plan_code) {
                        return false;
                    }
                }
                if let Some(user_email_filter) = &filters.user_email {
                    let user_email = emails.get(&p.end_user_id).cloned().unwrap_or_default();
                    if !user_email
                        .to_lowercase()
                        .contains(&user_email_filter.to_lowercase())
                    {
                        return false;
                    }
                }
                true
            })
            .cloned()
            .collect();

        // Sort by payment_date DESC NULLS LAST, then by created_at DESC (matches Postgres)
        filtered.sort_by(|a, b| match (b.payment_date, a.payment_date) {
            (Some(b_date), Some(a_date)) => b_date.cmp(&a_date),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => b.created_at.cmp(&a.created_at),
        });

        Ok(filtered
            .into_iter()
            .map(|p| {
                let user_email = emails
                    .get(&p.end_user_id)
                    .cloned()
                    .unwrap_or_else(|| "test@example.com".to_string());
                BillingPaymentWithUser {
                    payment: p,
                    user_email,
                }
            })
            .collect())
    }
}

// ============================================================================
// InMemoryEnabledPaymentProvidersRepo
// ============================================================================

#[derive(Default)]
pub struct InMemoryEnabledPaymentProvidersRepo {
    pub providers:
        Mutex<HashMap<(Uuid, PaymentProvider, PaymentMode), EnabledPaymentProviderProfile>>,
}

impl InMemoryEnabledPaymentProvidersRepo {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_providers(providers: Vec<EnabledPaymentProviderProfile>) -> Self {
        let map: HashMap<(Uuid, PaymentProvider, PaymentMode), EnabledPaymentProviderProfile> =
            providers
                .into_iter()
                .map(|p| ((p.domain_id, p.provider, p.mode), p))
                .collect();
        Self {
            providers: Mutex::new(map),
        }
    }
}

#[async_trait]
impl EnabledPaymentProvidersRepoTrait for InMemoryEnabledPaymentProvidersRepo {
    async fn list_by_domain(
        &self,
        domain_id: Uuid,
    ) -> AppResult<Vec<EnabledPaymentProviderProfile>> {
        Ok(self
            .providers
            .lock()
            .unwrap()
            .values()
            .filter(|p| p.domain_id == domain_id)
            .cloned()
            .collect())
    }

    async fn list_active_by_domain(
        &self,
        domain_id: Uuid,
    ) -> AppResult<Vec<EnabledPaymentProviderProfile>> {
        Ok(self
            .providers
            .lock()
            .unwrap()
            .values()
            .filter(|p| p.domain_id == domain_id && p.is_active)
            .cloned()
            .collect())
    }

    async fn get(
        &self,
        domain_id: Uuid,
        provider: PaymentProvider,
        mode: PaymentMode,
    ) -> AppResult<Option<EnabledPaymentProviderProfile>> {
        Ok(self
            .providers
            .lock()
            .unwrap()
            .get(&(domain_id, provider, mode))
            .cloned())
    }

    async fn enable(
        &self,
        domain_id: Uuid,
        provider: PaymentProvider,
        mode: PaymentMode,
        display_order: i32,
    ) -> AppResult<EnabledPaymentProviderProfile> {
        let mut providers = self.providers.lock().unwrap();
        let now = chrono::Utc::now().naive_utc();

        let profile = EnabledPaymentProviderProfile {
            id: Uuid::new_v4(),
            domain_id,
            provider,
            mode,
            is_active: true,
            display_order,
            created_at: Some(now),
            updated_at: Some(now),
        };

        providers.insert((domain_id, provider, mode), profile.clone());
        Ok(profile)
    }

    async fn disable(
        &self,
        domain_id: Uuid,
        provider: PaymentProvider,
        mode: PaymentMode,
    ) -> AppResult<()> {
        self.providers
            .lock()
            .unwrap()
            .remove(&(domain_id, provider, mode));
        Ok(())
    }

    async fn set_active(
        &self,
        domain_id: Uuid,
        provider: PaymentProvider,
        mode: PaymentMode,
        is_active: bool,
    ) -> AppResult<()> {
        let mut providers = self.providers.lock().unwrap();
        if let Some(p) = providers.get_mut(&(domain_id, provider, mode)) {
            p.is_active = is_active;
            p.updated_at = Some(chrono::Utc::now().naive_utc());
        }
        Ok(())
    }

    async fn set_display_order(
        &self,
        domain_id: Uuid,
        provider: PaymentProvider,
        mode: PaymentMode,
        display_order: i32,
    ) -> AppResult<()> {
        let mut providers = self.providers.lock().unwrap();
        if let Some(p) = providers.get_mut(&(domain_id, provider, mode)) {
            p.display_order = display_order;
            p.updated_at = Some(chrono::Utc::now().naive_utc());
        }
        Ok(())
    }

    async fn is_enabled(
        &self,
        domain_id: Uuid,
        provider: PaymentProvider,
        mode: PaymentMode,
    ) -> AppResult<bool> {
        Ok(self
            .providers
            .lock()
            .unwrap()
            .contains_key(&(domain_id, provider, mode)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_plan_repo_create_and_get() {
        let repo = InMemorySubscriptionPlanRepo::new();
        let domain_id = Uuid::new_v4();

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

        let plan = repo
            .create(domain_id, PaymentMode::Test, &input)
            .await
            .unwrap();
        assert_eq!(plan.code, "test");

        let found = repo.get_by_id(plan.id).await.unwrap();
        assert!(found.is_some());
    }

    #[tokio::test]
    async fn test_subscription_repo_unique_constraint() {
        let repo = InMemoryUserSubscriptionRepo::new();
        let domain_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let plan_id = Uuid::new_v4();

        let input = CreateSubscriptionInput {
            domain_id,
            payment_mode: PaymentMode::Test,
            end_user_id: user_id,
            plan_id,
            stripe_customer_id: "cus_test".to_string(),
            stripe_subscription_id: None,
            status: SubscriptionStatus::Active,
            current_period_start: None,
            current_period_end: None,
            trial_start: None,
            trial_end: None,
        };

        // First create succeeds
        repo.create(&input).await.unwrap();

        // Second create fails (same user, domain, mode)
        let result = repo.create(&input).await;
        assert!(matches!(result, Err(AppError::InvalidInput(_))));
    }

    #[tokio::test]
    async fn test_event_repo_append_only() {
        let repo = InMemorySubscriptionEventRepo::new();
        let sub_id = Uuid::new_v4();

        let input = CreateSubscriptionEventInput {
            subscription_id: sub_id,
            event_type: "created".to_string(),
            previous_status: None,
            new_status: Some(SubscriptionStatus::Active),
            stripe_event_id: Some("evt_123".to_string()),
            metadata: serde_json::json!({}),
            created_by: None,
        };

        repo.create(&input).await.unwrap();

        // Events are append-only
        let events = repo.list_by_subscription(sub_id).await.unwrap();
        assert_eq!(events.len(), 1);

        // Add another event
        let input2 = CreateSubscriptionEventInput {
            subscription_id: sub_id,
            event_type: "updated".to_string(),
            previous_status: Some(SubscriptionStatus::Active),
            new_status: Some(SubscriptionStatus::Canceled),
            stripe_event_id: Some("evt_456".to_string()),
            metadata: serde_json::json!({}),
            created_by: None,
        };
        repo.create(&input2).await.unwrap();

        let events = repo.list_by_subscription(sub_id).await.unwrap();
        assert_eq!(events.len(), 2);
    }

    #[tokio::test]
    async fn test_event_idempotency_check() {
        let repo = InMemorySubscriptionEventRepo::new();
        let sub_id = Uuid::new_v4();

        let input = CreateSubscriptionEventInput {
            subscription_id: sub_id,
            event_type: "created".to_string(),
            previous_status: None,
            new_status: Some(SubscriptionStatus::Active),
            stripe_event_id: Some("evt_123".to_string()),
            metadata: serde_json::json!({}),
            created_by: None,
        };

        repo.create(&input).await.unwrap();

        assert!(repo.exists_by_stripe_event_id("evt_123").await.unwrap());
        assert!(!repo.exists_by_stripe_event_id("evt_999").await.unwrap());
    }
}
