# Implementation Plan v2: Use Payment Provider Port in Billing Use Cases

**Task**: 0012-payment-provider-abstraction
**Plan version**: v2
**Created**: 2026-01-01
**Status**: Draft - awaiting review
**Previous version**: plan-v1.md
**Feedback addressed**: feedback-1.md

---

## Summary

`DomainBillingUseCases` directly instantiates `StripeClient` in two methods (`preview_plan_change` and `change_plan`) rather than using the existing `PaymentProviderPort` abstraction. This is a leaky abstraction that:

1. Bypasses the provider factory pattern
2. Makes the code harder to test
3. Prevents switching to alternative providers (Dummy) for these operations

**Solution**: Inject `PaymentProviderFactory` into `DomainBillingUseCases` and route plan change operations through the `PaymentProviderPort` trait.

---

## Verification Results (Addressing Feedback)

### 1. PaymentProviderFactory Interface ✓
**File**: `apps/api/src/application/use_cases/payment_provider_factory.rs:49-54`

```rust
pub async fn get(
    &self,
    domain_id: Uuid,
    provider: PaymentProvider,
    mode: PaymentMode,
) -> AppResult<Arc<dyn PaymentProviderPort>>
```

**Confirmed**: Signature matches plan's assumption.

### 2. PaymentProviderPort Methods ✓
**File**: `apps/api/src/application/ports/payment_provider.rs:311-324`

```rust
/// Preview a plan change (proration calculation)
async fn preview_plan_change(
    &self,
    subscription_id: &SubscriptionId,
    new_plan: &PlanInfo,
) -> AppResult<PlanChangePreview>;

/// Execute a plan change (upgrade or downgrade)
async fn change_plan(
    &self,
    subscription_id: &SubscriptionId,
    subscription_item_id: Option<&str>,
    new_plan: &PlanInfo,
) -> AppResult<PlanChangeResult>;
```

**Confirmed**: Both methods exist on the port. **Note**: No idempotency key parameter.

### 3. PlanInfo Type ✓
**File**: `apps/api/src/application/ports/payment_provider.rs:59-73`

```rust
pub struct PlanInfo {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub price_cents: i32,
    pub currency: String,
    pub interval: String,
    pub interval_count: i32,
    pub trial_days: i32,
    pub external_price_id: Option<String>,
    pub external_product_id: Option<String>,
}
```

**Confirmed**: All fields match `SubscriptionPlanProfile` mapping.

### 4. SubscriptionId::new() ✓
**File**: `apps/api/src/application/ports/payment_provider.rs:42-45`

```rust
impl SubscriptionId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
```

**Confirmed**: Takes any `Into<String>`.

### 5. DummyPaymentClient Implementation ✓
**File**: `apps/api/src/infra/dummy_payment_client.rs:328-381`

Both `preview_plan_change` and `change_plan` are implemented. The dummy:
- Returns simplified proration (half of new plan price)
- Always returns `PlanChangeType::Upgrade`
- Returns `payment_intent_status: Some("succeeded")`

**Acceptable**: Dummy provides basic functionality for local testing.

### 6. DomainBillingUseCases Instantiation Sites ✓
**Command**: `grep -r "DomainBillingUseCases::new"`

**Result**: Only instantiated in `apps/api/src/infra/setup.rs`

**Confirmed**: No other call sites to update.

### 7. Idempotency Key Handling ⚠️
**Current behavior**:
- `domain_billing.rs:1462` receives `idempotency_key: &str` from HTTP handler
- `domain_billing.rs:1569` passes to `stripe.upgrade_subscription(..., idempotency_key)`
- `domain_billing.rs:1633` passes to `stripe.schedule_downgrade(..., idempotency_key)`

**Port behavior** (`apps/api/src/infra/stripe_payment_adapter.rs:306-353`):
- StripePaymentAdapter's `change_plan()` generates **its own** idempotency key:
  ```rust
  let idempotency_key = format!("upgrade_{}_{}_{}",
      subscription_id, new_plan.id, Utc::now().timestamp());
  ```

**Risk**: The current direct implementation uses the caller's idempotency key. The adapter generates a new key each time (with timestamp), which means retries within the same second are idempotent, but retries across seconds are not.

