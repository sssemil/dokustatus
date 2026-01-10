use serde::{Deserialize, Serialize};

/// Stripe environment mode - test (sandbox) or live (production)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "stripe_mode", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum StripeMode {
    Test,
    Live,
}

impl StripeMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            StripeMode::Test => "test",
            StripeMode::Live => "live",
        }
    }

    /// Detect mode from Stripe key prefix.
    /// Test keys start with sk_test_ or pk_test_.
    /// Live keys start with sk_live_ or pk_live_.
    /// Restricted keys follow the same live/test prefix rules.
    pub fn from_key_prefix(key: &str) -> Self {
        if key.starts_with("sk_live_") || key.starts_with("pk_live_") || key.starts_with("rk_live_")
        {
            StripeMode::Live
        } else {
            StripeMode::Test
        }
    }

    /// Validate that a key's prefix matches the expected mode.
    /// Returns Ok(()) if the key matches, Err with message otherwise.
    pub fn validate_key_prefix(&self, key: &str, key_name: &str) -> Result<(), String> {
        let detected = Self::from_key_prefix(key);
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

impl Default for StripeMode {
    fn default() -> Self {
        StripeMode::Test
    }
}

impl std::fmt::Display for StripeMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for StripeMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "test" => Ok(StripeMode::Test),
            "live" => Ok(StripeMode::Live),
            _ => Err(format!(
                "Invalid stripe mode: {}. Must be 'test' or 'live'",
                s
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_key_prefix() {
        assert_eq!(
            StripeMode::from_key_prefix("sk_test_abc123"),
            StripeMode::Test
        );
        assert_eq!(
            StripeMode::from_key_prefix("pk_test_abc123"),
            StripeMode::Test
        );
        assert_eq!(
            StripeMode::from_key_prefix("sk_live_abc123"),
            StripeMode::Live
        );
        assert_eq!(
            StripeMode::from_key_prefix("pk_live_abc123"),
            StripeMode::Live
        );
        // Restricted keys follow live/test prefixes
        assert_eq!(
            StripeMode::from_key_prefix("rk_test_abc123"),
            StripeMode::Test
        );
        assert_eq!(
            StripeMode::from_key_prefix("rk_live_abc123"),
            StripeMode::Live
        );
    }

    #[test]
    fn test_validate_key_prefix() {
        let test_mode = StripeMode::Test;
        let live_mode = StripeMode::Live;

        assert!(
            test_mode
                .validate_key_prefix("sk_test_abc", "secret_key")
                .is_ok()
        );
        assert!(
            test_mode
                .validate_key_prefix("sk_live_abc", "secret_key")
                .is_err()
        );
        assert!(
            test_mode
                .validate_key_prefix("rk_test_abc", "secret_key")
                .is_ok()
        );
        assert!(
            test_mode
                .validate_key_prefix("rk_live_abc", "secret_key")
                .is_err()
        );
        assert!(
            live_mode
                .validate_key_prefix("sk_live_abc", "secret_key")
                .is_ok()
        );
        assert!(
            live_mode
                .validate_key_prefix("sk_test_abc", "secret_key")
                .is_err()
        );
        assert!(
            live_mode
                .validate_key_prefix("rk_live_abc", "secret_key")
                .is_ok()
        );
    }
}
