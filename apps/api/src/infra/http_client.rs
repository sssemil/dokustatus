//! HTTP client factory with consistent timeout configuration.
//!
//! All HTTP clients in the API should use this module to ensure proper timeout
//! behavior. New HTTP clients MUST use `build_client()` or `try_build_client()`
//! rather than constructing `reqwest::Client` directly.

use reqwest::Client;
use std::time::Duration;

/// Default connect timeout (TCP handshake + TLS).
pub const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// Default request timeout (total request/response time).
///
/// This is appropriate for external API calls (Stripe, Resend, Google OAuth)
/// which are expected to complete within seconds. If a future use case requires
/// longer timeouts (e.g., file uploads), create a separate builder with an
/// explicit extended timeout.
pub const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Build an HTTP client with default timeouts.
///
/// Panics if the client cannot be built (e.g., TLS misconfiguration).
/// This is acceptable for singleton constructors (StripeClient, DomainEmailSender)
/// since the app cannot function without HTTP clients.
///
/// For request-scoped client creation, prefer `try_build_client()`.
pub fn build_client() -> Client {
    Client::builder()
        .connect_timeout(DEFAULT_CONNECT_TIMEOUT)
        .timeout(DEFAULT_REQUEST_TIMEOUT)
        .build()
        .expect("Failed to build HTTP client")
}

/// Build an HTTP client with default timeouts, returning Result for use in
/// fallible contexts (e.g., request handlers).
pub fn try_build_client() -> Result<Client, reqwest::Error> {
    Client::builder()
        .connect_timeout(DEFAULT_CONNECT_TIMEOUT)
        .timeout(DEFAULT_REQUEST_TIMEOUT)
        .build()
}
