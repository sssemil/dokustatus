Reduce fragile manual SQL parameter tracking
Introduce a helper or builder for dynamic SQL parameter counting.

Checklist
- [x] Locate manual param_count usage
- [x] Introduce small builder helper
- [x] Refactor query construction

History
- 2026-01-01 06:52 Created from code review finding #19 Dynamic SQL parameter counting is fragile.
- 2026-01-01 06:55 Renamed file to 0019-sql-parameter-builder.md to use 4-digit task numbering.
- 2026-01-01 07:00 Created detailed implementation plan (plan-v1.md). Key findings:
  - SQLx already provides `QueryBuilder<Postgres>` with automatic parameter indexing
  - 3 functions in `billing_payment.rs` need refactoring: `list_by_domain()`, `get_payment_summary()`, `list_all_for_export()`
  - Can extract reusable `push_payment_filters()` helper for shared filter logic
- 2026-01-01 Created plan-v2.md addressing feedback:
  - Clarified date parameter reuse semantics (binding same value twice is OK)
  - Added closure pattern for count/data query reuse in list_by_domain()
  - Decided helper stays inline (no new module)
  - Added Before/After SQL comparison showing parameter index changes
  - Added rollback plan (simple git revert)
  - Removed unnecessary preliminary test phase
- 2026-01-01 Created plan-v3.md addressing feedback-2.md:
  - Replaced closure with shared `push_payment_filters()` private function
  - Added clarification on `build_query_scalar()` vs `build()`
  - Added Implementation Notes section with QueryBuilder gotchas
  - Confirmed no `./run db:prepare` needed (runtime query generation)
- 2026-01-01 08:30 Implementation completed:
  - Added `QueryBuilder` and `Postgres` imports to `billing_payment.rs`
  - Added `push_payment_filters()` helper function for shared filter logic
  - Refactored `list_by_domain()` to use QueryBuilder (removed ~80 lines of manual param_count code)
  - Refactored `get_payment_summary()` to use QueryBuilder (removed ~30 lines)
  - Refactored `list_all_for_export()` to use QueryBuilder (removed ~50 lines)
  - Fixed clippy warnings (replaced `status.clone()` with `*status`, removed unnecessary `mode.clone()`)
  - All verification passed: `./run api:fmt`, `./run api:build`, `./run api:lint`, `./run api:test` (93 tests pass)
  - Committed as f0281c8
- 2026-01-01 08:35 Task completed. Moving to outbound for merge.
