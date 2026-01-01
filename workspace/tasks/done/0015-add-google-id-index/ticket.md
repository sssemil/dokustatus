Add index for domain_end_users google_id lookup
Speed up domain+google_id queries with a composite index.

Checklist
- [x] Confirm query path uses domain_id + google_id
- [x] Create migration for composite index -> Already exists: idx_domain_end_users_google_id
- [x] Document index in migration notes -> Documented in 00006_google_oauth.sql

History
- 2026-01-01 06:52 Created from code review finding #15 Missing index on google_id lookup.
- 2026-01-01 06:55 Renamed file to 0015-add-google-id-index.md to use 4-digit task numbering.
- 2026-01-01 07:30 Created plan-v1.md. Analysis revealed an existing unique partial index `idx_domain_end_users_google_id ON domain_end_users(domain_id, google_id) WHERE google_id IS NOT NULL` was already created in migration 00006. This should serve the lookup query. Plan recommends verification with EXPLAIN ANALYZE before adding any new index.
- 2026-01-01: Created plan-v2.md addressing feedback. Confirmed next migration number is 00011. Added rollback procedure, success criteria, and prerequisite verification checklist. Primary recommendation: close as "already covered" since existing partial index should work.
- 2026-01-01: Created plan-v3.md (final revision). Addressed v2 feedback: separated planning from execution checklist, fixed test INSERT to use seed data and required columns, added index existence verification step, explained original finding #15 context, added post-close monitoring guidance. Ready for execution.
- 2026-01-01 14:47 Closed. Verified existing unique partial index `idx_domain_end_users_google_id` from migration 00006 covers the lookup query `WHERE domain_id = $1 AND google_id = $2` used in `get_by_domain_and_google_id()` at domain_end_user.rs:71. EXPLAIN ANALYZE shows Seq Scan which is expected for a 2-row table (cost-based optimizer prefers sequential scan for very small tables). No new migration needed. Finding #15 should be closed with note: "Index exists since migration 00006."
