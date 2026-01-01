# Plan: Unify StripeMode and PaymentMode Enums

**Status:** Draft v1
**Created:** 2026-01-01
**Task:** [0014-unify-payment-mode-enums](./ticket.md)

---

## Summary

The codebase has two nearly identical enums representing test/live mode concepts:

1. **`StripeMode`** (`apps/api/src/domain/entities/stripe_mode.rs`) - Stripe-specific test/live enum, used in database columns (`stripe_mode` type)
2. **`PaymentMode`** (`apps/api/src/domain/entities/payment_mode.rs`) - Generic payment provider mode, used in database columns (`payment_mode` type)

Both enums have the same variants (`Test`, `Live`) and identical helper methods (`as_str()`, `from_key_prefix()`, `validate_key_prefix()`). This duplication creates:
- Maintenance overhead (two files, duplicate tests)
- Confusion about which to use
- Unnecessary conversion boilerplate potential

### Approach Decision

**Keep `PaymentMode` as the unified type and deprecate `StripeMode`.**

Rationale:
- `PaymentMode` is more generic and aligns with the multi-provider architecture
- `PaymentMode` already has extra features (aliases like "sandbox"/"production" in `FromStr`)
- The `payment_mode` database column was introduced in migration 00010 as part of the provider-agnostic system
- `stripe_mode` columns are legacy from migration 00008

---

## Phase 1: Add Conversion Trait and Prepare PaymentMode

**Files to modify:**
- `apps/api/src/domain/entities/payment_mode.rs`
- `apps/api/src/domain/entities/stripe_mode.rs`

### Steps:

1. **Add From/Into conversions between StripeMode and PaymentMode**
   - Add `impl From<StripeMode> for PaymentMode` in `payment_mode.rs`
   - Add `impl From<PaymentMode> for StripeMode` in `stripe_mode.rs`

   This allows gradual migration without breaking existing code.

2. **Verify PaymentMode has all StripeMode functionality**
   - `as_str()` - exists
   - `from_stripe_key_prefix()` - exists (named differently)
   - `validate_stripe_key_prefix()` - exists (named differently)
   - `Default` - exists (Test)
   - `Display`, `FromStr` - exists
   - Tests - already comprehensive

---

## Phase 2: Migrate Internal Usage

**Goal:** Replace `StripeMode` with `PaymentMode` in function signatures and internal logic, while keeping database column types unchanged initially.

### Files to modify:

#### Domain Layer
- `apps/api/src/domain/entities/mod.rs` - Keep both exports for now

#### Application Layer
- `apps/api/src/application/use_cases/domain_billing.rs` (lines 14, 42, 44, etc.)
  - Change function parameters from `StripeMode` to `PaymentMode`
  - Update `BillingStripeConfigProfile.stripe_mode` type
  - Update `CreateSubscriptionInput.stripe_mode` type
  - Update all trait method signatures

- `apps/api/src/application/use_cases/domain.rs`
  - Update `DomainRepo::set_billing_stripe_mode` to `set_billing_payment_mode`

- `apps/api/src/application/ports/payment_provider.rs`
  - Verify using `PaymentMode` (already does)

#### Adapter Layer (HTTP)
- `apps/api/src/adapters/http/routes/domain.rs`
  - Update request/response DTOs to use `PaymentMode`

- `apps/api/src/adapters/http/routes/public_domain_auth.rs`
  - Update billing/subscription endpoints

#### Adapter Layer (Persistence)
- `apps/api/src/adapters/persistence/billing_stripe_config.rs`
  - Use `PaymentMode` in function signatures
  - Convert to/from `StripeMode` at DB boundary if needed

- `apps/api/src/adapters/persistence/subscription_plan.rs`
  - Same pattern

- `apps/api/src/adapters/persistence/user_subscription.rs`
  - Same pattern

- `apps/api/src/adapters/persistence/billing_payment.rs`
  - Same pattern

- `apps/api/src/adapters/persistence/domain.rs`
  - Update `billing_stripe_mode` to `billing_payment_mode`

- `apps/api/src/adapters/persistence/enabled_payment_providers.rs`
  - Already uses `PaymentMode`

#### Infrastructure Layer
- `apps/api/src/infra/stripe_payment_adapter.rs`
  - Convert `PaymentMode` to legacy `StripeMode` at Stripe API boundary

- `apps/api/src/infra/dummy_payment_client.rs`
  - Verify uses `PaymentMode` (should already)

---

## Phase 3: Database Column Unification (Deferred)

**Important:** This phase requires a database migration and should be carefully planned.

### Current State:
- **`stripe_mode`** column (type `stripe_mode` enum):
  - `domain_billing_stripe_config.stripe_mode`
  - `domains.billing_stripe_mode`
  - `subscription_plans.stripe_mode`
  - `user_subscriptions.stripe_mode`

- **`payment_mode`** column (type `payment_mode` enum):
  - `domain_billing_stripe_config.payment_mode` (nullable, added in 00010)
  - `domains.active_payment_mode` (nullable, added in 00010)
  - `subscription_plans.payment_mode` (nullable, added in 00010)
  - `user_subscriptions.payment_mode` (nullable, added in 00010)
  - `domain_enabled_payment_providers.mode`
  - `billing_payments.payment_mode`

