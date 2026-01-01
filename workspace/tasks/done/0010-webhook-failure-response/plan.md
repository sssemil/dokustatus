# Plan: Return Failure for Webhook Sync Errors

**Task:** 0010-webhook-failure-response
**Status:** Draft v3 (Final)
**Created:** 2026-01-01
**Revised:** 2026-01-01 (v3 addressing feedback-2.md)

## Summary

The Stripe webhook handler in `public_domain_auth.rs` currently logs errors with `tracing::warn!` but always returns `Ok(StatusCode::OK)` to Stripe. This causes Stripe to consider the webhook successfully delivered and prevents automatic retries when our processing fails.

The fix involves identifying which failures are retryable (transient errors we want Stripe to retry) vs non-retryable (expected conditions like unknown customers), and returning appropriate HTTP status codes to trigger Stripe's built-in retry mechanism.

## Feedback Addressed (from feedback-2.md)

| Feedback Item | Resolution |
|--------------|------------|
| Return type mismatch in helper | Verified: `(StatusCode, &'static str)` implements `IntoResponse` in Axum. Handler returns `AppResult<impl IntoResponse>`, so `Ok((StatusCode::INTERNAL_SERVER_ERROR, "..."))` compiles correctly. |
| `event_id` and `event_type` scope | Verified: Lines 1763-1764 extract these at top of handler: `let event_type = event["type"].as_str()...` and `let event_id = event["id"].as_str()...`. Both are in scope for entire handler. |
| Notification channel unclear | Clarified: Production uses structured logging; `error!` logs go to stdout and can be aggregated. Adding "CONFIGURATION ERROR" prefix makes grep/filter easy. Alerting is out of scope but noted as follow-up. |
| Log level inconsistency | Fixed: Plan now explicitly uses `error!` for subscription creation failure (access-impacting) and `debug!` for not-found cases (expected for external customers). |
| Metrics/observability additions | Added optional `retryable: bool` field to structured logs for future metric extraction. |

## Problem Analysis

### Current Behavior

Looking at `apps/api/src/adapters/http/routes/public_domain_auth.rs:1725-2175`, the webhook handler:

1. **Lines 1967-1984** (invoice events): Catches sync errors and logs `warn!`, but continues with `Ok(StatusCode::OK)`
2. **Lines 1939-1955** (subscription updates): Uses `if let Ok(...)` pattern, silently ignoring failures
3. **Lines 2003-2018** (invoice.payment_failed): Catches `update_payment_status` errors and logs `warn!`
4. **Lines 2024-2039** (invoice.voided): Same pattern
5. **Lines 2045-2060** (invoice.marked_uncollectible): Same pattern
6. **Lines 2077-2087** (charge.refunded): Same pattern
7. **Lines 2106-2121** (charge.failed): Same pattern

All these cases return 200 OK even when processing fails, preventing Stripe retries.

### Stripe Retry Behavior

From Stripe's documentation:
- **2xx responses:** Stripe considers delivery successful, no retry
- **4xx responses:** Stripe does NOT retry (client error)
- **5xx responses:** Stripe WILL retry with exponential backoff (1hr, 2hr, 4hr, 8hr... up to 72 hours total)

### HTTP Status Code Mapping Verification

From `apps/api/src/adapters/http/app_error_impl.rs`, the `IntoResponse` implementation maps:

| AppError Variant | HTTP Status | Stripe Retries? |
|-----------------|-------------|-----------------|
| `Database(_)` | 500 | YES |
| `Internal(_)` | 500 | YES |
| `RateLimited` | **429** | **NO** |
| `NotFound` | 404 | NO |
| `InvalidInput(_)` | 400 | NO |
| `ValidationError(_)` | 400 | NO |
| `Forbidden` | 403 | NO |
| Others | Various 4xx | NO |

