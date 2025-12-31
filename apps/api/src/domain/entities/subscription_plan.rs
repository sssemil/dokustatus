use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BillingInterval {
    Monthly,
    Yearly,
    Custom,
}

impl BillingInterval {
    pub fn as_str(&self) -> &'static str {
        match self {
            BillingInterval::Monthly => "monthly",
            BillingInterval::Yearly => "yearly",
            BillingInterval::Custom => "custom",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "monthly" => BillingInterval::Monthly,
            "yearly" => BillingInterval::Yearly,
            _ => BillingInterval::Custom,
        }
    }

    /// Convert to Stripe interval format
    pub fn to_stripe_interval(&self) -> &'static str {
        match self {
            BillingInterval::Monthly => "month",
            BillingInterval::Yearly => "year",
            BillingInterval::Custom => "month", // Custom uses month as base
        }
    }
}

#[derive(Debug, Clone)]
pub struct SubscriptionPlan {
    pub id: Uuid,
    pub domain_id: Uuid,
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    pub price_cents: i32,
    pub currency: String,
    pub interval: BillingInterval,
    pub interval_count: i32,
    pub trial_days: i32,
    pub features: Vec<String>,
    pub is_public: bool,
    pub display_order: i32,
    pub stripe_product_id: Option<String>,
    pub stripe_price_id: Option<String>,
    pub is_archived: bool,
    pub archived_at: Option<chrono::NaiveDateTime>,
    pub created_at: Option<chrono::NaiveDateTime>,
    pub updated_at: Option<chrono::NaiveDateTime>,
}
