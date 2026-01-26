use serde::{Deserialize, Serialize};

/// Subscription status values used in JWT claims and API responses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
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
    #[default]
    None,
}

impl SubscriptionStatus {
    /// Returns true if the subscription is in an active state (active or trialing).
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Active | Self::Trialing)
    }

    /// Returns true if the subscription is in a grace period (past due but not yet canceled).
    pub fn is_grace_period(&self) -> bool {
        matches!(self, Self::PastDue)
    }

    /// Returns true if the subscription allows access to paid features.
    pub fn has_access(&self) -> bool {
        matches!(self, Self::Active | Self::Trialing | Self::PastDue)
    }
}

impl std::fmt::Display for SubscriptionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Active => "active",
            Self::PastDue => "past_due",
            Self::Canceled => "canceled",
            Self::Trialing => "trialing",
            Self::Incomplete => "incomplete",
            Self::IncompleteExpired => "incomplete_expired",
            Self::Unpaid => "unpaid",
            Self::Paused => "paused",
            Self::None => "none",
        };
        write!(f, "{}", s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serde_roundtrip() {
        let status = SubscriptionStatus::Active;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, r#""active""#);

        let parsed: SubscriptionStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, status);
    }

    #[test]
    fn test_snake_case_serialization() {
        let status = SubscriptionStatus::PastDue;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, r#""past_due""#);
    }

    #[test]
    fn test_is_active() {
        assert!(SubscriptionStatus::Active.is_active());
        assert!(SubscriptionStatus::Trialing.is_active());
        assert!(!SubscriptionStatus::Canceled.is_active());
        assert!(!SubscriptionStatus::None.is_active());
    }

    #[test]
    fn test_has_access() {
        assert!(SubscriptionStatus::Active.has_access());
        assert!(SubscriptionStatus::Trialing.has_access());
        assert!(SubscriptionStatus::PastDue.has_access());
        assert!(!SubscriptionStatus::Canceled.has_access());
        assert!(!SubscriptionStatus::None.has_access());
    }
}