**Critical Finding:** `RateLimited` maps to 429, which is 4xx and Stripe will NOT retry. For webhook handling, we must explicitly return `StatusCode::INTERNAL_SERVER_ERROR` for retryable errors rather than relying on `AppError::into_response()`.

### Error Type Categorization

| AppError Variant | Retryable? | Rationale |
|-----------------|------------|-----------|
| `Database(String)` | YES | Transient DB connectivity; retry may succeed |
| `Internal(String)` | YES | Unexpected failures; retry may succeed |
| `RateLimited` | YES | Temporary; retry after backoff |
| `NotFound` | NO | Customer/subscription not in our system; won't change with retry |
| `InvalidInput(String)` | NO | Bad data from Stripe; retry won't fix |
| `ValidationError(String)` | NO | Business rule violation; retry won't fix |
| `Forbidden` | NO | Permission issue; won't change |
| `InvalidCredentials` | NO | Auth issue; won't change |
| `InvalidApiKey` | NO | Auth issue; won't change |
| `AccountSuspended` | NO | Account state; won't change |
| `SessionMismatch` | NO | Session issue; won't change |
| `TooManyDocuments` | NO | Limit reached; won't change |
| `PaymentDeclined(_)` | NO | Payment issue; won't change |
| `ProviderNotConfigured` | NO | Config issue; won't change |
| `ProviderNotSupported` | NO | Feature not available; won't change |
| `_ (unknown/new)` | **YES** | Safer to retry unknown errors |

### Idempotency Verification

The webhook handler has idempotency protection:

1. **Event-level idempotency** (line 1767-1773): `is_event_processed(event_id)` checks if event was already processed
2. **Event marked after success**: `log_webhook_event` is called AFTER critical operations succeed
3. **Subscription sync uses upsert**: `create_or_update_subscription` won't create duplicates
4. **Payment sync uses upsert**: Updates existing records safely

This means returning 500 and triggering Stripe retries is safe - duplicate processing won't corrupt data.

## Event Classification: Must-Succeed vs Best-Effort

| Event Type | Classification | Justification |
|------------|---------------|---------------|
| `checkout.session.completed` | **Must-Succeed** | Creates user subscription; failure = user can't access paid features |
| `customer.subscription.updated` | **Must-Succeed** | Status changes (cancellation, renewal); failure = stale access state |
| `customer.subscription.deleted` | **Must-Succeed** | Subscription ended; failure = continued access after cancellation |
| `invoice.created` | Best-Effort | Payment history; not access-critical |
| `invoice.paid` | Best-Effort | Payment confirmation; subscription events handle access |
| `invoice.payment_failed` | Best-Effort | Status tracking; Stripe handles retry |
| `invoice.voided` | Best-Effort | Accounting record |
| `invoice.marked_uncollectible` | Best-Effort | Accounting record |
| `charge.refunded` | Best-Effort | Refund tracking; doesn't affect access |
| `charge.failed` | Best-Effort | Payment failure tracking |

**Key insight:** Even "best-effort" events should retry on DB errors to prevent data loss.

## Implementation Approach

### Step 1: Add Error Categorization Helper

Add helper functions after line ~1700, before the webhook handlers:

