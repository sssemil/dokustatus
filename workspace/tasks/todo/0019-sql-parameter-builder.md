Reduce fragile manual SQL parameter tracking
Introduce a helper or builder for dynamic SQL parameter counting.

Checklist
- [ ] Locate manual param_count usage
- [ ] Introduce small builder helper
- [ ] Refactor query construction

History
- 2026-01-01 06:52 Created from code review finding #19 Dynamic SQL parameter counting is fragile.
- 2026-01-01 06:55 Renamed file to 0019-sql-parameter-builder.md to use 4-digit task numbering.
