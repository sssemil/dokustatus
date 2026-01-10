# Feedback on Plan v1: SQL Parameter Builder

**Reviewer:** Claude (Opus 4.5)
**Date:** 2026-01-01

## What's Good About the Plan

1. **Correct diagnosis of the problem.** The plan accurately identifies the fragility of manual `param_count` tracking and demonstrates understanding of the failure modes (off-by-one errors, mismatched bindings).

2. **Good choice of solution.** Using SQLx's built-in `QueryBuilder<Postgres>` is the right approach - it's idiomatic, already available in the codebase (SQLx 0.8), and requires no additional dependencies.

3. **Thorough affected file analysis.** The plan correctly identifies all 3 functions that need refactoring: `list_by_domain()`, `get_payment_summary()`, and `list_all_for_export()`.

4. **Practical phased approach.** Breaking the work into phases (understand API → refactor first function → extract helper → refactor remaining) reduces risk.

5. **Correct identification of code duplication.** The plan notes that `list_by_domain()` and `list_all_for_export()` share identical filter logic, making `push_payment_filters()` a sensible extraction.

6. **Edge cases documented.** Empty filters, all filters set, date filters with OR clauses, ILIKE wildcards - all covered.

## What's Missing or Unclear

### 1. **The date filter logic in the plan is WRONG**

The plan's proposed code (lines 81-84):
```rust
if let Some(date_from) = &filters.date_from {
    builder.push(" AND (bp.payment_date >= ").push_bind(date_from);
    builder.push(" OR bp.created_at >= ").push_bind(date_from);
    builder.push(")");
}
```

This will generate: `AND (bp.payment_date >= $N OR bp.created_at >= $M)` with **two different parameters** ($N and $M), both bound to the same value.

The **current code** (line 262-264) intentionally reuses the same parameter:
```rust
conditions.push(format!(
    "(bp.payment_date >= ${} OR bp.created_at >= ${})",
    param_count, param_count  // SAME param_count used twice
));
```

This isn't a bug in the current code - it's an optimization that reduces parameter count. With `QueryBuilder`, you'd need to bind the same value twice, which works but is semantically different. The plan should explicitly acknowledge this.

### 2. **Missing: How to handle the count query duplication**

The plan mentions two options for the count query (Option A vs Option B) and recommends Option B (separate builders). But it doesn't provide implementation details for this.

With separate builders, you'd duplicate the filter-pushing code unless you:
- Use a helper function that accepts `&mut QueryBuilder` (which the plan does propose)
- Or use a closure

The plan should show the actual code for how count and data queries would both use `push_payment_filters()`.

### 3. **Missing: Type annotations for QueryBuilder with row mapping**

The current code uses `sqlx::query(&data_query)` which returns `Query<Postgres, PgArguments>` and is then mapped via `row_to_payment_with_user`.

With `QueryBuilder`, calling `builder.build()` returns `QueryAs<...>` if you use `build_query_as()`, but the plan uses `build()` which returns `Query`. The plan should clarify:
- Will it use `build()` + manual row mapping (current approach)?
- Or `build_query_as::<BillingPaymentWithUser>()` + derive macro?

### 4. **Missing: `query_helpers.rs` decision**

The plan mentions creating `apps/api/src/adapters/persistence/query_helpers.rs` but marks it as "(optional, could be inline)". This should be decided upfront:
- If the helper is only used in `billing_payment.rs`, keep it inline
- If other files could benefit, create the module

Recommendation: Keep it inline since it's specific to payment filters.

### 5. **No verification of SQLx QueryBuilder API compatibility**

The plan references `/home/user/.cargo/registry/src/.../sqlx-core-0.8.0/src/query_builder.rs` but doesn't confirm:
- The exact method signatures
- Whether `push()` returns `&mut Self` (it does, enabling chaining)
- Whether the lifetime constraints allow the proposed helper function signature

### 6. **Lifetime complexity in helper function not addressed**

The proposed helper:
```rust
fn push_payment_filters<'a>(
    builder: &mut QueryBuilder<'a, Postgres>,
    filters: &'a PaymentListFilters,
)
```

This ties `filters` lifetime to the builder. If `PaymentListFilters` contains owned `String` values (which it likely does for `user_email`), this may cause issues when doing `format!("%{}%", user_email)` because the formatted string is temporary.

The plan should investigate the `PaymentListFilters` struct definition.

## Suggested Improvements

### 1. **Add a "Before/After" comparison section**

Show the exact SQL that will be generated before and after. This makes it easier to verify correctness:

**Before (current):**
```sql
WHERE bp.domain_id = $1 AND bp.stripe_mode = $2 AND bp.status = $3
      AND (bp.payment_date >= $4 OR bp.created_at >= $4)
      LIMIT $5 OFFSET $6
```

**After (with QueryBuilder):**
```sql
WHERE bp.domain_id = $1 AND bp.stripe_mode = $2 AND bp.status = $3
      AND (bp.payment_date >= $4 OR bp.created_at >= $5)
      LIMIT $6 OFFSET $7
```

The difference in parameter reuse should be documented.

### 2. **Clarify test strategy**

The plan mentions unit tests for the helper function, but:
- `builder.sql()` only returns the SQL string, not the bound values
- How do you verify the bindings are correct?

Consider adding an integration test that actually runs the queries against a test database and compares results.

### 3. **Add a rollback plan**

If issues are discovered post-merge, what's the rollback strategy? Since this is a refactor with no API changes, a simple `git revert` should work, but it's worth stating.

### 4. **Consider whether `push_payment_filters` should handle base conditions**

The helper currently only handles optional filters. Consider whether it should also handle the required `domain_id` and `stripe_mode` conditions for consistency. This would reduce duplication further.

### 5. **Phase 1 test may not be necessary**

"Create a minimal test to validate approach" - SQLx's QueryBuilder is well-documented and widely used. A quick manual test in a scratch file or REPL might suffice. Formal test can come in Phase 6.

## Risks and Concerns

### 1. **SQLX_OFFLINE mode compatibility**

The plan mentions `./run api:build` uses `SQLX_OFFLINE=true`. Need to verify that `QueryBuilder`-based queries work correctly in offline mode. `QueryBuilder` generates queries at runtime, so it doesn't use the cached query metadata in `.sqlx/`. This should be fine but deserves a note.

### 2. **Parameter count increase for date filters**

As noted above, the current code reuses parameters. With QueryBuilder, each `push_bind()` is a new parameter. For date range queries, this doubles the date parameters (from 2 to 4 if both `date_from` and `date_to` are set).

This is unlikely to cause issues, but PostgreSQL has a limit of 65535 parameters per query. Not a practical concern here, but worth noting.

### 3. **Error message regression**

If SQLx generates different error messages for invalid queries (e.g., type mismatches), debugging may require adjustment. Low risk.

### 4. **Performance testing**

The plan states "QueryBuilder is thin wrapper, no performance impact" - this is likely true, but there's no plan to verify it. Consider running the payment list endpoint with timing before/after.

## Summary

The plan is solid overall and addresses the core problem correctly. Main gaps:

1. **Must address date filter parameter reuse semantics** (Critical)
2. **Should clarify count query implementation details** (Important)
3. **Should decide on module location upfront** (Minor)
4. **Should verify lifetime constraints with PaymentListFilters** (Important)

Once these are addressed, the plan is ready for implementation.

---
*Feedback ends*
