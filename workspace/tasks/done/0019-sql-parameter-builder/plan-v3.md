# Implementation Plan v3: SQL Parameter Builder

**Plan:** [../../../plans/code-quality-improvements.md](../../../plans/code-quality-improvements.md) (if exists)
**Task:** [ticket.md](ticket.md)
**Created:** 2026-01-01
**Revision:** v3 (addresses feedback-2.md)

## Summary

The current codebase uses manual `param_count` tracking when building dynamic SQL queries with conditional WHERE clauses. This pattern is fragile because:
1. Parameter indices must be manually incremented
2. Order of bindings must match order of conditions added
3. Easy to introduce off-by-one errors or mismatched parameters

SQLx provides `QueryBuilder<Postgres>` that automatically handles parameter indexing through `push_bind()`. This plan refactors the fragile manual counting to use the built-in builder.

## Affected Files

### Primary file requiring refactoring:
- `apps/api/src/adapters/persistence/billing_payment.rs` - Contains 3 functions with manual `param_count`:
  - `list_by_domain()` (lines 238-368) - Dynamic filters for payment list
  - `get_payment_summary()` (lines 442-502) - Date range filters for summary
  - `list_all_for_export()` (lines 504-577) - Same filters as list_by_domain

### Helper location decision: **Inline private function**
A shared `push_payment_filters()` helper will be defined as a private function within `billing_payment.rs`. This centralizes filter logic and avoids duplication across the 3 functions.

## Key Design Decisions

### Decision 1: Parameter Reuse vs. Separate Parameters

**Current behavior** (uses same parameter twice):
```sql
WHERE (bp.payment_date >= $4 OR bp.created_at >= $4)
```

**With QueryBuilder** (each push_bind creates new parameter):
```sql
WHERE (bp.payment_date >= $4 OR bp.created_at >= $5)
```

**Decision:** Accept the parameter change. The SQL semantics are identical - both values are the same `NaiveDateTime`. PostgreSQL handles this efficiently. The slight increase in parameter count (from N to N+2 when both date filters are set) is negligible and well within PostgreSQL's 65535 parameter limit.

### Decision 2: Shared Helper Function (Updated from v2)

Rather than using a closure in `list_by_domain()` and duplicating inline code in `list_all_for_export()`, extract a standalone private function:

```rust
/// Pushes payment filter conditions to a QueryBuilder.
/// Assumes the builder already has base conditions (domain_id, stripe_mode).
fn push_payment_filters(
    builder: &mut QueryBuilder<'_, Postgres>,
    filters: &PaymentListFilters,
) {
    if let Some(status) = &filters.status {
        builder.push(" AND bp.status = ").push_bind(status.clone());
    }
    if let Some(date_from) = &filters.date_from {
        builder.push(" AND (bp.payment_date >= ").push_bind(*date_from);
        builder.push(" OR bp.created_at >= ").push_bind(*date_from);
        builder.push(")");
    }
    if let Some(date_to) = &filters.date_to {
        builder.push(" AND (bp.payment_date <= ").push_bind(*date_to);
        builder.push(" OR bp.created_at <= ").push_bind(*date_to);
        builder.push(")");
    }
    if let Some(plan_code) = &filters.plan_code {
        builder.push(" AND bp.plan_code = ").push_bind(plan_code.clone());
    }
    if let Some(user_email) = &filters.user_email {
        builder.push(" AND deu.email ILIKE ").push_bind(format!("%{}%", user_email));
    }
}
```

**Benefits over v2's approach:**
- Single source of truth for filter logic
- No closure lifetime complexity
- Both `list_by_domain()` and `list_all_for_export()` call the same function
- Easier to add new filter fields in the future

### Decision 3: `build_query_scalar()` vs `build()` (Clarified from v2)

- **`build_query_scalar()`**: Used for COUNT queries that return a single scalar value. Returns `QueryScalar` which directly maps to `i64`.
- **`build()`**: Used for queries that return full rows. Returns `Query` which gives `PgRow` for manual column extraction.

This matches the existing code patterns - count queries extract a scalar, data queries use `row_to_payment_with_user()`.

