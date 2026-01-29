use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WebhookEventType {
    UserCreated,
    UserDeleted,
    UserLogin,
    UserFrozen,
    UserUnfrozen,
    UserWhitelisted,
    UserUnwhitelisted,
    UserRolesChanged,
    UserInvited,
    SubscriptionCreated,
    SubscriptionUpdated,
    SubscriptionCanceled,
    PaymentSucceeded,
    PaymentFailed,
    WebhookTest,
}

impl WebhookEventType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::UserCreated => "user.created",
            Self::UserDeleted => "user.deleted",
            Self::UserLogin => "user.login",
            Self::UserFrozen => "user.frozen",
            Self::UserUnfrozen => "user.unfrozen",
            Self::UserWhitelisted => "user.whitelisted",
            Self::UserUnwhitelisted => "user.unwhitelisted",
            Self::UserRolesChanged => "user.roles_changed",
            Self::UserInvited => "user.invited",
            Self::SubscriptionCreated => "subscription.created",
            Self::SubscriptionUpdated => "subscription.updated",
            Self::SubscriptionCanceled => "subscription.canceled",
            Self::PaymentSucceeded => "payment.succeeded",
            Self::PaymentFailed => "payment.failed",
            Self::WebhookTest => "webhook.test",
        }
    }

    pub fn all_types() -> &'static [WebhookEventType] {
        &[
            Self::UserCreated,
            Self::UserDeleted,
            Self::UserLogin,
            Self::UserFrozen,
            Self::UserUnfrozen,
            Self::UserWhitelisted,
            Self::UserUnwhitelisted,
            Self::UserRolesChanged,
            Self::UserInvited,
            Self::SubscriptionCreated,
            Self::SubscriptionUpdated,
            Self::SubscriptionCanceled,
            Self::PaymentSucceeded,
            Self::PaymentFailed,
            Self::WebhookTest,
        ]
    }

    pub fn all_type_strings() -> Vec<&'static str> {
        Self::all_types().iter().map(|t| t.as_str()).collect()
    }
}

impl fmt::Display for WebhookEventType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for WebhookEventType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "user.created" => Ok(Self::UserCreated),
            "user.deleted" => Ok(Self::UserDeleted),
            "user.login" => Ok(Self::UserLogin),
            "user.frozen" => Ok(Self::UserFrozen),
            "user.unfrozen" => Ok(Self::UserUnfrozen),
            "user.whitelisted" => Ok(Self::UserWhitelisted),
            "user.unwhitelisted" => Ok(Self::UserUnwhitelisted),
            "user.roles_changed" => Ok(Self::UserRolesChanged),
            "user.invited" => Ok(Self::UserInvited),
            "subscription.created" => Ok(Self::SubscriptionCreated),
            "subscription.updated" => Ok(Self::SubscriptionUpdated),
            "subscription.canceled" => Ok(Self::SubscriptionCanceled),
            "payment.succeeded" => Ok(Self::PaymentSucceeded),
            "payment.failed" => Ok(Self::PaymentFailed),
            "webhook.test" => Ok(Self::WebhookTest),
            _ => Err(format!("unknown webhook event type: {}", s)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn as_str_roundtrips_through_from_str() {
        for event_type in WebhookEventType::all_types() {
            let s = event_type.as_str();
            let parsed: WebhookEventType = s.parse().unwrap();
            assert_eq!(*event_type, parsed);
        }
    }

    #[test]
    fn display_matches_as_str() {
        for event_type in WebhookEventType::all_types() {
            assert_eq!(format!("{}", event_type), event_type.as_str());
        }
    }

    #[test]
    fn from_str_rejects_unknown() {
        assert!("unknown.event".parse::<WebhookEventType>().is_err());
    }

    #[test]
    fn all_types_is_exhaustive() {
        assert_eq!(WebhookEventType::all_types().len(), 15);
    }

    #[test]
    fn all_type_strings_matches_all_types() {
        let strings = WebhookEventType::all_type_strings();
        let types = WebhookEventType::all_types();
        assert_eq!(strings.len(), types.len());
        for (s, t) in strings.iter().zip(types.iter()) {
            assert_eq!(*s, t.as_str());
        }
    }
}
