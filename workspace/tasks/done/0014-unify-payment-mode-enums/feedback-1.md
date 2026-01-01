# Feedback on Plan v1: Unify StripeMode and PaymentMode Enums

**Reviewer:** Claude Opus 4.5
**Date:** 2026-01-01
**Task:** 0014-unify-payment-mode-enums

---

## What's Good About the Plan

1. **Sound approach decision** - Keeping `PaymentMode` as the unified type is the right call. It aligns with the multi-provider architecture and is more future-proof than the Stripe-specific name.

2. **Good separation of concerns** - Deferring the database migration to a separate task is prudent. This reduces risk and keeps the task focused on Rust enum unification.

3. **Thorough file inventory** - The plan identifies most files that need modification and categorizes them by layer (domain, application, adapter, infra).

4. **Rollback plan** - Having a clear rollback strategy is important. The additive nature of From/Into conversions makes this low-risk.

5. **Edge cases identified** - Serialization compatibility, database reads, and webhook handlers are all correctly flagged.

6. **Clear implementation order** - The phased approach with specific ordering helps prevent circular dependency issues during migration.

---

## What's Missing or Unclear

### 1. **Incomplete file list for Phase 2**

The plan references `application/use_cases/domain.rs` but doesn't provide line numbers or specific function signatures. Need to verify:
- Does `DomainRepo::set_billing_stripe_mode` actually exist? If so, what trait defines it?
- What about `application/use_cases/subscription.rs` or similar files?

### 2. **Missing: How to handle dual-column reads**

The plan says "Convert at DB boundary" for persistence files but doesn't specify:
- Should queries read from `stripe_mode` or `payment_mode` columns?
- What happens if they're out of sync (though migration 00010 backfilled, edge cases exist)?
- Should we add a validation check that both columns match?

### 3. **API contract changes not addressed**

The plan mentions updating DTOs in HTTP routes but doesn't clarify:
- Will JSON keys change? (e.g., `stripe_mode` -> `payment_mode` in responses?)
- If so, how do we maintain backward compatibility for existing API consumers?
- If not, we need to specify that serialization names stay the same via `#[serde(rename = "...")]`

### 4. **Missing: TypeScript SDK changes**

The plan covers `apps/ui/types/billing.ts` but doesn't mention:
- `libs/reauth-sdk-ts/` - Does the SDK export StripeMode types?
- Demo apps (`apps/demo_api/`, `apps/demo_ui/`) - Do they reference these enums?

### 5. **No mention of imports cleanup**

After deprecating `StripeMode`, many files will have unused imports. The plan should specify running `cargo fix` or manually cleaning up `use` statements.

### 6. **Unclear: stripe_payment_adapter.rs handling**

The plan says "Convert `PaymentMode` to legacy `StripeMode` at Stripe API boundary" but:
- Is this conversion necessary if we're deprecating StripeMode?
- Shouldn't the Stripe adapter just accept `PaymentMode` directly and use its `as_str()` method?

---

## Suggested Improvements

### 1. **Add a verification phase at the start**

Before Phase 1, add a Phase 0 that:
- Runs `grep -r "StripeMode" apps/api/` to get exhaustive list of usages
- Runs `grep -r "stripe_mode" apps/ui/` for frontend
- Documents exact count of files to change

### 2. **Specify serialization strategy explicitly**

Add to Phase 2:
```rust
// In HTTP DTOs, preserve API compatibility:
#[serde(rename = "stripe_mode")]  // Keep old JSON key for now
payment_mode: PaymentMode,
```

Or if changing the API:
```rust
// New field name in JSON responses
payment_mode: PaymentMode,
```

Document which approach is chosen.

### 3. **Add SDK/Demo apps to scope**

Create Phase 4b:
- Check `libs/reauth-sdk-ts/src/` for StripeMode references
- Check `apps/demo_api/` and `apps/demo_ui/` for consistency
- Update if needed, or note "already using PaymentMode"

### 4. **Clarify column read strategy**

In Phase 2 persistence layer section, add:
```
Strategy: Read from `payment_mode` column (NOT NULL after migration 00010 backfill)
         Fallback: Read `stripe_mode` and convert if `payment_mode` is NULL
         Write: Update both columns for now (until migration removes stripe_mode)
```

### 5. **Add lint/build verification checkpoints**

After each phase:
- `cargo clippy --all-features` - Catch deprecation warnings
- `./run api:build` - Verify SQLx offline compilation
- `./run api:fmt` - Ensure formatting

### 6. **Consider creating a follow-up task now**

Since database migration is explicitly deferred, create a placeholder task:
```
0015-remove-stripe-mode-columns
- Make payment_mode columns NOT NULL
- Drop stripe_mode columns
- Clean up Postgres enum type
```

This ensures the cleanup doesn't get forgotten.

---

## Risks and Concerns

### 1. **Silent data divergence risk**

If `stripe_mode` and `payment_mode` columns can temporarily diverge (e.g., during partial migrations or manual DB edits), reading from only one column could cause issues. Consider adding a debug assertion during development:
```rust
debug_assert_eq!(row.stripe_mode.map(PaymentMode::from), row.payment_mode);
```

### 2. **API breaking change potential**

If JSON field names change from `stripe_mode` to `payment_mode`, this is a breaking change for API consumers. The plan doesn't state whether this is intentional. Recommend:
- Keep `stripe_mode` JSON keys for now (use `#[serde(rename)]`)
- Change to `payment_mode` in a separate, documented breaking change

### 3. **TypeScript type safety**

The frontend `StripeMode` type alias (line 35 in billing.ts) may be used in multiple places. Need to:
- Search for all usages before removal
- Ensure TypeScript build catches all issues (`./run ui:build`)

### 4. **Test coverage gap**

The plan mentions "Run existing `payment_mode.rs` tests" but doesn't specify:
- Are there tests for the billing flows that use these enums?
- Do integration tests cover both test/live modes?
- Will deprecation warnings fail CI?

### 5. **Order dependency in Phase 2**

The plan lists persistence layer changes after application layer, but in practice:
- Application layer depends on persistence layer trait signatures
- Consider updating traits first, then implementations

---

## Summary

The plan is well-structured and makes the right architectural decisions. The main gaps are:
1. **API contract clarity** - Specify JSON field naming strategy
2. **SDK/Demo coverage** - Check for StripeMode usage in TypeScript SDK
3. **Column read strategy** - Clarify which column is source of truth during transition
4. **Verification steps** - Add explicit build/lint checkpoints between phases

Recommend addressing these points before implementation to avoid rework.

---

## Recommended Next Steps

1. Run `grep -rn "StripeMode" apps/` to validate file list completeness
2. Decide on API JSON field naming (breaking change or preserve via serde rename?)
3. Check `libs/reauth-sdk-ts/` for enum references
4. Update plan v2 with clarifications
5. Create placeholder task 0015 for database cleanup
