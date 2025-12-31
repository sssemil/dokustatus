use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

use crate::app_error::{AppError, AppResult};
use crate::application::use_cases::domain_billing::SubscriptionClaims;
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
    pub domain: String,     // root domain (e.g., "example.com")
    pub roles: Vec<String>, // user's roles (e.g., ["admin", "user"])
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
