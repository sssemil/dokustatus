# Plan v3 - Redact sensitive Stripe error logs

## Summary

The `handle_response` method in `apps/api/src/infra/stripe_client.rs` logs full Stripe API response bodies on error (line 606) and parse failures (line 626). These bodies may contain sensitive customer data including emails, payment method details, addresses, and personally identifiable information. The fix introduces a `redact_stripe_body` helper function that extracts only safe debugging information (error type, message, code) while omitting raw body content.

## Changes from v2

Addressed feedback from review:

1. **Verified StripeErrorResponse struct**: Confirmed struct exists at line 848-858. Field names are correct:
   - `error.error_type: String` (uses `#[serde(rename = "type")]`)
   - `error.message: Option<String>`
   - `error.code: Option<String>`

   The plan's code is compatible with these field types.

2. **Added fmt step to testing approach**: Added `./run api:fmt` to the verification checklist.

3. **Line numbers verified**: Re-verified against current file state:
   - Line 606: `tracing::error!(status = %status, body = %body, "Stripe API error");`
   - Lines 619-622: `AppError::Internal(format!("Stripe API error: {} - {}", status, body))`
   - Lines 625-626: `tracing::error!(body = %body, error = %e, "Failed to parse Stripe response")`

4. **Added inline comment for fallback**: Will add a comment explaining why byte count is logged for non-error responses.

5. **Confirmed test visibility**: Private functions are accessible from the same module's `#[cfg(test)]` block.

## Verified struct definitions

```rust
// Line 848-858
pub struct StripeErrorResponse {
    pub error: StripeError,
}

pub struct StripeError {
    #[serde(rename = "type")]
    pub error_type: String,          // Always present
    pub message: Option<String>,     // Optional
    pub code: Option<String>,        // Optional
}
```

## Step-by-step implementation approach

### Step 1: Create redaction helper function

Add a private `redact_stripe_body` function in `stripe_client.rs` immediately before `handle_response` (around line 590):

```rust
/// Extracts safe debugging info from a Stripe response body.
///
/// # Security
/// Never logs raw body content to avoid exposing customer PII (emails,
/// addresses, payment details). For non-error responses that fail to parse,
/// logs only the byte count for correlation with Stripe's dashboard.
fn redact_stripe_body(body: &str) -> String {
    if let Ok(error) = serde_json::from_str::<StripeErrorResponse>(body) {
        format!(
            "type={}, code={:?}, message={:?}",
            error.error.error_type,
            error.error.code,
            error.error.message
        )
    } else {
        // Non-Stripe-error response (HTML error pages, unexpected JSON).
        // Log byte count to help correlate with Stripe dashboard logs.
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

**Lines 625-626** (parse failure):
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

Add tests to the existing `#[cfg(test)]` module. Tests go after the existing webhook signature tests:

```rust
// -------------------------------------------------------------------------
// Body redaction
// -------------------------------------------------------------------------

#[test]
fn test_redact_stripe_body_valid_error() {
    let body = r#"{"error":{"type":"card_error","code":"card_declined","message":"Your card was declined."}}"#;
    let result = redact_stripe_body(body);
    assert!(result.contains("type=card_error"));
    assert!(result.contains(r#"code=Some("card_declined")"#));
    assert!(result.contains(r#"message=Some("Your card was declined.")"#));
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
    // Customer object with PII that should NOT be logged
    let body = r#"{"id":"cus_123","email":"user@example.com"}"#;
    let result = redact_stripe_body(body);
    assert!(result.starts_with("<body redacted,"));
    assert!(!result.contains("email"));
    assert!(!result.contains("user@example.com"));
}
```

## Files to modify

- `apps/api/src/infra/stripe_client.rs`:
  - Add `redact_stripe_body` helper function (~15 lines, before line 595)
  - Modify `handle_response` method (lines 606, 619-622, 625-626)
  - Add 5 unit tests (~40 lines, in existing test module)

## Testing approach

1. **Format**: Run `./run api:fmt` to ensure code style compliance
2. **Lint**: Run `./run api:lint` to verify no warnings
3. **All tests**: Run `./run api:test` to verify existing tests still pass and new tests work
4. **Build**: Run `./run api:build` to ensure release compilation succeeds

## Edge cases handled

| Scenario | Handling |
|----------|----------|
| Valid Stripe error JSON | Extracts type, code, message |
| Non-JSON error bodies (500 errors) | Returns `<body redacted, N bytes>` |
| Empty bodies | Returns `<body redacted, 0 bytes>` |
| HTML error pages (outages) | JSON parse fails, returns byte count |
| JSON but not Stripe error structure | Returns `<body redacted, N bytes>` |
| PII in non-error response | Never logged, only byte count |

## Debugging sufficiency

After this change, developers still get:
- HTTP status code (always logged)
- Error type (e.g., `card_error`, `invalid_request_error`)
- Error code (e.g., `card_declined`, `resource_missing`)
- Error message (e.g., "Your card was declined")
- For parse failures: body length for correlation with Stripe dashboard logs

## Risks

| Risk | Severity | Mitigation |
|------|----------|------------|
| Reduced debugging capability | Low | Error type/code/message covers 95% of debugging needs; Stripe dashboard has full details |
| Breaking error message parsing | Very Low | Searched codebase - no code parses these error message strings |
| StripeErrorResponse struct mismatch | None | Verified struct at lines 848-858, fields match expected types |

## Future enhancements (out of scope)

- **Request-Id logging**: Stripe returns a `Request-Id` header useful for correlating with their dashboard. Would require refactoring `handle_response` to extract headers before consuming body.
- **Structured logging**: Could return a struct implementing Display with separate fields. Current string format is sufficient.