### Decision 4: Row Mapping Approach

Continue using `build()` + manual row mapping via `row_to_payment_with_user()`. This matches the current pattern and avoids requiring derive macros on the structs.

**Note on `row.get()`:** The existing code uses `row.get()` which panics on column mismatch. This plan preserves that behavior. A future cleanup could migrate to `row.try_get()` for explicit error handling, but that's out of scope.

## Before/After SQL Comparison

### list_by_domain() - Before:
```sql
SELECT ... FROM billing_payments bp
JOIN domain_end_users deu ON bp.end_user_id = deu.id
WHERE bp.domain_id = $1 AND bp.stripe_mode = $2
  AND bp.status = $3
  AND (bp.payment_date >= $4 OR bp.created_at >= $4)
  AND (bp.payment_date <= $5 OR bp.created_at <= $5)
  AND bp.plan_code = $6
  AND deu.email ILIKE $7
ORDER BY bp.payment_date DESC NULLS LAST, bp.created_at DESC
LIMIT $8 OFFSET $9
```

### list_by_domain() - After:
```sql
SELECT ... FROM billing_payments bp
JOIN domain_end_users deu ON bp.end_user_id = deu.id
WHERE bp.domain_id = $1 AND bp.stripe_mode = $2
  AND bp.status = $3
  AND (bp.payment_date >= $4 OR bp.created_at >= $5)
  AND (bp.payment_date <= $6 OR bp.created_at <= $7)
  AND bp.plan_code = $8
  AND deu.email ILIKE $9
ORDER BY bp.payment_date DESC NULLS LAST, bp.created_at DESC
LIMIT $10 OFFSET $11
```

**Key difference:** Date parameters are now separate ($4/$5 instead of $4/$4). Semantically identical.

## Step-by-Step Implementation

### Phase 1: Add QueryBuilder Import

Add to imports at top of `billing_payment.rs`:
```rust
use sqlx::QueryBuilder;
use sqlx::postgres::Postgres;
```

### Phase 2: Add Shared Helper Function

Add the `push_payment_filters` function as a private function in the module:

```rust
/// Pushes payment filter conditions to a QueryBuilder.
/// Caller must ensure builder already has base WHERE conditions.
/// Expects table aliases: `bp` for billing_payments, `deu` for domain_end_users.
fn push_payment_filters(
    builder: &mut QueryBuilder<'_, Postgres>,
    filters: &PaymentListFilters,
) {
    if let Some(status) = &filters.status {
        builder.push(" AND bp.status = ").push_bind(status.clone());
    }
    if let Some(date_from) = &filters.date_from {
        builder.push(" AND (bp.payment_date >= ").push_bind(*date_from);
        builder.push(" OR bp.created_at >= ").push_bind(*date_from);
        builder.push(")");
    }
    if let Some(date_to) = &filters.date_to {
        builder.push(" AND (bp.payment_date <= ").push_bind(*date_to);
        builder.push(" OR bp.created_at <= ").push_bind(*date_to);
        builder.push(")");
    }
    if let Some(plan_code) = &filters.plan_code {
        builder.push(" AND bp.plan_code = ").push_bind(plan_code.clone());
    }
    if let Some(user_email) = &filters.user_email {
        builder.push(" AND deu.email ILIKE ").push_bind(format!("%{}%", user_email));
    }
}
```

### Phase 3: Refactor `list_by_domain()`

