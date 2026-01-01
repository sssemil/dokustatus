Handle JSON parse errors in persistence
Stop swallowing JSON parse failures and surface or log them.

Checklist
- [x] Find unwrap_or_default JSON parsing sites
- [x] Add error logging/propagation
- [x] Add tests or migration check if needed

History
- 2026-01-01 06:52 Created from code review finding #9 Silently swallowed JSON parse errors.
- 2026-01-01 06:55 Renamed file to 0009-json-parse-error-handling.md to use 4-digit task numbering.
- 2026-01-01 07:15 Created plan-v1.md. Found 3 deserialization sites (domain_end_user.rs:13, subscription_plan.rs:18, user_subscription.rs:162) and 3 serialization sites. Plan: add reusable helper with logging, update all sites, add unit tests.
- 2026-01-01 12:51 Added plan feedback in feedback-1.md.
- 2026-01-01 13:02 Created plan-v2.md addressing feedback: added NULL semantics, reference-based helper, structured log fields, JSON truncation, fail-closed safety analysis.
- 2026-01-01 12:54 Added feedback-2.md with plan review notes.
- 2026-01-01 13:10 Created plan-v3.md addressing feedback-2: added explicit Value::Null handling, changed serialization errors to propagate instead of silent fallback, verified SQLx JSONB NULL behavior.
- 2026-01-01 16:45 Implementation complete. Added parse_json_with_fallback helper to mod.rs with NULL handling and log truncation. Updated domain_end_user.rs (row_to_profile, set_roles), subscription_plan.rs (row_to_profile, create, update), user_subscription.rs (list_by_domain_and_mode). Added 6 unit tests. Build and tests pass (59 tests). Commit: e9f57a7.
