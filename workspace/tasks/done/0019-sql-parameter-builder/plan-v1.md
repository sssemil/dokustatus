# Implementation Plan: SQL Parameter Builder

**Plan:** [../../../plans/code-quality-improvements.md](../../../plans/code-quality-improvements.md) (if exists)
**Task:** [ticket.md](ticket.md)
**Created:** 2026-01-01

## Summary

The current codebase uses manual `param_count` tracking when building dynamic SQL queries with conditional WHERE clauses. This pattern is fragile because:
1. Parameter indices must be manually incremented
2. Order of bindings must match order of conditions added
3. Easy to introduce off-by-one errors or mismatched parameters
4. Conditions using the same parameter twice (like date ranges) require careful handling

SQLx already provides `QueryBuilder<Postgres>` that automatically handles parameter indexing through `push_bind()`. This plan refactors the fragile manual counting to use the built-in builder.

## Affected Files

### Primary file requiring refactoring:
- `apps/api/src/adapters/persistence/billing_payment.rs` - Contains 3 functions with manual `param_count`:
  - `list_by_domain()` (lines 238-368) - Dynamic filters for payment list
  - `get_payment_summary()` (lines 442-502) - Date range filters for summary
  - `list_all_for_export()` (lines 504-577) - Same filters as list_by_domain

### Files to create:
- `apps/api/src/adapters/persistence/query_helpers.rs` - Reusable helper for dynamic WHERE clauses (optional, could be inline)

### Files for reference:
- `apps/api/src/adapters/persistence/mod.rs` - Module exports
- `/home/user/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/sqlx-core-0.8.0/src/query_builder.rs` - SQLx QueryBuilder API

## Step-by-Step Implementation

### Phase 1: Understand and Test SQLx QueryBuilder

1. **Verify QueryBuilder availability**
   - SQLx 0.8 includes `QueryBuilder` by default
   - No additional features needed in Cargo.toml

2. **Create a minimal test to validate approach**
   - Add a test in `billing_payment.rs` that builds a query with QueryBuilder
   - Verify parameter placeholders are correctly generated ($1, $2, etc.)

### Phase 2: Refactor `list_by_domain()` Function

1. **Current pattern (lines 249-356):**
   ```rust
   let mut conditions: Vec<String> = vec![...];
   let mut param_count = 2;

   if filters.status.is_some() {
       param_count += 1;
       conditions.push(format!("bp.status = ${}", param_count));
   }
   // ... more conditions

   // Build query string, then bind in same order
   let mut data_q = sqlx::query(&data_query).bind(domain_id).bind(mode);
   if let Some(status) = &filters.status {
       data_q = data_q.bind(status);
   }
   ```

2. **New pattern using QueryBuilder:**
   ```rust
   use sqlx::QueryBuilder;
   use sqlx::postgres::Postgres;

   let mut builder: QueryBuilder<Postgres> = QueryBuilder::new(format!(
       "SELECT {}, deu.email as user_email FROM billing_payments bp ...",
       SELECT_COLS
   ));

   builder.push(" WHERE bp.domain_id = ").push_bind(domain_id);
   builder.push(" AND bp.stripe_mode = ").push_bind(mode);

   if let Some(status) = &filters.status {
       builder.push(" AND bp.status = ").push_bind(status);
   }
   if let Some(date_from) = &filters.date_from {
       builder.push(" AND (bp.payment_date >= ").push_bind(date_from);
       builder.push(" OR bp.created_at >= ").push_bind(date_from);
       builder.push(")");
   }
   // ... more conditions

   let query = builder.build();
   let rows = query.fetch_all(&self.pool).await?;
   ```

3. **Handle the duplicate count query**
   - The function runs two queries: count and data
   - Option A: Build query string once, use for both (requires manual parameter handling for count)
   - Option B: Use separate builders for each (cleaner, slight duplication)
   - **Recommendation: Option B** - cleaner and avoids complexity

### Phase 3: Refactor Helper for Reusable Filter Building

Since `list_by_domain()` and `list_all_for_export()` use identical filter logic, extract a helper:

