# Plan: Return Failure for Webhook Sync Errors

**Task:** 0010-webhook-failure-response
**Status:** Draft v1 (revised after Codex review)
**Created:** 2026-01-01
**Revised:** 2026-01-01

## Summary

The Stripe webhook handler in `public_domain_auth.rs` currently logs errors with `tracing::warn!` but always returns `Ok(StatusCode::OK)` to Stripe. This causes Stripe to consider the webhook successfully delivered and prevents automatic retries when our processing fails.

The fix involves identifying which failures are retryable (transient errors we want Stripe to retry) vs non-retryable (expected conditions like unknown customers), and returning appropriate HTTP status codes to trigger Stripe's built-in retry mechanism.

## Codex Review Feedback (Addressed)

The following issues were raised by Codex and are addressed in this plan:

1. **Error taxonomy:** Confirmed that functions return typed `AppError` enum, not `anyhow`. Added explicit mapping table.
2. **Idempotency:** Verified existing idempotency check via `is_event_processed` (line 1769). Subscription/payment sync uses upsert patterns. Safe for retries.
3. **Non-critical updates masking DB errors:** Changed approach - DB errors should be retryable even for status updates.
4. **checkout.session.completed justification:** Clarified this is "must succeed" because it grants user subscription access.
5. **Testing gap:** Added unit test requirement for error categorization helper.

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
- **5xx responses:** Stripe WILL retry with exponential backoff (up to 72 hours)

### Error Type Analysis (from AppError enum)

The codebase uses typed errors via `AppError` in `apps/api/src/application/app_error.rs`. Here's the categorization:

| AppError Variant | Retryable? | Rationale |
|-----------------|------------|-----------|
| `Database(String)` | YES | Transient DB connectivity; retry may succeed |
| `Internal(String)` | YES | Unexpected failures; retry may succeed |
| `NotFound` | NO | Customer/subscription not in our system; won't change with retry |
| `InvalidInput(String)` | NO | Bad data from Stripe; retry won't fix |
| `ValidationError(String)` | NO | Business rule violation; retry won't fix |
| `RateLimited` | YES | Temporary; retry after backoff |
| `Forbidden` | NO | Permission issue; won't change |
| `InvalidCredentials` | NO | Auth issue; won't change |
| Others | YES (default) | Safer to retry unknown errors |

### Idempotency Verification

The webhook handler has idempotency protection:

1. **Event-level idempotency** (line 1767-1773): `is_event_processed(event_id)` checks if event was already processed
2. **Subscription sync uses upsert** (line 1898): `create_or_update_subscription` won't create duplicates
3. **Payment sync uses upsert** (line 2039): `upsert_from_stripe` updates existing records

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

**Key insight:** Even "best-effort" events should retry on DB errors to prevent data loss. The distinction is whether missing the event affects user access (must-succeed) or just reporting (best-effort).

## Implementation Approach

### Step 1: Add Error Categorization Helper

Add a helper function near the webhook handler to classify errors:

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
    }
}
```

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
    fn test_not_found_is_not_retryable() {
        assert!(!is_retryable_error(&AppError::NotFound));
    }

    #[test]
    fn test_invalid_input_is_not_retryable() {
        assert!(!is_retryable_error(&AppError::InvalidInput("bad data".into())));
    }
}
```

### Step 3: Refactor checkout.session.completed (Must-Succeed)

This is the most critical path - it grants subscription access. Changes:

1. Replace `if let Ok(...)` with explicit `match`
2. Return 500 for retryable errors
3. Log and continue (200) for non-retryable errors

**Before (lines 1792-1870):**
```rust
if let Ok(stripe_sub) = stripe.get_subscription(sub_id).await {
    // ... nested if lets that swallow errors
}
```

