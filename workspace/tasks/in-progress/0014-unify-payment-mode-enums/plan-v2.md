# Plan: Unify StripeMode and PaymentMode Enums

**Status:** Draft v2
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

## Phase 0: Verification and Inventory (NEW)

**Goal:** Establish complete file inventory and verify assumptions before making changes.

### Steps:

1. **Backend inventory complete** (from grep analysis):
   - **127 occurrences** of `StripeMode` across Rust files
   - Files affected: 10 files in apps/api/src/

2. **Frontend inventory complete**:
   - `apps/ui/types/billing.ts`: 1 reference (`stripe_mode: StripeMode` in `SubscriptionPlan`)
   - SDK (`libs/reauth-sdk-ts/`): **No StripeMode references** - already clean
   - Demo apps: **No StripeMode references** - already clean

3. **Database column status** (from migration 00010):
   - Both `stripe_mode` and `payment_mode` columns exist
   - `payment_mode` columns were backfilled from `stripe_mode`
   - Columns are in sync; both are nullable in most tables

### Verification Checkpoint:
```bash
./run api:build  # Verify current state compiles
./run api:test   # Verify tests pass before changes
```

---

## Phase 1: Add From/Into Conversions

**Files to modify:**
- `apps/api/src/domain/entities/payment_mode.rs`
- `apps/api/src/domain/entities/stripe_mode.rs`

### Steps:

1. **Add bidirectional conversions:**

In `payment_mode.rs`:
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

In `stripe_mode.rs`:
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

2. **Add tests for conversions:**
```rust
#[test]
fn test_from_stripe_mode() {
    assert_eq!(PaymentMode::from(StripeMode::Test), PaymentMode::Test);
    assert_eq!(PaymentMode::from(StripeMode::Live), PaymentMode::Live);
}
```

### Verification Checkpoint:
```bash
cargo clippy --all-features -p reauth-api
./run api:test
```

---

## Phase 2: Migrate Application Layer

**Goal:** Replace `StripeMode` with `PaymentMode` in function signatures while keeping DB columns unchanged.

### API Contract Decision (CLARIFIED)

**Strategy: Preserve JSON field names for backward compatibility.**

For HTTP DTOs that currently use `stripe_mode`, keep the JSON key using serde:
```rust
#[serde(rename = "stripe_mode")]  // Preserve API compatibility
pub mode: PaymentMode,
```

This means API consumers see no breaking change.

### Files to modify (with line counts):

#### 2.1 Application Layer - Use Cases

**`apps/api/src/application/use_cases/domain_billing.rs`** (Primary - ~50 occurrences)

Change all `StripeMode` parameter types to `PaymentMode`:
- Line 14: Update import
- Line 30, 42, 70, 119, 206, 245: Struct field types
- Line 353, 363, 370, 382, 388, 394, 399, 410, 415, 426, 436, 442, 455, 466, 471, 475: Trait method signatures
- Lines 506-544, 608, 652, 690, 721-722, 762, 780, 802: Implementation functions
- Lines 857-858: Remove manual conversion (now have From impl)
- Lines 1100, 1180, 1244, 1973: Additional usages
- Lines 2055, 2246: Test and struct definitions

**`apps/api/src/application/use_cases/domain.rs`** (3 occurrences)
- Line 11: Update import
- Line 32: `set_billing_stripe_mode` parameter type
- Line 208: `DomainDetails.billing_stripe_mode` field type

**`apps/api/src/application/use_cases/payment_provider_factory.rs`** (5 occurrences)
- Lines 83-87, 119-120: Remove manual PaymentMode->StripeMode conversion (use From impl)

**`apps/api/src/application/use_cases/domain_auth.rs`** (2 occurrences)
- Line 1582: Update import in test
- Line 1893: Test mock parameter

#### 2.2 Adapter Layer - HTTP Routes

**`apps/api/src/adapters/http/routes/domain.rs`** (5 occurrences)
- Line 1041: Update import
- Lines 1046, 1084, 1118, 1140, 1145: DTO field types
- Add `#[serde(rename = "stripe_mode")]` to preserve JSON field names

