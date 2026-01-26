use serde::{Deserialize, Serialize};

/// JWT claims for domain end-users.
///
/// Issued by the Reauth API and verified by SDKs.
/// Contains user identity, roles, and subscription information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainEndUserClaims {
    /// User ID (subject) - the end_user_id
    pub sub: String,

    /// Domain ID (UUID as string)
    pub domain_id: String,

    /// Root domain (e.g., "example.com")
    pub domain: String,

    /// User's roles (e.g., ["admin", "user"])
    pub roles: Vec<String>,

    /// Subscription information (always present)
    pub subscription: SubscriptionClaims,

    /// Token expiration (Unix timestamp)
    pub exp: i64,

    /// Token issued at (Unix timestamp)
    pub iat: i64,
}

/// Subscription info embedded in JWT claims.
///
/// Uses snake_case to match the JSON format used in tokens.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SubscriptionClaims {
    /// Current subscription status (e.g., "active", "trialing", "none")
    pub status: String,

    /// Machine-readable plan identifier (e.g., "pro")
    pub plan_code: Option<String>,

    /// Human-readable plan name (e.g., "Pro Plan")
    pub plan_name: Option<String>,

    /// Unix timestamp when current period ends
    pub current_period_end: Option<i64>,

    /// Whether subscription will cancel at period end
    pub cancel_at_period_end: Option<bool>,

    /// Unix timestamp when trial ends (if applicable)
    pub trial_ends_at: Option<i64>,

    /// Stripe subscription ID (for backend use)
    pub subscription_id: Option<String>,
}

impl SubscriptionClaims {
    /// Create a "none" subscription for users without a subscription.
    pub fn none() -> Self {
        Self {
            status: "none".to_string(),
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subscription_claims_none() {
        let sub = SubscriptionClaims::none();
        assert_eq!(sub.status, "none");
        assert!(sub.plan_code.is_none());
        assert!(sub.plan_name.is_none());
    }

    #[test]
    fn test_domain_end_user_claims_serde() {
        let claims = DomainEndUserClaims {
            sub: "user123".to_string(),
            domain_id: "00000000-0000-0000-0000-000000000001".to_string(),
            domain: "example.com".to_string(),
            roles: vec!["user".to_string()],
            subscription: SubscriptionClaims {
                status: "active".to_string(),
                plan_code: Some("pro".to_string()),
                plan_name: Some("Pro Plan".to_string()),
                current_period_end: Some(1735689600),
                cancel_at_period_end: Some(false),
                trial_ends_at: None,
                subscription_id: Some("sub_123".to_string()),
            },
            exp: 1735689600,
            iat: 1735603200,
        };

        let json = serde_json::to_string(&claims).unwrap();
        let parsed: DomainEndUserClaims = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.sub, "user123");
        assert_eq!(parsed.domain, "example.com");
        assert_eq!(parsed.subscription.status, "active");
    }
}
