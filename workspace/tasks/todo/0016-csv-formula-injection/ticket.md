Improve CSV export formula injection guard
Refine formula detection to avoid false positives/negatives.

Checklist
- [ ] Review formula detection logic
- [ ] Adjust '-' handling and add '|' case if needed
- [ ] Add tests for edge cases

History
- 2026-01-01 06:52 Created from code review finding #16 CSV export formula injection incomplete.
- 2026-01-01 06:55 Renamed file to 0016-csv-formula-injection.md to use 4-digit task numbering.
