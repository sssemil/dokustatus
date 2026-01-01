# Plan: Add HTTP Client Timeouts

**Task:** 0008-http-client-timeouts
**Status:** Draft v1
**Created:** 2026-01-01

## Summary

All HTTP clients in the codebase currently use `reqwest::Client::new()` which does not configure any request timeouts. This means that if a remote service (Stripe, Resend, Google OAuth) becomes unresponsive, requests can hang indefinitely, blocking server resources and potentially causing cascading failures.

The fix involves adding sensible timeout and connect timeout configurations to all HTTP client constructors.

## Current State

### Files with HTTP Clients

1. **`apps/api/src/infra/stripe_client.rs:17-18`** - `StripeClient::new()`
   - Uses `Client::new()` with no timeout
   - Called from multiple locations (7 call sites)

2. **`apps/api/src/infra/domain_email.rs:18-20`** - `DomainEmailSender::new()`
   - Uses `Client::new()` with no timeout
   - Singleton created in `setup.rs:49`

3. **`apps/api/src/adapters/http/routes/public_domain_auth.rs:2274`** - `exchange_google_code()`
   - Creates inline `reqwest::Client::new()` for Google OAuth token exchange

4. **`apps/api/src/adapters/http/routes/public_domain_auth.rs:2344`** - `fetch_google_jwks()`
   - Creates inline `reqwest::Client::new()` for fetching Google public keys

## Implementation Approach

### Step 1: Define Timeout Constants

Add timeout constants to a central location. Reasonable defaults:
- **Connect timeout:** 5 seconds (time to establish TCP connection)
- **Request timeout:** 30 seconds (total time for request/response)

Location: Add to `apps/api/src/infra/mod.rs` or create a new `apps/api/src/infra/http_client.rs` module with a builder helper.

### Step 2: Update StripeClient

Modify `apps/api/src/infra/stripe_client.rs`:

```rust
impl StripeClient {
    pub fn new(secret_key: String) -> Self {
        Self {
            client: Client::builder()
                .connect_timeout(Duration::from_secs(5))
                .timeout(Duration::from_secs(30))
                .build()
                .expect("Failed to build HTTP client"),
            secret_key,
        }
    }
}
```

Add `use std::time::Duration;` at the top.

### Step 3: Update DomainEmailSender

Modify `apps/api/src/infra/domain_email.rs`:

```rust
impl DomainEmailSender {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .connect_timeout(Duration::from_secs(5))
                .timeout(Duration::from_secs(30))
                .build()
                .expect("Failed to build HTTP client"),
        }
    }
}
```

Add `use std::time::Duration;` at the top.

### Step 4: Update Google OAuth Functions

Modify `apps/api/src/adapters/http/routes/public_domain_auth.rs`:

Option A (inline fix):
```rust
async fn exchange_google_code(...) -> AppResult<GoogleTokenResponse> {
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| AppError::Internal(format!("Failed to build HTTP client: {}", e)))?;
    // ...
}
```

Option B (better - extract shared client): Create a helper function or reuse a client from app state. Since these functions are called infrequently (on OAuth callback), the inline approach is acceptable.

Add `use std::time::Duration;` at the top.

## Files to Modify

| File | Change |
|------|--------|
| `apps/api/src/infra/stripe_client.rs` | Update `new()` to use `Client::builder()` with timeouts |
| `apps/api/src/infra/domain_email.rs` | Update `new()` to use `Client::builder()` with timeouts |
| `apps/api/src/adapters/http/routes/public_domain_auth.rs` | Update 2 inline `Client::new()` calls in `exchange_google_code()` and `fetch_google_jwks()` |

## Timeout Values

| Timeout | Value | Rationale |
|---------|-------|-----------|
| Connect timeout | 5 seconds | DNS + TCP handshake should complete quickly; if not, the service is likely unreachable |
| Request timeout | 30 seconds | Stripe/Resend/Google APIs should respond within seconds; 30s provides margin for slow operations while preventing indefinite hangs |

## Testing Approach

1. **Unit tests are not strictly necessary** - The existing tests in `stripe_client.rs` verify signature validation, not network behavior. Timeout configuration is a structural change that can be verified by code review.

2. **Manual verification:**
   - Run `./run api:build` to ensure code compiles
   - Run `./run api:test` to ensure existing tests pass
   - Optionally test timeout behavior with a mock server (out of scope for this task)

3. **Existing test suite:** Run `cargo test` in `apps/api` to verify no regressions.

## Edge Cases

1. **Client builder failure:** `Client::builder().build()` can fail (e.g., TLS issues). Use `.expect()` for struct constructors since app can't function without HTTP clients. For inline functions, use `map_err()` to convert to `AppError`.

2. **Timeout too short:** 30 seconds should be sufficient for normal API calls. If specific endpoints need longer (e.g., large file uploads), they can override with per-request timeouts, but this is not needed for current use cases.

3. **Retries:** This task only adds timeouts. Retry logic (with exponential backoff) would be a separate enhancement if needed.

## Checklist

- [ ] Update `stripe_client.rs` to use `Client::builder()` with timeouts
- [ ] Update `domain_email.rs` to use `Client::builder()` with timeouts
- [ ] Update `public_domain_auth.rs` Google OAuth clients with timeouts
- [ ] Run `./run api:build` to verify compilation
- [ ] Run `./run api:test` to verify existing tests pass
- [ ] Move task to done

## History

- 2026-01-01 07:00 Created plan-v1.md with detailed implementation approach
