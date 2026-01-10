# Implementation Plan v1: Use Payment Provider Port in Billing Use Cases

**Task**: 0012-payment-provider-abstraction
**Plan version**: v1
**Created**: 2026-01-01
**Status**: Draft - awaiting review

---

## Summary

`DomainBillingUseCases` directly instantiates `StripeClient` in two methods (`preview_plan_change` and `change_plan`) rather than using the existing `PaymentProviderPort` abstraction. This is a leaky abstraction that:

1. Bypasses the provider factory pattern
2. Makes the code harder to test
3. Prevents switching to alternative providers (Dummy) for these operations

**Solution**: Inject `PaymentProviderFactory` into `DomainBillingUseCases` and route plan change operations through the `PaymentProviderPort` trait.

---

## Current State Analysis

### Direct StripeClient Usage

**File**: `apps/api/src/application/use_cases/domain_billing.rs`

| Line | Method | Usage |
|------|--------|-------|
| 1348 | `preview_plan_change()` | `use crate::infra::stripe_client::StripeClient;` |
| 1406-1407 | `preview_plan_change()` | `let stripe = StripeClient::new(stripe_secret);` |
| 1464 | `change_plan()` | `use crate::infra::stripe_client::StripeClient;` |
| 1508-1509 | `change_plan()` | `let stripe = StripeClient::new(stripe_secret);` |

Both methods:
1. Get the Stripe secret key via `get_stripe_secret_key_for_mode()`
2. Directly create a `StripeClient::new(stripe_secret)`
3. Call Stripe-specific operations

### Existing Abstractions

| Component | Location | Purpose |
|-----------|----------|---------|
| `PaymentProviderPort` | `apps/api/src/application/ports/payment_provider.rs` | Trait defining provider-agnostic operations |
| `PaymentProviderFactory` | `apps/api/src/application/use_cases/payment_provider_factory.rs` | Creates provider instances with decrypted credentials |
| `StripePaymentAdapter` | `apps/api/src/infra/stripe_payment_adapter.rs` | Implements `PaymentProviderPort` for Stripe |
| `DummyPaymentClient` | `apps/api/src/infra/dummy_payment_client.rs` | Implements `PaymentProviderPort` for testing |

The port already has the needed methods:
- `preview_plan_change(&self, subscription_id: &SubscriptionId, new_plan: &PlanInfo) -> AppResult<PlanChangePreview>`
- `change_plan(&self, subscription_id: &SubscriptionId, subscription_item_id: Option<&str>, new_plan: &PlanInfo) -> AppResult<PlanChangeResult>`

---

## Why This Is a Problem

### 1. Leaky Abstraction
The use cases layer imports from `crate::infra::stripe_client`, violating clean architecture. The `infra` layer should only be accessed via ports defined in `application`.

### 2. Testing Difficulty
Unit tests cannot mock the `StripeClient` because it's instantiated inline. This forces integration tests against real Stripe APIs.

### 3. Provider Lock-in
The code cannot use the Dummy provider for plan changes during local development, even though `DummyPaymentClient` implements `preview_plan_change` and `change_plan`.

### 4. Duplication
The `PaymentProviderFactory` already handles credential decryption and provider instantiation. The direct usage duplicates this logic.

---

## Step-by-Step Implementation

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

### Step 3: Add Helper Method to Get Active Provider

**File**: `apps/api/src/application/use_cases/domain_billing.rs`

Add a helper method to get the appropriate provider for a domain:

