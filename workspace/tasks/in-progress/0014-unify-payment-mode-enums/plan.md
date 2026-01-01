# Plan: Unify StripeMode and PaymentMode Enums

**Status:** Draft v3
**Created:** 2026-01-01
**Revised:** 2026-01-01
**Task:** [0014-unify-payment-mode-enums](./ticket.md)

---

## Summary

The codebase has two nearly identical enums representing test/live mode:

1. **`StripeMode`** (`apps/api/src/domain/entities/stripe_mode.rs`) - Stripe-specific, tied to `stripe_mode` DB columns
2. **`PaymentMode`** (`apps/api/src/domain/entities/payment_mode.rs`) - Generic, tied to `payment_mode` DB columns

Both have identical variants (`Test`, `Live`) and near-identical methods. This creates maintenance overhead and confusion.

### Approach Decision

**Keep `PaymentMode` as the unified type and deprecate `StripeMode`.**

Rationale:
- `PaymentMode` is more generic and aligns with multi-provider architecture
- `PaymentMode` already has extra features (aliases like "sandbox"/"production" in `FromStr`)
- Migration 00010 added `payment_mode` columns alongside `stripe_mode` with backfill
- New code should use `PaymentMode` exclusively

---

## Phase 0: Verification and Inventory

**Goal:** Establish complete file inventory and verify assumptions before making changes.

### 0.1 Backend Inventory

**127 occurrences** of `StripeMode` across Rust files:

| File | Count | Type |
|------|-------|------|
| `application/use_cases/domain_billing.rs` | ~50 | Use case logic |
| `adapters/http/routes/public_domain_auth.rs` | 8 | HTTP handlers |
| `adapters/persistence/user_subscription.rs` | 8 | DB queries |
| `adapters/persistence/subscription_plan.rs` | 7 | DB queries |
| `adapters/http/routes/domain.rs` | 5 | HTTP handlers |
| `adapters/persistence/billing_payment.rs` | 5 | DB queries |
| `application/use_cases/payment_provider_factory.rs` | 5 | Factory logic |
| `adapters/persistence/billing_stripe_config.rs` | 4 | DB queries |
| `application/use_cases/domain.rs` | 3 | Use case logic |
| `adapters/persistence/domain.rs` | 2 | DB queries |
| `application/use_cases/domain_auth.rs` | 2 | Test code |

### 0.2 Test File Inventory (NEW)

Run before implementation:
```bash
grep -rn "StripeMode" apps/api/tests/       # Integration tests
grep -rn "StripeMode" --include="*_test.rs" apps/api/src/  # Inline tests
```

Expected locations:
- `domain_auth.rs` lines 1582, 1893 (mocks)
- `domain_billing.rs` test modules
- Possible integration tests in `apps/api/tests/`

### 0.3 Frontend Inventory

| Location | Status |
|----------|--------|
| `apps/ui/types/billing.ts` | 1 reference (needs alias update) |
| `libs/reauth-sdk-ts/` | **Clean** - already uses PaymentMode |
| `apps/demo_api/`, `apps/demo_ui/` | **Clean** - no StripeMode |

### 0.4 Database Column Status

From migration 00010:
- Both `stripe_mode` and `payment_mode` columns exist
- `payment_mode` columns were backfilled from `stripe_mode`
- Columns are in sync; both are nullable in most tables

### 0.5 SQLx Cache Check (NEW)

Before starting, check for cached queries:
```bash
grep -l "stripe_mode" .sqlx/*.json 2>/dev/null | head -5
ls -la .sqlx/ | head -10
```

This determines if SQLx cache regeneration is needed.

### 0.6 Enum Attribute Verification (NEW)

Verify both enums have identical serialization attributes:
```rust
// StripeMode (expected)
#[sqlx(type_name = "stripe_mode", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]

// PaymentMode (expected)
#[sqlx(type_name = "payment_mode", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
```

Both must serialize to "test"/"live" (lowercase) for compatibility.

