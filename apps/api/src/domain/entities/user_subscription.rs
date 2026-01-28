use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "subscription_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum SubscriptionStatus {
    Active,
    PastDue,
    Canceled,
    Trialing,
    Incomplete,
    IncompleteExpired,
    Unpaid,
    Paused,
}

impl SubscriptionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            SubscriptionStatus::Active => "active",
            SubscriptionStatus::PastDue => "past_due",
            SubscriptionStatus::Canceled => "canceled",
            SubscriptionStatus::Trialing => "trialing",
            SubscriptionStatus::Incomplete => "incomplete",
            SubscriptionStatus::IncompleteExpired => "incomplete_expired",
            SubscriptionStatus::Unpaid => "unpaid",
            SubscriptionStatus::Paused => "paused",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "active" => SubscriptionStatus::Active,
            "past_due" => SubscriptionStatus::PastDue,
            "canceled" | "cancelled" => SubscriptionStatus::Canceled,
            "trialing" => SubscriptionStatus::Trialing,
            "incomplete" => SubscriptionStatus::Incomplete,
            "incomplete_expired" => SubscriptionStatus::IncompleteExpired,
            "unpaid" => SubscriptionStatus::Unpaid,
            "paused" => SubscriptionStatus::Paused,
            _ => SubscriptionStatus::Incomplete,
        }
    }

    /// Convert from Stripe subscription status string
    pub fn from_stripe(s: &str) -> Self {
        match s {
            "active" => SubscriptionStatus::Active,
            "past_due" => SubscriptionStatus::PastDue,
            "canceled" => SubscriptionStatus::Canceled,
            "trialing" => SubscriptionStatus::Trialing,
            "incomplete" => SubscriptionStatus::Incomplete,
            "incomplete_expired" => SubscriptionStatus::IncompleteExpired,
            "unpaid" => SubscriptionStatus::Unpaid,
            "paused" => SubscriptionStatus::Paused,
            _ => SubscriptionStatus::Incomplete,
        }
    }

    /// Returns true if user should have access to subscription features
    pub fn is_active(&self) -> bool {
        matches!(
            self,
            SubscriptionStatus::Active | SubscriptionStatus::Trialing
        )
    }

    /// Returns true if user is in a grace period (past due but not yet canceled)
    pub fn is_grace_period(&self) -> bool {
        matches!(self, SubscriptionStatus::PastDue)
    }
}

#[derive(Debug, Clone)]
pub struct UserSubscription {
    pub id: Uuid,
    pub domain_id: Uuid,
    pub end_user_id: Uuid,
    pub plan_id: Uuid,
    pub status: SubscriptionStatus,
    pub stripe_customer_id: String,
    pub stripe_subscription_id: Option<String>,
    pub current_period_start: Option<chrono::NaiveDateTime>,
    pub current_period_end: Option<chrono::NaiveDateTime>,
    pub trial_start: Option<chrono::NaiveDateTime>,
    pub trial_end: Option<chrono::NaiveDateTime>,
    pub cancel_at_period_end: bool,
    pub canceled_at: Option<chrono::NaiveDateTime>,
    pub manually_granted: bool,
    pub granted_by: Option<Uuid>,
    pub granted_at: Option<chrono::NaiveDateTime>,
    pub created_at: Option<chrono::NaiveDateTime>,
    pub updated_at: Option<chrono::NaiveDateTime>,
    // Rate limiting for plan changes
    pub changes_this_period: i32,
    pub period_changes_reset_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone)]
pub struct SubscriptionEvent {
    pub id: Uuid,
    pub subscription_id: Uuid,
    pub event_type: String,
    pub previous_status: Option<SubscriptionStatus>,
    pub new_status: Option<SubscriptionStatus>,
    pub stripe_event_id: Option<String>,
    pub metadata: serde_json::Value,
    pub created_by: Option<Uuid>,
    pub created_at: Option<chrono::NaiveDateTime>,
}