```rust
/// Get the active payment provider for a domain.
///
/// This method determines the correct provider and mode based on:
/// 1. Enabled providers for the domain
/// 2. Current billing configuration (StripeMode -> PaymentMode)
async fn get_active_provider(
    &self,
    domain_id: Uuid,
) -> AppResult<Arc<dyn PaymentProviderPort>> {
    // Get the domain's active mode
    let stripe_mode = self.get_active_mode(domain_id).await?;
    let payment_mode = match stripe_mode {
        StripeMode::Test => PaymentMode::Test,
        StripeMode::Live => PaymentMode::Live,
    };

    // Check enabled providers - prefer Stripe if enabled, else Dummy for test mode
    let enabled = self
        .enabled_providers_repo
        .list_active_by_domain(domain_id)
        .await?;

    // Find a provider that matches the mode
    let provider_type = enabled
        .iter()
        .find(|p| p.mode == payment_mode)
        .map(|p| p.provider)
        .unwrap_or_else(|| {
            // Default to Stripe if configured, otherwise Dummy
            if payment_mode == PaymentMode::Test {
                PaymentProvider::Dummy
            } else {
                PaymentProvider::Stripe
            }
        });

    self.provider_factory.get(domain_id, provider_type, payment_mode).await
}
```

### Step 4: Convert PlanInfo Helper

**File**: `apps/api/src/application/use_cases/domain_billing.rs`

Add a helper to convert `SubscriptionPlanProfile` to `PlanInfo`:

