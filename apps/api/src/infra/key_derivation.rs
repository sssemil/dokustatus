use secrecy::SecretString;
use uuid::Uuid;

/// Derives a JWT signing secret from an API key using HKDF-SHA256.
///
/// # Parameters
/// - `api_key`: The raw API key (e.g., "sk_live_...")
/// - `domain_id`: UUID of the domain (used as salt for domain isolation)
///
/// # Returns
/// A 32-byte secret suitable for HS256 signing, hex-encoded in a SecretString
///
/// This delegates to `reauth_types::derive_jwt_secret` for the actual derivation,
/// ensuring consistency between the API and SDKs.
pub fn derive_jwt_secret(api_key: &str, domain_id: &Uuid) -> SecretString {
    let secret = reauth_types::derive_jwt_secret(api_key, domain_id);
    SecretString::new(secret.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use secrecy::ExposeSecret;

    #[test]
    fn test_derive_jwt_secret_deterministic() {
        let api_key = "sk_live_test123";
        let domain_id = Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();

        let secret1 = derive_jwt_secret(api_key, &domain_id);
        let secret2 = derive_jwt_secret(api_key, &domain_id);

        assert_eq!(secret1.expose_secret(), secret2.expose_secret());
    }

    #[test]
    fn test_derive_jwt_secret_domain_isolation() {
        let api_key = "sk_live_test123";
        let domain_a = Uuid::new_v4();
        let domain_b = Uuid::new_v4();

        let secret_a = derive_jwt_secret(api_key, &domain_a);
        let secret_b = derive_jwt_secret(api_key, &domain_b);

        assert_ne!(secret_a.expose_secret(), secret_b.expose_secret());
    }

    #[test]
    fn test_derive_jwt_secret_key_isolation() {
        let domain_id = Uuid::new_v4();

        let secret_a = derive_jwt_secret("sk_live_key_a", &domain_id);
        let secret_b = derive_jwt_secret("sk_live_key_b", &domain_id);

        assert_ne!(secret_a.expose_secret(), secret_b.expose_secret());
    }

    #[test]
    fn test_derive_jwt_secret_output_length() {
        let api_key = "sk_live_test";
        let domain_id = Uuid::new_v4();

        let secret = derive_jwt_secret(api_key, &domain_id);
        // 32 bytes hex-encoded = 64 characters
        assert_eq!(secret.expose_secret().len(), 64);
    }

    #[test]
    fn test_derive_jwt_secret_test_vector() {
        // Test vector for cross-platform consistency
        let api_key = "sk_live_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
        let domain_id = Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();

        let secret = derive_jwt_secret(api_key, &domain_id);

        // This value should match the TypeScript SDK implementation
        // Computed once and verified in both Rust and TypeScript
        assert_eq!(
            secret.expose_secret(),
            "dfb12778c74e91b676bcce824f1da0d50a6bbd29f395a47b5d80f8ecc44682e5"
        );
    }
}
