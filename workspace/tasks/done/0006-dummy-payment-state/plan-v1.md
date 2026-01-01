# Implementation Plan v1: Persist Dummy Payment Provider State

**Task**: 0006-dummy-payment-state
**Plan created**: 2026-01-01
**Status**: Finalized after Codex review

---

## Summary

The issue is that `DummyPaymentClient::get_subscription()` returns fabricated "always-active" data instead of reading actual subscription state from the database.

**Current behavior** in `apps/api/src/infra/dummy_payment_client.rs:303-327`:
```rust
async fn get_subscription(
    &self,
    subscription_id: &SubscriptionId,
) -> AppResult<Option<SubscriptionInfo>> {
    if subscription_id.as_str().starts_with("dummy_sub_") {
        let now = Utc::now();
        Ok(Some(SubscriptionInfo {
            customer_id: CustomerId::new("dummy_cus_unknown"),  // Wrong!
            status: SubscriptionStatus::Active,  // Always returns active!
            ...
        }))
    } else {
        Ok(None)
    }
}
```

---

## Code Audit Results

**Key finding**: `PaymentProviderPort::get_subscription()` is **NOT called** for dummy subscriptions anywhere in the codebase:

| File | Usage | Provider |
|------|-------|----------|
| `domain_billing.rs:1257, 1334` | `StripeClient::get_subscription()` | Stripe only |
| `public_domain_auth.rs:1775` | `StripeClient::get_subscription()` | Stripe only |
| `stripe_payment_adapter.rs:185,223,283` | Internal implementation | Stripe |

All subscription reads for dummy provider go through `UserSubscriptionRepo` (database), which correctly stores and retrieves state. The checkout flow properly persists via `create_or_update_subscription()`.

**The fabricated data in `DummyPaymentClient::get_subscription()` is a dead code path** - but it's still a footgun if someone were to call it in the future.

---

## Codex Review Feedback

Codex identified these issues with the original plan:

1. **High**: Adding `supports_external_subscription_lookup()` without changing the return value doesn't fix the bug - callers would still get fake "active" data
2. **Medium**: Tests codifying placeholder behavior make it harder to fix later
3. **Suggested fix**: Change `get_subscription()` to return `Ok(None)` instead of fabricated data

---

## Final Solution: Return `None` Instead of Placeholder

The simplest and cleanest fix:

1. **Change `DummyPaymentClient::get_subscription()` to return `Ok(None)`**
   - Makes it clear this method is not supported for inline providers
   - Prevents any caller from getting fake "active" data
   - No callers need to be updated (code audit shows none exist)

2. **Update documentation** explaining the behavior

3. **Add tests** that verify the `None` return behavior

---

## Step-by-Step Implementation

### Step 1: Update DummyPaymentClient::get_subscription()

File: `apps/api/src/infra/dummy_payment_client.rs`

**Before:**
```rust
async fn get_subscription(
    &self,
    subscription_id: &SubscriptionId,
) -> AppResult<Option<SubscriptionInfo>> {
    // Dummy provider can't look up stored subscriptions
    // This would need to be handled by the database
    if subscription_id.as_str().starts_with("dummy_sub_") {
        let now = Utc::now();
        Ok(Some(SubscriptionInfo {
            subscription_id: subscription_id.clone(),
            customer_id: CustomerId::new("dummy_cus_unknown"),
            status: SubscriptionStatus::Active,
            current_period_start: Some(now),
            current_period_end: Some(now + Duration::days(30)),
            trial_start: None,
            trial_end: None,
            cancel_at_period_end: false,
            canceled_at: None,
            price_id: None,
            subscription_item_id: None,
        }))
    } else {
        Ok(None)
    }
}
```

**After:**
```rust
async fn get_subscription(
    &self,
    _subscription_id: &SubscriptionId,
) -> AppResult<Option<SubscriptionInfo>> {
    // Dummy provider does not support external subscription lookup.
    // Subscriptions are created and managed entirely in the local database.
    // Callers should use UserSubscriptionRepo to read subscription state.
    //
    // Returning None instead of placeholder data prevents callers from
    // accidentally treating fabricated data as authoritative.
    tracing::debug!("Dummy provider: get_subscription returns None - use database instead");
    Ok(None)
}
```