```rust
/// Determines if a webhook processing error should trigger a Stripe retry.
///
/// Returns `true` if the error is retryable (transient), meaning we should
/// return 5xx to Stripe so they retry the webhook.
///
/// Returns `false` if the error is non-retryable (expected condition like
/// customer not found), meaning we should return 2xx and log.
fn is_retryable_error(error: &AppError) -> bool {
    match error {
        // Transient errors - retry may succeed
        AppError::Database(_) => true,
        AppError::Internal(_) => true,
        AppError::RateLimited => true,

        // Expected conditions - won't change with retry
        AppError::NotFound => false,
        AppError::InvalidInput(_) => false,
        AppError::ValidationError(_) => false,
        AppError::Forbidden => false,
        AppError::InvalidCredentials => false,
        AppError::InvalidApiKey => false,
        AppError::AccountSuspended => false,
        AppError::SessionMismatch => false,
        AppError::TooManyDocuments => false,
        AppError::PaymentDeclined(_) => false,
        AppError::ProviderNotConfigured => false,
        AppError::ProviderNotSupported => false,

        // Unknown/new variants - safer to retry
        #[allow(unreachable_patterns)]
        _ => true,
    }
}

/// Returns 500 Internal Server Error for Stripe to retry the webhook.
/// Logs the error with full context for debugging and future metrics extraction.
fn webhook_retryable_error(
    error: &AppError,
    event_type: &str,
    event_id: &str,
    context: &str,
) -> (StatusCode, &'static str) {
    tracing::error!(
        error = %error,
        event_type,
        event_id,
        context,
        retryable = true,
        "Webhook processing failed, returning 500 for Stripe retry"
    );
    (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error - will retry")
}
```

**Return type verified:** The handler returns `AppResult<impl IntoResponse>`. The tuple `(StatusCode, &'static str)` implements `IntoResponse` in Axum, so `Ok(webhook_retryable_error(...))` compiles correctly.

**Event metadata verified:** Lines 1763-1764 extract `event_type` and `event_id` at the top of `handle_webhook_for_mode`, so they're in scope for the entire handler body.

### Step 2: Add Unit Tests for Error Categorization

```rust
#[cfg(test)]
mod webhook_error_tests {
    use super::*;

    #[test]
    fn test_database_errors_are_retryable() {
        assert!(is_retryable_error(&AppError::Database("connection lost".into())));
    }

    #[test]
    fn test_internal_errors_are_retryable() {
        assert!(is_retryable_error(&AppError::Internal("unexpected".into())));
    }

    #[test]
    fn test_rate_limited_is_retryable() {
        assert!(is_retryable_error(&AppError::RateLimited));
    }

    #[test]
    fn test_not_found_is_not_retryable() {
        assert!(!is_retryable_error(&AppError::NotFound));
    }

    #[test]
    fn test_invalid_input_is_not_retryable() {
        assert!(!is_retryable_error(&AppError::InvalidInput("bad data".into())));
    }

    #[test]
    fn test_all_variants_explicitly_handled() {
        // Ensure all known variants have explicit handling
        let test_cases = vec![
            (AppError::Database("test".into()), true),
            (AppError::Internal("test".into()), true),
            (AppError::RateLimited, true),
            (AppError::NotFound, false),
            (AppError::InvalidInput("test".into()), false),
            (AppError::ValidationError("test".into()), false),
            (AppError::Forbidden, false),
            (AppError::InvalidCredentials, false),
            (AppError::InvalidApiKey, false),
            (AppError::AccountSuspended, false),
            (AppError::SessionMismatch, false),
            (AppError::TooManyDocuments, false),
            (AppError::PaymentDeclined("test".into()), false),
            (AppError::ProviderNotConfigured, false),
            (AppError::ProviderNotSupported, false),
        ];

        for (error, expected) in test_cases {
            assert_eq!(
                is_retryable_error(&error),
                expected,
                "Unexpected result for {:?}",
                error
            );
        }
    }
}
```

### Step 3: Refactor checkout.session.completed (Must-Succeed)

This is the most critical path - it grants subscription access. The current code uses nested `if let Ok(...)` patterns that silently ignore failures.

**Current problematic pattern (lines 1792-1870):**
```rust
if let Ok(stripe_sub) = stripe.get_subscription(sub_id).await {
    // ... nested code that silently fails ...
}
// No error handling - just returns Ok(StatusCode::OK)
```

