# Plan: Add HTTP Client Timeouts

**Task:** 0008-http-client-timeouts
**Status:** Draft v3
**Created:** 2026-01-01
**Revised:** 2026-01-01

## Summary

All HTTP clients in the codebase currently use `reqwest::Client::new()` which does not configure any request timeouts. This means that if a remote service (Stripe, Resend, Google OAuth) becomes unresponsive, requests can hang indefinitely, blocking server resources and potentially causing cascading failures.

The fix involves creating a shared HTTP client builder helper and applying consistent timeout configurations across all client instantiation points.

## Discovery Verification

### Expanded Search Patterns

Searched for all reqwest usages with multiple patterns to ensure complete coverage:

```bash
rg 'Client::new\(\)|Client::builder\(\)' apps/ libs/
rg 'reqwest::get\(|reqwest::blocking|ClientBuilder' apps/ libs/
rg 'use reqwest' apps/ libs/
rg 'reqwest' --type toml
```

### Identified HTTP Clients

| Location | Type | Usage |
|----------|------|-------|
| `apps/api/src/infra/stripe_client.rs:18` | `Client::new()` | StripeClient constructor |
| `apps/api/src/infra/domain_email.rs:20` | `Client::new()` | DomainEmailSender constructor (Resend API) |
| `apps/api/src/adapters/http/routes/public_domain_auth.rs:2274` | `reqwest::Client::new()` | Google OAuth token exchange |
| `apps/api/src/adapters/http/routes/public_domain_auth.rs:2344` | `reqwest::Client::new()` | Google JWKS fetch |

### Exclusions Confirmed

- **No `reqwest::get()` or `reqwest::blocking`** usage found anywhere
- **No `ClientBuilder` aliases** or re-exports
- **`libs/` contains no reqwest usages** – the TypeScript SDK does not use Rust HTTP clients
- **No other Rust binaries** in the repo use reqwest
- **Cargo.toml** shows reqwest is only a dependency of `apps/api`

## Implementation Approach

### Step 1: Create Shared HTTP Client Builder

Create `apps/api/src/infra/http_client.rs` with a reusable builder that applies consistent timeout defaults:

```rust
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
```

Register in `apps/api/src/infra/mod.rs`:
```rust
pub mod http_client;
```

### Step 2: Update StripeClient

Modify `apps/api/src/infra/stripe_client.rs`:

```rust
use crate::infra::http_client;

impl StripeClient {
    pub fn new(secret_key: String) -> Self {
        Self {
            client: http_client::build_client(),
            secret_key,
        }
    }
}
```

Remove `use reqwest::Client;` (not needed after change).

### Step 3: Update DomainEmailSender

Modify `apps/api/src/infra/domain_email.rs`:

```rust
use crate::infra::http_client;

impl DomainEmailSender {
    pub fn new() -> Self {
        Self {
            client: http_client::build_client(),
        }
    }
}
```

Keep `use reqwest::Client;` for the struct field type.

### Step 4: Update Google OAuth Functions

Modify `apps/api/src/adapters/http/routes/public_domain_auth.rs`:

**Decision: Use inline `try_build_client()` for OAuth functions.**

Rationale: These functions are called infrequently (only on OAuth callback). Creating a shared client in app state would require threading it through multiple layers for minimal benefit. The inline approach is simpler and still provides proper timeouts.

```rust
use crate::infra::http_client;
use tracing::error;

async fn exchange_google_code(
    code: &str,
    client_id: &str,
    client_secret: &str,
    redirect_uri: &str,
    code_verifier: &str,
) -> AppResult<GoogleTokenResponse> {
    let client = http_client::try_build_client().map_err(|e| {
        error!(error = %e, "Failed to build HTTP client for Google OAuth token exchange");
        AppError::Internal("Failed to build HTTP client".into())
    })?;
    // ... rest unchanged
}

async fn fetch_google_jwks() -> AppResult<GoogleJwks> {
    let client = http_client::try_build_client().map_err(|e| {
        error!(error = %e, "Failed to build HTTP client for Google JWKS fetch");
        AppError::Internal("Failed to build HTTP client".into())
    })?;
    // ... rest unchanged
}
```

**Note on logging:** Failures are logged with `tracing::error!` to ensure 500s have a traceable cause in logs. The error message returned to clients is generic to avoid leaking internal details.

## Files to Modify

| File | Change |
|------|--------|
| `apps/api/src/infra/http_client.rs` | **NEW**: Shared client builder with timeout constants |
| `apps/api/src/infra/mod.rs` | Add `pub mod http_client;` |
| `apps/api/src/infra/stripe_client.rs` | Use `http_client::build_client()` |
| `apps/api/src/infra/domain_email.rs` | Use `http_client::build_client()` |
| `apps/api/src/adapters/http/routes/public_domain_auth.rs` | Use `http_client::try_build_client()` with logging in 2 functions |

## Timeout Values

