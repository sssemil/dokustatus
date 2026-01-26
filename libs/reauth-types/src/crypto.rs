use hkdf::Hkdf;
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use sha2::Sha256;
use uuid::Uuid;

use crate::{DomainEndUserClaims, JwtError};

/// Derives a JWT signing secret from an API key using HKDF-SHA256.
///
/// # Arguments
/// * `api_key` - The raw API key (e.g., "sk_live_...")
/// * `domain_id` - UUID of the domain (used as salt for domain isolation)
///
/// # Returns
/// A 64-character hex-encoded string representing 32 bytes.
/// This string's ASCII bytes are used as the JWT signing secret.
///
/// # Example
/// ```
/// use uuid::Uuid;
/// use reauth_types::derive_jwt_secret;
///
/// let api_key = "sk_live_test123";
/// let domain_id = Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();
/// let secret = derive_jwt_secret(api_key, &domain_id);
/// assert_eq!(secret.len(), 64); // 32 bytes hex-encoded
/// ```
pub fn derive_jwt_secret(api_key: &str, domain_id: &Uuid) -> String {
    let salt = domain_id.as_bytes();
    let info = b"reauth-jwt-v1";

    let hk = Hkdf::<Sha256>::new(Some(salt), api_key.as_bytes());
    let mut output = [0u8; 32];
    hk.expand(info, &mut output)
        .expect("32 bytes is valid for SHA256 HKDF expand");

    hex::encode(output)
}

/// Verifies a JWT token and returns the claims.
///
/// # Arguments
/// * `token` - The JWT token string
/// * `secret` - The hex-encoded HKDF output (64 chars) from `derive_jwt_secret`
/// * `clock_skew_seconds` - Tolerance for clock skew (typically 60 seconds)
///
/// # Returns
/// The verified `DomainEndUserClaims` or a `JwtError`.
pub fn verify_jwt(
    token: &str,
    secret: &str,
    clock_skew_seconds: u64,
) -> Result<DomainEndUserClaims, JwtError> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.leeway = clock_skew_seconds;

    let token_data = decode::<DomainEndUserClaims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )?;

    Ok(token_data.claims)
}

/// Peeks at the domain_id claim without verifying the signature.
///
/// Used to determine which domain's keys to fetch for verification.
///
/// # Security Note
/// This function does NOT verify the token. The returned domain_id
/// should only be used to fetch potential signing keys, then the
/// token must be properly verified with `verify_jwt`.
pub fn peek_domain_id(token: &str) -> Result<Uuid, JwtError> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.insecure_disable_signature_validation();
    validation.validate_exp = false;

    let token_data = decode::<DomainEndUserClaims>(
        token,
        &DecodingKey::from_secret(b"ignored"),
        &validation,
    )?;

    Uuid::parse_str(&token_data.claims.domain_id)
        .map_err(|_| JwtError::InvalidClaims("Invalid domain_id format".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_derive_jwt_secret_deterministic() {
        let api_key = "sk_live_test123";
        let domain_id = Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();

        let secret1 = derive_jwt_secret(api_key, &domain_id);
        let secret2 = derive_jwt_secret(api_key, &domain_id);

        assert_eq!(secret1, secret2);
    }

    #[test]
    fn test_derive_jwt_secret_domain_isolation() {
        let api_key = "sk_live_test123";
        let domain_a = Uuid::new_v4();
        let domain_b = Uuid::new_v4();

        let secret_a = derive_jwt_secret(api_key, &domain_a);
        let secret_b = derive_jwt_secret(api_key, &domain_b);

        assert_ne!(secret_a, secret_b);
    }

    #[test]
    fn test_derive_jwt_secret_key_isolation() {
        let domain_id = Uuid::new_v4();

        let secret_a = derive_jwt_secret("sk_live_key_a", &domain_id);
        let secret_b = derive_jwt_secret("sk_live_key_b", &domain_id);

        assert_ne!(secret_a, secret_b);
    }

    #[test]
    fn test_derive_jwt_secret_output_length() {
        let api_key = "sk_live_test";
        let domain_id = Uuid::new_v4();

        let secret = derive_jwt_secret(api_key, &domain_id);
        // 32 bytes hex-encoded = 64 characters
        assert_eq!(secret.len(), 64);
    }

    #[test]
    fn test_derive_jwt_secret_test_vector() {
        // Test vector for cross-platform consistency
        // This MUST match the TypeScript SDK and Rust API implementations
        let api_key = "sk_live_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
        let domain_id = Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();

        let secret = derive_jwt_secret(api_key, &domain_id);

        assert_eq!(
            secret,
            "dfb12778c74e91b676bcce824f1da0d50a6bbd29f395a47b5d80f8ecc44682e5"
        );
    }
}