```rust
async fn list_by_domain(
    &self,
    domain_id: Uuid,
    mode: StripeMode,
    filters: &PaymentListFilters,
    page: i32,
    per_page: i32,
) -> AppResult<PaginatedPayments> {
    let offset = (page - 1) * per_page;

    // Count query
    let mut count_builder: QueryBuilder<Postgres> = QueryBuilder::new(
        "SELECT COUNT(*) FROM billing_payments bp \
         JOIN domain_end_users deu ON bp.end_user_id = deu.id \
         WHERE bp.domain_id = "
    );
    count_builder.push_bind(domain_id);
    count_builder.push(" AND bp.stripe_mode = ").push_bind(mode.clone());
    push_payment_filters(&mut count_builder, filters);

    let total: i64 = count_builder
        .build_query_scalar()
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::from)?;

    // Data query
    let mut data_builder: QueryBuilder<Postgres> = QueryBuilder::new(format!(
        "SELECT {}, deu.email as user_email \
         FROM billing_payments bp \
         JOIN domain_end_users deu ON bp.end_user_id = deu.id \
         WHERE bp.domain_id = ",
        SELECT_COLS
    ));
    data_builder.push_bind(domain_id);
    data_builder.push(" AND bp.stripe_mode = ").push_bind(mode);
    push_payment_filters(&mut data_builder, filters);
    data_builder.push(" ORDER BY bp.payment_date DESC NULLS LAST, bp.created_at DESC");
    data_builder.push(" LIMIT ").push_bind(per_page);
    data_builder.push(" OFFSET ").push_bind(offset);

    let rows = data_builder
        .build()
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::from)?;

    let payments: Vec<BillingPaymentWithUser> =
        rows.into_iter().map(row_to_payment_with_user).collect();
    let total_pages = ((total as f64) / (per_page as f64)).ceil() as i32;

    Ok(PaginatedPayments {
        payments,
        total,
        page,
        per_page,
        total_pages,
    })
}
```

### Phase 4: Refactor `get_payment_summary()`

This function has simpler date-only filters (no `PaymentListFilters` struct). Handle inline:

```rust
async fn get_payment_summary(
    &self,
    domain_id: Uuid,
    mode: StripeMode,
    date_from: Option<NaiveDateTime>,
    date_to: Option<NaiveDateTime>,
) -> AppResult<PaymentSummary> {
    let mut builder: QueryBuilder<Postgres> = QueryBuilder::new(
        "SELECT \
            COALESCE(SUM(CASE WHEN status = 'paid' THEN amount_paid_cents ELSE 0 END), 0) as total_revenue_cents, \
            COALESCE(SUM(amount_refunded_cents), 0) as total_refunded_cents, \
            COUNT(*) as payment_count, \
            COUNT(*) FILTER (WHERE status = 'paid') as successful_payments, \
            COUNT(*) FILTER (WHERE status IN ('failed', 'uncollectible', 'void')) as failed_payments \
         FROM billing_payments \
         WHERE domain_id = "
    );

    builder.push_bind(domain_id);
    builder.push(" AND stripe_mode = ").push_bind(mode);

    if let Some(df) = &date_from {
        builder.push(" AND (payment_date >= ").push_bind(*df);
        builder.push(" OR created_at >= ").push_bind(*df);
        builder.push(")");
    }
    if let Some(dt) = &date_to {
        builder.push(" AND (payment_date <= ").push_bind(*dt);
        builder.push(" OR created_at <= ").push_bind(*dt);
        builder.push(")");
    }

    let row = builder
        .build()
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::from)?;

    Ok(PaymentSummary {
        total_revenue_cents: row.get("total_revenue_cents"),
        total_refunded_cents: row.get("total_refunded_cents"),
        payment_count: row.get("payment_count"),
        successful_payments: row.get("successful_payments"),
        failed_payments: row.get("failed_payments"),
    })
}
```

**Why not use shared helper:** `get_payment_summary()` uses raw date parameters, not `PaymentListFilters`. The date filter pattern is the same but the function signature differs. Extracting a separate `push_date_filters()` would add complexity for minimal benefit.

### Phase 5: Refactor `list_all_for_export()`

Now uses the shared helper:

```rust
async fn list_all_for_export(
    &self,
    domain_id: Uuid,
    mode: StripeMode,
    filters: &PaymentListFilters,
) -> AppResult<Vec<BillingPaymentWithUser>> {
    let mut builder: QueryBuilder<Postgres> = QueryBuilder::new(format!(
        "SELECT {}, deu.email as user_email \
         FROM billing_payments bp \
         JOIN domain_end_users deu ON bp.end_user_id = deu.id \
         WHERE bp.domain_id = ",
        SELECT_COLS
    ));

    builder.push_bind(domain_id);
    builder.push(" AND bp.stripe_mode = ").push_bind(mode);
    push_payment_filters(&mut builder, filters);
    builder.push(" ORDER BY bp.payment_date DESC NULLS LAST, bp.created_at DESC");

    let rows = builder
        .build()
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::from)?;

    Ok(rows.into_iter().map(row_to_payment_with_user).collect())
}
```