**`apps/api/src/adapters/http/routes/public_domain_auth.rs`** (8 occurrences)
- Line 1805: Update import
- Lines 1815, 1826, 1835: Webhook handler parameters
- Lines 2858, 2872, 2996, 3011: Test code

#### 2.3 Adapter Layer - Persistence

**Column Read Strategy (CLARIFIED):**
```
- Read from: `payment_mode` column (backfilled in migration 00010)
- Write to: Both `payment_mode` AND `stripe_mode` columns (dual-write)
- This ensures backward compatibility until cleanup migration
```

**`apps/api/src/adapters/persistence/domain.rs`** (2 occurrences)
- Line 9: Update import
- Line 186: Function parameter type
- Update SQL queries to read from `payment_mode`, write to both columns

**`apps/api/src/adapters/persistence/billing_stripe_config.rs`** (4 occurrences)
- Line 9: Update import
- Lines 30, 68, 100: Function parameter types

**`apps/api/src/adapters/persistence/subscription_plan.rs`** (7 occurrences)
- Line 13: Update import
- Lines 76, 95, 121, 138, 292, 307: Function parameter types

**`apps/api/src/adapters/persistence/user_subscription.rs`** (8 occurrences)
- Line 15: Update import
- Lines 71, 105, 124, 298, 366, 382, 397: Function parameter types

**`apps/api/src/adapters/persistence/billing_payment.rs`** (5 occurrences)
- Line 15: Update import
- Lines 186, 241, 445, 507: Function parameter types

### Verification Checkpoint:
```bash
cargo clippy --all-features -p reauth-api
./run api:fmt
./run api:build  # SQLx offline build
./run api:test
```

---

## Phase 3: Update Frontend Types

**Goal:** Align TypeScript types with backend changes while preserving API compatibility.

### Files to modify:

**`apps/ui/types/billing.ts`**

1. **Line 35: Keep `StripeMode` type alias** (for backward compatibility in existing code):
   ```typescript
   // Legacy type alias - use PaymentMode for new code
   export type StripeMode = PaymentMode;
   ```

2. **Lines 43, 141, 148, 152: Keep using `StripeMode`** (since JSON field names unchanged)
   - `StripeConfigStatus.active_mode: StripeMode`
   - `UpdateStripeConfigInput.mode: StripeMode`
   - `DeleteStripeConfigInput.mode: StripeMode`
   - `SetBillingModeInput.mode: StripeMode`

3. **Line 50: `SubscriptionPlan.stripe_mode`** - Keep field name (matches JSON response)

4. **Lines 216-221: Helper functions** - Keep using `StripeMode` parameter type
   - `getModeLabel(mode: StripeMode)`
   - `getModeBadgeColor(mode: StripeMode)`

**Summary:** Minimal frontend changes needed since we preserve JSON field names. The `StripeMode` type alias already equals `PaymentMode`.

### Verification Checkpoint:
```bash
./run ui:build  # TypeScript compilation check
```

---

## Phase 4: Deprecate StripeMode

**Goal:** Mark `StripeMode` as deprecated to prevent new usage while maintaining compatibility.

### Files to modify:

**`apps/api/src/domain/entities/stripe_mode.rs`**

Add deprecation notice:
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

**`apps/api/src/domain/entities/mod.rs`**

Add comment about deprecation:
```rust
// stripe_mode is DEPRECATED - use payment_mode for new code
pub mod stripe_mode;
pub mod payment_mode;
```

### Cleanup unused imports:

Run after all changes:
```bash
cargo fix --allow-dirty --edition -p reauth-api
cargo clippy --all-features -p reauth-api -- -W clippy::all
./run api:fmt
```

This will show deprecation warnings for remaining usages (expected in persistence layer until DB migration).

---

## Phase 5: Create Follow-up Task (NEW)

Create placeholder task for database cleanup:

**File:** `workspace/tasks/todo/0015-remove-stripe-mode-columns.md`

