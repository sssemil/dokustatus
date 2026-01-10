Fix MRR precision loss
Avoid integer division truncation in MRR calculation.

Checklist
- [x] Review MRR computation path
- [x] Use safe arithmetic (scaled ints or f64)
- [x] Add test for yearly plan rounding

History
- 2026-01-01 06:52 Created from code review finding #11 Integer division precision loss in MRR.
- 2026-01-01 06:55 Renamed file to 0011-mrr-precision-fix.md to use 4-digit task numbering.
- 2026-01-01 07:10 Created detailed implementation plan (plan-v1.md). Identified the bug at `domain_billing.rs:1818-1824` where integer division truncates MRR. Fix will use f64 for calculation with rounding at the end.
- 2026-01-01 07:45 Created plan-v2.md addressing feedback: made helper function mandatory, verified code location at lines 1818-1825, clarified plan_stats structure, added comprehensive unit tests.
- 2026-01-01 08:30 Created plan-v3.md (final revision): verified PlanDistribution struct, added MONTHS_PER_YEAR constant, added debug_assert, documented wildcard match behavior, added PlanAccumulator type alias, expanded test coverage. Plan ready for implementation.
- 2026-01-01 09:15 Implemented fix following plan-v3. Changes made to `apps/api/src/application/use_cases/domain_billing.rs`:
  1. Added MONTHS_PER_YEAR constant (line 23)
  2. Added calculate_monthly_amount_cents helper function (lines 39-52) using f64 arithmetic
  3. Updated get_analytics() to use f64 accumulation with PlanAccumulator type alias (lines 1844-1870)
  4. Updated plan_distribution conversion to round f64 values (lines 1875-1883)
  5. Added 13 unit tests covering yearly/monthly plans, edge cases, and accumulation precision
  All tests pass (76 total). Build, format, and lint checks successful.
- 2026-01-01 09:20 Task complete. Committed as 4a7bcf0. Moving to outbound for merge.