### Verification Checkpoint:
```bash
./run api:build  # Verify current state compiles
./run api:test   # Verify tests pass before changes
git stash        # Ensure clean working directory
```

---

## Phase 1: Add From/Into Conversions

**Files to modify:**
- `apps/api/src/domain/entities/payment_mode.rs`
- `apps/api/src/domain/entities/stripe_mode.rs`

### 1.1 Add Conversion in payment_mode.rs

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

### 1.2 Add Conversion in stripe_mode.rs

```rust
impl From<crate::domain::entities::payment_mode::PaymentMode> for StripeMode {
    fn from(mode: crate::domain::entities::payment_mode::PaymentMode) -> Self {
        match mode {
            crate::domain::entities::payment_mode::PaymentMode::Test => StripeMode::Test,
            crate::domain::entities::payment_mode::PaymentMode::Live => StripeMode::Live,
        }
    }
}
```

### 1.3 Add Conversion Tests

```rust
#[cfg(test)]
mod conversion_tests {
    use super::*;
    use crate::domain::entities::stripe_mode::StripeMode;

    #[test]
    fn test_payment_mode_from_stripe_mode() {
        assert_eq!(PaymentMode::from(StripeMode::Test), PaymentMode::Test);
        assert_eq!(PaymentMode::from(StripeMode::Live), PaymentMode::Live);
    }

    #[test]
    fn test_stripe_mode_from_payment_mode() {
        assert_eq!(StripeMode::from(PaymentMode::Test), StripeMode::Test);
        assert_eq!(StripeMode::from(PaymentMode::Live), StripeMode::Live);
    }
}
```

### Verification Checkpoint + Commit:
```bash
cargo clippy --all-features -p reauth-api
./run api:test
git add -A && git commit -m "feat(billing): add From conversions between StripeMode and PaymentMode"
```

---

## Phase 2: Migrate Codebase to PaymentMode

**Goal:** Replace `StripeMode` with `PaymentMode` in function signatures.

### Implementation Order (REVISED - persistence first)

The implementation should proceed in this order to avoid trait/implementation mismatches:

1. **Domain layer traits** (if any trait signatures use StripeMode)
2. **Persistence layer** (DB queries - convert one file at a time)
3. **Application layer** (use cases)
4. **Adapter layer** (HTTP routes)

### 2.1 API Contract Strategy

**Preserve JSON field names for backward compatibility:**

For HTTP DTOs that currently use `stripe_mode`, keep the JSON key:
```rust
#[serde(rename = "stripe_mode")]  // Preserve API compatibility
pub mode: PaymentMode,
```

### 2.2 Dual-Write SQL Strategy (NEW - with examples)

**Read from:** `payment_mode` column (source of truth, backfilled in migration 00010)
**Write to:** Both `payment_mode` AND `stripe_mode` columns

Since both columns have identical underlying representation ("test"/"live" strings), writes can use:

```rust
// Simple case: both columns accept same literal value
sqlx::query!(
    r#"UPDATE domains
       SET payment_mode = $1, billing_stripe_mode = $1
       WHERE id = $2"#,
    mode as PaymentMode,
    domain_id
)

// If type casting needed (unlikely but possible):
sqlx::query!(
    r#"UPDATE domains
       SET payment_mode = $1::payment_mode,
           billing_stripe_mode = $1::text::stripe_mode
       WHERE id = $2"#,
    mode.to_string(),
    domain_id
)
```

For queries that currently read `stripe_mode`, change to read `payment_mode`:
```rust
// Before
sqlx::query_as!(... "SELECT stripe_mode FROM ...")

// After
sqlx::query_as!(... "SELECT payment_mode FROM ...")
```

### 2.3 Persistence Layer Changes

Update each file, compile after each change:

**`apps/api/src/adapters/persistence/domain.rs`** (2 occurrences)
```bash
# After updating:
cargo check -p reauth-api
```

**`apps/api/src/adapters/persistence/billing_stripe_config.rs`** (4 occurrences)
```bash
cargo check -p reauth-api
```

