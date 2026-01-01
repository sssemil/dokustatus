# Implementation Plan v3: Persist Dummy Payment Provider State

**Task**: 0006-dummy-payment-state
**Plan version**: v3 (final revision)
**Created**: 2026-01-01
**Status**: Ready for implementation

---

## Summary

The dummy payment provider returns fabricated "always-active" subscription data from `get_subscription()` instead of indicating that external lookups are not supported. This is a footgun if future code calls this method expecting accurate data.

**Solution**: Return `None` from `get_subscription()` and `get_customer()` to signal that external lookups are not supported. The database (`UserSubscriptionRepo`) is the source of truth for subscription state.

---

## Feedback from v2 Addressed

| Feedback | Resolution |
|----------|------------|
| Test IDs assume constructors accept arbitrary strings | **Verified**: `CustomerId::new` and `SubscriptionId::new` use `impl Into<String>` with no validation. Existing tests use `CustomerId::new("dummy_cus_test")`. New tests will use same pattern. |
| Docstrings mention Coinbase but it's not implemented | **Updated**: Docstrings will say "External providers (Stripe)" only. Added note that Coinbase is planned but not implemented. |
| Plan doesn't mention updating task artifacts per workspace rules | **Added**: Implementation checklist includes updating task History and moving to done. Timestamp format: `YYYY-MM-DD HH:MM`. |
| Constructor signature not confirmed | **Verified**: `DummyPaymentClient::new(Uuid)` takes domain_id as Uuid. Factory calls `DummyPaymentClient::new(domain_id)`. |
| No module-level comment explaining lookup behavior | **Added**: Step 5 adds module-level doc comment to `dummy_payment_client.rs`. |
| Caller handling of None unclear | **Enhanced**: Trait docstrings now explicitly explain callers should use database for dummy provider. Also confirmed all current call sites use StripeClient directly, not via trait. |

---

## Pre-Implementation Audit Results

### Call site verification

**`get_subscription()` calls in codebase**:

| Location | Caller Type | Impact |
|----------|-------------|--------|
| `stripe_payment_adapter.rs:185,223,283` | `self.client.get_subscription()` - internal Stripe client | None - Stripe-specific |
| `domain_billing.rs:1257,1334` | `stripe.get_subscription()` - direct StripeClient | None - Stripe-specific |
| `public_domain_auth.rs:1775` | `stripe.get_subscription()` - direct StripeClient | None - Stripe-specific |

**`get_customer()` calls in codebase**:

| Location | Caller Type | Impact |
|----------|-------------|--------|
| `stripe_payment_adapter.rs:102` | `self.client.get_customer()` - internal Stripe client | None - Stripe-specific |
| `domain_billing.rs:1363` | `stripe.get_customer()` - direct StripeClient | None - Stripe-specific |

**Conclusion**: No code calls `DummyPaymentClient::get_subscription()` or `get_customer()` via the trait. All calls are Stripe-specific. Safe to change.

### Test/fixture dependency check (from v2)

No tests assert on `get_subscription()` or `get_customer()` return values. Tests only verify ID generation format (`starts_with("dummy_cus_")`).

---

## Rationale: Why Return None

**Checklist item**: "Add minimal persistence (memory/redis) **or doc**"

We're implementing the "doc" approach because:

1. **Database persistence already works**: The checkout flow uses `create_or_update_subscription()` to persist subscription state to `UserSubscriptionRepo`. Reading state from the DB is correct and already implemented.

2. **No callers exist**: Code audit confirms no code path calls `DummyPaymentClient::get_subscription()`.

3. **Returning None is explicit**: It clearly signals "this operation is not supported" rather than returning fabricated data that could be mistaken for real state.

4. **Prevents future bugs**: If someone adds a call to `get_subscription()` for dummy provider, they'll get `None` and need to handle it, rather than silently receiving fake "active" status.

---

## Step-by-Step Implementation

### Step 1: Update DummyPaymentClient::get_subscription()

**File**: `apps/api/src/infra/dummy_payment_client.rs`