```markdown
# Remove stripe_mode Database Columns

Cleanup task following 0014-unify-payment-mode-enums.

## Checklist
- [ ] Create migration 00011
- [ ] Make payment_mode columns NOT NULL
- [ ] Drop stripe_mode columns from all tables
- [ ] Rename billing_stripe_mode to active_payment_mode on domains table
- [ ] Drop stripe_mode Postgres enum type
- [ ] Remove StripeMode Rust type entirely
- [ ] Update persistence layer to stop dual-writes

## History
- 2026-01-01 Created as follow-up to task 0014.
```

---

## Testing Strategy

### Unit Tests:
1. Run existing `payment_mode.rs` tests
2. Add From conversion tests
3. Verify deprecation warnings appear (but don't fail build)

### Integration Tests:
```bash
./run api:test
```

### Build Verification:
```bash
./run api:build     # SQLx offline build (most important)
./run ui:build      # TypeScript check
cargo clippy -p reauth-api -- -D warnings 2>&1 | grep -v "deprecated"
```

### Manual Testing:
- Create/update Stripe config via UI
- Switch billing mode test -> live
- Verify subscription plans display correctly

---

## Edge Cases

### 1. Serialization Compatibility
- Both enums serialize to "test"/"live" strings
- JSON responses unchanged (using `#[serde(rename)]`)

### 2. Database Column Sync
- Both `stripe_mode` and `payment_mode` columns contain same data
- Dual-write ensures they stay in sync until cleanup migration

### 3. Webhook Handlers
- Stripe webhooks use hardcoded `StripeMode::Test` / `StripeMode::Live`
- Will be converted via From impl at call sites

---

## Implementation Order

1. Phase 0: Verify current build/tests pass
2. Phase 1: Add From/Into conversions
3. Phase 2: Migrate application layer (bottom-up: domain -> application -> adapters)
4. Phase 3: Update frontend types (minimal changes)
5. Phase 4: Add deprecation notices
6. Phase 5: Create follow-up task
7. Final verification: build, test, lint

---

## Rollback Plan

If issues arise:
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
| `application/use_cases/domain_billing.rs` | ~50 | Replace StripeMode with PaymentMode |
| `application/use_cases/domain.rs` | 3 | Replace StripeMode with PaymentMode |
| `application/use_cases/payment_provider_factory.rs` | 5 | Use From impl instead of match |
| `application/use_cases/domain_auth.rs` | 2 | Update test imports |
| `adapters/http/routes/domain.rs` | 5 | Update DTOs, add serde rename |
| `adapters/http/routes/public_domain_auth.rs` | 8 | Update webhook handlers |
| `adapters/persistence/domain.rs` | 2 | Update function signatures |
| `adapters/persistence/billing_stripe_config.rs` | 4 | Update function signatures |
| `adapters/persistence/subscription_plan.rs` | 7 | Update function signatures |
| `adapters/persistence/user_subscription.rs` | 8 | Update function signatures |
| `adapters/persistence/billing_payment.rs` | 5 | Update function signatures |

### Frontend - Minimal Changes
| File | Changes |
|------|---------|
| `apps/ui/types/billing.ts` | Make StripeMode alias to PaymentMode |

### No Changes Needed
| File | Status |
|------|--------|
| `libs/reauth-sdk-ts/` | Already uses PaymentMode only |
| `apps/demo_api/`, `apps/demo_ui/` | No StripeMode references |
| `adapters/persistence/enabled_payment_providers.rs` | Already uses PaymentMode |

---

## Risk Assessment

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| API breaking change | Low | Using serde rename to preserve JSON keys |
| Column sync issues | Very Low | Dual-write strategy, data already synced |
| Deprecation warnings in CI | Medium | Allow deprecation warnings until task 0015 |
| TypeScript type errors | Low | Minimal changes, StripeMode = PaymentMode |

---

## Revision History

- 2026-01-01 v1: Initial plan created
- 2026-01-01 v2: Addressed feedback:
  - Added Phase 0 (verification/inventory)
  - Clarified API contract strategy (preserve JSON field names with serde rename)
  - Confirmed SDK/demo apps need no changes
  - Added column read/write strategy for persistence layer
  - Added Phase 5 (create follow-up task 0015)
  - Added verification checkpoints between phases
  - Added risk assessment table
  - Made frontend changes minimal (type alias approach)