**`apps/api/src/adapters/persistence/subscription_plan.rs`** (7 occurrences)
```bash
cargo check -p reauth-api
```

**`apps/api/src/adapters/persistence/user_subscription.rs`** (8 occurrences)
```bash
cargo check -p reauth-api
```

**`apps/api/src/adapters/persistence/billing_payment.rs`** (5 occurrences)
```bash
cargo check -p reauth-api
```

### 2.4 SQLx Cache Regeneration (NEW)

After all persistence layer changes:
```bash
./run infra           # Ensure DB is running
./run db:prepare      # Regenerate .sqlx/ query cache
./run api:build       # Verify offline build with new cache
```

### Verification Checkpoint + Commit:
```bash
./run api:test
git add -A && git commit -m "refactor(persistence): replace StripeMode with PaymentMode in DB layer"
```

### 2.5 Application Layer Changes

**`apps/api/src/application/use_cases/domain_billing.rs`** (~50 occurrences)
- Update import: `use crate::domain::entities::payment_mode::PaymentMode;`
- Replace all `StripeMode` types with `PaymentMode`
- Remove manual conversion code (now have From impl)

**`apps/api/src/application/use_cases/domain.rs`** (3 occurrences)
- Update `set_billing_stripe_mode` parameter type
- Update `DomainDetails.billing_stripe_mode` field type

**`apps/api/src/application/use_cases/payment_provider_factory.rs`** (5 occurrences)
- Replace manual `match` conversions with `.into()` calls

**`apps/api/src/application/use_cases/domain_auth.rs`** (2 occurrences)
- Update test imports and mock parameters

### Verification Checkpoint + Commit:
```bash
cargo clippy --all-features -p reauth-api
./run api:test
git add -A && git commit -m "refactor(application): replace StripeMode with PaymentMode in use cases"
```

### 2.6 Adapter Layer (HTTP Routes)

**`apps/api/src/adapters/http/routes/domain.rs`** (5 occurrences)
- Update DTO field types to `PaymentMode`
- Add `#[serde(rename = "stripe_mode")]` for API compatibility

**`apps/api/src/adapters/http/routes/public_domain_auth.rs`** (8 occurrences)
- Update webhook handler parameter types
- Use `.into()` for any remaining conversions

### Verification Checkpoint + Commit:
```bash
./run api:build
./run api:test
git add -A && git commit -m "refactor(http): replace StripeMode with PaymentMode in routes"
```

---

## Phase 3: Update Frontend Types

**Goal:** Align TypeScript types with backend changes.

### 3.1 Update billing.ts

**`apps/ui/types/billing.ts`**

Make `StripeMode` a type alias:
```typescript
// PaymentMode is the canonical type
export type PaymentMode = 'test' | 'live';

// Legacy type alias - use PaymentMode for new code
export type StripeMode = PaymentMode;
```

Keep all existing usages of `StripeMode` since:
- JSON field names are preserved via serde rename
- `StripeMode` is now just an alias to `PaymentMode`
- No functional change for existing code

### Verification Checkpoint:
```bash
./run ui:build  # TypeScript compilation check
```

---

## Phase 4: Deprecate StripeMode

**Goal:** Mark `StripeMode` as deprecated to prevent new usage.

### 4.1 Add Deprecation to stripe_mode.rs

```rust
/// Stripe-specific mode enum - DEPRECATED
///
/// Use [`PaymentMode`] instead for all new code.
/// This type exists only for backward compatibility with existing
/// database columns (`stripe_mode`). It will be removed when
/// migration 00011 drops the `stripe_mode` columns.
#[deprecated(since = "0.1.0", note = "Use PaymentMode instead")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "stripe_mode", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum StripeMode {
    Test,
    Live,
}
```

### 4.2 Suppress Deprecation Warnings (NEW)

In files that still need StripeMode for DB compatibility (persistence layer), add suppression:

```rust
// At the top of persistence files that must use StripeMode
#[allow(deprecated)]
use crate::domain::entities::stripe_mode::StripeMode;
```