**Before** (lines ~303-327):
```rust
async fn get_subscription(
    &self,
    subscription_id: &SubscriptionId,
) -> AppResult<Option<SubscriptionInfo>> {
    if subscription_id.as_str().starts_with("dummy_sub_") {
        let now = Utc::now();
        Ok(Some(SubscriptionInfo {
            subscription_id: subscription_id.clone(),
            customer_id: CustomerId::new("dummy_cus_unknown"),
            status: SubscriptionStatus::Active,
            current_period_start: Some(now),
            current_period_end: Some(now + Duration::days(30)),
            // ...
        }))
    } else {
        Ok(None)
    }
}
```

**After**:
```rust
async fn get_subscription(
    &self,
    _subscription_id: &SubscriptionId,
) -> AppResult<Option<SubscriptionInfo>> {
    // Dummy provider does not support external subscription lookup.
    // Subscriptions are created inline and stored in the local database.
    // Use UserSubscriptionRepo for authoritative subscription state.
    //
    // Returning None prevents callers from receiving fabricated "active" data.
    tracing::trace!("Dummy provider: get_subscription not supported, use database");
    Ok(None)
}
```

### Step 2: Update get_customer() Similarly

**File**: `apps/api/src/infra/dummy_payment_client.rs`

**Before** (lines ~155-165):
```rust
async fn get_customer(&self, customer_id: &CustomerId) -> AppResult<Option<CustomerInfo>> {
    if customer_id.as_str().starts_with("dummy_cus_") {
        Ok(Some(CustomerInfo {
            customer_id: customer_id.clone(),
            email: None,
            metadata: HashMap::new(),
        }))
    } else {
        Ok(None)
    }
}
```

**After**:
```rust
async fn get_customer(&self, _customer_id: &CustomerId) -> AppResult<Option<CustomerInfo>> {
    // Dummy provider does not support external customer lookup.
    // Customer data exists only in the local database.
    tracing::trace!("Dummy provider: get_customer not supported, use database");
    Ok(None)
}
```

### Step 3: Update PaymentProviderPort Documentation

**File**: `apps/api/src/application/ports/payment_provider.rs`

Update trait docstrings to clarify behavior across providers:

```rust
/// Get subscription information from the payment provider.
///
/// # Provider Behavior
/// - **Stripe**: Queries the Stripe API for current subscription state
/// - **Dummy**: Returns `None` - subscription state is in the local database only
/// - **Coinbase**: Not yet implemented (returns `ProviderNotSupported` from factory)
///
/// # Return Value
/// - `Some(info)` - Subscription found in external provider
/// - `None` - Subscription not found or provider doesn't support external lookup
///
/// For dummy provider, callers should use `UserSubscriptionRepo` to read subscription state.
async fn get_subscription(
    &self,
    subscription_id: &SubscriptionId,
) -> AppResult<Option<SubscriptionInfo>>;

/// Get customer information by ID from the payment provider.
///
/// # Provider Behavior
/// - **Stripe**: Queries the Stripe API for customer data
/// - **Dummy**: Returns `None` - customer data is in the local database only
/// - **Coinbase**: Not yet implemented
///
/// For dummy provider, customer data is only available in the local database.
async fn get_customer(&self, customer_id: &CustomerId) -> AppResult<Option<CustomerInfo>>;
```

### Step 4: Add Module-Level Documentation

**File**: `apps/api/src/infra/dummy_payment_client.rs`

Add at the top of the file (after any existing module doc):

```rust
//! # Dummy Payment Client
//!
//! A payment provider implementation for local development and testing.
//!
//! ## Lookup Behavior
//!
//! Unlike external providers (Stripe), the dummy provider does **not** support
//! external lookups. The following methods return `None`:
//! - `get_subscription()` - Use `UserSubscriptionRepo` instead
//! - `get_customer()` - Customer data is database-only
//!
//! This is intentional: dummy subscriptions/customers are created inline and
//! persisted to the local database. The database is the source of truth.
```