### Step 2: Update get_customer() Similarly

File: `apps/api/src/infra/dummy_payment_client.rs`

**Before:**
```rust
async fn get_customer(&self, customer_id: &CustomerId) -> AppResult<Option<CustomerInfo>> {
    // For dummy provider, we can't look up stored customers
    // Just return basic info based on the customer ID
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

**After:**
```rust
async fn get_customer(&self, _customer_id: &CustomerId) -> AppResult<Option<CustomerInfo>> {
    // Dummy provider does not support external customer lookup.
    // Customer data exists only in the local database.
    tracing::debug!("Dummy provider: get_customer returns None - use database instead");
    Ok(None)
}
```

### Step 3: Update PaymentProviderPort Documentation

File: `apps/api/src/application/ports/payment_provider.rs`

Update docstrings:

```rust
/// Get subscription information from the payment provider.
///
/// For external providers (Stripe, Coinbase), this queries the provider's API.
/// For inline providers (Dummy), this returns `None` - use `UserSubscriptionRepo`
/// for the authoritative subscription state.
async fn get_subscription(
    &self,
    subscription_id: &SubscriptionId,
) -> AppResult<Option<SubscriptionInfo>>;

/// Get customer information by ID from the payment provider.
///
/// For external providers (Stripe, Coinbase), this queries the provider's API.
/// For inline providers (Dummy), this returns `None` - customer data is in the
/// local database only.
async fn get_customer(&self, customer_id: &CustomerId) -> AppResult<Option<CustomerInfo>>;
```

### Step 4: Add/Update Tests

File: `apps/api/src/infra/dummy_payment_client.rs`

Add to existing `#[cfg(test)]` module:

```rust
#[tokio::test]
async fn test_get_subscription_returns_none() {
    let client = DummyPaymentClient::new(Uuid::new_v4());

    // Even for valid dummy subscription IDs, returns None
    // This is intentional - real data comes from the database
    let result = client
        .get_subscription(&SubscriptionId::new("dummy_sub_12345"))
        .await
        .unwrap();
    assert!(result.is_none());

    // Also returns None for non-dummy IDs
    let result = client
        .get_subscription(&SubscriptionId::new("stripe_sub_xyz"))
        .await
        .unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn test_get_customer_returns_none() {
    let client = DummyPaymentClient::new(Uuid::new_v4());

    // Returns None - customer data is in database only
    let result = client
        .get_customer(&CustomerId::new("dummy_cus_12345"))
        .await
        .unwrap();
    assert!(result.is_none());
}
```

---

## Files to Modify

| File | Changes |
|------|---------|
| `apps/api/src/infra/dummy_payment_client.rs` | Change `get_subscription()` and `get_customer()` to return `None`, add tests |
| `apps/api/src/application/ports/payment_provider.rs` | Update docstrings for `get_subscription()` and `get_customer()` |

---

## Testing Approach

1. **Unit tests**: New tests verify `None` return behavior (above)
2. **Existing tests**: Run `./run api:test` to ensure no regressions
3. **Build verification**: Run `./run api:build` to verify compilation

---

## Why This Approach

1. **Minimal change**: Only 2 files modified
2. **Safe**: No current callers affected (code audit confirmed)
3. **Defensive**: Future callers get `None` instead of fake "active" data
4. **Clear**: Documentation explains the behavior
5. **Testable**: Easy to write tests that verify the behavior

---

## Not Needed

- **Redis/memory persistence**: Database already works correctly
- **New trait methods**: Adding `supports_external_subscription_lookup()` would be over-engineering for this case
- **Caller updates**: No callers exist that need updating

---

## Checklist Alignment

From ticket:
- [x] Audit dummy provider get_subscription behavior
- [x] Add minimal persistence (memory/redis) or doc *(doc approach - DB already works)*
- [x] Add a test or usage note

---

## History

- 2026-01-01 07:15 Created plan-v1.md with Option C approach
- 2026-01-01 07:25 Codex review provided feedback - Option C doesn't change runtime behavior
- 2026-01-01 07:30 Revised plan: return `None` instead of placeholder (per Codex suggestion #1)
