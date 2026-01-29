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

#[derive(Debug, Clone, Serialize)]
pub struct UserAuthPayload {
    pub user_id: String,
    pub auth_method: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct UserIdPayload {
    pub user_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct UserRolesChangedPayload {
    pub user_id: String,
    pub old_roles: Vec<String>,
    pub new_roles: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SubscriptionCreatedPayload {
    pub user_id: String,
    pub plan_id: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SubscriptionCanceledPayload {
    pub user_id: String,
    pub subscription_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SubscriptionPlanChangedPayload {
    pub user_id: String,
    pub from_plan_code: String,
    pub to_plan_code: String,
    pub change_type: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SubscriptionStatusChangedPayload {
    pub user_id: String,
    pub plan_id: String,
    pub old_status: String,
    pub new_status: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PaymentPayload {
    pub user_id: String,
    pub amount_cents: i64,
    pub currency: String,
    pub invoice_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct WebhookTestPayload {
    pub endpoint_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct WebhookEnvelope<T: Serialize> {
    pub id: String,
    #[serde(rename = "type")]
    pub event_type: String,
    pub api_version: String,
    pub created_at: String,
    pub domain_id: String,
    pub data: T,
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

    #[test]
    fn user_auth_payload_serializes_expected_fields() {
        let payload = UserAuthPayload {
            user_id: "u1".into(),
            auth_method: "magic_link".into(),
        };
        let v = serde_json::to_value(&payload).unwrap();
        assert_eq!(v["user_id"], "u1");
        assert_eq!(v["auth_method"], "magic_link");
        assert_eq!(v.as_object().unwrap().len(), 2);
    }

    #[test]
    fn user_id_payload_serializes_expected_fields() {
        let payload = UserIdPayload {
            user_id: "u2".into(),
        };
        let v = serde_json::to_value(&payload).unwrap();
        assert_eq!(v["user_id"], "u2");
        assert_eq!(v.as_object().unwrap().len(), 1);
    }

    #[test]
    fn user_roles_changed_payload_serializes_expected_fields() {
        let payload = UserRolesChangedPayload {
            user_id: "u3".into(),
            old_roles: vec!["admin".into()],
            new_roles: vec!["admin".into(), "editor".into()],
        };
        let v = serde_json::to_value(&payload).unwrap();
        assert_eq!(v["user_id"], "u3");
        assert_eq!(v["old_roles"], serde_json::json!(["admin"]));
        assert_eq!(v["new_roles"], serde_json::json!(["admin", "editor"]));
        assert_eq!(v.as_object().unwrap().len(), 3);
    }

    #[test]
    fn subscription_plan_changed_and_status_changed_have_disjoint_fields() {
        let plan = SubscriptionPlanChangedPayload {
            user_id: "u".into(),
            from_plan_code: "free".into(),
            to_plan_code: "pro".into(),
            change_type: "upgrade".into(),
        };
        let status = SubscriptionStatusChangedPayload {
            user_id: "u".into(),
            plan_id: "plan_1".into(),
            old_status: "trialing".into(),
            new_status: "active".into(),
        };
        let pv = serde_json::to_value(&plan).unwrap();
        let sv = serde_json::to_value(&status).unwrap();

        let plan_keys: std::collections::HashSet<&str> =
            pv.as_object().unwrap().keys().map(|k| k.as_str()).collect();
        let status_keys: std::collections::HashSet<&str> =
            sv.as_object().unwrap().keys().map(|k| k.as_str()).collect();

        // Only user_id should be shared
        let shared: Vec<&&str> = plan_keys.intersection(&status_keys).collect();
        assert_eq!(shared.len(), 1);
        assert!(shared.contains(&&"user_id"));
    }

    #[test]
    fn payment_payload_serializes_expected_fields() {
        let payload = PaymentPayload {
            user_id: "u4".into(),
            amount_cents: 1999,
            currency: "usd".into(),
            invoice_id: "in_123".into(),
        };
        let v = serde_json::to_value(&payload).unwrap();
        assert_eq!(v["user_id"], "u4");
        assert_eq!(v["amount_cents"], 1999);
        assert_eq!(v["currency"], "usd");
        assert_eq!(v["invoice_id"], "in_123");
        assert_eq!(v.as_object().unwrap().len(), 4);
    }

    #[test]
    fn webhook_envelope_renames_event_type_to_type() {
        let envelope = WebhookEnvelope {
            id: "evt_1".into(),
            event_type: "user.created".into(),
            api_version: "2025-01-01".into(),
            created_at: "2025-01-01T00:00:00Z".into(),
            domain_id: "d1".into(),
            data: UserIdPayload {
                user_id: "u5".into(),
            },
        };
        let v = serde_json::to_value(&envelope).unwrap();
        assert!(v.get("type").is_some());
        assert!(v.get("event_type").is_none());
        assert_eq!(v["type"], "user.created");
        assert_eq!(v["data"]["user_id"], "u5");
    }

    #[test]
    fn subscription_created_payload_serializes_expected_fields() {
        let payload = SubscriptionCreatedPayload {
            user_id: "u6".into(),
            plan_id: "plan_pro".into(),
            status: "active".into(),
        };
        let v = serde_json::to_value(&payload).unwrap();
        assert_eq!(v["user_id"], "u6");
        assert_eq!(v["plan_id"], "plan_pro");
        assert_eq!(v["status"], "active");
        assert_eq!(v.as_object().unwrap().len(), 3);
    }

    #[test]
    fn subscription_canceled_payload_serializes_expected_fields() {
        let payload = SubscriptionCanceledPayload {
            user_id: "u7".into(),
            subscription_id: "sub_123".into(),
        };
        let v = serde_json::to_value(&payload).unwrap();
        assert_eq!(v["user_id"], "u7");
        assert_eq!(v["subscription_id"], "sub_123");
        assert_eq!(v.as_object().unwrap().len(), 2);
    }

    #[test]
    fn webhook_test_payload_serializes_expected_fields() {
        let payload = WebhookTestPayload {
            endpoint_id: "ep_456".into(),
        };
        let v = serde_json::to_value(&payload).unwrap();
        assert_eq!(v["endpoint_id"], "ep_456");
        assert_eq!(v.as_object().unwrap().len(), 1);
    }
}