| Timeout | Value | Rationale |
|---------|-------|-----------|
| Connect timeout | 5 seconds | DNS + TCP + TLS handshake should complete quickly; if not, the service is likely unreachable |
| Request timeout | 30 seconds | Stripe/Resend/Google APIs should respond within seconds; 30s provides margin for slow operations while preventing indefinite hangs |

### Expected Response Times

All current HTTP clients target fast external APIs:

| Endpoint | Expected Response | 30s Appropriate? |
|----------|------------------|------------------|
| Stripe API (payments, subscriptions) | < 2s typical | Yes |
| Resend API (email sending) | < 5s typical | Yes |
| Google OAuth token exchange | < 1s typical | Yes |
| Google JWKS fetch | < 1s typical | Yes |

If a future use case requires longer timeouts (e.g., large file uploads, batch operations), a separate builder with extended timeout should be created rather than increasing the default.

### Configurability Decision

**Hardcoded values are acceptable for this task.** Rationale:
- 5s/30s are conservative defaults used by many HTTP clients
- These services (Stripe, Resend, Google) have well-documented SLAs and rarely require tuning
- Adding env-based configuration adds complexity without clear benefit today
- If tuning is needed later, the centralized `http_client.rs` module makes it trivial to add env vars

Future enhancement (out of scope): Add `HTTP_CONNECT_TIMEOUT_SECS` and `HTTP_REQUEST_TIMEOUT_SECS` env vars if production monitoring shows need.

## Testing Approach

1. **Compile-time verification:**
   - Run `./run api:build` to ensure code compiles

2. **Existing test suite:**
   - Run `./run api:test` to verify no regressions
   - Stripe webhook signature tests in `stripe_client.rs` do not exercise network; they will pass unchanged

3. **Runtime verification (manual):**
   - With local infra running, exercise OAuth flow and email sending to confirm clients work
   - Timeout behavior itself is structural and doesn't require explicit testing

## Operational Notes

### Startup Panic on TLS Misconfiguration

`build_client()` uses `.expect()`, which will panic at startup if:
- System TLS certificates are missing or invalid
- Native TLS backend is misconfigured

This is intentional: the app cannot function without HTTP clients, and failing fast at startup is preferable to failing later in production with cryptic errors.

**Mitigation for CI/dev:** Both the CI Docker images and local dev environments include standard root CA certificates. The `rust:1.xx` base images and standard Linux distros ship with `ca-certificates`. No special handling is needed.

For `try_build_client()` used in request handlers, failures are converted to `AppError::Internal` and returned as 500s, with errors logged for traceability.

### Connection Pooling

- `StripeClient` and `DomainEmailSender` are singletons created at startup; they benefit from connection pooling automatically
- Google OAuth inline clients are per-request; no pooling benefit, but this is acceptable given low call frequency

**Future consideration (out of scope):** If OAuth traffic grows significantly and connection pooling becomes valuable, consider moving the OAuth client to shared app state. This would be a straightforward refactor since `http_client::build_client()` is already centralized.

### Preventing Drift

The module-level doc comment in `http_client.rs` explicitly instructs future developers to use this module for all new HTTP clients. This documentation serves as a guardrail against introducing new unguarded `reqwest::Client::new()` calls.

## Checklist

- [ ] Verify expanded discovery patterns (confirm no additional reqwest usages)
- [ ] Create `apps/api/src/infra/http_client.rs` with shared builder and module docs
- [ ] Register `http_client` module in `apps/api/src/infra/mod.rs`
- [ ] Update `stripe_client.rs` to use `http_client::build_client()`
- [ ] Update `domain_email.rs` to use `http_client::build_client()`
- [ ] Update `public_domain_auth.rs` Google OAuth clients with `http_client::try_build_client()` and logging
- [ ] Run `./run api:build` to verify compilation
- [ ] Run `./run api:test` to verify existing tests pass
- [ ] Move task to done

## Changes from v2

- **Expanded discovery:** Added `reqwest::get()`, `reqwest::blocking`, `ClientBuilder`, and crate-wide reqwest search patterns. Confirmed `libs/` has no Rust HTTP clients.
- **Enhanced module documentation:** Added module-level doc comment to `http_client.rs` instructing future developers to use this module, preventing drift.
- **Added logging:** `try_build_client()` failures in request handlers are now logged with `tracing::error!` for traceability.
- **Expected response times:** Added table documenting which endpoints are covered and confirming 30s is appropriate for all.
- **TLS mitigation note:** Added note confirming CI/dev images include root certs, addressing panic concern.
- **Future pooling note:** Added consideration for moving OAuth clients to shared state if traffic grows.

## History

- 2026-01-01 07:00 Created plan-v1.md with detailed implementation approach
- 2026-01-01 07:15 Created plan-v2.md addressing feedback: added discovery verification, shared http_client module, decided on OAuth approach, documented configurability decision
- 2026-01-01 07:30 Created plan-v3.md addressing feedback: expanded discovery patterns, enhanced module documentation, added error logging, documented expected response times, clarified TLS/pooling concerns
