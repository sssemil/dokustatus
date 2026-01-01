# Plan: Add HTTP Client Timeouts

**Task:** 0008-http-client-timeouts
**Status:** Draft v2
**Created:** 2026-01-01
**Revised:** 2026-01-01

## Summary

All HTTP clients in the codebase currently use `reqwest::Client::new()` which does not configure any request timeouts. This means that if a remote service (Stripe, Resend, Google OAuth) becomes unresponsive, requests can hang indefinitely, blocking server resources and potentially causing cascading failures.

The fix involves creating a shared HTTP client builder helper and applying consistent timeout configurations across all client instantiation points.

## Discovery Verification

Searched for all HTTP client usages with `rg 'Client::new\(\)|Client::builder\(\)'` and `rg 'reqwest::Client'`:

| Location | Type | Usage |
|----------|------|-------|
| `apps/api/src/infra/stripe_client.rs:18` | `Client::new()` | StripeClient constructor |
| `apps/api/src/infra/domain_email.rs:20` | `Client::new()` | DomainEmailSender constructor (Resend API) |
| `apps/api/src/adapters/http/routes/public_domain_auth.rs:2274` | `reqwest::Client::new()` | Google OAuth token exchange |
| `apps/api/src/adapters/http/routes/public_domain_auth.rs:2344` | `reqwest::Client::new()` | Google JWKS fetch |

**Exclusions:** No other HTTP clients found in `apps/*` or `libs/*`.

## Implementation Approach

### Step 1: Create Shared HTTP Client Builder

Create `apps/api/src/infra/http_client.rs` with a reusable builder that applies consistent timeout defaults:

```rust
use reqwest::Client;
use std::time::Duration;

/// Default connect timeout (TCP handshake + TLS)
pub const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// Default request timeout (total request/response time)
pub const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Build an HTTP client with default timeouts.
///
/// Panics if the client cannot be built (e.g., TLS misconfiguration).
/// This is acceptable for constructors since the app cannot function without HTTP clients.
pub fn build_client() -> Client {
    Client::builder()
        .connect_timeout(DEFAULT_CONNECT_TIMEOUT)
        .timeout(DEFAULT_REQUEST_TIMEOUT)
        .build()
        .expect("Failed to build HTTP client")
}

/// Build an HTTP client with default timeouts, returning Result for use in fallible contexts.
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

async fn exchange_google_code(
    code: &str,
    client_id: &str,
    client_secret: &str,
    redirect_uri: &str,
    code_verifier: &str,
) -> AppResult<GoogleTokenResponse> {
    let client = http_client::try_build_client()
        .map_err(|e| AppError::Internal(format!("Failed to build HTTP client: {}", e)))?;
    // ... rest unchanged
}

async fn fetch_google_jwks() -> AppResult<GoogleJwks> {
    let client = http_client::try_build_client()
        .map_err(|e| AppError::Internal(format!("Failed to build HTTP client: {}", e)))?;
    // ... rest unchanged
}
```

## Files to Modify

| File | Change |
|------|--------|
| `apps/api/src/infra/http_client.rs` | **NEW**: Shared client builder with timeout constants |
| `apps/api/src/infra/mod.rs` | Add `pub mod http_client;` |
| `apps/api/src/infra/stripe_client.rs` | Use `http_client::build_client()` |
| `apps/api/src/infra/domain_email.rs` | Use `http_client::build_client()` |
| `apps/api/src/adapters/http/routes/public_domain_auth.rs` | Use `http_client::try_build_client()` in 2 functions |

## Timeout Values

| Timeout | Value | Rationale |
|---------|-------|-----------|
| Connect timeout | 5 seconds | DNS + TCP + TLS handshake should complete quickly; if not, the service is likely unreachable |
| Request timeout | 30 seconds | Stripe/Resend/Google APIs should respond within seconds; 30s provides margin for slow operations while preventing indefinite hangs |

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

For `try_build_client()` used in request handlers, failures are converted to `AppError::Internal` and returned as 500s.

### Connection Pooling

- `StripeClient` and `DomainEmailSender` are singletons created at startup; they benefit from connection pooling automatically
- Google OAuth inline clients are per-request; no pooling benefit, but this is acceptable given low call frequency

## Checklist

- [ ] Create `apps/api/src/infra/http_client.rs` with shared builder
- [ ] Register `http_client` module in `apps/api/src/infra/mod.rs`
- [ ] Update `stripe_client.rs` to use `http_client::build_client()`
- [ ] Update `domain_email.rs` to use `http_client::build_client()`
- [ ] Update `public_domain_auth.rs` Google OAuth clients with `http_client::try_build_client()`
- [ ] Run `./run api:build` to verify compilation
- [ ] Run `./run api:test` to verify existing tests pass
- [ ] Move task to done

## Changes from v1

- **Added:** Discovery verification step confirming all HTTP client locations
- **Added:** Shared `http_client.rs` module to centralize timeout configuration and prevent drift
- **Clarified:** Decided on inline approach for Google OAuth with explicit rationale
- **Clarified:** Timeout configurability decision with justification for hardcoding
- **Added:** Operational notes on startup panics and connection pooling

## History

- 2026-01-01 07:00 Created plan-v1.md with detailed implementation approach
- 2026-01-01 07:15 Created plan-v2.md addressing feedback: added discovery verification, shared http_client module, decided on OAuth approach, documented configurability decision