```rust
/// Pushes payment filter conditions to the query builder
fn push_payment_filters<'a>(
    builder: &mut QueryBuilder<'a, Postgres>,
    filters: &'a PaymentListFilters,
) {
    if let Some(status) = &filters.status {
        builder.push(" AND bp.status = ").push_bind(status);
    }
    if let Some(date_from) = &filters.date_from {
        builder.push(" AND (bp.payment_date >= ").push_bind(date_from);
        builder.push(" OR bp.created_at >= ").push_bind(date_from);
        builder.push(")");
    }
    if let Some(date_to) = &filters.date_to {
        builder.push(" AND (bp.payment_date <= ").push_bind(date_to);
        builder.push(" OR bp.created_at <= ").push_bind(date_to);
        builder.push(")");
    }
    if let Some(plan_code) = &filters.plan_code {
        builder.push(" AND bp.plan_code = ").push_bind(plan_code);
    }
    if let Some(user_email) = &filters.user_email {
        builder.push(" AND deu.email ILIKE ").push_bind(format!("%{}%", user_email));
    }
}
```

### Phase 4: Refactor `get_payment_summary()` Function

1. Similar pattern to Phase 2
2. Uses simpler date range filters only
3. Can use same approach with QueryBuilder

### Phase 5: Refactor `list_all_for_export()` Function

1. Reuse `push_payment_filters()` helper from Phase 3
2. Nearly identical to `list_by_domain()` without pagination

### Phase 6: Testing

1. **Unit tests for helper function:**
   ```rust
   #[cfg(test)]
   mod tests {
       use super::*;

       #[test]
       fn test_push_payment_filters_empty() {
           let filters = PaymentListFilters::default();
           let mut builder: QueryBuilder<Postgres> = QueryBuilder::new("SELECT * FROM t WHERE 1=1");
           push_payment_filters(&mut builder, &filters);
           assert_eq!(builder.sql(), "SELECT * FROM t WHERE 1=1");
       }

       #[test]
       fn test_push_payment_filters_with_status() {
           let filters = PaymentListFilters {
               status: Some(PaymentStatus::Paid),
               ..Default::default()
           };
           let mut builder: QueryBuilder<Postgres> = QueryBuilder::new("SELECT * FROM t WHERE 1=1");
           push_payment_filters(&mut builder, &filters);
           assert!(builder.sql().contains("AND bp.status = $1"));
       }

       #[test]
       fn test_push_payment_filters_with_all() {
           // Test with all filters set to verify correct parameter numbering
       }
   }
   ```

2. **Integration tests (if DB available):**
   - Existing queries should produce same results
   - Run `./run api:test` to validate

### Phase 7: Cleanup

1. Remove unused `param_count` variable declarations
2. Ensure no unused imports
3. Run `./run api:fmt` and `./run api:lint`

## Edge Cases to Handle

1. **Empty filters** - Query should work with no optional filters
2. **All filters set** - All conditions should chain correctly with AND
3. **Date filters using same value twice** - The date_from and date_to conditions use the same value in OR clauses; with QueryBuilder, each `push_bind()` creates a new parameter
4. **ILIKE with wildcards** - The `%` wrapping for user_email must happen before binding
5. **NULL handling** - Optional filters should only add conditions when Some

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| QueryBuilder generates different SQL | Write tests comparing old and new query structures |
| Performance regression | QueryBuilder is thin wrapper, no performance impact |
| Breaking existing functionality | Comprehensive integration tests before/after |
| Lifetime issues with push_bind | Use owned values or references that outlive builder |

## Success Criteria

1. All manual `param_count` tracking removed from `billing_payment.rs`
2. No changes to query behavior or results
3. `./run api:test` passes
4. `./run api:build` succeeds (SQLX_OFFLINE=true)
5. `./run api:lint` passes with no new warnings

## Estimated Complexity

- **Low complexity** - SQLx QueryBuilder is well-documented and straightforward
- **Main work** - Refactoring 3 functions and adding tests
- **Risk** - Low, as this is an internal refactor with no API changes

---

## History

- 2026-01-01 07:00 Created initial plan based on codebase exploration