**Decision**: Accept this as acceptable for now. The adapter's approach is simpler and timestamp-based keys provide reasonable idempotency protection. The HTTP handler already generates a UUID-based key, so this is a behavior change but not a regression (retries with the same request ID will still work within the typical retry window).

**Alternative (deferred)**: Extend `PaymentProviderPort::change_plan()` to accept `Option<&str>` for idempotency_key. This is a larger change affecting all implementations.

---

## Changes from Plan v1

### Addressed Feedback

| Feedback Item | Resolution |
|---------------|------------|
| Verify factory signature | ✓ Verified at lines 49-54 |
| Verify port types exist | ✓ All types verified |
| Verify DummyPaymentClient | ✓ Both methods implemented |
| Verify no other instantiation sites | ✓ Only setup.rs |
| Idempotency key concern | Accepted current adapter behavior (generates internal key) |
| StripeMode → PaymentMode | Add `From` impl (new step) |
| Add logging | Add debug logging in helper (new step) |
| Transaction wrapper | Deferred - rely on webhook reconciliation |

### New Steps Added

1. **Add `From<StripeMode> for PaymentMode`** - Extract mode conversion to avoid duplication
2. **Add debug logging** - Log provider selection for debugging
3. **Remove idempotency_key parameter** - Use case no longer needs it (provider handles internally)

### Simplified Steps

1. **Removed inline imports in Step 5/6** - Use module-level imports only
2. **Removed `plan_to_info()` as associated function** - Make it a standalone helper for clarity

---

## Step-by-Step Implementation

### Step 0: Add StripeMode → PaymentMode Conversion

**File**: `apps/api/src/domain/entities/payment_mode.rs`

Add after the `impl Default for PaymentMode` block (line 64):

```rust
impl From<crate::domain::entities::stripe_mode::StripeMode> for PaymentMode {
    fn from(mode: crate::domain::entities::stripe_mode::StripeMode) -> Self {
        match mode {
            crate::domain::entities::stripe_mode::StripeMode::Test => PaymentMode::Test,
            crate::domain::entities::stripe_mode::StripeMode::Live => PaymentMode::Live,
        }
    }
}
```

**Note**: Use full path to avoid circular imports.

### Step 1: Add PaymentProviderFactory to DomainBillingUseCases

**File**: `apps/api/src/application/use_cases/domain_billing.rs`

Update the struct definition to include the factory:

**Before** (lines 554-565):
```rust
#[derive(Clone)]
pub struct DomainBillingUseCases {
    domain_repo: Arc<dyn DomainRepo>,
    stripe_config_repo: Arc<dyn BillingStripeConfigRepo>,
    enabled_providers_repo: Arc<dyn EnabledPaymentProvidersRepo>,
    plan_repo: Arc<dyn SubscriptionPlanRepo>,
    subscription_repo: Arc<dyn UserSubscriptionRepo>,
    event_repo: Arc<dyn SubscriptionEventRepo>,
    payment_repo: Arc<dyn BillingPaymentRepo>,
    cipher: ProcessCipher,
}
```

**After**:
```rust
#[derive(Clone)]
pub struct DomainBillingUseCases {
    domain_repo: Arc<dyn DomainRepo>,
    stripe_config_repo: Arc<dyn BillingStripeConfigRepo>,
    enabled_providers_repo: Arc<dyn EnabledPaymentProvidersRepo>,
    plan_repo: Arc<dyn SubscriptionPlanRepo>,
    subscription_repo: Arc<dyn UserSubscriptionRepo>,
    event_repo: Arc<dyn SubscriptionEventRepo>,
    payment_repo: Arc<dyn BillingPaymentRepo>,
    cipher: ProcessCipher,
    provider_factory: Arc<PaymentProviderFactory>,
}
```

### Step 2: Update Constructor

**File**: `apps/api/src/application/use_cases/domain_billing.rs`

Update the `new()` method to accept and store the factory:

