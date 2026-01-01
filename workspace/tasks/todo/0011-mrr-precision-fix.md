Fix MRR precision loss
Avoid integer division truncation in MRR calculation.

Checklist
- [ ] Review MRR computation path
- [ ] Use safe arithmetic (scaled ints or f64)
- [ ] Add test for yearly plan rounding

History
- 2026-01-01 06:52 Created from code review finding #11 Integer division precision loss in MRR.
- 2026-01-01 06:55 Renamed file to 0011-mrr-precision-fix.md to use 4-digit task numbering.
