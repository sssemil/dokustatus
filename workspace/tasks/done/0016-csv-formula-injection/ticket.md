Improve CSV export formula injection guard
Refine formula detection to avoid false positives/negatives.

Checklist
- [x] Review formula detection logic
- [x] Adjust '-' handling and add '|' case if needed
- [x] Add tests for edge cases

History
- 2026-01-01 06:52 Created from code review finding #16 CSV export formula injection incomplete.
- 2026-01-01 06:55 Renamed file to 0016-csv-formula-injection.md to use 4-digit task numbering.
- 2026-01-01 07:15 Created plan-v1.md with detailed implementation plan. Key changes: add `|` to dangerous chars, adjust `-` handling to allow negative numbers, add comprehensive tests.
- 2026-01-01 07:45 Created plan-v2.md addressing feedback. Key additions: handle `-.5` decimals, verify no other CSV exports exist, document leading whitespace behavior, add integration testing step.
- 2026-01-01 08:15 Created plan-v3.md (final revision). Key additions: specific integration test commands with curl/endpoints, Unicode formula markers documented as out of scope, raw strings for test readability, expanded CSV library search, clarified test module placement, added performance note.
- 2026-01-01 09:30 Implementation complete:
  - Verified no other CSV export paths exist (only `domain_billing.rs` via `domain.rs` route)
  - Updated `escape_csv_field` function in `apps/api/src/application/use_cases/domain_billing.rs`:
    - Added `|` (pipe) to dangerous characters list for DDE attack prevention
    - Added smart `-` handling: allows negative numbers (`-100`, `-.5`) while blocking formula-like patterns (`-cmd`, `--`)
    - Enhanced docstring with detailed documentation of behavior and design decisions
  - Added comprehensive test module `escape_csv_field_tests` with 9 test functions covering:
    - Normal values, formula prefixes (`=`, `+`, `@`), pipe DDE vectors
    - Minus handling edge cases, tab/CR prefixes, standard CSV escaping
    - Combined scenarios, edge cases, Unicode formula markers (documented as out of scope)
  - All 9 tests pass, release build verified with SQLX_OFFLINE=true
- 2026-01-01 09:35 Task complete. Committed as 20b7618. Ready for merge.
- 2026-01-01 15:30 Merge agent: rebase onto origin/main complete (branch was already up to date). Ready for squash merge.
- 2026-01-01 15:45 Merge agent: verified rebase up to date with origin/main (b7adc63). Confirmed ready for squash merge.
- 2026-01-01 17:50 Merge agent: final verification - branch is 34 commits ahead, 0 behind origin/main. Rebase confirmed complete.