This prevents CI failures while keeping deprecation visible elsewhere. Document that these annotations will be removed in task 0015.

### 4.3 Update mod.rs

```rust
// stripe_mode is DEPRECATED - use payment_mode for new code
pub mod stripe_mode;
pub mod payment_mode;
```

### 4.4 Cleanup and Lint

```bash
cargo fix --allow-dirty --edition -p reauth-api
cargo clippy --all-features -p reauth-api 2>&1 | grep -v "deprecated"
./run api:fmt
```

### Verification Checkpoint + Commit:
```bash
./run api:build
./run api:test
git add -A && git commit -m "chore: deprecate StripeMode enum, prefer PaymentMode"
```

---

## Phase 5: Create Follow-up Task

Create placeholder task for database cleanup:

**File:** `workspace/tasks/todo/0015-remove-stripe-mode-columns.md`

```markdown
# Remove stripe_mode Database Columns

Cleanup task following 0014-unify-payment-mode-enums.

## Prerequisites
- Task 0014 must be complete
- All StripeMode usages converted to PaymentMode

## Checklist
- [ ] Create migration 00011
- [ ] Make payment_mode columns NOT NULL
- [ ] Drop stripe_mode columns from all tables
- [ ] Rename billing_stripe_mode to active_payment_mode on domains table
- [ ] Drop stripe_mode Postgres enum type
- [ ] Remove StripeMode Rust type entirely
- [ ] Update persistence layer to stop dual-writes
- [ ] Remove #[allow(deprecated)] annotations

## History
- 2026-01-01 Created as follow-up to task 0014.
```

### Verification + Final Commit:
```bash
git add -A && git commit -m "chore: create follow-up task 0015 for stripe_mode column removal"
```

---

## Testing Strategy

