# Feedback on Plan v1: Payment Provider Abstraction

**Reviewer**: Claude Opus 4.5
**Date**: 2026-01-01
**Plan version reviewed**: v1

---

## What's Good About the Plan

### 1. Clear Problem Statement
The plan correctly identifies the architectural violation: `DomainBillingUseCases` imports directly from `crate::infra::stripe_client`, bypassing the established `PaymentProviderPort` abstraction. The four problems listed (leaky abstraction, testing difficulty, provider lock-in, duplication) are accurate and well-articulated.

### 2. Thorough Code Analysis
The plan provides exact line numbers for the problematic code (1348, 1406-1407, 1464, 1508-1509) and documents the existing abstractions (`PaymentProviderPort`, `PaymentProviderFactory`, `StripePaymentAdapter`, `DummyPaymentClient`). This level of detail will make implementation faster.

### 3. Step-by-Step Structure
Breaking the work into 8 discrete steps with before/after code snippets makes the plan executable. Each step is small enough to implement and verify independently.

### 4. Edge Cases Documented
The plan anticipates several edge cases (no enabled providers, manually granted subscriptions, mode mismatches) and documents how each should be handled.

### 5. Clear Scope Boundaries
The "Out of Scope" section explicitly excludes related but separate work (StripeMode removal, webhook handling, checkout flow). This prevents scope creep.

---

## What's Missing or Unclear

### 1. PaymentProviderFactory Interface Not Verified
The plan assumes `PaymentProviderFactory::get(domain_id, provider_type, payment_mode)` exists with this signature, but doesn't verify it. **Action needed**: Read the actual factory implementation to confirm the method signature matches.

### 2. Type Conversion Details Incomplete
The `plan_to_info()` helper shows field mappings, but:
- Does `PlanInfo` exist in the payment provider port? The plan references it but doesn't verify.
- What type is `SubscriptionId::new()`? Is it a simple wrapper or does it do validation?
- The `PlanChangePreview` and `PlanChangeResult` conversions reference port types with prefixes (e.g., `PortPlanChangePreview`) but don't verify these types exist with matching fields.

**Risk**: If these types don't match, the implementation will fail at compile time.

### 3. DummyPaymentClient Behavior Not Verified
The plan claims `DummyPaymentClient` already implements `preview_plan_change` and `change_plan`, but doesn't verify the implementation is complete enough to support the refactored use case. What does the dummy return? Is it useful for local development?

### 4. Error Handling Gaps
- The `get_active_provider()` helper has fallback logic for when no providers are enabled, but what if the factory fails to create the provider (e.g., missing Stripe config)?
- Should there be different error messages for "Stripe not configured" vs "Dummy doesn't support this operation"?

### 5. StripeMode to PaymentMode Mapping Fragile
The plan introduces `get_active_provider()` which maps `StripeMode` to `PaymentMode`. This creates a second place where this mapping exists. Is there already a utility for this conversion? Duplicating this logic risks divergence.

### 6. Idempotency Key Dropped
In Step 6, `change_plan()` receives `_idempotency_key: &str` but the plan comments "Now handled by the provider". However:
- Does the provider interface accept an idempotency key?
- If not, is this a regression? Stripe uses idempotency keys to prevent duplicate charges.

**Risk**: Silently dropping the idempotency key could cause duplicate charges on retry.

### 7. No Rollback Strategy
What happens if the provider's `change_plan()` succeeds but the local database update (`update_plan()`) fails? The plan doesn't address transactional consistency.

---

## Suggested Improvements

### 1. Verify Factory and Port Interfaces First
Before implementing, read:
- `apps/api/src/application/use_cases/payment_provider_factory.rs` - confirm `get()` signature
- `apps/api/src/application/ports/payment_provider.rs` - confirm `PlanInfo`, `SubscriptionId`, `PlanChangePreview`, `PlanChangeResult` exist with expected fields
- `apps/api/src/infra/dummy_payment_client.rs` - confirm plan change methods are implemented

