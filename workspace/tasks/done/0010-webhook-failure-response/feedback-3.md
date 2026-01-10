# Feedback: Plan v3 for Webhook Failure Response

**Reviewer:** Claude (Opus 4.5)
**Date:** 2026-01-01
**Plan Version:** v3 (Final)

---

## What's Good About the Plan

1. **Thorough feedback incorporation**: Plan v3 directly addresses all concerns from feedback-1 and feedback-2 with a clear resolution table. The verification work (return type, event_id/event_type scope, log levels) reduces implementation risk.

2. **Solid return type verification**: The confirmation that `(StatusCode, &'static str)` implements `IntoResponse` and is compatible with `AppResult<impl IntoResponse>` removes a potential blocker. Line references (1763-1764) for event metadata extraction are helpful.

3. **Consistent log level policy**: The distinction is now clear:
   - `error!` for retryable failures, config errors, and critical non-retryable failures
   - `debug!` for expected conditions (not found)
   - `warn!` for non-critical failures (event logging)

4. **Structured logging with `retryable: bool`**: This field enables future metrics extraction without requiring infrastructure changes now.

5. **Stripe retry schedule reference**: Including the 1hr → 2hr → 4hr exponential backoff context helps operators understand the urgency window.

6. **Comprehensive checklist**: The 14-item checklist maps directly to implementation steps and verification gates.

7. **Edge cases enumerated**: Race conditions, partial failures, unknown event types, signature failures, retry storms - all covered with explicit handling decisions.

---

## What's Missing or Unclear

### 1. `is_event_processed` timing still implicit

The plan states "Event marked after success" but doesn't show the actual code path. The idempotency check at line 1767-1773 runs early, but where exactly does `log_webhook_event` (which presumably marks as processed) get called?

If this happens inside one of the `if let Ok(...)` blocks that's being refactored, the new error handling could inadvertently skip the event marking, causing duplicate processing on retry.

**Action needed:** Trace the exact `log_webhook_event` call locations and confirm they remain on the success path after refactoring.

### 2. No code example for signature verification error path

Line 1757 is referenced for signature failures but no code is shown. Is it already returning an error that triggers a 4xx? If so, that's intentional (Stripe shouldn't retry with a bad signature). If it returns 5xx, that could cause unnecessary retries.

**Action needed:** Verify signature verification failure returns 4xx (not 5xx) and document this is correct behavior.

### 3. Test coverage for `webhook_retryable_error` helper

The plan adds unit tests for `is_retryable_error` but not for `webhook_retryable_error`. While simpler, it logs and returns a tuple - worth a test to confirm the log format and return value.

**Action needed (minor):** Consider adding a test that calls `webhook_retryable_error` and verifies the return type is `(StatusCode::INTERNAL_SERVER_ERROR, &str)`.

### 4. No example of the `log_webhook_event` failure handling

The plan shows:
```rust
if let Err(e) = app_state.billing_use_cases.log_webhook_event(...).await {
    tracing::warn!(error = %e, event_id, "Failed to log webhook event (non-critical)");
}
```

But should this also include `event_type` and `retryable` for consistency with the logging standards table?

**Action needed (minor):** Align `log_webhook_event` failure logs with the standard (add `event_type`, `retryable: false`).

### 5. Missing domain context in error logs

The webhook handler operates per-domain (line 1742 mentions domain lookup). Error logs don't consistently include `domain_id`. For multi-tenant debugging, this is valuable.

**Action needed (minor):** Consider adding `domain_id` to `webhook_retryable_error` and structured logs where available.

---

## Suggested Improvements

### 1. Add a quick smoke test to the checklist

After implementation, before deploying:
```bash
# Send a test webhook and verify 200
stripe trigger checkout.session.completed

# Intentionally break DB connection and verify 500
# (manual - stop postgres, send webhook, check response)
```

This is mentioned in "Testing Approach" but making it a formal checklist item ensures it's not skipped.

### 2. Consider a brief code block for the complete refactored handler

The plan shows before/after for individual sections but not how they compose. A short pseudocode showing the overall flow would help:

```rust
fn handle_webhook_for_mode(...) -> AppResult<impl IntoResponse> {
    // 1. Verify signature (return 4xx on failure)
    // 2. Check idempotency (return 200 if already processed)
    // 3. Extract event_id, event_type
    // 4. Match on event type, handle each with:
    //    - Success: proceed, log at info!
    //    - Retryable error: return 500 via webhook_retryable_error
    //    - Non-retryable error: log at debug!/error!, return 200
    // 5. Mark event processed (log_webhook_event) after critical success
    // 6. Return 200
}
```

### 3. Add CONFIGURATION ERROR handling for subscription price_id lookup too

The plan handles missing plan with `error!` + "CONFIGURATION ERROR". But what about the subscription lookup itself failing with an unexpected Stripe API error vs the subscription not found? Consider distinguishing:
- Stripe API error → retryable
- Subscription not found in Stripe → weird, log as warning (Stripe sent event about nonexistent subscription?)

### 4. Document which Stripe events are expected for external customers

The plan notes "NotFound = expected for external customers" but doesn't explain what "external customers" means. Adding a one-liner would help:

> External customers: Stripe may send webhook events for customers created outside our system (e.g., via Stripe dashboard or other integrations).

---

## Risks and Concerns

### 1. Retry exhaustion is a silent failure

If Stripe retries for 72 hours and then gives up, there's no escalation. The `error!` logs exist but could be missed in a noisy production environment.

**Mitigation:** Out of scope for this task, but note as follow-up: consider a weekly report of webhook events that failed permanently (no corresponding success within 72 hours of first failure).

### 2. Log noise from expected conditions

Logging `debug!` for every invoice sync where customer isn't in our system could be high volume if this Stripe account handles external customers too.

**Mitigation:** `debug!` level is correct - it won't appear by default in production. Ensure log level is set appropriately in production config.

### 3. Refactoring multiple event handlers increases regression risk

Seven different event handler sections are being modified. Even with tests, subtle differences in behavior could slip through.

**Mitigation:**
- Run full test suite (`./run api:test`)
- Use Stripe CLI to trigger each event type manually before merge
- Review diff carefully for copy-paste errors

### 4. The `#[allow(unreachable_patterns)]` on `_ => true`

If `AppError` is an exhaustive enum and the compiler catches all variants, the `_ => true` will never match. This is intentional (safety net for future variants), but the `#[allow(unreachable_patterns)]` suppresses the warning. Confirm this is the intended approach vs using `#[non_exhaustive]` on `AppError` or other patterns.

**Mitigation:** Document why the allow is there (future-proofing).

---

## Summary

Plan v3 is comprehensive and well-researched. The previous feedback has been addressed with verification work. The remaining gaps are minor:

1. Trace `log_webhook_event` placement to confirm idempotency marking survives refactor
2. Verify signature failure returns 4xx
3. Minor logging consistency tweaks (`event_type`, `domain_id` where available)

**Recommendation:** **Ready to implement.** These gaps can be verified during implementation rather than requiring another plan revision.

---

## Checklist for Implementation

Before coding:
- [ ] Trace `log_webhook_event` call sites - confirm they're after critical operations

During coding:
- [ ] Follow the plan's step-by-step checklist
- [ ] Include `domain_id` in logs where available (minor enhancement)
- [ ] Ensure `log_webhook_event` failures log with `event_type` and `retryable: false`

After coding:
- [ ] `./run api:build` passes
- [ ] `./run api:test` passes
- [ ] `./run api:lint` passes
- [ ] Manual test: `stripe trigger checkout.session.completed` → 200
- [ ] Review diff for copy-paste errors across the 7 handlers
