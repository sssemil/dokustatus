# Plan v2 - Redact sensitive Stripe error logs

## Summary

The `handle_response` method in `apps/api/src/infra/stripe_client.rs` logs full Stripe API response bodies on error (line 606) and parse failures (line 626). These bodies may contain sensitive customer data including emails, payment method details, addresses, and personally identifiable information. The fix introduces a `redact_stripe_body` helper function that extracts only safe debugging information (error type, message, code) while omitting raw body content.

## Changes from v1

Addressed feedback from review:

1. **Verified scope**: Searched for other Stripe logging locations - confirmed only two locations in `handle_response` log bodies (lines 606 and 626). No webhook handlers or other methods log raw Stripe data.

2. **Request-Id header**: Decided not to pursue for this PR. The current `handle_response` doesn't have access to response headers after calling `.text()`. Adding this would require refactoring to extract headers before consuming the response body. This can be a follow-up enhancement.

3. **Error message sensitivity**: Accepting the risk of logging `error.message`. Stripe error messages are standardized and don't contain PII. Resource IDs (like `cus_abc123`) in messages are necessary for debugging and aren't considered sensitive.

4. **Added doc comment**: Added security-focused documentation to the helper function.

5. **Verified test module structure**: Tests module at line 975+ uses `super::*`, `hmac`, `sha2`. New tests will follow the same pattern but only need `super::*`.

## Step-by-step implementation approach

### Step 1: Create redaction helper function

Add a private `redact_stripe_body` function in `stripe_client.rs` near the `handle_response` method (around line 590):

```rust
/// Extracts safe debugging info from a Stripe response body.
/// Never logs raw body content to avoid exposing customer PII.
fn redact_stripe_body(body: &str) -> String {
    if let Ok(error) = serde_json::from_str::<StripeErrorResponse>(body) {
        format!(
            "type={}, code={:?}, message={:?}",
            error.error.error_type,
            error.error.code,
            error.error.message
        )
    } else {
        format!("<body redacted, {} bytes>", body.len())
    }
}
```

### Step 2: Update error logging in `handle_response`

Replace the two problematic log lines:

**Line 606** (Stripe API error):
```rust
// Before:
tracing::error!(status = %status, body = %body, "Stripe API error");

// After:
tracing::error!(status = %status, error_details = %redact_stripe_body(&body), "Stripe API error");
```

**Line 626** (parse failure):
```rust
// Before:
tracing::error!(body = %body, error = %e, "Failed to parse Stripe response");

// After:
tracing::error!(body_len = body.len(), error = %e, "Failed to parse Stripe response");
```

### Step 3: Sanitize error messages

The `AppError::Internal` on lines 619-622 includes raw body in the error message. Sanitize it:

```rust
// Before:
return Err(AppError::Internal(format!(
    "Stripe API error: {} - {}",
    status, body
)));

// After:
return Err(AppError::Internal(format!(
    "Stripe API error: {} - {}",
    status, redact_stripe_body(&body)
)));
```

### Step 4: Add unit tests

Add tests to the existing `#[cfg(test)]` module (after line 1065). These tests only need `super::*`:

```rust
// -------------------------------------------------------------------------
// Body redaction
// -------------------------------------------------------------------------

#[test]
fn test_redact_stripe_body_valid_error() {
    let body = r#"{"error":{"type":"card_error","code":"card_declined","message":"Your card was declined."}}"#;
    let result = redact_stripe_body(body);
    assert!(result.contains("type=card_error"));
    assert!(result.contains("code=Some(\"card_declined\")"));
    assert!(result.contains("message=Some(\"Your card was declined.\")"));
}

#[test]
fn test_redact_stripe_body_minimal_error() {
    let body = r#"{"error":{"type":"api_error"}}"#;
    let result = redact_stripe_body(body);
    assert!(result.contains("type=api_error"));
    assert!(result.contains("code=None"));
    assert!(result.contains("message=None"));
}

#[test]
fn test_redact_stripe_body_invalid_json() {
    let body = "not valid json";
    let result = redact_stripe_body(body);
    assert_eq!(result, "<body redacted, 14 bytes>");
}

#[test]
fn test_redact_stripe_body_empty() {
    let result = redact_stripe_body("");
    assert_eq!(result, "<body redacted, 0 bytes>");
}

#[test]
fn test_redact_stripe_body_non_error_json() {
    let body = r#"{"id":"cus_123","email":"user@example.com"}"#;
    let result = redact_stripe_body(body);
    assert!(result.starts_with("<body redacted,"));
}
```

## Files to modify

- `apps/api/src/infra/stripe_client.rs`:
  - Add `redact_stripe_body` helper function with doc comment (~12 lines, around line 590)
  - Modify `handle_response` method (lines 606, 619-622, 626)
  - Add 5 unit tests (~35 lines, after line 1065)

## Testing approach

1. **Unit tests**: Run `./run api:test` to execute the new `redact_stripe_body` tests
2. **Build verification**: Run `./run api:build` to ensure compilation succeeds
3. **Lint check**: Run `./run api:lint` to verify no warnings

## Edge cases handled

| Scenario | Handling |
|----------|----------|
| Valid Stripe error JSON | Extracts type, code, message |
| Non-JSON error bodies (500 errors) | Returns `<body redacted, N bytes>` |
| Empty bodies | Returns `<body redacted, 0 bytes>` |
| HTML error pages (outages) | JSON parse fails, returns byte count |
| JSON but not Stripe error structure | Returns `<body redacted, N bytes>` |

## Debugging sufficiency

After this change, developers still get:
- HTTP status code
- Error type (e.g., `card_error`, `invalid_request_error`)
- Error code (e.g., `card_declined`, `resource_missing`)
- Error message (e.g., "Your card was declined")
- For parse failures: body length for correlation with Stripe logs

## Risks

| Risk | Severity | Mitigation |
|------|----------|------------|
| Reduced debugging capability | Low | Error type/code/message covers 95% of debugging needs; Stripe dashboard has full details |
| Breaking error message parsing | Very Low | Searched codebase - no code parses these error message strings |
| StripeErrorResponse struct mismatch | Very Low | serde ignores unknown fields; fallback handles parse failures |

## Future enhancements (out of scope)

- **Request-Id logging**: Stripe returns a `Request-Id` header useful for correlating with their dashboard. Would require refactoring `handle_response` to extract headers before consuming body.
- **Structured logging**: Could return a struct implementing Display with separate fields. Current string format is sufficient.