**After:**
```rust
let stripe_sub = match stripe.get_subscription(sub_id).await {
    Ok(s) => s,
    Err(e) if is_retryable_error(&e) => {
        tracing::error!(error = %e, sub_id, "Failed to fetch Stripe subscription, will retry");
        return Err(e);
    }
    Err(e) => {
        tracing::warn!(error = %e, sub_id, "Non-retryable error fetching subscription");
        return Ok(StatusCode::OK);
    }
};

// Find plan - NotFound here means config issue, not retryable
let plan = match app_state.billing_use_cases
    .get_plan_by_stripe_price_id(domain.id, stripe_mode, &stripe_sub.price_id())
    .await
{
    Ok(Some(p)) => p,
    Ok(None) => {
        tracing::warn!(price_id = stripe_sub.price_id(), "No plan found for price_id, skipping");
        return Ok(StatusCode::OK);
    }
    Err(e) if is_retryable_error(&e) => {
        tracing::error!(error = %e, "Failed to look up plan, will retry");
        return Err(e);
    }
    Err(e) => {
        tracing::warn!(error = %e, "Non-retryable error looking up plan");
        return Ok(StatusCode::OK);
    }
};

// Create subscription - MUST succeed for user access
let created_sub = match app_state.billing_use_cases
    .create_or_update_subscription(&input)
    .await
{
    Ok(s) => s,
    Err(e) if is_retryable_error(&e) => {
        tracing::error!(error = %e, user_id = %user_id, "Failed to create subscription, will retry");
        return Err(e);
    }
    Err(e) => {
        // Even non-retryable errors here are concerning - log as error
        tracing::error!(error = %e, user_id = %user_id, "Non-retryable subscription creation failure");
        return Ok(StatusCode::OK);
    }
};

// Event logging is non-critical
if let Err(e) = app_state.billing_use_cases.log_webhook_event(...).await {
    tracing::warn!(error = %e, "Failed to log webhook event (non-critical)");
}
```

### Step 4: Refactor customer.subscription.* Events (Must-Succeed)

Change from `if let Ok(...)` to explicit error handling:

**Before (lines 1939-1955):**
```rust
if let Ok(updated_sub) = app_state.billing_use_cases
    .update_subscription_from_stripe(stripe_sub_id, &update)
    .await
{
    // log event
}
```

**After:**
```rust
match app_state.billing_use_cases
    .update_subscription_from_stripe(stripe_sub_id, &update)
    .await
{
    Ok(updated_sub) => {
        // Log event - non-critical, don't fail on logging errors
        if let Err(e) = app_state.billing_use_cases.log_webhook_event(...).await {
            tracing::warn!(error = %e, "Failed to log subscription update event");
        }
    }
    Err(e) if is_retryable_error(&e) => {
        tracing::error!(
            error = %e,
            stripe_sub_id,
            event_type,
            "Failed to update subscription, will retry"
        );
        return Err(e);
    }
    Err(e) => {
        // NotFound = subscription not in our system, expected
        tracing::debug!(
            error = %e,
            stripe_sub_id,
            "Subscription not found in our system, skipping"
        );
    }
}
```

### Step 5: Refactor Invoice Sync Events (Best-Effort with DB Retry)

For `invoice.created`, `invoice.paid`, etc. - best-effort but retry on DB errors:

**Before (lines 1967-1984):**
```rust
match app_state.billing_use_cases.sync_invoice_from_webhook(...).await {
    Ok(_) => { tracing::info!(...); }
    Err(e) => { tracing::warn!(...); }  // Always continues
}
```

**After:**
```rust
match app_state.billing_use_cases.sync_invoice_from_webhook(...).await {
    Ok(_payment) => {
        tracing::info!("Synced payment from {} event: {}", event_type, event_id);
    }
    Err(e) if is_retryable_error(&e) => {
        // DB error - retry to prevent data loss
        tracing::error!(
            error = %e,
            event_type,
            event_id,
            "Failed to sync invoice (retryable), will retry"
        );
        return Err(e);
    }
    Err(e) => {
        // NotFound = customer not in our system, expected
        tracing::debug!(
            error = %e,
            event_type,
            event_id,
            "Could not sync invoice (non-retryable), skipping"
        );
    }
}
```