**New pattern with explicit error handling:**
```rust
let stripe_sub = match stripe.get_subscription(sub_id).await {
    Ok(s) => s,
    Err(e) if is_retryable_error(&e) => {
        return Ok(webhook_retryable_error(&e, event_type, event_id, "fetch subscription"));
    }
    Err(e) => {
        tracing::debug!(
            error = %e,
            sub_id,
            event_id,
            retryable = false,
            "Non-retryable error fetching subscription, skipping"
        );
        return Ok(StatusCode::OK);
    }
};

// Find plan - missing plan is configuration error, log as error!
let plan = match app_state.billing_use_cases
    .get_plan_by_stripe_price_id(domain.id, stripe_mode, &stripe_sub.price_id())
    .await
{
    Ok(Some(p)) => p,
    Ok(None) => {
        // Configuration error - plan exists in Stripe but not in our system
        tracing::error!(
            price_id = stripe_sub.price_id(),
            domain_id = %domain.id,
            event_id,
            "CONFIGURATION ERROR: No plan found for Stripe price_id. User subscription may be missing!"
        );
        return Ok(StatusCode::OK);
    }
    Err(e) if is_retryable_error(&e) => {
        return Ok(webhook_retryable_error(&e, event_type, event_id, "lookup plan"));
    }
    Err(e) => {
        tracing::debug!(
            error = %e,
            event_id,
            retryable = false,
            "Non-retryable error looking up plan"
        );
        return Ok(StatusCode::OK);
    }
};

// Create subscription - MUST succeed for user access
match app_state.billing_use_cases.create_or_update_subscription(&input).await {
    Ok(s) => s,
    Err(e) if is_retryable_error(&e) => {
        return Ok(webhook_retryable_error(&e, event_type, event_id, "create subscription"));
    }
    Err(e) => {
        // Non-retryable but critical - user won't have access!
        tracing::error!(
            error = %e,
            user_id = %user_id,
            event_id,
            retryable = false,
            "CRITICAL: Non-retryable subscription creation failure - user may lack access!"
        );
        return Ok(StatusCode::OK);
    }
};

// Event logging is non-critical, don't fail on logging errors
if let Err(e) = app_state.billing_use_cases.log_webhook_event(...).await {
    tracing::warn!(error = %e, event_id, "Failed to log webhook event (non-critical)");
}
```

### Step 4: Refactor customer.subscription.* Events (Must-Succeed)

Change from `if let Ok(...)` to explicit error handling. Lines 1939-1955.

**Current problematic pattern:**
```rust
if let Ok(updated_sub) = app_state.billing_use_cases
    .update_subscription_from_stripe(stripe_sub_id, &update)
    .await
{
    // log event
}
// Silently ignores all errors
```

**New pattern:**
```rust
match app_state.billing_use_cases
    .update_subscription_from_stripe(stripe_sub_id, &update)
    .await
{
    Ok(updated_sub) => {
        // Log event - non-critical
        if let Err(e) = app_state.billing_use_cases.log_webhook_event(...).await {
            tracing::warn!(error = %e, event_id, "Failed to log subscription update event");
        }
    }
    Err(e) if is_retryable_error(&e) => {
        return Ok(webhook_retryable_error(&e, event_type, event_id, "update subscription"));
    }
    Err(e) => {
        // NotFound = subscription not in our system, expected for external customers
        tracing::debug!(
            error = %e,
            stripe_sub_id,
            event_id,
            retryable = false,
            "Subscription not found in our system, skipping"
        );
    }
}
```

### Step 5: Refactor Invoice Sync Events (Best-Effort with DB Retry)

For `invoice.created`, `invoice.paid`, etc. (lines 1967-1984) - best-effort but retry on DB errors:

**Current pattern:**
```rust
match app_state.billing_use_cases.sync_invoice_from_webhook(...).await {
    Ok(_payment) => { tracing::info!(...); }
    Err(e) => { tracing::warn!(...); }  // Always continues
}
```