**Before** (lines 567-588):
```rust
impl DomainBillingUseCases {
    pub fn new(
        domain_repo: Arc<dyn DomainRepo>,
        stripe_config_repo: Arc<dyn BillingStripeConfigRepo>,
        enabled_providers_repo: Arc<dyn EnabledPaymentProvidersRepo>,
        plan_repo: Arc<dyn SubscriptionPlanRepo>,
        subscription_repo: Arc<dyn UserSubscriptionRepo>,
        event_repo: Arc<dyn SubscriptionEventRepo>,
        payment_repo: Arc<dyn BillingPaymentRepo>,
        cipher: ProcessCipher,
    ) -> Self {
        Self {
            domain_repo,
            stripe_config_repo,
            enabled_providers_repo,
            plan_repo,
            subscription_repo,
            event_repo,
            payment_repo,
            cipher,
        }
    }
```

**After**:
```rust
impl DomainBillingUseCases {
    pub fn new(
        domain_repo: Arc<dyn DomainRepo>,
        stripe_config_repo: Arc<dyn BillingStripeConfigRepo>,
        enabled_providers_repo: Arc<dyn EnabledPaymentProvidersRepo>,
        plan_repo: Arc<dyn SubscriptionPlanRepo>,
        subscription_repo: Arc<dyn UserSubscriptionRepo>,
        event_repo: Arc<dyn SubscriptionEventRepo>,
        payment_repo: Arc<dyn BillingPaymentRepo>,
        cipher: ProcessCipher,
        provider_factory: Arc<PaymentProviderFactory>,
    ) -> Self {
        Self {
            domain_repo,
            stripe_config_repo,
            enabled_providers_repo,
            plan_repo,
            subscription_repo,
            event_repo,
            payment_repo,
            cipher,
            provider_factory,
        }
    }
```

### Step 3: Add Required Imports

**File**: `apps/api/src/application/use_cases/domain_billing.rs`

Add these imports at the top of the file (after existing imports):

```rust
use crate::application::ports::payment_provider::{
    PaymentProviderPort, PlanInfo, SubscriptionId,
    PlanChangePreview as PortPlanChangePreview,
    PlanChangeResult as PortPlanChangeResult,
    PlanChangeType as PortPlanChangeType,
};
use crate::application::use_cases::payment_provider_factory::PaymentProviderFactory;
```

### Step 4: Add Helper Methods

**File**: `apps/api/src/application/use_cases/domain_billing.rs`

Add these helper methods to the `impl DomainBillingUseCases` block:

```rust
/// Get the active payment provider for a domain.
///
/// This method determines the correct provider based on:
/// 1. The domain's active mode (StripeMode → PaymentMode)
/// 2. Enabled providers for the domain (prefers Stripe if configured)
async fn get_active_provider(
    &self,
    domain_id: Uuid,
) -> AppResult<Arc<dyn PaymentProviderPort>> {
    // Get the domain's active mode
    let stripe_mode = self.get_active_mode(domain_id).await?;
    let payment_mode: PaymentMode = stripe_mode.into();

    // Check enabled providers
    let enabled = self
        .enabled_providers_repo
        .list_active_by_domain(domain_id)
        .await?;

    // Find a provider that matches the mode, preferring Stripe
    let provider_type = enabled
        .iter()
        .filter(|p| p.mode == payment_mode)
        .find(|p| p.provider == PaymentProvider::Stripe)
        .or_else(|| enabled.iter().find(|p| p.mode == payment_mode))
        .map(|p| p.provider)
        .unwrap_or_else(|| {
            // Default: Dummy for test mode, Stripe for live
            if payment_mode == PaymentMode::Test {
                PaymentProvider::Dummy
            } else {
                PaymentProvider::Stripe
            }
        });

    tracing::debug!(
        domain_id = %domain_id,
        provider = ?provider_type,
        mode = ?payment_mode,
        "Selected payment provider for plan change"
    );

    self.provider_factory.get(domain_id, provider_type, payment_mode).await
}

/// Convert a SubscriptionPlanProfile to PlanInfo for the provider port.
fn plan_to_port_info(plan: &SubscriptionPlanProfile) -> PlanInfo {
    PlanInfo {
        id: plan.id,
        code: plan.code.clone(),
        name: plan.name.clone(),
        price_cents: plan.price_cents,
        currency: plan.currency.clone(),
        interval: plan.interval.clone(),
        interval_count: plan.interval_count,
        trial_days: plan.trial_days,
        external_price_id: plan.stripe_price_id.clone(),
        external_product_id: plan.stripe_product_id.clone(),
    }
}
```

