# Feedback on Plan v1 - Redact sensitive Stripe error logs

**Reviewer:** Claude Opus 4.5
**Date:** 2026-01-01

---

## What's Good About the Plan

1. **Accurate problem identification**: The plan correctly identifies the three problematic locations (lines 606, 619-622, 626) in `handle_response`. I verified these match the actual code.

2. **Reuses existing types**: The `redact_stripe_body` helper reuses `StripeErrorResponse`, which is already defined in the codebase (line 848). No new types needed.

3. **Fallback for unparseable bodies**: The plan handles non-JSON responses (HTML error pages, empty bodies, 500 errors) gracefully with `<body redacted, N bytes>`.

4. **Minimal scope**: Only modifies one file (`stripe_client.rs`) and adds a small helper function. Low blast radius.

5. **Test plan is sensible**: The four test cases cover the main scenarios (valid error JSON, invalid JSON, empty string, non-error JSON structure).

6. **Debugging sufficiency analysis**: The plan explicitly confirms that error type, code, and message are still available for debugging.

---

## What's Missing or Unclear

### 1. **Error message sensitivity check**

The plan extracts `error.message` which could potentially contain user-provided data. For example, Stripe error messages like:

```
"No such customer: 'cus_abc123'"
"No such price: 'price_xyz'"
```

These contain resource IDs that may be sensitive. Consider whether `error.message` should be truncated or if this is acceptable.

**Recommendation:** Accept this risk - Stripe error messages are standardized and don't contain PII. Resource IDs are necessary for debugging.

### 2. **Missing search for other Stripe logging locations**

The plan only addresses `handle_response`. There may be other places in the codebase that log Stripe data:
- Webhook handlers
- Success response logging (if any)
- Other Stripe client methods

**Recommendation:** Before implementation, run a search like:
```bash
rg -n "tracing::(info|warn|error|debug).*[Ss]tripe" apps/api/
rg -n "body.*%body" apps/api/src/infra/stripe_client.rs
```

### 3. **No mention of existing tests module structure**

The plan says "Add tests to the existing `#[cfg(test)]` module" but doesn't check what's already there. The tests module (line 975+) exists and uses `hmac`/`sha2`. New tests should follow the same pattern.

### 4. **`redact_stripe_body` visibility not specified**

The function is shown without `pub`. Confirm it should be private (it should be, since it's only used internally).

---

## Suggested Improvements

### 1. **Consider logging a hash/trace ID**

For correlation with Stripe dashboard logs, consider adding a request ID if available:

```rust
tracing::error!(
    status = %status,
    error_details = %redact_stripe_body(&body),
    // If Stripe returns a request ID header, include it
    "Stripe API error"
);
```

Stripe returns `Request-Id` header on all responses. This would help correlate logs with Stripe's dashboard without exposing sensitive data.

**Priority:** Low - nice to have for debugging, but not required for this security fix.

### 2. **Use structured logging fields**

Instead of formatting into a single string, consider structured fields:

```rust
fn redact_stripe_body(body: &str) -> RedactedError {
    // Return a struct that implements Display
}
```

Or log each field separately:
```rust
tracing::error!(
    status = %status,
    error_type = %error.error.error_type,
    error_code = ?error.error.code,
    error_message = ?error.error.message,
    "Stripe API error"
);
```

**Priority:** Low - the current approach works fine.

### 3. **Add doc comment to `redact_stripe_body`**

Since this is a security-sensitive function, add a brief doc comment explaining why it exists:

```rust
/// Extracts safe debugging info from a Stripe response body.
/// Never logs raw body content to avoid exposing customer PII.
fn redact_stripe_body(body: &str) -> String { ... }
```

---

## Risks and Concerns

### 1. **Risk: Breaking changes to error messages (LOW)**

The `AppError::Internal` message format changes from raw body to redacted. If any code parses these error messages (unlikely but possible), it would break.

**Mitigation:** Search for usages of the error message string pattern.

### 2. **Risk: Reduced debugging capability (LOW)**

Removing raw bodies means less information in production logs. However:
- Error type/code/message covers 95% of debugging needs
- Stripe dashboard has full request details
- This is the correct security tradeoff

### 3. **Risk: Parse failure edge case (VERY LOW)**

If `StripeErrorResponse` struct doesn't match Stripe's actual error format (e.g., Stripe adds new fields), parsing might fail unexpectedly. However:
- `serde` ignores unknown fields by default
- The fallback to byte-count handles this gracefully

---

## Summary

**Verdict: APPROVE with minor suggestions**

The plan is solid and addresses the security concern correctly. The implementation is minimal, well-scoped, and handles edge cases appropriately.

Before starting implementation:
1. Search for other Stripe logging locations in the codebase
2. Verify the request ID header idea isn't worth pursuing

The plan can proceed as written.