### Step 6: Refactor Payment Status Updates (Best-Effort with DB Retry)

For `invoice.payment_failed`, `invoice.voided`, `charge.refunded`, etc.:

**Before (example from lines 2003-2018):**
```rust
if let Err(e) = app_state.billing_use_cases.update_payment_status(...).await {
    tracing::warn!(...);
}
```

**After:**
```rust
if let Err(e) = app_state.billing_use_cases.update_payment_status(...).await {
    if is_retryable_error(&e) {
        tracing::error!(
            error = %e,
            invoice_id,
            "Failed to update payment status (retryable), will retry"
        );
        return Err(e);
    } else {
        // Non-retryable: record might not exist yet
        tracing::warn!(
            error = %e,
            invoice_id,
            "Could not update payment status - record may not exist"
        );
    }
}
```

## Files to Modify

| File | Change |
|------|--------|
| `apps/api/src/adapters/http/routes/public_domain_auth.rs` | Add `is_retryable_error` helper, refactor `handle_webhook_for_mode` function, add unit tests |

## Specific Line Ranges

1. **Add helper function** (after line ~1700): `is_retryable_error` function
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
   - Test all AppError variants are explicitly categorized

2. **Compile-time verification:**
   - Run `./run api:build` to ensure code compiles
   - Run `./run api:lint` to check for warnings

3. **Existing test suite:**
   - Run `./run api:test` to verify no regressions
   - Webhook signature tests in `stripe_client.rs` still pass

4. **Manual verification:**
   - Use Stripe CLI to send test webhooks: `stripe trigger checkout.session.completed`
   - Verify 200 response on success
   - Simulate database errors (stop postgres) and verify 500 response

## Edge Cases

1. **Race conditions:** Stripe may send events out of order. The idempotency check handles duplicates. This is existing behavior and not addressed by this change.

2. **Partial failures:** If subscription creation succeeds but event logging fails, we return 200 (correct - the critical operation succeeded).

3. **Unknown event types:** The `_` match arm (line 2170) logs debug and returns 200. This is correct - we shouldn't block on events we don't handle.

4. **Signature verification failures:** Already handled correctly - returns error immediately (line 1757), which becomes 401.

5. **Domain not found:** Already handled correctly - returns NotFound error (line 1742), which becomes 404.

6. **Retry storms:** Stripe's exponential backoff (starting at 1 hour) prevents overwhelming the system. The idempotency check provides additional protection.

## Not In Scope (Future Considerations)

- **Dead letter queue:** Store persistently failing events for manual review
- **Webhook event replay:** Admin endpoint to manually retry events
- **Alerting:** Prometheus metrics or alerts for repeated webhook failures
- **Event ordering:** Handle out-of-order events (e.g., subscription.deleted before subscription.created)

## Checklist

- [ ] Add `is_retryable_error` helper function
- [ ] Add unit tests for `is_retryable_error`
- [ ] Refactor `checkout.session.completed` handler
- [ ] Refactor `customer.subscription.*` handlers
- [ ] Refactor `invoice.*` sync handlers
- [ ] Refactor payment status update handlers
- [ ] Run `./run api:build` to verify compilation
- [ ] Run `./run api:lint` to check for warnings
- [ ] Run `./run api:test` to verify all tests pass
- [ ] Test with `stripe trigger` CLI (manual)
- [ ] Update ticket history with completion notes
- [ ] Move task to done

## History

- 2026-01-01 Created plan-v1.md with detailed implementation approach
- 2026-01-01 Revised plan-v1.md based on Codex review feedback:
  - Confirmed typed `AppError` enum is used (not anyhow)
  - Verified idempotency protection exists
  - Changed approach: DB errors are retryable even for "best-effort" events
  - Added event classification table (must-succeed vs best-effort)
  - Added unit test requirement for error categorization helper
