use serde::{Deserialize, Serialize};

/// Payment mode - test (sandbox) or live (production) environment
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "payment_mode", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum PaymentMode {
    #[default]
    Test,
    Live,
}

impl PaymentMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            PaymentMode::Test => "test",
            PaymentMode::Live => "live",
        }
    }

    /// Whether this mode is production (live)
    pub fn is_production(&self) -> bool {
        matches!(self, PaymentMode::Live)
    }

    /// Whether this mode is test (sandbox)
    pub fn is_test(&self) -> bool {
        matches!(self, PaymentMode::Test)
    }

    /// Detect mode from Stripe key prefix.
    /// Test keys start with sk_test_ or pk_test_.
    /// Live keys start with sk_live_ or pk_live_.
    /// Restricted keys follow the same live/test prefix rules.
    pub fn from_stripe_key_prefix(key: &str) -> Self {
        if key.starts_with("sk_live_") || key.starts_with("pk_live_") || key.starts_with("rk_live_")
        {
            PaymentMode::Live
        } else {
            PaymentMode::Test
        }
    }

    /// Validate that a Stripe key's prefix matches the expected mode.
    /// Returns Ok(()) if the key matches, Err with message otherwise.
    pub fn validate_stripe_key_prefix(&self, key: &str, key_name: &str) -> Result<(), String> {
        let detected = Self::from_stripe_key_prefix(key);
        if detected != *self {
            Err(format!(
                "{} has {} prefix but {} mode was expected",
                key_name,
                detected.as_str(),
                self.as_str()
            ))
        } else {
            Ok(())
        }
    }
}

impl std::fmt::Display for PaymentMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for PaymentMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "test" => Ok(PaymentMode::Test),
            "live" => Ok(PaymentMode::Live),
            _ => Err(format!(
                "Invalid payment mode: {}. Must be 'test' or 'live'",
                s
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_stripe_key_prefix() {
        assert_eq!(
            PaymentMode::from_stripe_key_prefix("sk_test_abc123"),
            PaymentMode::Test
        );
        assert_eq!(
            PaymentMode::from_stripe_key_prefix("pk_test_abc123"),
            PaymentMode::Test
        );
        assert_eq!(
            PaymentMode::from_stripe_key_prefix("sk_live_abc123"),
            PaymentMode::Live
        );
        assert_eq!(
            PaymentMode::from_stripe_key_prefix("pk_live_abc123"),
            PaymentMode::Live
        );
        // Restricted keys follow live/test prefixes
        assert_eq!(
            PaymentMode::from_stripe_key_prefix("rk_test_abc123"),
            PaymentMode::Test
        );
        assert_eq!(
            PaymentMode::from_stripe_key_prefix("rk_live_abc123"),
            PaymentMode::Live
        );
    }

    #[test]
    fn test_validate_stripe_key_prefix() {
        let test_mode = PaymentMode::Test;
        let live_mode = PaymentMode::Live;

        assert!(
            test_mode
                .validate_stripe_key_prefix("sk_test_abc", "secret_key")
                .is_ok()
        );
        assert!(
            test_mode
                .validate_stripe_key_prefix("sk_live_abc", "secret_key")
                .is_err()
        );
        assert!(
            test_mode
                .validate_stripe_key_prefix("rk_test_abc", "secret_key")
                .is_ok()
        );
        assert!(
            test_mode
                .validate_stripe_key_prefix("rk_live_abc", "secret_key")
                .is_err()
        );
        assert!(
            live_mode
                .validate_stripe_key_prefix("sk_live_abc", "secret_key")
                .is_ok()
        );
        assert!(
            live_mode
                .validate_stripe_key_prefix("sk_test_abc", "secret_key")
                .is_err()
        );
        assert!(
            live_mode
                .validate_stripe_key_prefix("rk_live_abc", "secret_key")
                .is_ok()
        );
    }

    #[test]
    fn test_from_str() {
        assert_eq!("test".parse::<PaymentMode>().unwrap(), PaymentMode::Test);
        assert_eq!("live".parse::<PaymentMode>().unwrap(), PaymentMode::Live);
        assert!("sandbox".parse::<PaymentMode>().is_err());
        assert!("production".parse::<PaymentMode>().is_err());
        assert!("invalid".parse::<PaymentMode>().is_err());
    }

    #[test]
    fn test_is_production() {
        assert!(!PaymentMode::Test.is_production());
        assert!(PaymentMode::Live.is_production());
        assert!(PaymentMode::Test.is_test());
        assert!(!PaymentMode::Live.is_test());
    }

    #[test]
    fn test_as_str_all_variants() {
        assert_eq!(PaymentMode::Test.as_str(), "test");
        assert_eq!(PaymentMode::Live.as_str(), "live");
    }

    #[test]
    fn test_display_matches_as_str() {
        for variant in [PaymentMode::Test, PaymentMode::Live] {
            assert_eq!(format!("{}", variant), variant.as_str());
        }
    }

    #[test]
    fn test_from_str_case_insensitive() {
        assert_eq!("TEST".parse::<PaymentMode>().unwrap(), PaymentMode::Test);
        assert_eq!("Test".parse::<PaymentMode>().unwrap(), PaymentMode::Test);
        assert_eq!("LIVE".parse::<PaymentMode>().unwrap(), PaymentMode::Live);
        assert_eq!("Live".parse::<PaymentMode>().unwrap(), PaymentMode::Live);
    }
}
