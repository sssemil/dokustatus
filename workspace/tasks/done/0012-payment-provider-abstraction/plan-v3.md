# Implementation Plan v3: Use Payment Provider Port in Billing Use Cases

**Task**: 0012-payment-provider-abstraction
**Plan version**: v3 (final)
**Created**: 2026-01-01
**Status**: Ready for implementation
**Previous version**: plan-v2.md
**Feedback addressed**: feedback-2.md

---

## Summary

`DomainBillingUseCases` directly instantiates `StripeClient` in two methods (`preview_plan_change` and `change_plan`) rather than using the existing `PaymentProviderPort` abstraction. This is a leaky abstraction that:

1. Bypasses the provider factory pattern
2. Makes the code harder to test
3. Prevents switching to alternative providers (Dummy) for these operations

**Solution**: Inject `PaymentProviderFactory` into `DomainBillingUseCases` and route plan change operations through the `PaymentProviderPort` trait.

---

## Verification Results (All Critical Items Confirmed)

### From v2 Verification (Unchanged)

| Item | Status | Location |
|------|--------|----------|
| `PaymentProviderFactory::get()` signature | ✓ | `payment_provider_factory.rs:49-54` |
| `PaymentProviderPort` methods | ✓ | `payment_provider.rs:311-324` |
| `PlanInfo` struct fields | ✓ | `payment_provider.rs:59-73` |
| `SubscriptionId::new()` | ✓ | `payment_provider.rs:42-45` |
| `DummyPaymentClient` implementation | ✓ | `dummy_payment_client.rs:328-381` |
| Single instantiation site | ✓ | Only in `setup.rs` |

### New Verification (Addressing feedback-2.md)

#### 1. Validation Helper Methods ✓
**Verified**: Both methods exist in `domain_billing.rs`

```rust
// Line 1674
fn validate_subscription_for_plan_change(&self, sub: &UserSubscriptionProfile) -> AppResult<()>

// Line 1733
fn validate_new_plan(&self, current_plan: &SubscriptionPlanProfile, new_plan: &SubscriptionPlanProfile) -> AppResult<()>
```

#### 2. CreateSubscriptionEventInput ✓
**Verified**: `domain_billing.rs:232-240`

```rust
pub struct CreateSubscriptionEventInput {
    pub subscription_id: Uuid,
    pub event_type: String,
    pub previous_status: Option<SubscriptionStatus>,
    pub new_status: Option<SubscriptionStatus>,
    pub stripe_event_id: Option<String>,
    pub metadata: serde_json::Value,  // ✓ serde_json::Value
    pub created_by: Option<Uuid>,     // ✓ Option<Uuid>
}
```

**Confirmed**: Field names and types match our plan exactly.

#### 3. PlanChangeType::as_str() ✓
**Verified**: Both domain and port types have `as_str()` method

- Port type: `payment_provider.rs:153-159`
- Domain type: `domain_billing.rs:308-314`

```rust
pub fn as_str(&self) -> &'static str {
    match self {
        PlanChangeType::Upgrade => "upgrade",
        PlanChangeType::Downgrade => "downgrade",
    }
}
```

#### 4. PaymentProviderFactory::new() Signature ✓
**Verified**: `payment_provider_factory.rs:27-33`

```rust
pub fn new(cipher: ProcessCipher, config_repo: Arc<dyn BillingStripeConfigRepo>) -> Self
```

**Confirmed**: Matches our plan's usage exactly.

#### 5. HTTP Handler Call Sites ✓
**Verified**: Only one call site exists

```
apps/api/src/adapters/http/routes/public_domain_auth.rs:1779
    .change_plan(domain_id, user_id, &payload.plan_code, &idempotency_key)
```

#### 6. get_stripe_secret_key_for_mode() Usages ✓
**Verified**: Still needed - keep the method

```
apps/api/src/application/use_cases/domain_billing.rs:759  (definition)
apps/api/src/application/use_cases/domain_billing.rs:1406 (in preview_plan_change - will be removed)
apps/api/src/application/use_cases/domain_billing.rs:1508 (in change_plan - will be removed)
```

