//! Rust SDK for Reauth authentication.
//!
//! This SDK provides server-side authentication for Rust backends integrating with Reauth.
//!
//! # Features
//!
//! - **Token verification** - Verify JWTs locally using HKDF-derived secrets (no network calls)
//! - **Token extraction** - Extract tokens from Authorization headers or cookies
//! - **Developer API** - Fetch full user details via the Developer API
//!
//! # Example
//!
//! ```rust,ignore
//! use reauth_sdk::{ReauthClient, ReauthConfig};
//!
//! let client = ReauthClient::new(ReauthConfig {
//!     domain: "example.com".to_string(),
//!     api_key: "sk_live_...".to_string(),
//!     clock_skew_seconds: None,
//! })?;
//!
//! // Verify a token
//! let claims = client.verify_token("eyJ...")?;
//! println!("User ID: {}", claims.sub);
//! ```

mod client;
mod error;
mod extract;

pub use client::{ReauthClient, ReauthConfig};
pub use error::ReauthError;
pub use extract::Headers;

// Re-export shared types for convenience
pub use reauth_types::{
    DomainEndUserClaims, JwtError, SubscriptionClaims, SubscriptionStatus, UserDetails,
};