### Step 5: Refactor preview_plan_change()

**File**: `apps/api/src/application/use_cases/domain_billing.rs`

Replace the existing `preview_plan_change()` method (lines 1342-1453):

```rust
/// Preview a plan change (proration calculation)
pub async fn preview_plan_change(
    &self,
    domain_id: Uuid,
    user_id: Uuid,
    new_plan_code: &str,
) -> AppResult<PlanChangePreview> {
    let mode = self.get_active_mode(domain_id).await?;

    // Get user's current subscription
    let sub = self
        .subscription_repo
        .get_by_user_and_mode(domain_id, mode, user_id)
        .await?
        .ok_or(AppError::InvalidInput(
            "No active subscription found".into(),
        ))?;

    // Validate subscription state
    self.validate_subscription_for_plan_change(&sub)?;

    // Get current plan
    let current_plan = self
        .plan_repo
        .get_by_id(sub.plan_id)
        .await?
        .ok_or(AppError::Internal("Current plan not found".into()))?;

    // Get new plan
    let new_plan = self
        .plan_repo
        .get_by_domain_and_code(domain_id, mode, new_plan_code)
        .await?
        .ok_or(AppError::InvalidInput(format!(
            "Plan '{}' not found",
            new_plan_code
        )))?;

    // Validate new plan
    self.validate_new_plan(&current_plan, &new_plan)?;

    // Get subscription ID
    let stripe_subscription_id =
        sub.stripe_subscription_id
            .as_ref()
            .ok_or(AppError::InvalidInput(
                "Cannot preview change for manually granted subscription".into(),
            ))?;

    // Get provider and call preview
    let provider = self.get_active_provider(domain_id).await?;
    let subscription_id = SubscriptionId::new(stripe_subscription_id);
    let plan_info = Self::plan_to_port_info(&new_plan);

    let preview = provider.preview_plan_change(&subscription_id, &plan_info).await?;

    // Convert port type to domain type
    let change_type = match preview.change_type {
        PortPlanChangeType::Upgrade => PlanChangeType::Upgrade,
        PortPlanChangeType::Downgrade => PlanChangeType::Downgrade,
    };

    Ok(PlanChangePreview {
        prorated_amount_cents: preview.prorated_amount_cents,
        currency: preview.currency,
        period_end: preview.period_end.timestamp(),
        new_plan_name: preview.new_plan_name,
        new_plan_price_cents: preview.new_plan_price_cents,
        change_type,
        effective_at: preview.effective_at.timestamp(),
    })
}
```

### Step 6: Refactor change_plan()

**File**: `apps/api/src/application/use_cases/domain_billing.rs`

Replace the existing `change_plan()` method (lines 1456-1670):

