# Feedback on Plan v3: Add Google ID Index

**Reviewer:** Claude Opus 4.5
**Date:** 2026-01-01

---

## What's Good

1. **Clear executive summary with actionable recommendation** — The "close as already covered" decision is well-justified and prevents unnecessary work.

2. **Execution checklist at the top** — Having the concrete steps upfront makes this immediately actionable. Previous feedback was addressed.

3. **Thorough investigation of existing index** — The plan correctly identifies that `idx_domain_end_users_google_id` from migration 00006 should cover the query. The partial index with `WHERE google_id IS NOT NULL` will work for equality checks.

4. **Fixed INSERT statement** — Using the seeded domain_id and including the required `roles` column resolves the v2 issues. The UPSERT pattern is robust.

5. **Contingency migration is well-prepared** — If the unlikely case occurs where the index isn't used, the fallback migration uses `CONCURRENTLY` and has a clear rollback.

6. **Result interpretation table** — The table explaining when to close vs. investigate based on Seq Scan + table size is practical.

7. **Post-close monitoring guidance** — Specifying the exact endpoint (`/auth/google/callback`) and threshold (P95 > 100ms) gives actionable follow-up criteria.

8. **History and changes tracking** — The v2→v3 changes table makes review efficient.

---

## What's Missing or Unclear

1. **No verification that the query code matches the plan's assumption**
   The plan assumes `get_by_domain_and_google_id()` exists in `domain_end_user.rs:65-79` and that `domain_auth.rs:1285` and `domain_auth.rs:1343` call it. These line numbers should be verified during execution — if the code has moved or changed, the plan's references become misleading.

2. **DATABASE_URL not defined**
   Step 1 says `psql "$DATABASE_URL"` but doesn't specify how to obtain this. For local dev, it's typically in `.env` or printed by `./run infra`. Add a note like:
   ```bash
   # Get DATABASE_URL from your .env or run:
   source apps/api/.env
   psql "$DATABASE_URL"
   ```

3. **Cleanup of test data not mentioned**
   The plan inserts `test-google@example.com` but doesn't mention removing it afterward. If this is local dev only, that's fine, but explicitly stating "test data will remain in local dev database" or adding a cleanup step would be cleaner.

4. **No explicit handling of "index exists but is different"**
   What if the index name exists but has different columns or a different condition? The verification step lists expected output but doesn't say what to do if the index definition doesn't match. (Unlikely, but worth a note.)

---

## Suggested Improvements

1. **Add file verification step to checklist**
   Before Step 1, add:
   ```
   - [ ] Grep for get_by_domain_and_google_id to confirm query location
   ```
   This ensures the code matches the plan's assumptions.

2. **Simplify the psql connection**
   Consider using `./run` if there's a db:shell command, or add the exact connection string for local dev:
   ```bash
   psql postgresql://reauth:reauth@localhost:5432/reauth
   ```

3. **Make cleanup optional but explicit**
   Add to the checklist:
   ```
   - [ ] (Optional) Clean up test user: DELETE FROM domain_end_users WHERE email = 'test-google@example.com';
   ```

4. **Consider adding timing to EXPLAIN ANALYZE output expectations**
   For thoroughness, note that typical execution time should be sub-millisecond for an indexed lookup. This helps catch issues where the index is used but performance is still poor due to other factors.

---

## Risks or Concerns

1. **Low risk: Migration 00006 wasn't actually applied**
   If someone is working with a fresh or partially migrated database, the index might not exist. Step 1 catches this, but it's worth noting in the risk table.

2. **Very low risk: Partial index edge case**
   If the application ever queries for `google_id = NULL` (which semantically doesn't make sense for OAuth), the partial index won't help. The current query code passes a non-null value, so this isn't an issue — just documenting for completeness.

3. **No risk but worth noting: Test data pollutes seeded database**
   The test INSERT adds a user that wasn't in the original seed. This is fine for local dev but could cause confusion if someone expects the seeded data to be pristine.

---

## Verdict

**Plan is ready for execution.** The suggestions above are minor refinements. The core approach — verify first, close if covered, monitor afterward — is sound.

Proceed with execution using the checklist in plan-v3.md.