### Phase 6: Verification and Testing

1. **Compile check:**
   ```bash
   ./run api:build
   ```
   This uses `SQLX_OFFLINE=true` so no database needed. QueryBuilder generates queries at runtime, so no `./run db:prepare` update needed.

2. **Format and lint:**
   ```bash
   ./run api:fmt && ./run api:lint
   ```

3. **Run tests:**
   ```bash
   ./run api:test
   ```

4. **Manual verification (if database available):**
   - Test payment list with no filters
   - Test with single filter (status only)
   - Test with all filters set
   - Test date range filters specifically
   - Verify row counts match before/after

## Implementation Notes

QueryBuilder gotchas to keep in mind:
- **1-indexed parameters:** `push_bind()` handles this automatically
- **Method chaining:** `push()` and `push_bind()` return `&mut Self`, enabling chaining like `builder.push(" AND ").push_bind(val)`
- **Builder consumption:** `build()` and `build_query_scalar()` consume the builder, so must be called last
- **Type requirements:** Values passed to `push_bind()` must implement `sqlx::Encode<Postgres>`. All types used here (`Uuid`, `StripeMode`, `String`, `NaiveDateTime`, `i32`) already do.

## Edge Cases Handled

| Case | Handling |
|------|----------|
| Empty filters | Only base conditions (domain_id, stripe_mode) are added |
| All filters set | All conditions chained correctly with AND |
| Date filters binding same value twice | Each push_bind creates separate parameter - semantically identical |
| ILIKE wildcards | `format!("%{}%", user_email)` creates owned String before binding |
| NULL optional fields | Only add conditions when `Some` |
| Empty string user_email | Will search for `%%` which matches all - same as current behavior |

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| SQLX_OFFLINE compatibility | QueryBuilder generates queries at runtime, doesn't use cached metadata. Compile check confirms this works. |
| Lifetime issues | Filter values are cloned/dereferenced before binding where needed. No lifetime coupling between builder and filters. |
| Behavioral regression | Same SQL semantics despite different parameter numbering. Integration tests verify results. |
| `StripeMode` Encode trait | Will be caught by compiler if missing (existing code compiles, so this is fine) |

## Rollback Plan

If issues discovered post-merge:
1. `git revert <commit>` - This is a pure refactor with no API changes
2. The revert will restore the working `param_count` pattern

## Success Criteria

1. All manual `param_count` tracking removed from `billing_payment.rs`
2. No changes to query behavior or results
3. `./run api:build` succeeds (SQLX_OFFLINE=true)
4. `./run api:fmt` produces no changes
5. `./run api:lint` passes with no new warnings
6. `./run api:test` passes

## Complexity Assessment

- **Low complexity** - SQLx QueryBuilder is well-documented and straightforward
- **Main work** - Adding 1 helper function, refactoring 3 functions
- **Risk** - Low, as this is an internal refactor with no API changes

---

## History

- 2026-01-01 07:00 Created initial plan (v1)
- 2026-01-01 Revised to v2 addressing feedback:
  - Clarified parameter reuse semantics (Decision 1)
  - Added count/data query strategy with closure pattern (Decision 2)
  - Decided on inline helper location (no new module)
  - Added Before/After SQL comparison
  - Clarified row mapping approach
  - Added rollback plan
  - Removed unnecessary Phase 1 test step
- 2026-01-01 Revised to v3 addressing feedback-2.md:
  - Replaced closure pattern with shared `push_payment_filters()` private function (Decision 2)
  - Added clarification on `build_query_scalar()` vs `build()` (Decision 3)
  - Added note about `row.get()` panic behavior and future cleanup option
  - Added Implementation Notes section with QueryBuilder gotchas
  - Confirmed no `./run db:prepare` update needed (QueryBuilder is runtime)
  - `get_payment_summary()` keeps inline date filters (different signature from PaymentListFilters)