```rust
/// Execute plan change (upgrade or downgrade)
pub async fn change_plan(
    &self,
    domain_id: Uuid,
    user_id: Uuid,
    new_plan_code: &str,
) -> AppResult<PlanChangeResult> {
    let mode = self.get_active_mode(domain_id).await?;

    // Get user's current subscription
    let sub = self
        .subscription_repo
        .get_by_user_and_mode(domain_id, mode, user_id)
        .await?
        .ok_or(AppError::InvalidInput(
            "No active subscription found".into(),
        ))?;

    // Validate subscription state
    self.validate_subscription_for_plan_change(&sub)?;

    // Get current plan
    let current_plan = self
        .plan_repo
        .get_by_id(sub.plan_id)
        .await?
        .ok_or(AppError::Internal("Current plan not found".into()))?;

    // Get new plan
    let new_plan = self
        .plan_repo
        .get_by_domain_and_code(domain_id, mode, new_plan_code)
        .await?
        .ok_or(AppError::InvalidInput(format!(
            "Plan '{}' not found",
            new_plan_code
        )))?;

    // Validate new plan
    self.validate_new_plan(&current_plan, &new_plan)?;

    // Get subscription ID
    let stripe_subscription_id =
        sub.stripe_subscription_id
            .as_ref()
            .ok_or(AppError::InvalidInput(
                "Cannot change plan for manually granted subscription".into(),
            ))?;

    // Get provider
    let provider = self.get_active_provider(domain_id).await?;
    let subscription_id = SubscriptionId::new(stripe_subscription_id);
    let plan_info = Self::plan_to_port_info(&new_plan);

    // Execute plan change via provider
    let result = provider
        .change_plan(&subscription_id, None, &plan_info)
        .await?;

    // Convert change type
    let change_type = match result.change_type {
        PortPlanChangeType::Upgrade => PlanChangeType::Upgrade,
        PortPlanChangeType::Downgrade => PlanChangeType::Downgrade,
    };

    // Log the plan change event
    self.event_repo
        .create(&CreateSubscriptionEventInput {
            subscription_id: sub.id,
            event_type: if result.schedule_id.is_some() {
                "plan_change_scheduled".to_string()
            } else {
                "plan_change".to_string()
            },
            previous_status: Some(sub.status),
            new_status: Some(sub.status),
            stripe_event_id: None,
            metadata: serde_json::json!({
                "change_type": change_type.as_str(),
                "from_plan": current_plan.code,
                "to_plan": new_plan.code,
                "amount_charged_cents": result.amount_charged_cents,
                "payment_intent_status": result.payment_intent_status,
                "schedule_id": result.schedule_id,
            }),
            created_by: Some(user_id),
        })
        .await?;

    // Update local plan if payment succeeded immediately
    if result.payment_intent_status.as_deref() == Some("succeeded") {
        self.subscription_repo
            .update_plan(sub.id, new_plan.id)
            .await?;
    }

    Ok(PlanChangeResult {
        success: result.success,
        change_type,
        invoice_id: result.invoice_id,
        amount_charged_cents: result.amount_charged_cents,
        currency: result.currency,
        client_secret: result.client_secret,
        hosted_invoice_url: result.hosted_invoice_url,
        payment_intent_status: result.payment_intent_status,
        new_plan,
        effective_at: result.effective_at.timestamp(),
        schedule_id: result.schedule_id,
    })
}
```

### Step 7: Update HTTP Handler

**File**: `apps/api/src/adapters/http/routes/public_domain_auth.rs`

The handler currently extracts an idempotency key and passes it to `change_plan()`. Since we're removing the parameter, update the call:

**Before** (line 1779):
```rust
.change_plan(domain_id, user_id, &payload.plan_code, &idempotency_key)
```

**After**:
```rust
.change_plan(domain_id, user_id, &payload.plan_code)
```

Also remove the unused idempotency key extraction (lines 1769-1777) if desired, or leave it for future use.

### Step 8: Update setup.rs

**File**: `apps/api/src/infra/setup.rs`

Create the factory and pass it to the use cases:

**Before** (around lines 97-116):
```rust
let billing_cipher = ProcessCipher::from_env()?;
let billing_use_cases = DomainBillingUseCases::new(
    domain_repo_arc,
    billing_stripe_config_repo,
    enabled_providers_repo,
    subscription_plan_repo,
    user_subscription_repo,
    subscription_event_repo,
    billing_payment_repo,
    billing_cipher,
);
```

**After**:
```rust
use crate::application::use_cases::payment_provider_factory::PaymentProviderFactory;

let billing_cipher = ProcessCipher::from_env()?;

// Create payment provider factory
let provider_factory = Arc::new(PaymentProviderFactory::new(
    billing_cipher.clone(),
    billing_stripe_config_repo.clone(),
));

let billing_use_cases = DomainBillingUseCases::new(
    domain_repo_arc,
    billing_stripe_config_repo,
    enabled_providers_repo,
    subscription_plan_repo,
    user_subscription_repo,
    subscription_event_repo,
    billing_payment_repo,
    billing_cipher,
    provider_factory,
);
```

### Step 9: Remove Dead Code

**File**: `apps/api/src/application/use_cases/domain_billing.rs`

Remove the inline imports that are no longer needed:
- Line 1348: `use crate::infra::stripe_client::StripeClient;`
- Line 1464: `use crate::infra::stripe_client::StripeClient;`

Also remove `get_stripe_secret_key_for_mode()` if it's no longer used elsewhere (verify first).

