Validate plan code characters
Restrict plan code to safe URL-friendly characters.

Checklist
- [x] Review plan code validation rules
- [x] Add character whitelist validation
- [x] Update error messages/tests

History
- 2026-01-01 06:52 Created from code review finding #17 Plan code not validated for special characters.
- 2026-01-01 06:55 Renamed file to 0017-plan-code-validation.md to use 4-digit task numbering.
- 2026-01-25 15:10 Started implementation. Reviewed existing validation (length only).
- 2026-01-25 15:15 Added `is_valid_plan_code()` validator in validators.rs with 9 unit tests.
- 2026-01-25 15:20 Updated `create_plan()` to normalize to lowercase and use new validator.
- 2026-01-25 15:25 Added 6 integration tests for plan code validation edge cases.

Implementation details:
- Character whitelist: `[a-z0-9_-]` (lowercase alphanumeric + hyphen + underscore)
- Must start with letter or number (not hyphen/underscore)
- Length: 1-50 characters
- Normalize to lowercase before validation
- No trimming - reject whitespace outright
- Files modified: validators.rs, domain_billing.rs
- 2026-01-25 15:30 All 257 tests pass, task complete. Moved to done.
