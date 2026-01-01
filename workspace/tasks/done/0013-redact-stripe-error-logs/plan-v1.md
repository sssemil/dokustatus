# Plan v1 - Redact sensitive Stripe error logs

## Summary

The `handle_response` method in `apps/api/src/infra/stripe_client.rs` logs full Stripe API response bodies on error (line 606) and parse failures (line 626). These bodies may contain sensitive customer data including emails, payment method details, addresses, and personally identifiable information. The fix introduces a `redact_stripe_body` helper function that extracts only the safe, useful debugging information (error type, message, code) while omitting raw body content.

## Step-by-step implementation approach

### Step 1: Create redaction helper function

Add a `redact_stripe_body` function in `stripe_client.rs` that:
- Attempts to parse the body as JSON
- If it's a Stripe error response, extracts only `error.type`, `error.message`, `error.code`
- If parsing fails, returns a placeholder like `"<unparseable body, {len} bytes>"`
- Never logs the raw body content

```rust
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

### Step 3: Verify error messages remain useful

The `AppError::Internal` returned on line 619-622 currently includes the raw body in the error message. This should also be sanitized:

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

Add tests to the existing `#[cfg(test)]` module in `stripe_client.rs`:
- Test `redact_stripe_body` with a valid Stripe error JSON
- Test `redact_stripe_body` with invalid JSON (returns byte count)
- Test `redact_stripe_body` with an empty string
- Test `redact_stripe_body` with JSON that is not a Stripe error structure

## Files to modify

- `apps/api/src/infra/stripe_client.rs`:
  - Add `redact_stripe_body` helper function (~10 lines)
  - Modify `handle_response` method (lines 606, 619-622, 626)
  - Add 4 unit tests (~30 lines)

## Testing approach

1. **Unit tests**: Run `./run api:test` to execute the new `redact_stripe_body` tests
2. **Build verification**: Run `./run api:build` to ensure compilation succeeds
3. **Lint check**: Run `./run api:lint` to verify no warnings
4. **Manual verification**: Review log output structure to confirm:
   - Error type, code, message are still visible for debugging
   - No raw JSON bodies containing customer data appear in logs

## Edge cases to handle

1. **Non-JSON error bodies**: Some Stripe errors (e.g., 500 errors) may return non-JSON bodies. The helper handles this by returning `<body redacted, N bytes>`.

2. **Empty bodies**: Edge case where Stripe returns an empty response. The helper returns `<body redacted, 0 bytes>`.

3. **Nested sensitive data**: Stripe error responses themselves don't contain customer data in the `error` object, only in the main response body. By extracting only `error.type/message/code`, we avoid any nested PII.

4. **HTML error pages**: During outages, Stripe may return HTML. The JSON parse fails and we return byte count only.

5. **Debugging sufficiency**: Developers still get:
   - HTTP status code
   - Error type (e.g., `card_error`, `invalid_request_error`)
   - Error code (e.g., `card_declined`, `resource_missing`)
   - Error message (e.g., "Your card was declined")
   - For parse failures: body length for correlation with Stripe logs
