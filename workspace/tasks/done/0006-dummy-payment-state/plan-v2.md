# Implementation Plan v2: Persist Dummy Payment Provider State

**Task**: 0006-dummy-payment-state
**Plan version**: v2 (revision addressing feedback-1)
**Created**: 2026-01-01
**Status**: Ready for review

---

## Summary

The dummy payment provider returns fabricated "always-active" subscription data from `get_subscription()` instead of indicating that external lookups are not supported. This is a footgun if future code calls this method expecting accurate data.

**Solution**: Return `None` from `get_subscription()` and `get_customer()` to signal that external lookups are not supported. The database (`UserSubscriptionRepo`) is the source of truth for subscription state.

---

## Feedback from v1 Addressed

| Feedback | Resolution |
|----------|------------|
| No confirmation that returning None won't break tests/fixtures | **Verified**: Grepped codebase for `dummy_sub_` and `dummy_cus_` - no tests assert on `get_subscription()` or `get_customer()` return values. All usages are in creation paths (generating IDs). See Audit Results below. |
| Documentation update scope limited to docstrings | **Updated**: Confirmed no developer-facing docs exist for payment providers (only product vision docs in `docs/vision/`). Docstrings are the appropriate location. |
| Logging impact - debug log might be noisy | **Updated**: Changed to `tracing::trace!` level instead of `debug!`. Trace is off by default and only visible when explicitly enabled. |
| Plan doesn't specify how to handle existing tests asserting on placeholder fields | **Verified**: No such tests exist. Existing tests only verify ID format (`starts_with("dummy_cus_")`), not the internal get methods. |
| Add explicit rationale linking to checklist | **Added**: Section below links fix to checklist item about documenting limitations. |

---

## Pre-Implementation Audit Results

### Search for test/fixture dependencies on placeholder data

```bash
grep -r "dummy_sub_\|dummy_cus_" apps/api/
```

**Findings**:

| Location | Usage | Impact of Change |
|----------|-------|------------------|
| `dummy_payment_client.rs:38,43` | ID generation (`dummy_cus_{user_id}`, `dummy_sub_{uuid}`) | None - creation path |
| `dummy_payment_client.rs:159` | `get_customer()` prefix check | Will be removed |
| `dummy_payment_client.rs:287` | `confirm_payment()` hardcoded customer_id | Separate method, unaffected |
| `dummy_payment_client.rs:309-313` | `get_subscription()` placeholder | Will be removed |
| `dummy_payment_client.rs:412` | `get_invoice_pdf()` placeholder | Separate issue, out of scope |
| `dummy_payment_client.rs:509,515,541,565,592` | Tests using ID format | Only test ID generation, not get methods |
| `domain_billing.rs:1837` | Creating customer_id format | Creation path |
| `public_domain_auth.rs:2504,2525,2645,2660` | Creating IDs in auth flow | Creation path |

**Conclusion**: No tests or production code depend on `get_subscription()` or `get_customer()` returning `Some`. Safe to change.

### Call site verification (from v1 audit)

`get_subscription()` is only called for Stripe provider in:
- `domain_billing.rs:1257,1334` (explicit Stripe calls)
- `public_domain_auth.rs:1775` (Stripe client)
- `stripe_payment_adapter.rs:185,223,283` (internal)

No calls to `DummyPaymentClient::get_subscription()` exist.

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
/// - **External providers (Stripe, Coinbase)**: Queries the provider's API
/// - **Inline providers (Dummy)**: Returns `None` - subscription state is in the database only
///
/// For dummy provider, use `UserSubscriptionRepo` to read authoritative subscription state.
async fn get_subscription(
    &self,
    subscription_id: &SubscriptionId,
) -> AppResult<Option<SubscriptionInfo>>;

/// Get customer information by ID from the payment provider.
///
/// # Provider Behavior
/// - **External providers (Stripe, Coinbase)**: Queries the provider's API
/// - **Inline providers (Dummy)**: Returns `None` - customer data is in the database only
async fn get_customer(&self, customer_id: &CustomerId) -> AppResult<Option<CustomerInfo>>;
```

### Step 4: Add Unit Tests

**File**: `apps/api/src/infra/dummy_payment_client.rs`

Add to existing `#[cfg(test)]` module:

```rust
#[tokio::test]
async fn test_get_subscription_returns_none() {
    let client = DummyPaymentClient::new(Uuid::new_v4());

    // Dummy provider returns None for all subscription lookups
    // This is intentional - subscription state is in the database
    let result = client
        .get_subscription(&SubscriptionId::new("dummy_sub_12345"))
        .await
        .unwrap();
    assert!(result.is_none(), "Dummy provider should return None for get_subscription");

    // Also None for non-dummy IDs (provider doesn't distinguish)
    let result = client
        .get_subscription(&SubscriptionId::new("sub_stripe_xyz"))
        .await
        .unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn test_get_customer_returns_none() {
    let client = DummyPaymentClient::new(Uuid::new_v4());

    // Dummy provider returns None - customer data is database-only
    let result = client
        .get_customer(&CustomerId::new("dummy_cus_12345"))
        .await
        .unwrap();
    assert!(result.is_none(), "Dummy provider should return None for get_customer");
}
```

---

## Files to Modify

| File | Changes |
|------|---------|
| `apps/api/src/infra/dummy_payment_client.rs` | Simplify `get_subscription()` and `get_customer()` to return `None`, add tests |
| `apps/api/src/application/ports/payment_provider.rs` | Add docstrings explaining provider-specific behavior |

---

## Testing Plan

1. **New unit tests**: Verify `None` return behavior (Step 4)
2. **Existing tests**: Run `./run api:test` - expect no regressions (no tests depend on placeholder data)
3. **Build verification**: Run `./run api:build` - verify compilation
4. **Pre-deploy**: Run `./run api:build` with `SQLX_OFFLINE=true`

---

## Risk Mitigation

| Risk | Mitigation |
|------|------------|
| Future callers assume `Some` without handling `None` | Docstrings on trait clearly document behavior. Rust's `Option` type forces callers to handle `None`. |
| Accidental reintroduction of placeholder data | New tests explicitly verify `None` behavior. Code review will catch regressions. |
| Silent behavior change | No current callers exist (verified). Any new caller will get `None` which Rust forces handling. |

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
- [x] Add a test or usage note — **Adding unit tests + trait docstrings**

---

## History

- 2026-01-01 07:15 Created plan-v1.md
- 2026-01-01 07:30 v1 finalized after Codex review
- 2026-01-01 08:00 Created plan-v2.md addressing feedback-1:
  - Added pre-implementation audit confirming no test dependencies
  - Changed logging from `debug!` to `trace!` to reduce noise
  - Confirmed no developer docs exist beyond docstrings
  - Added explicit rationale section linking to checklist
  - Added risk mitigation table
  - Documented out-of-scope related issues