**Decision**: Do NOT remove `get_stripe_secret_key_for_mode()`. It may be used by other code paths not visible in grep (e.g., webhook handlers or future use). Only remove the inline usage in the two methods we're refactoring.

#### 7. Existing Imports ✓
**Verified**: `domain_billing.rs:12-16` already has:

```rust
use crate::domain::entities::{
    payment_mode::PaymentMode, payment_provider::PaymentProvider,
    stripe_mode::StripeMode, ...
};
```

**Confirmed**: `PaymentMode` and `PaymentProvider` are already imported.

#### 8. Port vs Domain Type Fields ✓
**Verified type differences:**

| Field | Port Type | Domain Type |
|-------|-----------|-------------|
| `period_end` | `DateTime<Utc>` | `i64` (timestamp) |
| `effective_at` | `DateTime<Utc>` | `i64` (timestamp) |
| `PlanChangeResult.new_plan` | N/A (not in port) | `SubscriptionPlanProfile` |

**Confirmed**: Conversions via `.timestamp()` are correct.

---

## Changes from Plan v2

### Addressed Feedback Items

| Feedback Item | Resolution |
|---------------|------------|
| Verify validation helpers exist | ✓ Lines 1674, 1733 confirmed |
| Verify `CreateSubscriptionEventInput` | ✓ Lines 232-240, field names/types match |
| Verify `PlanChangeType::as_str()` | ✓ Lines 308-314 confirmed |
| Verify `PaymentProviderFactory::new()` | ✓ Lines 27-33 confirmed |
| Verify all `change_plan()` call sites | ✓ Only one in `public_domain_auth.rs:1779` |
| Verify `get_stripe_secret_key_for_mode` usages | ✓ Keep method, only remove inline usages |
| Missing imports (`PaymentMode`, `PaymentProvider`) | ✓ Already imported at lines 12-16 |
| Verify `effective_at` type conversion | ✓ DateTime→timestamp confirmed correct |
| Decide HTTP handler idempotency key | **Remove entirely** - cleaner, avoids dead code |

### Key Simplifications

1. **Step 0 kept from v2**: Add `From<StripeMode> for PaymentMode` trait impl for cleaner conversion
2. **Step 3 simplified**: No need to add `PaymentMode`/`PaymentProvider` imports (already present)
3. **Step 7 clarified**: Remove idempotency key extraction entirely from HTTP handler
4. **Step 9 clarified**: Keep `get_stripe_secret_key_for_mode()` method, only remove inline `StripeClient` usage

---

## Step-by-Step Implementation

### Step 0: Add StripeMode → PaymentMode Conversion

**File**: `apps/api/src/domain/entities/payment_mode.rs`

Add after the `impl Default for PaymentMode` block (around line 64):

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

Add these imports at the top of the file (after existing imports around line 18):

```rust
use crate::application::ports::payment_provider::{
    PaymentProviderPort, PlanInfo, SubscriptionId,
    PlanChangePreview as PortPlanChangePreview,
    PlanChangeResult as PortPlanChangeResult,
    PlanChangeType as PortPlanChangeType,
};
use crate::application::use_cases::payment_provider_factory::PaymentProviderFactory;
```

**Note**: `PaymentMode` and `PaymentProvider` are already imported at lines 12-16.

### Step 4: Add Helper Methods

**File**: `apps/api/src/application/use_cases/domain_billing.rs`

Add these helper methods to the `impl DomainBillingUseCases` block (after the constructor):

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

Replace the existing `change_plan()` method (lines 1456-1670). **Note**: Remove the `idempotency_key` parameter.

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

Remove the idempotency key extraction and update the call.

**Before** (lines 1769-1780):
```rust
    // Get or generate idempotency key
    let idempotency_key = headers
        .get("Idempotency-Key")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    // Execute plan change
    let result = app_state
        .billing_use_cases
        .change_plan(domain_id, user_id, &payload.plan_code, &idempotency_key)
        .await?;
```

