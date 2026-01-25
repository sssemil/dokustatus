# Plan: Unify StripeMode and PaymentMode Enums

**Ticket**: `workspace/tasks/todo/0014-unify-payment-mode-enums/ticket.md`
**Goal**: Remove redundant `StripeMode` enum and consolidate on `PaymentMode`

---

## Background

Both enums have identical variants (`Test`, `Live`). Migration 00010 already added `payment_mode` columns and backfilled data from `stripe_mode`. The cleanup (migration 00011) was never created.

**Current state**:
- Database has BOTH `stripe_mode` and `payment_mode` columns
- Rust code uses `StripeMode` in ~13 files, `PaymentMode` in ~14 files
- Explicit conversion boilerplate in `payment_provider_factory.rs:83-88`

---

## Implementation Strategy

**Approach**: Single-phase cleanup (WIP project, no real users - can be aggressive)

Since there are no real users, we can simplify:
1. Create migration 00011 that drops `stripe_mode` columns immediately
2. Update all Rust code to use only `payment_mode`
3. No need for dual-write, backwards compat, or phased rollout

---

## Simplified Approach (WIP - No Real Users)

Since this is a WIP project without real users:
- **No dual-write needed** - just run migration and update code together
- **No backwards compat needed** - can break API response shape
- **No phased rollout needed** - deploy everything at once
- **Can drop data** - if backfill is complex, just truncate and reseed

---

## Code Changes

Run migration 00011 FIRST (drops `stripe_mode` columns), then update code.
No dual-write needed since we're dropping columns in migration.

**Enum Variant Parity**: Both enums have IDENTICAL variants (`Test`, `Live`).
The cast is safe because variant names match. Variants are **FROZEN** until Phase B drops `stripe_mode`.

This ensures:
- `stripe_mode` constraints remain satisfied
- `payment_mode` is the source of truth for reads
- No data inconsistency during transition

### Rust Code Updates (after migration)

**1. Delete StripeMode Entity**
- DELETE `apps/api/src/domain/entities/stripe_mode.rs`
- Remove `pub mod stripe_mode;` from `apps/api/src/domain/entities/mod.rs`
- Remove `From<StripeMode>` impl from `payment_mode.rs`

**2. Update Profile Types** (`domain_billing.rs`)
- Remove `stripe_mode: StripeMode` fields from all profiles
- Change `payment_mode: Option<PaymentMode>` to `payment_mode: PaymentMode` (now NOT NULL in DB)
- Update all repo trait method signatures: `StripeMode` → `PaymentMode`

**3. Update Persistence Layer** (5 files)
- Remove all `stripe_mode` from queries (column no longer exists)
- Change all imports from `stripe_mode::StripeMode` to `payment_mode::PaymentMode`
- Update method signatures

**4. Update Domain Use Case** (`domain.rs`)
- Change `billing_stripe_mode` → `billing_payment_mode`

**5. Remove Conversion Boilerplate** (`payment_provider_factory.rs`)
- Delete the temporary StripeMode → PaymentMode conversion code

**6. Update HTTP Routes** (4 files)
- Replace all `StripeMode` with `PaymentMode` in request/response types
- No backwards compat needed (WIP project)

**7. TypeScript Cleanup** (`apps/ui/types/billing.ts`)
- Remove `StripeMode` type
- Update all interfaces to use `PaymentMode`

---

## Migration 00011: Drop stripe_mode (run first)

**File**: `apps/api/migrations/00011_cleanup_stripe_mode.sql`

