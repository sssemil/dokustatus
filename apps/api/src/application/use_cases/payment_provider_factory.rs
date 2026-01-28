use std::sync::Arc;
use uuid::Uuid;

use crate::{
    app_error::{AppError, AppResult},
    application::ports::payment_provider::PaymentProviderPort,
    domain::entities::{payment_mode::PaymentMode, payment_provider::PaymentProvider},
    infra::{
        crypto::ProcessCipher, dummy_payment_client::DummyPaymentClient,
        stripe_payment_adapter::StripePaymentAdapter,
    },
};

use super::domain_billing::BillingStripeConfigRepoTrait;

/// Factory for creating payment provider instances based on configuration.
///
/// The factory handles:
/// - Decrypting stored credentials
/// - Validating provider + mode combinations
/// - Instantiating the appropriate provider client
pub struct PaymentProviderFactory {
    cipher: ProcessCipher,
    config_repo: Arc<dyn BillingStripeConfigRepoTrait>,
    #[cfg(test)]
    test_provider_override: Option<Arc<dyn PaymentProviderPort>>,
}

impl PaymentProviderFactory {
    pub fn new(cipher: ProcessCipher, config_repo: Arc<dyn BillingStripeConfigRepoTrait>) -> Self {
        Self {
            cipher,
            config_repo,
            #[cfg(test)]
            test_provider_override: None,
        }
    }

    #[cfg(test)]
    pub fn with_provider_override(mut self, provider: Arc<dyn PaymentProviderPort>) -> Self {
        self.test_provider_override = Some(provider);
        self
    }

    /// Get a payment provider instance for the given domain, provider, and mode.
    ///
    /// # Arguments
    /// * `domain_id` - The domain to get the provider for
    /// * `provider` - The payment provider type
    /// * `mode` - The payment mode (test/live)
    ///
    /// # Returns
    /// An instance of the payment provider implementing PaymentProviderPort
    ///
    /// # Errors
    /// - `ProviderNotSupported` if the provider is not yet implemented (e.g., Coinbase)
    /// - `ProviderNotConfigured` if the provider requires configuration but none exists
    /// - `InvalidInput` if the provider doesn't support the requested mode
    pub async fn get(
        &self,
        domain_id: Uuid,
        provider: PaymentProvider,
        mode: PaymentMode,
    ) -> AppResult<Arc<dyn PaymentProviderPort>> {
        #[cfg(test)]
        if let Some(ref override_provider) = self.test_provider_override {
            return Ok(override_provider.clone());
        }

        // Validate that the provider supports this mode
        if !provider.supports_mode(mode) {
            return Err(AppError::InvalidInput(format!(
                "{} does not support {} mode",
                provider.display_name(),
                mode.as_ref()
            )));
        }

        match provider {
            PaymentProvider::Stripe => self.get_stripe_provider(domain_id, mode).await,
            PaymentProvider::Dummy => {
                // Dummy provider doesn't need configuration
                Ok(Arc::new(DummyPaymentClient::new(domain_id)))
            }
            PaymentProvider::Coinbase => {
                // Coinbase is not yet implemented
                Err(AppError::ProviderNotSupported)
            }
        }
    }

    /// Get a Stripe provider for the given domain and mode.
    async fn get_stripe_provider(
        &self,
        domain_id: Uuid,
        mode: PaymentMode,
    ) -> AppResult<Arc<dyn PaymentProviderPort>> {
        // Fetch the Stripe configuration
        let config = self
            .config_repo
            .get_by_domain_and_mode(domain_id, mode)
            .await?
            .ok_or(AppError::ProviderNotConfigured)?;

        // Decrypt the secret key
        let secret_key = self.cipher.decrypt(&config.stripe_secret_key_encrypted)?;

        Ok(Arc::new(StripePaymentAdapter::new(secret_key, mode)))
    }

    /// Check if a provider is configured for the given domain.
    ///
    /// For providers that don't require configuration (e.g., Dummy), this always returns true.
    pub async fn is_configured(
        &self,
        domain_id: Uuid,
        provider: PaymentProvider,
        mode: PaymentMode,
    ) -> AppResult<bool> {
        match provider {
            PaymentProvider::Dummy => {
                // Dummy is always "configured" - no external setup needed
                Ok(provider.supports_mode(mode))
            }
            PaymentProvider::Stripe => {
                let config = self
                    .config_repo
                    .get_by_domain_and_mode(domain_id, mode)
                    .await?;
                Ok(config.is_some())
            }
            PaymentProvider::Coinbase => {
                // Coinbase is not yet implemented
                Ok(false)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;
}
