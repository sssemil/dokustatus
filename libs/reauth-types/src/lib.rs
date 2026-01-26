//! Shared types and crypto primitives for Reauth authentication.
//!
//! This crate provides:
//! - JWT claims structures (`DomainEndUserClaims`, `SubscriptionClaims`)
//! - Subscription status enum
//! - API response types
//! - HKDF key derivation and JWT verification primitives

mod claims;
mod crypto;
mod errors;
mod responses;
mod subscription;

pub use claims::{DomainEndUserClaims, SubscriptionClaims};
pub use crypto::{derive_jwt_secret, peek_domain_id, verify_jwt};
pub use errors::{ErrorCode, JwtError};
pub use responses::UserDetails;
pub use subscription::SubscriptionStatus;