### Step 5: Add Unit Tests

**File**: `apps/api/src/infra/dummy_payment_client.rs`

Add to existing `#[cfg(test)]` module:

```rust
#[tokio::test]
async fn test_get_subscription_returns_none() {
    let client = DummyPaymentClient::new(Uuid::new_v4());

    // Dummy provider returns None for all subscription lookups
    // This is intentional - subscription state is in the database
    let result = client
        .get_subscription(&SubscriptionId::new("dummy_sub_test"))
        .await
        .unwrap();
    assert!(result.is_none(), "Dummy provider should return None for get_subscription");
}

#[tokio::test]
async fn test_get_customer_returns_none() {
    let client = DummyPaymentClient::new(Uuid::new_v4());

    // Dummy provider returns None - customer data is database-only
    let result = client
        .get_customer(&CustomerId::new("dummy_cus_test"))
        .await
        .unwrap();
    assert!(result.is_none(), "Dummy provider should return None for get_customer");
}
```

---

## Files to Modify

| File | Changes |
|------|---------|
| `apps/api/src/infra/dummy_payment_client.rs` | Add module doc, simplify `get_subscription()` and `get_customer()` to return `None`, add tests |
| `apps/api/src/application/ports/payment_provider.rs` | Add docstrings explaining provider-specific behavior |

---

## Testing Plan

1. **New unit tests**: Verify `None` return behavior (Step 5)
2. **Existing tests**: Run `./run api:test` - expect no regressions
3. **Build verification**: Run `./run api:build` with `SQLX_OFFLINE=true`

---

## Risk Mitigation

| Risk | Mitigation |
|------|------------|
| Future callers assume `Some` without handling `None` | Trait docstrings document behavior. Rust's `Option` type forces callers to handle `None`. |
| Accidental reintroduction of placeholder data | New tests explicitly verify `None` behavior. Code review will catch regressions. |
| Silent behavior change | No current callers exist (verified). Any new caller will get `None` which Rust forces handling. |
| Caller treats `None` as error instead of "use database" | Trait docstrings explicitly state callers should use `UserSubscriptionRepo`. Module doc reinforces this. |

---

## Out of Scope

These related issues exist but are separate from this task:

1. **`get_invoice_pdf()` placeholder** (line 412): Uses `dummy_cus_unknown` but is a different method for invoice generation
2. **`confirm_payment()` hardcoded customer_id** (line 287): Returns `dummy_cus_confirmed` but this is for payment confirmation flow, not lookup

These could be addressed in a follow-up task if desired.

---

## Checklist Alignment

From ticket:
- [x] Audit dummy provider get_subscription behavior
- [x] Add minimal persistence (memory/redis) or doc — **Implementing "doc" approach: docstrings + returning None**
- [x] Add a test or usage note — **Adding unit tests + trait/module docstrings**

---

## Implementation Checklist

When implementing this plan:

1. [ ] Read and understand `dummy_payment_client.rs` before editing
2. [ ] Make changes per Steps 1-5
3. [ ] Run `./run api:test` to verify no regressions
4. [ ] Run `./run api:build` to verify compilation
5. [ ] Update ticket.md History with timestamp (format: `YYYY-MM-DD HH:MM`)
6. [ ] Mark ticket checklist items complete
7. [ ] Commit changes with descriptive message
8. [ ] Move ticket to `/workspace/tasks/done/` when complete

---

## History

- 2026-01-01 07:15 Created plan-v1.md
- 2026-01-01 07:30 v1 finalized after Codex review
- 2026-01-01 08:00 Created plan-v2.md addressing feedback-1
- 2026-01-01 12:30 Created plan-v3.md addressing feedback-2:
  - Verified test ID constructors accept any string
  - Updated Coinbase mentions to clarify "not yet implemented"
  - Added task artifact update checklist
  - Confirmed DummyPaymentClient::new(Uuid) signature
  - Added module-level documentation step
  - Enhanced trait docstrings with caller guidance
  - Verified all get_subscription/get_customer calls are Stripe-specific