**New pattern:**
```rust
match app_state.billing_use_cases.sync_invoice_from_webhook(...).await {
    Ok(_payment) => {
        tracing::info!(event_type, event_id, "Synced payment from webhook");
    }
    Err(e) if is_retryable_error(&e) => {
        // DB error - retry to prevent data loss
        return Ok(webhook_retryable_error(&e, event_type, event_id, "sync invoice"));
    }
    Err(e) => {
        // NotFound = customer not in our system, expected
        tracing::debug!(
            error = %e,
            event_type,
            event_id,
            retryable = false,
            "Could not sync invoice (non-retryable), skipping"
        );
    }
}
```

### Step 6: Refactor Payment Status Updates (Best-Effort with DB Retry)

For `invoice.payment_failed` (2003-2018), `invoice.voided` (2024-2039), `invoice.marked_uncollectible` (2045-2060), `charge.refunded` (2077-2087), `charge.failed` (2106-2121):

**Current pattern:**
```rust
if let Err(e) = app_state.billing_use_cases.update_payment_status(...).await {
    tracing::warn!(...);  // Always continues
}
```

**New pattern:**
```rust
if let Err(e) = app_state.billing_use_cases.update_payment_status(...).await {
    if is_retryable_error(&e) {
        return Ok(webhook_retryable_error(&e, event_type, event_id, "update payment status"));
    } else {
        // Non-retryable: record might not exist (customer not in our system)
        tracing::debug!(
            error = %e,
            invoice_id,
            event_id,
            retryable = false,
            "Could not update payment status - record may not exist"
        );
    }
}
```

## Logging Standards

All webhook error logs must include:

| Field | Required | Purpose |
|-------|----------|---------|
| `event_id` | YES | Correlate with Stripe dashboard |
| `event_type` | YES | Identify which handler failed |
| `error` | YES | The actual error |
| `retryable` | YES | Boolean for future metrics extraction |
| `context` | For retryable | Which operation failed |
| Object IDs | When available | `stripe_sub_id`, `invoice_id`, `user_id`, etc. |