```rust
/// Convert a SubscriptionPlanProfile to PlanInfo for the provider port.
fn plan_to_info(plan: &SubscriptionPlanProfile) -> PlanInfo {
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

Replace the direct `StripeClient` usage with the provider port:

**Before** (lines 1342-1453):
```rust
pub async fn preview_plan_change(
    &self,
    domain_id: Uuid,
    user_id: Uuid,
    new_plan_code: &str,
) -> AppResult<PlanChangePreview> {
    use crate::infra::stripe_client::StripeClient;
    // ... get mode, subscription, plans ...
    let stripe_secret = self.get_stripe_secret_key_for_mode(domain_id, mode).await?;
    let stripe = StripeClient::new(stripe_secret);
    // ... Stripe-specific calls ...
}
```

**After**:
```rust
pub async fn preview_plan_change(
    &self,
    domain_id: Uuid,
    user_id: Uuid,
    new_plan_code: &str,
) -> AppResult<PlanChangePreview> {
    use crate::application::ports::payment_provider::{
        PlanInfo, SubscriptionId, PlanChangePreview as PortPlanChangePreview,
    };

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
    let plan_info = Self::plan_to_info(&new_plan);

    let preview = provider.preview_plan_change(&subscription_id, &plan_info).await?;

    // Convert to domain type
    Ok(PlanChangePreview {
        prorated_amount_cents: preview.prorated_amount_cents,
        currency: preview.currency,
        period_end: preview.period_end.timestamp(),
        new_plan_name: preview.new_plan_name,
        new_plan_price_cents: preview.new_plan_price_cents,
        change_type: match preview.change_type {
            crate::application::ports::payment_provider::PlanChangeType::Upgrade => PlanChangeType::Upgrade,
            crate::application::ports::payment_provider::PlanChangeType::Downgrade => PlanChangeType::Downgrade,
        },
        effective_at: preview.effective_at.timestamp(),
    })
}
```

### Step 6: Refactor change_plan()

**File**: `apps/api/src/application/use_cases/domain_billing.rs`

Replace the direct `StripeClient` usage with the provider port:

```rust
pub async fn change_plan(
    &self,
    domain_id: Uuid,
    user_id: Uuid,
    new_plan_code: &str,
    _idempotency_key: &str, // Now handled by the provider
) -> AppResult<PlanChangeResult> {
    use crate::application::ports::payment_provider::{
        PlanInfo, SubscriptionId,
        PlanChangeType as PortPlanChangeType,
    };

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
    let plan_info = Self::plan_to_info(&new_plan);

    // Execute plan change via provider
    let result = provider
        .change_plan(&subscription_id, None, &plan_info)
        .await?;

    // Log the plan change event
    let change_type = match result.change_type {
        PortPlanChangeType::Upgrade => PlanChangeType::Upgrade,
        PortPlanChangeType::Downgrade => PlanChangeType::Downgrade,
    };

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
        new_plan: new_plan,
        effective_at: result.effective_at.timestamp(),
        schedule_id: result.schedule_id,
    })
}
```

### Step 7: Update setup.rs

**File**: `apps/api/src/infra/setup.rs`

Create the factory and pass it to the use cases:

**Before** (lines 97-116):
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

### Step 8: Add Required Imports

**File**: `apps/api/src/application/use_cases/domain_billing.rs`

Add imports at the top of the file:

```rust
use crate::application::ports::payment_provider::{
    PaymentProviderPort, PlanInfo, SubscriptionId,
    PlanChangePreview as PortPlanChangePreview,
    PlanChangeResult as PortPlanChangeResult,
    PlanChangeType as PortPlanChangeType,
};
use crate::application::use_cases::payment_provider_factory::PaymentProviderFactory;
use crate::domain::entities::payment_mode::PaymentMode;
```

Remove the inline imports within functions:
- Line 1348: Remove `use crate::infra::stripe_client::StripeClient;`
- Line 1464: Remove `use crate::infra::stripe_client::StripeClient;`

---

## Files to Modify

| File | Changes |
|------|---------|
| `apps/api/src/application/use_cases/domain_billing.rs` | Add factory field, update constructor, add helpers, refactor `preview_plan_change()` and `change_plan()` |
| `apps/api/src/infra/setup.rs` | Create and inject `PaymentProviderFactory` |

---

## Testing Plan

### 1. Unit Tests (Future Enhancement)
With the factory injection, we can now mock the provider:
```rust
// Future: mock provider for unit testing
let mock_provider = Arc::new(MockPaymentProvider::new());
let mock_factory = MockPaymentProviderFactory::returning(mock_provider);
```

### 2. Integration Tests
```bash
# Run existing tests to ensure no regressions
./run api:test
```

### 3. Build Verification
```bash
# Verify compilation
./run api:build
```

### 4. Manual Testing
1. Start local dev environment with Stripe test mode
2. Create a subscription via checkout
3. Preview plan upgrade - verify proration info
4. Execute plan change - verify success
5. Verify dummy provider works for plan changes in test mode

---

## Edge Cases to Handle

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
| Breaking existing functionality | Run full test suite before/after |
| Type mismatches between domain and port types | Add explicit conversion helpers |
| Missing idempotency | Provider implementations handle idempotency internally |
| Regression in Stripe-specific behavior | StripePaymentAdapter wraps existing StripeClient logic |

---

## Out of Scope

These items are related but not part of this task:

1. **Removing StripeMode enum**: The codebase uses both `StripeMode` and `PaymentMode`. Full migration to `PaymentMode` is a larger refactor.
2. **Webhook handling**: Webhooks still use StripeClient directly for signature verification.
3. **Checkout flow refactor**: The main checkout flow is in `public_domain_auth.rs` and also uses StripeClient directly.
4. **PaymentProviderFactory tests**: Adding unit tests for the factory itself.

---

## Implementation Checklist

When implementing this plan:

1. [ ] Read `domain_billing.rs` lines 1342-1670 before editing
2. [ ] Add factory field and update constructor (Steps 1-2)
3. [ ] Add helper methods (Steps 3-4)
4. [ ] Refactor `preview_plan_change()` (Step 5)
5. [ ] Refactor `change_plan()` (Step 6)
6. [ ] Update `setup.rs` (Step 7)
7. [ ] Add imports, remove inline imports (Step 8)
8. [ ] Run `./run api:test` to verify no regressions
9. [ ] Run `./run api:build` to verify compilation
10. [ ] Update ticket.md History with timestamp (format: `YYYY-MM-DD HH:MM`)
11. [ ] Mark ticket checklist items complete
12. [ ] Commit changes with descriptive message
13. [ ] Move ticket to `/workspace/tasks/done/` when complete

---

## History

- 2026-01-01 Created plan-v1.md from code review finding #12