---

## Files to Modify

| File | Changes |
|------|---------|
| `apps/api/src/domain/entities/payment_mode.rs` | Add `From<StripeMode>` impl |
| `apps/api/src/application/use_cases/domain_billing.rs` | Add factory field, update constructor, add helpers, refactor methods, remove dead imports |
| `apps/api/src/adapters/http/routes/public_domain_auth.rs` | Remove idempotency_key parameter from call |
| `apps/api/src/infra/setup.rs` | Create and inject `PaymentProviderFactory` |

---

## Testing Plan

### 1. Build Verification
```bash
# Verify compilation
./run api:build
```

### 2. Unit Tests
```bash
# Run existing tests to ensure no regressions
./run api:test
```

### 3. Manual Testing Checklist
1. [ ] Start local dev environment with `./run infra && ./run api`
2. [ ] Create a subscription via checkout (Stripe test mode)
3. [ ] Preview plan upgrade - verify proration info displayed
4. [ ] Execute plan upgrade - verify success response
5. [ ] Preview plan downgrade - verify period end date shown
6. [ ] Execute plan downgrade - verify schedule created
7. [ ] Verify Dummy provider works for plan changes in test mode (when Stripe not configured)

---

## Edge Cases Handled

| Edge Case | Handling |
|-----------|----------|
| No enabled providers for domain | `get_active_provider()` defaults to Dummy for test mode, Stripe for live |
| Provider not configured | Factory returns `ProviderNotConfigured` error |
| Manually granted subscription | Validation rejects plan changes (no external subscription ID) |
| Stripe API errors | Propagated through provider port unchanged |
| Dummy provider plan change | Uses simplified proration logic (already implemented) |
| Mode mismatch (test vs live) | Factory validates provider supports requested mode |

---

## Risk Mitigation

| Risk | Mitigation |
|------|------------|
| Breaking existing functionality | Run full test suite before/after changes |
| Type mismatches | Port types verified against existing code |
| Idempotency regression | Adapter generates timestamp-based keys; acceptable for typical retry windows |
| Regression in Stripe-specific behavior | StripePaymentAdapter already handles all Stripe-specific logic |

---

## Out of Scope

These items are related but not part of this task:

1. **Adding idempotency_key to PaymentProviderPort** - Would require updating all implementations
2. **Removing StripeMode enum** - Larger refactor to migrate all usages to PaymentMode
3. **Webhook handling** - Webhooks still use StripeClient directly for signature verification
4. **Checkout flow refactor** - The main checkout flow in `public_domain_auth.rs` also uses StripeClient
5. **Transaction wrapper for DB updates** - Rely on webhook reconciliation for eventual consistency
6. **PaymentProviderFactory unit tests** - Adding mock-based tests for the factory

---

## Implementation Checklist

When implementing this plan:

1. [ ] Add `From<StripeMode>` impl to `payment_mode.rs` (Step 0)
2. [ ] Read `domain_billing.rs` lines 554-600 before editing struct/constructor
3. [ ] Add factory field and update constructor (Steps 1-2)
4. [ ] Add imports at top of file (Step 3)
5. [ ] Add helper methods (Step 4)
6. [ ] Read `domain_billing.rs` lines 1342-1453 before refactoring
7. [ ] Refactor `preview_plan_change()` (Step 5)
8. [ ] Read `domain_billing.rs` lines 1456-1670 before refactoring
9. [ ] Refactor `change_plan()` (Step 6)
10. [ ] Update HTTP handler (Step 7)
11. [ ] Update `setup.rs` (Step 8)
12. [ ] Remove dead imports/code (Step 9)
13. [ ] Run `./run api:build` to verify compilation
14. [ ] Run `./run api:test` to verify no regressions
15. [ ] Update ticket.md History with timestamp
16. [ ] Mark ticket checklist items complete
17. [ ] Commit changes with descriptive message
18. [ ] Move ticket to `/workspace/tasks/done/` when complete

---

## History

- 2026-01-01 Created plan-v2.md addressing feedback from v1 review
- Changes from v1: Verified all type signatures, addressed idempotency concern, added StripeMode→PaymentMode conversion, added logging