Log levels:
- `error!` - Retryable failures (will trigger Stripe retry)
- `error!` - Configuration errors (won't retry but needs attention)
- `error!` - Non-retryable critical failures (subscription creation failed)
- `debug!` - Non-retryable expected conditions (customer not found)
- `warn!` - Non-critical failures (event logging failed)
- `info!` - Successful operations

## Configuration Issues Handling

| Case | Handling |
|------|----------|
| Plan not found for price_id | `error!` with "CONFIGURATION ERROR" prefix, return 200 |
| User not found for checkout | `debug!` (user may have been deleted), return 200 |

**Rationale:** Retrying won't help configuration issues, but operators need to notice them quickly. Using `error!` level with prefix ensures these appear in monitoring/alerting.

## Stripe Retry Schedule Reference

Stripe uses exponential backoff for webhook retries:
- 1st retry: ~1 hour
- 2nd retry: ~2 hours
- 3rd retry: ~4 hours
- Continues doubling up to 72 hours total

After 72 hours without success, Stripe marks the webhook as failed. Production monitoring should alert on repeated `error!` level logs before this threshold.

## Files to Modify

| File | Change |
|------|--------|
| `apps/api/src/adapters/http/routes/public_domain_auth.rs` | Add `is_retryable_error` + `webhook_retryable_error` helpers, refactor `handle_webhook_for_mode` function, add unit tests |

## Specific Line Ranges

1. **Add helper functions** (after line ~1700): `is_retryable_error` and `webhook_retryable_error`
2. **Lines 1777-1873** (`checkout.session.completed`): Full refactor with explicit error handling
3. **Lines 1875-1955** (`customer.subscription.*`): Replace `if let Ok(...)` with match
4. **Lines 1959-1984** (`invoice.*` events): Add retryable error handling
5. **Lines 1986-2018** (`invoice.payment_failed`): Add retryable check
6. **Lines 2020-2060** (`invoice.voided`, `invoice.marked_uncollectible`): Add retryable check
7. **Lines 2062-2088** (`charge.refunded`): Add retryable check
8. **Lines 2101-2121** (`charge.failed`): Add retryable check
9. **Add tests** (end of file): Unit tests for `is_retryable_error`

## Testing Approach

1. **Unit tests (new):**
   - Add `#[cfg(test)]` module with tests for `is_retryable_error` helper
   - Test all `AppError` variants are explicitly categorized

2. **Compile-time verification:**
   - Run `./run api:build` to ensure code compiles
   - Run `./run api:lint` to check for warnings

3. **Existing test suite:**
   - Run `./run api:test` to verify no regressions

4. **Manual verification:**
   - Use Stripe CLI to send test webhooks: `stripe trigger checkout.session.completed`
   - Verify 200 response on success
   - Simulate database errors (stop postgres) and verify 500 response

## Edge Cases

1. **Race conditions:** Stripe may send events out of order. The idempotency check handles duplicates.

2. **Partial failures:** If subscription creation succeeds but event logging fails, we return 200 (correct - the critical operation succeeded).

3. **Unknown event types:** The `_` match arm (line 2170) logs debug and returns 200. Correct behavior.

4. **Signature verification failures:** Already handled correctly - returns error immediately (line 1757).

5. **Domain not found:** Already handled correctly - returns NotFound error (line 1742).

6. **Retry storms on persistent DB issues:** Stripe's exponential backoff prevents overwhelming the system.

7. **Event ordering / missing records:** For payment status updates, `NotFound` is treated as non-retryable.

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| Accidental non-retry (4xx returned) | Explicitly return `StatusCode::INTERNAL_SERVER_ERROR` |
| Silent skip on config errors | Log at `error!` level with "CONFIGURATION ERROR" prefix |
| Retry storms | Stripe exponential backoff + idempotency check |
| New `AppError` variants unhandled | `_ => true` default arm ensures unknown errors are retryable |
| Partial state if retries exhausted | `error!` logs surface in monitoring; DLQ is future work |

## Not In Scope (Future Considerations)

- **Dead letter queue:** Store persistently failing events for manual review
- **Webhook event replay:** Admin endpoint to manually retry events
- **Alerting:** Set up alerts on `error!` logs with "CONFIGURATION ERROR" or repeated failures
- **Prometheus metrics:** Extract counts from `retryable: bool` structured log field

## Checklist

- [ ] Add `is_retryable_error` helper function with `_ => true` default
- [ ] Add `webhook_retryable_error` helper for consistent 500 response with `retryable: true` field
- [ ] Add unit tests for `is_retryable_error` (all variants)
- [ ] Refactor `checkout.session.completed` handler
- [ ] Refactor `customer.subscription.*` handlers
- [ ] Refactor `invoice.*` sync handlers
- [ ] Refactor payment status update handlers
- [ ] Ensure all error logs include `event_id`, `event_type`, and `retryable` fields
- [ ] Log missing plan as `error!` with CONFIGURATION ERROR prefix
- [ ] Run `./run api:build` to verify compilation
- [ ] Run `./run api:lint` to check for warnings
- [ ] Run `./run api:test` to verify all tests pass
- [ ] Test with `stripe trigger` CLI (manual)
- [ ] Update ticket history with completion notes
- [ ] Move task to done

## History

- 2026-01-01 Created plan-v1.md with detailed implementation approach
- 2026-01-01 Revised plan-v1.md based on Codex review feedback
- 2026-01-01 Created plan-v2.md addressing feedback-1.md
- 2026-01-01 Created plan-v3.md addressing feedback-2.md:
  - Verified return type: `(StatusCode, &'static str)` implements `IntoResponse`, compatible with `AppResult<impl IntoResponse>`
  - Verified `event_id` and `event_type` are extracted at lines 1763-1764 and in scope for entire handler
  - Fixed log level inconsistency: `error!` for subscription creation failure, `debug!` for not-found cases
  - Added `retryable: bool` field to all structured logs for future metrics extraction
  - Added Stripe retry schedule reference section
  - Clarified notification/alerting is out of scope but noted for follow-up