Add verification results to the plan before proceeding.

### 2. Add Idempotency Key to Provider Interface
If the port doesn't currently accept idempotency keys, extend it:
```rust
async fn change_plan(
    &self,
    subscription_id: &SubscriptionId,
    subscription_item_id: Option<&str>,
    new_plan: &PlanInfo,
    idempotency_key: Option<&str>, // Add this
) -> AppResult<PlanChangeResult>;
```

### 3. Extract StripeMode → PaymentMode Conversion
If this mapping doesn't already exist as a shared utility, create it:
```rust
impl From<StripeMode> for PaymentMode {
    fn from(mode: StripeMode) -> Self {
        match mode {
            StripeMode::Test => PaymentMode::Test,
            StripeMode::Live => PaymentMode::Live,
        }
    }
}
```

Place in `domain/entities/payment_mode.rs` to avoid duplication.

### 4. Add Logging
The refactored methods should log when switching providers:
```rust
tracing::debug!(
    domain_id = %domain_id,
    provider = ?provider_type,
    mode = ?payment_mode,
    "Using payment provider for plan change"
);
```

This aids debugging when behavior differs between Stripe and Dummy.

### 5. Consider a Transaction Wrapper
For `change_plan()`, consider wrapping the provider call and DB update in a recoverable pattern:
```rust
// Execute plan change
let result = provider.change_plan(...).await?;

// Update local state - log warning on failure, don't fail the operation
if let Err(e) = self.subscription_repo.update_plan(sub.id, new_plan.id).await {
    tracing::error!(?e, "Failed to update local plan after provider change");
    // Provider succeeded, so don't return error
    // Schedule reconciliation instead
}
```

Alternatively, rely on webhook reconciliation (which the plan notes is out of scope).

---

## Risks and Concerns

### 1. High Risk: Silent Idempotency Regression
Dropping the idempotency key parameter is dangerous. If users retry a failed request, they could be charged twice. **Must verify** the provider handles this internally or extend the interface.

### 2. Medium Risk: Type Mismatch at Compile Time
The plan provides detailed code but doesn't verify the port types match. If `PlanInfo` or `SubscriptionId` have different fields, implementation will stall while fixing mismatches.

### 3. Medium Risk: Breaking Change to Constructor
Adding `provider_factory` to `DomainBillingUseCases::new()` is a breaking change. Any other call sites (besides `setup.rs`) will fail to compile. The plan only updates `setup.rs` - verify there are no other instantiation sites.

### 4. Low Risk: Dummy Provider Incomplete
If `DummyPaymentClient` returns stub data for plan changes, local testing won't be representative. Acceptable for this task, but note it in the PR.

### 5. Low Risk: Enabled Providers Query N+1
`get_active_provider()` calls `list_active_by_domain()` on every plan change operation. If this is slow, consider caching or passing the enabled providers list from the caller.

---

## Pre-Implementation Checklist (Recommended)

Before writing code, verify:

1. [ ] `PaymentProviderFactory::get()` signature matches plan's assumption
2. [ ] `PaymentProviderPort` has `preview_plan_change` and `change_plan` methods
3. [ ] `PlanInfo` type exists and has the fields shown in `plan_to_info()`
4. [ ] `SubscriptionId::new()` exists and takes a string
5. [ ] `DummyPaymentClient` implements both methods
6. [ ] No other files instantiate `DomainBillingUseCases` besides `setup.rs`
7. [ ] Idempotency key handling is addressed (provider internal OR interface change)

---

## Summary

The plan is well-structured and identifies the right problem. The main concerns are:

1. **Critical**: Idempotency key handling must be resolved before implementation
2. **Important**: Verify type signatures before coding to avoid compile-time surprises
3. **Nice-to-have**: Add logging, extract mode conversion, consider DB failure handling

Recommend: Address the pre-implementation checklist items, then proceed with implementation.
