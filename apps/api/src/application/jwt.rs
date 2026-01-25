use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

use crate::app_error::{AppError, AppResult};
use crate::application::use_cases::api_key::ApiKeyWithRaw;
use crate::application::use_cases::domain_billing::SubscriptionClaims;
use crate::infra::key_derivation::derive_jwt_secret;
use secrecy::ExposeSecret;

// ============================================================================
// Workspace User Claims
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub exp: i64,
    pub iat: i64,
}

pub fn issue(user_id: Uuid, secret: &secrecy::SecretString, ttl: Duration) -> AppResult<String> {
    let now = OffsetDateTime::now_utc().unix_timestamp();
    let exp = now + ttl.whole_seconds();
    let claims = Claims {
        sub: user_id.to_string(),
        iat: now,
        exp,
    };
    let header = Header::new(Algorithm::HS256);
    encode(
        &header,
        &claims,
        &EncodingKey::from_secret(secret.expose_secret().as_bytes()),
    )
    .map_err(|e| AppError::Internal(e.to_string()))
}

pub fn verify(token: &str, secret: &secrecy::SecretString) -> AppResult<Claims> {
    let validation = Validation::new(Algorithm::HS256);
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.expose_secret().as_bytes()),
        &validation,
    )
    .map(|data| data.claims)
    .map_err(|e| AppError::Internal(e.to_string()))
}

// ============================================================================
// Domain End-User Claims
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct DomainEndUserClaims {
    pub sub: String, // end_user_id
    pub domain_id: String,
    pub domain: String,                   // root domain (e.g., "example.com")
    pub roles: Vec<String>,               // user's roles (e.g., ["admin", "user"])
    pub subscription: SubscriptionClaims, // subscription info (always present)
    pub exp: i64,
    pub iat: i64,
}

pub fn issue_domain_end_user(
    end_user_id: Uuid,
    domain_id: Uuid,
    domain: &str,
    roles: Vec<String>,
    subscription: SubscriptionClaims,
    secret: &secrecy::SecretString,
    ttl: Duration,
) -> AppResult<String> {
    let now = OffsetDateTime::now_utc().unix_timestamp();
    let exp = now + ttl.whole_seconds();

    // Use root domain in claim
    let root_domain = crate::application::helpers::domain_parsing::get_root_domain(domain);

    let claims = DomainEndUserClaims {
        sub: end_user_id.to_string(),
        domain_id: domain_id.to_string(),
        domain: root_domain,
        roles,
        subscription,
        iat: now,
        exp,
    };
    let header = Header::new(Algorithm::HS256);
    encode(
        &header,
        &claims,
        &EncodingKey::from_secret(secret.expose_secret().as_bytes()),
    )
    .map_err(|e| AppError::Internal(e.to_string()))
}

pub fn verify_domain_end_user(
    token: &str,
    secret: &secrecy::SecretString,
) -> AppResult<DomainEndUserClaims> {
    let validation = Validation::new(Algorithm::HS256);
    decode::<DomainEndUserClaims>(
        token,
        &DecodingKey::from_secret(secret.expose_secret().as_bytes()),
        &validation,
    )
    .map(|data| data.claims)
    .map_err(|e| AppError::Internal(e.to_string()))
}

// ============================================================================
// Derived Secret JWT Functions (from API Key via HKDF)
// ============================================================================

/// Issue a domain end-user JWT using a secret derived from an API key.
/// Includes `kid` header for efficient key lookup during verification.
pub fn issue_domain_end_user_derived(
    end_user_id: Uuid,
    domain_id: Uuid,
    domain: &str,
    roles: Vec<String>,
    subscription: SubscriptionClaims,
    api_key: &ApiKeyWithRaw,
    ttl: Duration,
) -> AppResult<String> {
    let now = OffsetDateTime::now_utc().unix_timestamp();
    let exp = now + ttl.whole_seconds();

    // Use root domain in claim
    let root_domain = crate::application::helpers::domain_parsing::get_root_domain(domain);

    let claims = DomainEndUserClaims {
        sub: end_user_id.to_string(),
        domain_id: domain_id.to_string(),
        domain: root_domain,
        roles,
        subscription,
        iat: now,
        exp,
    };

    // Include kid header for O(1) key lookup during verification
    let mut header = Header::new(Algorithm::HS256);
    header.kid = Some(api_key.id.to_string());

    // Derive secret from API key using HKDF
    let secret = derive_jwt_secret(&api_key.raw_key, &domain_id);

    encode(
        &header,
        &claims,
        &EncodingKey::from_secret(secret.expose_secret().as_bytes()),
    )
    .map_err(|e| AppError::Internal(e.to_string()))
}