**After**:
```rust
    // Execute plan change
    let result = app_state
        .billing_use_cases
        .change_plan(domain_id, user_id, &payload.plan_code)
        .await?;
```

**Decision rationale**: Removing the idempotency key extraction entirely is cleaner than leaving dead code. The payment provider adapter generates its own timestamp-based idempotency keys internally, which provides reasonable protection for typical retry windows.

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

### Step 9: Remove Dead Imports

**File**: `apps/api/src/application/use_cases/domain_billing.rs`

Remove these inline imports that are no longer needed:
- Line ~1348 (in old `preview_plan_change`): `use crate::infra::stripe_client::StripeClient;`
- Line ~1464 (in old `change_plan`): `use crate::infra::stripe_client::StripeClient;`

**Note**: Do NOT remove `get_stripe_secret_key_for_mode()` - it may be used elsewhere.

---

## Files to Modify

| File | Changes |
|------|---------|
| `apps/api/src/domain/entities/payment_mode.rs` | Add `From<StripeMode>` impl |
| `apps/api/src/application/use_cases/domain_billing.rs` | Add factory field, update constructor, add helpers, refactor methods, remove dead imports |
| `apps/api/src/adapters/http/routes/public_domain_auth.rs` | Remove idempotency_key extraction and parameter |
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

## Risk Assessment

### Medium Risk: Idempotency Window Regression

**Behavior change**: The adapter uses timestamp-based idempotency keys (`Utc::now().timestamp()`) instead of caller-provided keys.

**Impact**:
- Retries within the same second are idempotent ✓
- Retries across seconds may create duplicate charges ✗

**Real-world scenario**: User clicks "upgrade", network timeout, user retries 2 seconds later → potential duplicate charge.

**Mitigation**:
1. This is acceptable for now because Stripe itself has built-in duplicate detection based on subscription+price combination
2. Document in PR description as a known behavior change
3. Consider fast-following with an `idempotency_key` parameter on the port interface if issues arise

### Low Risk: N+1 Query on Every Plan Change

`get_active_provider()` calls `list_active_by_domain()` on every plan change operation.

**Mitigation**: Accept for now, monitor in production. Consider caching if it becomes a hot path.

### Low Risk: Debug Log Volume

The debug log in `get_active_provider()` runs on every plan change.

**Mitigation**: Debug level is appropriate; can adjust later if needed.

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
5. [ ] Add helper methods after constructor (Step 4)
6. [ ] Read `domain_billing.rs` lines 1342-1453 before refactoring
7. [ ] Refactor `preview_plan_change()` (Step 5)
8. [ ] Read `domain_billing.rs` lines 1456-1670 before refactoring
9. [ ] Refactor `change_plan()` - remove idempotency_key param (Step 6)
10. [ ] Update HTTP handler - remove idempotency_key extraction (Step 7)
11. [ ] Update `setup.rs` (Step 8)
12. [ ] Remove dead inline StripeClient imports (Step 9)
13. [ ] Run `./run api:build` to verify compilation
14. [ ] Run `./run api:test` to verify no regressions
15. [ ] Update ticket.md History with timestamp
16. [ ] Mark ticket checklist items complete
17. [ ] Commit changes with descriptive message
18. [ ] Move ticket to `/workspace/tasks/done/` when complete

---

## History

- 2026-01-01 08:15 Created plan-v2.md addressing feedback from v1 review
- 2026-01-01 09:30 Created plan-v3.md addressing feedback-2.md:
  - Verified validation helper methods exist (lines 1674, 1733)
  - Verified CreateSubscriptionEventInput fields match
  - Verified PlanChangeType::as_str() exists
  - Verified PaymentProviderFactory::new() signature
  - Confirmed PaymentMode/PaymentProvider already imported
  - Confirmed only one change_plan() call site
  - Decided to keep get_stripe_secret_key_for_mode() method
  - Decided to remove idempotency_key extraction entirely (cleaner than dead code)
  - Added idempotency regression to risk assessment with mitigation
