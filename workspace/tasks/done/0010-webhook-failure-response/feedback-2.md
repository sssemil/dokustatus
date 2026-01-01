# Feedback: Plan v2 for Webhook Failure Response

**Reviewer:** Claude (Opus 4.5)
**Date:** 2026-01-01
**Plan Version:** v2

---

## What's Good About the Plan

1. **Comprehensive error categorization**: The plan correctly identifies all `AppError` variants and classifies them as retryable vs non-retryable with sound rationale. The `_ => true` default for unknown variants is a safe choice.

2. **Explicit HTTP 500 return**: The plan correctly identifies that `RateLimited` maps to 429 (4xx) and Stripe won't retry. Explicitly returning `StatusCode::INTERNAL_SERVER_ERROR` rather than relying on `AppError::into_response()` is the right approach.

3. **Event classification table**: Distinguishing "must-succeed" (subscription/access changes) from "best-effort" (invoice tracking) events provides clarity on what matters most.

4. **Idempotency verification**: The plan confirms that `is_event_processed` is checked early and `log_webhook_event` marks completion after critical operations, making retries safe.

5. **Consistent logging helper**: The `webhook_retryable_error` helper ensures uniform logging with required fields (`event_id`, `event_type`, `context`, `error`).

6. **Thorough testing approach**: Unit tests for the `is_retryable_error` helper plus manual Stripe CLI testing covers the critical paths.

7. **Configuration error handling**: Elevating missing plan to `error!` level with "CONFIGURATION ERROR" prefix will help operators notice issues quickly.

---

## What's Missing or Unclear

### 1. Return type mismatch in `webhook_retryable_error`

The helper returns `(StatusCode, &'static str)`:
```rust
fn webhook_retryable_error(...) -> (StatusCode, &'static str)
```

But the webhook handler returns `Result<impl IntoResponse, AppError>`. The usage in step 3:
```rust
return Ok(webhook_retryable_error(&e, ...));
```

This won't compile as-is. Need to verify:
- What is the actual return type of `handle_webhook_for_mode`?
- Should the helper return a type that implements `IntoResponse`?

**Action needed:** Check the actual handler return type and adjust the helper accordingly. A tuple `(StatusCode, &'static str)` does implement `IntoResponse` in Axum, but confirm this is compatible with the actual handler signature.

### 2. `event_id` and `event_type` scope/availability

The plan references `event_id` and `event_type` in logging, but:
- Where are these values extracted from the Stripe event?
- Are they in scope at all the locations where `webhook_retryable_error` is called?
- The helper takes `event_type: &str` and `event_id: &str` but doesn't show where these come from.

**Action needed:** Verify these values are accessible at all call sites, or document how they're extracted from the Stripe event struct.

### 3. Notification channel for webhook failures is unclear

The plan logs at `error!` level for retryable failures, but doesn't specify:
- What happens to these logs in production?
- Is there existing alerting on error-level logs?
- Will the "CONFIGURATION ERROR" prefix trigger any specific alert rules?

**Action needed:** Either confirm existing alerting covers `error!` logs, or note this as a follow-up to set up.

### 4. Log levels for non-retryable critical failures

In step 3, a non-retryable subscription creation failure logs at what level?
```rust
Err(e) => {
    tracing::error!(...)  // "Non-retryable subscription creation failure"
```

But in step 4 (subscription update), non-retryable is `debug!`:
```rust
Err(e) => {
    tracing::debug!(...)  // "Subscription not found in our system, skipping"
```

The logging standards section shows:
- `warn!` - Non-retryable failures on critical paths

This is inconsistent. Should checkout failures be `error!` or `warn!`? The standards say `warn!`.

**Action needed:** Align the code examples with the logging standards. Recommend:
- `error!` for subscription creation failure (access-impacting)
- `debug!` for not-found cases (expected for external customers)

### 5. No metrics/observability additions

While explicitly out of scope, consider at minimum adding structured logs that could be converted to metrics later:
- Count of retryable vs non-retryable errors
- Count by event type

This doesn't require Prometheus setup, just consistent structured log fields that can be parsed.

**Action needed (optional):** Consider adding a `retryable: bool` field to error logs for easier parsing.

---

## Suggested Improvements

### 1. Extract event metadata early and pass through

Rather than passing `event_type: &str` and `event_id: &str` to every call, extract these once at the top of `handle_webhook_for_mode` and use them consistently:

```rust
let event_id = event.id.as_str();
let event_type = event.type_.as_str();  // or however Stripe SDK exposes it
```

This ensures they're always available and reduces repetition.

### 2. Consider a macro or wrapper for the error handling pattern

The pattern:
```rust
match some_operation().await {
    Ok(v) => v,
    Err(e) if is_retryable_error(&e) => {
        return Ok(webhook_retryable_error(&e, event_type, event_id, "context"));
    }
    Err(e) => {
        tracing::debug!(...);
        return Ok(StatusCode::OK);
    }
}
```

Appears 7+ times. Consider either:
- A helper like `handle_webhook_result(result, event_type, event_id, context)` that does the match
- Keep explicit matches for readability but document the pattern

### 3. Add a summary comment at the top of the handler

After implementation, add a comment explaining the retry strategy:
```rust
/// Webhook handler: Returns 500 for retryable errors (DB, internal) to trigger
/// Stripe retry. Returns 200 for non-retryable errors (not found, validation)
/// and successful processing. See is_retryable_error for classification.
```

### 4. Document the Stripe retry schedule

Add to the file or a README: Stripe retries at 1 hour, 2 hours, 4 hours, etc., up to 72 hours. This helps operators understand the urgency of fixing issues.

---

## Risks and Concerns

### 1. Partial state if retry ultimately fails

If Stripe exhausts retries (72 hours) without success:
- User may have paid but lacks access
- No automatic escalation/notification

**Mitigation:** Rely on the `error!` logs + monitoring. Consider adding a follow-up task for a dead-letter queue.

### 2. Testing coverage for error paths

Unit tests cover `is_retryable_error`, but not the actual webhook handler paths. Integration testing with simulated DB failures is manual.

**Mitigation:** The manual test with stopping postgres is good. Consider adding a note about running this before production deployment.

### 3. Rollback safety

If issues are discovered post-deploy, can this be rolled back? The change is purely behavioral (returning 500 vs 200), so rollback is safe - no data model changes.

### 4. Thundering herd on recovery

If the database was down and comes back, Stripe will retry all accumulated failed webhooks. The idempotency check protects against duplicates, but there may be a burst of traffic.

**Mitigation:** Stripe's exponential backoff spreads retries out. Monitor API load after recovery.

---

## Summary

The plan is well-researched and addresses the original issue correctly. The main gaps are:
1. Verify return type compatibility for the helper function
2. Confirm event metadata (`event_id`, `event_type`) is in scope at all call sites
3. Align log levels between examples and the logging standards section

These are minor implementation details that can be resolved during coding. The plan is **ready to implement** with the above clarifications.

---

**Recommendation:** Proceed to implementation. Address the return type question first to avoid rework.