### Unit Tests:
1. Run existing `payment_mode.rs` tests
2. Verify new From conversion tests pass
3. Confirm deprecation warnings appear (but don't fail build)

### Integration Tests:
```bash
./run api:test
```

### Build Verification:
```bash
./run api:build     # SQLx offline build (most important)
./run ui:build      # TypeScript check
```

### Manual Testing:
- Create/update Stripe config via UI
- Switch billing mode test -> live
- Verify subscription plans display correctly

---

## Edge Cases

### 1. Serialization Compatibility
- Both enums serialize to "test"/"live" strings (lowercase)
- JSON responses unchanged (using `#[serde(rename)]`)

### 2. Database Column Sync
- Both columns contain same data (from migration 00010 backfill)
- Dual-write ensures they stay in sync until cleanup migration

### 3. Webhook Handlers
- Stripe webhooks use hardcoded `StripeMode::Test` / `StripeMode::Live`
- Will be converted via From impl at call sites

### 4. SQLx Type Inference
- SQLx may see StripeMode and PaymentMode as distinct types
- Solution: Explicitly annotate query return types as PaymentMode
- Regenerate SQLx cache after changes

---

## Commit Strategy (NEW)

**Approach:** One commit per phase for easy rollback.

| Phase | Commit Message |
|-------|----------------|
| 1 | `feat(billing): add From conversions between StripeMode and PaymentMode` |
| 2.3 | `refactor(persistence): replace StripeMode with PaymentMode in DB layer` |
| 2.5 | `refactor(application): replace StripeMode with PaymentMode in use cases` |
| 2.6 | `refactor(http): replace StripeMode with PaymentMode in routes` |
| 3 | (part of 2.6 or separate if needed) |
| 4 | `chore: deprecate StripeMode enum, prefer PaymentMode` |
| 5 | `chore: create follow-up task 0015 for stripe_mode column removal` |

**Final squash:** After all phases pass, squash commits into single PR commit if desired.

---

## Rollback Plan

If issues arise:
- Each phase has its own commit, can revert to last-known-good
- From/Into conversions are additive, safe to revert
- No database migrations means no data changes
- Frontend type alias approach is non-breaking
- Deprecation attribute can be removed without functional impact

---

## Out of Scope

- Database migration to remove `stripe_mode` columns (task 0015)
- Renaming database columns/types (task 0015)
- Changing API response JSON field names (would be breaking change)
- SDK changes (already uses PaymentMode)

---

## Files Summary

### Backend - Must Modify

| File | Occurrences | Changes |
|------|-------------|---------|
| `domain/entities/payment_mode.rs` | +1 impl | Add `From<StripeMode>` |
| `domain/entities/stripe_mode.rs` | +deprecation | Add `From<PaymentMode>`, `#[deprecated]` |
| `adapters/persistence/domain.rs` | 2 | Update signatures, dual-write SQL |
| `adapters/persistence/billing_stripe_config.rs` | 4 | Update signatures, dual-write SQL |
| `adapters/persistence/subscription_plan.rs` | 7 | Update signatures, dual-write SQL |
| `adapters/persistence/user_subscription.rs` | 8 | Update signatures, dual-write SQL |
| `adapters/persistence/billing_payment.rs` | 5 | Update signatures, dual-write SQL |
| `application/use_cases/domain_billing.rs` | ~50 | Replace StripeMode with PaymentMode |
| `application/use_cases/domain.rs` | 3 | Replace StripeMode with PaymentMode |
| `application/use_cases/payment_provider_factory.rs` | 5 | Use From impl instead of match |
| `application/use_cases/domain_auth.rs` | 2 | Update test imports |
| `adapters/http/routes/domain.rs` | 5 | Update DTOs, add serde rename |
| `adapters/http/routes/public_domain_auth.rs` | 8 | Update webhook handlers |

### Frontend - Minimal Changes

| File | Changes |
|------|---------|
| `apps/ui/types/billing.ts` | Make StripeMode alias to PaymentMode |

### No Changes Needed

| Location | Status |
|----------|--------|
| `libs/reauth-sdk-ts/` | Already uses PaymentMode only |
| `apps/demo_api/`, `apps/demo_ui/` | No StripeMode references |
| `adapters/persistence/enabled_payment_providers.rs` | Already uses PaymentMode |

---

## Risk Assessment

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| API breaking change | Low | Using serde rename to preserve JSON keys |
| Column sync issues | Very Low | Dual-write strategy, data already synced |
| Deprecation warnings in CI | Medium | Use `#[allow(deprecated)]` on necessary files |
| TypeScript type errors | Low | Minimal changes, StripeMode = PaymentMode |
| SQLx type inference mismatch | Medium | Compile after each file, regenerate cache |
| Merge conflicts | Low | Complete task quickly, pull main frequently |

---

## Pre-Implementation Checklist (NEW)

Before starting Phase 1:
- [ ] Verify both enums have identical `serde` and `sqlx` rename attributes
- [ ] Check if `.sqlx/` directory has cached queries referencing StripeMode
- [ ] Confirm CI config for deprecation warning handling
- [ ] Pull latest main and verify no new StripeMode usages added
- [ ] Ensure local infra is running for SQLx cache regeneration

---

## Revision History

- 2026-01-01 v1: Initial plan created
- 2026-01-01 v2: Addressed feedback:
  - Added Phase 0 (verification/inventory)
  - Clarified API contract strategy (preserve JSON field names)
  - Confirmed SDK/demo apps need no changes
  - Added column read/write strategy for persistence layer
  - Added Phase 5 (create follow-up task 0015)
  - Added verification checkpoints between phases
  - Added risk assessment table
- 2026-01-01 v3: Addressed feedback-2:
  - Added SQLx cache regeneration step (`./run db:prepare`) after Phase 2.3
  - Added dual-write SQL code examples showing exact syntax
  - Added test file inventory step in Phase 0
  - Added `#[allow(deprecated)]` suppression strategy for persistence layer
  - Revised implementation order: persistence → application → adapters
  - Added commit checkpoint recommendations (one commit per phase)
  - Added pre-implementation checklist
  - Improved enum attribute verification step