```sql
-- ============================================================================
-- Cleanup stripe_mode - WIP project, no real users, just drop everything
-- ============================================================================

-- Backfill any NULLs before making NOT NULL
UPDATE subscription_plans SET payment_mode = COALESCE(payment_mode, stripe_mode, 'test');
UPDATE user_subscriptions SET payment_mode = COALESCE(payment_mode, stripe_mode, 'test');
UPDATE billing_payments SET payment_mode = COALESCE(payment_mode, stripe_mode, 'test');
UPDATE domain_billing_stripe_config SET payment_mode = COALESCE(payment_mode, stripe_mode, 'test');
UPDATE domains SET billing_payment_mode = COALESCE(billing_payment_mode, billing_stripe_mode, 'test');

-- Drop indexes referencing stripe_mode
DROP INDEX IF EXISTS idx_subscription_plans_mode;
DROP INDEX IF EXISTS idx_user_subscriptions_mode;
DROP INDEX IF EXISTS idx_billing_payments_domain_mode;
DROP INDEX IF EXISTS idx_billing_payments_domain_mode_date;

-- Drop stripe_mode columns (constraints auto-drop with columns)
ALTER TABLE domains DROP COLUMN IF EXISTS billing_stripe_mode;
ALTER TABLE domain_billing_stripe_config DROP COLUMN IF EXISTS stripe_mode;
ALTER TABLE subscription_plans DROP COLUMN IF EXISTS stripe_mode;
ALTER TABLE user_subscriptions DROP COLUMN IF EXISTS stripe_mode;
ALTER TABLE billing_payments DROP COLUMN IF EXISTS stripe_mode;

-- Make payment_mode NOT NULL now that stripe_mode is gone
ALTER TABLE subscription_plans ALTER COLUMN payment_mode SET NOT NULL;
ALTER TABLE user_subscriptions ALTER COLUMN payment_mode SET NOT NULL;
ALTER TABLE billing_payments ALTER COLUMN payment_mode SET NOT NULL;
ALTER TABLE domain_billing_stripe_config ALTER COLUMN payment_mode SET NOT NULL;
ALTER TABLE domains ALTER COLUMN billing_payment_mode SET NOT NULL;

-- Add unique constraints on payment_mode (replacing stripe_mode constraints)
ALTER TABLE domain_billing_stripe_config
  ADD CONSTRAINT domain_billing_stripe_config_domain_payment_mode_key UNIQUE (domain_id, payment_mode);
ALTER TABLE subscription_plans
  ADD CONSTRAINT subscription_plans_domain_payment_mode_code_key UNIQUE (domain_id, payment_mode, code);
ALTER TABLE user_subscriptions
  ADD CONSTRAINT user_subscriptions_domain_payment_mode_end_user_id_key UNIQUE (domain_id, payment_mode, end_user_id);
ALTER TABLE billing_payments
  ADD CONSTRAINT billing_payments_domain_payment_mode_stripe_invoice_id_key UNIQUE (domain_id, payment_mode, stripe_invoice_id);

-- Create new indexes on payment_mode
CREATE INDEX idx_subscription_plans_payment_mode ON subscription_plans(domain_id, payment_mode);
CREATE INDEX idx_user_subscriptions_payment_mode ON user_subscriptions(domain_id, payment_mode);
CREATE INDEX idx_billing_payments_payment_mode ON billing_payments(domain_id, payment_mode);
CREATE INDEX idx_billing_payments_payment_mode_date ON billing_payments(domain_id, payment_mode, payment_date DESC NULLS LAST);

-- Drop the enum type
DROP TYPE IF EXISTS stripe_mode;
```

---

## Verification Steps

1. `./run db:migrate` - Run migration 00011
2. `./run db:prepare` - Regenerate SQLx offline data
3. `./run api:build` - Verify API builds
4. `./run api:test` - Run unit tests
5. `./run dev:seed` - Reseed local data if needed

---

## Files Modified (Summary)

| File | Action |
|------|--------|
| `apps/api/migrations/00011_cleanup_stripe_mode.sql` | CREATE |
| `apps/api/src/domain/entities/stripe_mode.rs` | DELETE |
| `apps/api/src/domain/entities/payment_mode.rs` | Remove From<StripeMode> |
| `apps/api/src/domain/entities/mod.rs` | Remove stripe_mode export |
| `apps/api/src/application/use_cases/domain_billing.rs` | Update profile types & traits |
| `apps/api/src/application/use_cases/domain.rs` | Update DomainProfile |
| `apps/api/src/application/use_cases/payment_provider_factory.rs` | Remove conversion |
| `apps/api/src/adapters/persistence/billing_stripe_config.rs` | Remove stripe_mode from queries |
| `apps/api/src/adapters/persistence/subscription_plan.rs` | Remove stripe_mode from queries |
| `apps/api/src/adapters/persistence/user_subscription.rs` | Remove stripe_mode from queries |
| `apps/api/src/adapters/persistence/billing_payment.rs` | Remove stripe_mode from queries |
| `apps/api/src/adapters/persistence/domain.rs` | Remove stripe_mode from queries |
| `apps/api/src/adapters/http/routes/domain.rs` | Replace StripeMode with PaymentMode |
| `apps/api/src/adapters/http/routes/public_domain_auth/billing_webhooks.rs` | Update handler |
| `apps/api/src/adapters/http/routes/public_domain_auth/billing_dummy.rs` | Update input |
| `apps/api/src/adapters/http/routes/public_domain_auth/common.rs` | Update export |
| `apps/ui/types/billing.ts` | Remove StripeMode type |

---

## Execution Order

1. Create migration 00011
2. Run `./run db:migrate`
3. Run `./run db:prepare` (update SQLx offline data)
4. Update all Rust code
5. Update TypeScript types
6. Run `./run api:build` && `./run api:test`
7. Reseed local data if needed
