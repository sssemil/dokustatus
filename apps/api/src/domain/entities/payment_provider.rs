use serde::{Deserialize, Serialize};

use super::payment_mode::PaymentMode;

/// Payment provider type - the payment processor used for billing
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "payment_provider", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum PaymentProvider {
    #[default]
    Stripe,
    Dummy,
    Coinbase,
}

impl PaymentProvider {
    pub fn as_str(&self) -> &'static str {
        match self {
            PaymentProvider::Stripe => "stripe",
            PaymentProvider::Dummy => "dummy",
            PaymentProvider::Coinbase => "coinbase",
        }
    }

    /// Human-readable display name for the provider
    pub fn display_name(&self) -> &'static str {
        match self {
            PaymentProvider::Stripe => "Stripe",
            PaymentProvider::Dummy => "Test Provider",
            PaymentProvider::Coinbase => "Coinbase Commerce",
        }
    }

    /// Whether this provider requires external checkout (redirect to provider's hosted page)
    pub fn requires_external_checkout(&self) -> bool {
        matches!(self, PaymentProvider::Stripe | PaymentProvider::Coinbase)
    }

    /// Whether this provider supports the given payment mode
    pub fn supports_mode(&self, mode: PaymentMode) -> bool {
        match self {
            // Stripe and Coinbase support both test and live modes
            PaymentProvider::Stripe | PaymentProvider::Coinbase => true,
            // Dummy provider only operates in test mode
            PaymentProvider::Dummy => mode == PaymentMode::Test,
        }
    }

    /// Get the default mode for this provider
    pub fn default_mode(&self) -> PaymentMode {
        match self {
            PaymentProvider::Dummy => PaymentMode::Test,
            _ => PaymentMode::Test, // Default to test for safety
        }
    }

    /// Whether this provider is the dummy/test provider
    pub fn is_dummy(&self) -> bool {
        matches!(self, PaymentProvider::Dummy)
    }

    /// Whether this provider is Stripe
    pub fn is_stripe(&self) -> bool {
        matches!(self, PaymentProvider::Stripe)
    }

    /// Whether this provider is Coinbase
    pub fn is_coinbase(&self) -> bool {
        matches!(self, PaymentProvider::Coinbase)
    }

    /// All available providers
    pub fn all() -> &'static [PaymentProvider] {
        &[
            PaymentProvider::Stripe,
            PaymentProvider::Dummy,
            PaymentProvider::Coinbase,
        ]
    }

    /// Providers that are currently implemented
    pub fn implemented() -> &'static [PaymentProvider] {
        &[PaymentProvider::Stripe, PaymentProvider::Dummy]
    }
}

impl std::fmt::Display for PaymentProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for PaymentProvider {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "stripe" => Ok(PaymentProvider::Stripe),
            "dummy" => Ok(PaymentProvider::Dummy),
            "coinbase" => Ok(PaymentProvider::Coinbase),
            _ => Err(format!(
                "Invalid payment provider: {}. Must be 'stripe', 'dummy', or 'coinbase'",
                s
            )),
        }
    }
}

/// Represents a specific provider configuration (provider + mode combination)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub provider: PaymentProvider,
    pub mode: PaymentMode,
}

impl ProviderConfig {
    pub fn new(provider: PaymentProvider, mode: PaymentMode) -> Result<Self, String> {
        if !provider.supports_mode(mode) {
            return Err(format!(
                "Provider {} does not support {} mode",
                provider.display_name(),
                mode.as_str()
            ));
        }
        Ok(Self { provider, mode })
    }

    /// Create a Stripe test configuration
    pub fn stripe_test() -> Self {
        Self {
            provider: PaymentProvider::Stripe,
            mode: PaymentMode::Test,
        }
    }

    /// Create a Stripe live configuration
    pub fn stripe_live() -> Self {
        Self {
            provider: PaymentProvider::Stripe,
            mode: PaymentMode::Live,
        }
    }

    /// Create a dummy provider configuration (always test mode)
    pub fn dummy() -> Self {
        Self {
            provider: PaymentProvider::Dummy,
            mode: PaymentMode::Test,
        }
    }

    /// Human-readable display name
    pub fn display_name(&self) -> String {
        if self.mode.is_production() {
            self.provider.display_name().to_string()
        } else {
            format!("{} (Test)", self.provider.display_name())
        }
    }

    /// Whether this is a production configuration
    pub fn is_production(&self) -> bool {
        self.mode.is_production()
    }

    /// Whether this is a test configuration
    pub fn is_test(&self) -> bool {
        self.mode.is_test()
    }
}

impl std::fmt::Display for ProviderConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}_{}", self.provider.as_str(), self.mode.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_modes() {
        assert!(PaymentProvider::Stripe.supports_mode(PaymentMode::Test));
        assert!(PaymentProvider::Stripe.supports_mode(PaymentMode::Live));
        assert!(PaymentProvider::Dummy.supports_mode(PaymentMode::Test));
        assert!(!PaymentProvider::Dummy.supports_mode(PaymentMode::Live));
        assert!(PaymentProvider::Coinbase.supports_mode(PaymentMode::Test));
        assert!(PaymentProvider::Coinbase.supports_mode(PaymentMode::Live));
    }

    #[test]
    fn test_provider_config_new() {
        assert!(ProviderConfig::new(PaymentProvider::Stripe, PaymentMode::Test).is_ok());
        assert!(ProviderConfig::new(PaymentProvider::Stripe, PaymentMode::Live).is_ok());
        assert!(ProviderConfig::new(PaymentProvider::Dummy, PaymentMode::Test).is_ok());
        assert!(ProviderConfig::new(PaymentProvider::Dummy, PaymentMode::Live).is_err());
    }

    #[test]
    fn test_provider_config_display() {
        assert_eq!(
            ProviderConfig::stripe_test().display_name(),
            "Stripe (Test)"
        );
        assert_eq!(ProviderConfig::stripe_live().display_name(), "Stripe");
        assert_eq!(
            ProviderConfig::dummy().display_name(),
            "Test Provider (Test)"
        );
    }

    #[test]
    fn test_from_str() {
        assert_eq!(
            "stripe".parse::<PaymentProvider>().unwrap(),
            PaymentProvider::Stripe
        );
        assert_eq!(
            "dummy".parse::<PaymentProvider>().unwrap(),
            PaymentProvider::Dummy
        );
        assert!("test".parse::<PaymentProvider>().is_err());
        assert!("invalid".parse::<PaymentProvider>().is_err());
    }
}