/// Verify a domain end-user JWT using multiple API keys.
/// Tries the key matching `kid` first (if present), then falls back to all keys.
pub fn verify_domain_end_user_multi(
    token: &str,
    keys: &[ApiKeyWithRaw],
) -> AppResult<DomainEndUserClaims> {
    if keys.is_empty() {
        return Err(AppError::InvalidCredentials);
    }

    // Try to decode header to get kid for efficient lookup
    let header = jsonwebtoken::decode_header(token)
        .map_err(|e| AppError::Internal(format!("Invalid token header: {}", e)))?;

    // If kid is present, try that key first
    if let Some(kid) = &header.kid {
        if let Ok(kid_uuid) = Uuid::parse_str(kid) {
            if let Some(key) = keys.iter().find(|k| k.id == kid_uuid) {
                let secret = derive_jwt_secret(&key.raw_key, &key.domain_id);
                if let Ok(claims) = verify_domain_end_user(token, &secret) {
                    return Ok(claims);
                }
            }
        }
    }

    // Fall back to trying all keys
    for key in keys {
        let secret = derive_jwt_secret(&key.raw_key, &key.domain_id);
        if let Ok(claims) = verify_domain_end_user(token, &secret) {
            return Ok(claims);
        }
    }

    Err(AppError::InvalidCredentials)
}

/// Peek at the domain_id claim without verifying the signature.
/// Used to determine which domain's keys to fetch for verification.
pub fn peek_domain_id_from_token(token: &str) -> AppResult<Uuid> {
    // Decode without verification to extract domain_id
    let mut validation = Validation::new(Algorithm::HS256);
    validation.insecure_disable_signature_validation();
    validation.validate_exp = false;
    validation.validate_aud = false;

    let token_data = decode::<DomainEndUserClaims>(
        token,
        &DecodingKey::from_secret(b"ignored"), // Key is ignored when validation is disabled
        &validation,
    )
    .map_err(|e| AppError::Internal(format!("Invalid token format: {}", e)))?;

    Uuid::parse_str(&token_data.claims.domain_id)
        .map_err(|e| AppError::Internal(format!("Invalid domain_id in token: {}", e)))
}

#[cfg(test)]
mod derived_jwt_tests {
    use super::*;

    fn test_subscription() -> SubscriptionClaims {
        SubscriptionClaims {
            status: "active".to_string(),
            plan_code: Some("pro".to_string()),
            plan_name: Some("Pro Plan".to_string()),
            current_period_end: Some(1735689600),
            cancel_at_period_end: Some(false),
            trial_ends_at: None,
            subscription_id: Some("sub_test123".to_string()),
        }
    }

    fn test_api_key(domain_id: Uuid) -> ApiKeyWithRaw {
        ApiKeyWithRaw {
            id: Uuid::new_v4(),
            domain_id,
            raw_key: "sk_live_test123".to_string(),
        }
    }

    #[test]
    fn test_issue_and_verify_derived_jwt() {
        let user_id = Uuid::new_v4();
        let domain_id = Uuid::new_v4();
        let api_key = test_api_key(domain_id);

        // Issue token
        let token = issue_domain_end_user_derived(
            user_id,
            domain_id,
            "example.com",
            vec!["user".to_string()],
            test_subscription(),
            &api_key,
            Duration::hours(1),
        )
        .unwrap();

        // Verify with same key
        let keys = vec![api_key];
        let claims = verify_domain_end_user_multi(&token, &keys).unwrap();

        assert_eq!(claims.sub, user_id.to_string());
        assert_eq!(claims.domain_id, domain_id.to_string());
        assert_eq!(claims.domain, "example.com");
        assert_eq!(claims.roles, vec!["user"]);
    }

    #[test]
    fn test_verify_rejects_wrong_api_key() {
        let user_id = Uuid::new_v4();
        let domain_id = Uuid::new_v4();
        let signing_key = ApiKeyWithRaw {
            id: Uuid::new_v4(),
            domain_id,
            raw_key: "sk_live_correct".to_string(),
        };
        let wrong_key = ApiKeyWithRaw {
            id: Uuid::new_v4(),
            domain_id,
            raw_key: "sk_live_wrong".to_string(),
        };

        let token = issue_domain_end_user_derived(
            user_id,
            domain_id,
            "example.com",
            vec![],
            test_subscription(),
            &signing_key,
            Duration::hours(1),
        )
        .unwrap();

        // Verification with wrong key fails
        let result = verify_domain_end_user_multi(&token, &[wrong_key]);
        assert!(result.is_err());
    }