### Recommendation:
The migration 00010 already added `payment_mode` columns alongside `stripe_mode` and backfilled data. The original migration mentioned "Cleanup happens in migration 00011" which suggests a follow-up migration was planned.

**For this task, we will NOT create the cleanup migration.** The scope is to unify the Rust enum usage, not the database schema. A separate task should handle:
1. Making `payment_mode` columns NOT NULL
2. Dropping `stripe_mode` columns
3. Renaming `billing_stripe_mode` to `active_payment_mode`
4. Dropping the `stripe_mode` Postgres enum type

---

## Phase 4: Update Frontend Types

**Files to modify:**
- `apps/ui/types/billing.ts`
  - Remove `StripeMode` type alias (line 35)
  - Update `StripeConfigStatus.active_mode` to use `PaymentMode`
  - Update `SubscriptionPlan.stripe_mode` to `payment_mode`
  - Update `UpdateStripeConfigInput.mode` to use `PaymentMode`
  - Update `DeleteStripeConfigInput.mode` to use `PaymentMode`
  - Update `SetBillingModeInput.mode` to use `PaymentMode`
  - Update helper functions `getModeLabel`, `getModeBadgeColor`

- `apps/ui/app/(app)/domains/[id]/page.tsx`
  - Update any StripeMode references

---

## Phase 5: Deprecate StripeMode

**Files to modify:**
- `apps/api/src/domain/entities/stripe_mode.rs`
  - Add `#[deprecated]` attribute to `StripeMode` enum
  - Keep file for backward compatibility

- `apps/api/src/domain/entities/mod.rs`
  - Add comment about deprecation

---

## Testing Approach

1. **Unit Tests:**
   - Run existing `payment_mode.rs` tests (already comprehensive)
   - Add tests for `From` conversions between StripeMode and PaymentMode

2. **Integration Tests:**
   - `./run api:test` - Run full API test suite
   - Manual testing of billing flows

3. **Build Verification:**
   - `./run api:build` - Ensure offline SQLx build passes
   - Check for any type errors from enum changes

4. **Frontend Tests:**
   - `./run ui:build` - TypeScript compilation check

---

## Edge Cases

1. **Serialization Compatibility:**
   - Both enums serialize to "test"/"live" strings
   - API responses should be unaffected
   - JWT subscription claims use string status, not affected

2. **Database Reads:**
   - SQLx `#[sqlx(type_name = "payment_mode")]` handles the Postgres enum
   - Need to ensure persistence layer converts correctly for `stripe_mode` columns

3. **Webhook Handlers:**
   - Stripe webhooks may pass mode indicators
   - Need to handle both key prefix detection and explicit mode params

4. **Enabled Payment Providers:**
   - Already uses `PaymentMode` throughout
   - No changes needed for this table

---

## Implementation Order

1. Add From/Into conversions (Phase 1)
2. Update application layer use cases (Phase 2 - partial)
3. Update persistence layer to use PaymentMode with conversions (Phase 2)
4. Update HTTP routes (Phase 2)
5. Update frontend types (Phase 4)
6. Add deprecation notice to StripeMode (Phase 5)
7. Run tests and verify

---

## Rollback Plan

If issues arise:
- The From/Into conversions are additive and safe to revert
- No database migrations means no data changes
- Frontend changes are isolated to type aliases

---

## Out of Scope

- Database migration to remove `stripe_mode` columns (separate task)
- Renaming database columns/types (separate task)
- Changing API response shapes (should remain "test"/"live" strings)

---

## Files Summary

### Must Modify (Rust Backend)
| File | Changes |
|------|---------|
| `domain/entities/payment_mode.rs` | Add `From<StripeMode>` impl |
| `domain/entities/stripe_mode.rs` | Add `From<PaymentMode>` impl, deprecation |
| `application/use_cases/domain_billing.rs` | Replace StripeMode with PaymentMode in signatures |
| `adapters/persistence/billing_stripe_config.rs` | Convert at DB boundary |
| `adapters/persistence/subscription_plan.rs` | Convert at DB boundary |
| `adapters/persistence/user_subscription.rs` | Convert at DB boundary |
| `adapters/persistence/billing_payment.rs` | Convert at DB boundary |
| `adapters/persistence/domain.rs` | Update field name handling |
| `adapters/http/routes/domain.rs` | Update DTOs |
| `adapters/http/routes/public_domain_auth.rs` | Update billing endpoints |

### Must Modify (Frontend)
| File | Changes |
|------|---------|
| `apps/ui/types/billing.ts` | Remove StripeMode, update types |
| `apps/ui/app/(app)/domains/[id]/page.tsx` | Update mode references |

### Verify/Check
| File | Status |
|------|--------|
| `adapters/persistence/enabled_payment_providers.rs` | Already uses PaymentMode |
| `application/ports/payment_provider.rs` | Already uses PaymentMode |
| `infra/stripe_payment_adapter.rs` | May need conversion at Stripe boundary |

---

## History

- 2026-01-01 07:XX: Created plan v1 based on codebase exploration