    #[test]
    fn test_verify_rejects_wrong_domain() {
        let user_id = Uuid::new_v4();
        let domain_a = Uuid::new_v4();
        let domain_b = Uuid::new_v4();
        let api_key_a = test_api_key(domain_a);
        let api_key_b = test_api_key(domain_b);

        let token = issue_domain_end_user_derived(
            user_id,
            domain_a,
            "a.com",
            vec![],
            test_subscription(),
            &api_key_a,
            Duration::hours(1),
        )
        .unwrap();

        // Same API key but different domain fails (different salt = different secret)
        let result = verify_domain_end_user_multi(&token, &[api_key_b]);
        assert!(result.is_err());
    }

    #[test]
    fn test_verify_multi_tries_all_keys() {
        let user_id = Uuid::new_v4();
        let domain_id = Uuid::new_v4();
        let old_key = ApiKeyWithRaw {
            id: Uuid::new_v4(),
            domain_id,
            raw_key: "sk_live_old".to_string(),
        };
        let new_key = ApiKeyWithRaw {
            id: Uuid::new_v4(),
            domain_id,
            raw_key: "sk_live_new".to_string(),
        };

        // Token signed with old key
        let token = issue_domain_end_user_derived(
            user_id,
            domain_id,
            "example.com",
            vec![],
            test_subscription(),
            &old_key,
            Duration::hours(1),
        )
        .unwrap();

        // Verify with multiple keys (new key first, but old key works)
        let keys = vec![new_key, old_key];
        let claims = verify_domain_end_user_multi(&token, &keys).unwrap();
        assert_eq!(claims.sub, user_id.to_string());
    }

    #[test]
    fn test_verify_multi_fails_when_no_matching_key() {
        let user_id = Uuid::new_v4();
        let domain_id = Uuid::new_v4();
        let actual_key = ApiKeyWithRaw {
            id: Uuid::new_v4(),
            domain_id,
            raw_key: "sk_live_actual".to_string(),
        };
        let wrong_key = ApiKeyWithRaw {
            id: Uuid::new_v4(),
            domain_id,
            raw_key: "sk_live_wrong".to_string(),
        };

        let token = issue_domain_end_user_derived(
            user_id,
            domain_id,
            "example.com",
            vec![],
            test_subscription(),
            &actual_key,
            Duration::hours(1),
        )
        .unwrap();

        // Verification with different key fails
        let result = verify_domain_end_user_multi(&token, &[wrong_key]);
        assert!(result.is_err());
    }

    #[test]
    fn test_jwt_includes_kid_header() {
        let user_id = Uuid::new_v4();
        let domain_id = Uuid::new_v4();
        let api_key_id = Uuid::new_v4();
        let api_key = ApiKeyWithRaw {
            id: api_key_id,
            domain_id,
            raw_key: "sk_live_test".to_string(),
        };

        let token = issue_domain_end_user_derived(
            user_id,
            domain_id,
            "example.com",
            vec![],
            test_subscription(),
            &api_key,
            Duration::hours(1),
        )
        .unwrap();

        // Decode header without verification
        let header = jsonwebtoken::decode_header(&token).unwrap();
        assert_eq!(header.kid, Some(api_key_id.to_string()));
    }

    #[test]
    fn test_peek_domain_id_from_token() {
        let user_id = Uuid::new_v4();
        let domain_id = Uuid::new_v4();
        let api_key = test_api_key(domain_id);

        let token = issue_domain_end_user_derived(
            user_id,
            domain_id,
            "example.com",
            vec![],
            test_subscription(),
            &api_key,
            Duration::hours(1),
        )
        .unwrap();

        // Peek extracts domain_id without verification
        let peeked_domain_id = peek_domain_id_from_token(&token).unwrap();
        assert_eq!(peeked_domain_id, domain_id);
    }

    #[test]
    fn test_verify_multi_empty_keys_fails() {
        let result = verify_domain_end_user_multi("some.token.here", &[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_kid_provides_fast_lookup() {
        let user_id = Uuid::new_v4();
        let domain_id = Uuid::new_v4();
        let matching_key = ApiKeyWithRaw {
            id: Uuid::new_v4(),
            domain_id,
            raw_key: "sk_live_match".to_string(),
        };
        let other_key = ApiKeyWithRaw {
            id: Uuid::new_v4(),
            domain_id,
            raw_key: "sk_live_other".to_string(),
        };

        let token = issue_domain_end_user_derived(
            user_id,
            domain_id,
            "example.com",
            vec![],
            test_subscription(),
            &matching_key,
            Duration::hours(1),
        )
        .unwrap();

        // The matching key is second in the list, but kid header should find it directly
        let keys = vec![other_key, matching_key];
        let claims = verify_domain_end_user_multi(&token, &keys).unwrap();
        assert_eq!(claims.sub, user_id.to_string());
    }
}
