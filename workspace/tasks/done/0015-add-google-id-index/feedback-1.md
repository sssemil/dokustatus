# Feedback on Plan v1: Add Google ID Index

**Reviewer:** Claude (Opus 4.5)
**Date:** 2026-01-01
**Plan Version:** v1

---

## What's Good About the Plan

1. **Thorough investigation before action.** The plan correctly identifies that a unique partial index already exists (`idx_domain_end_users_google_id`) and questions whether a new index is even needed. This shows good engineering judgment—verify first, implement only if necessary.

2. **Well-researched PostgreSQL behavior.** The explanation of how PostgreSQL handles partial indexes with equality predicates (`google_id = $2` implies `google_id IS NOT NULL`) is accurate and demonstrates solid database knowledge.

3. **Clear decision framework.** The "Scenario A / Scenario B" breakdown gives a logical path forward depending on verification results.

4. **Production safety considerations.** Using `CREATE INDEX CONCURRENTLY` and `IF NOT EXISTS` are the right choices for production migrations.

5. **Complete index inventory.** The table listing all existing indexes on `domain_end_users` provides good context for decision-making.

6. **Identified the actual query locations.** Citing `domain_auth.rs:1285` and `domain_auth.rs:1343` shows the query pattern was verified in code.

---

## What's Missing or Unclear

### 1. No EXPLAIN ANALYZE output included

The plan repeatedly recommends running EXPLAIN ANALYZE but doesn't include actual results. The verification step should have been done **before** writing the plan, not left as a recommendation. Without this data, we're planning in the dark.

**Action needed:** Run the EXPLAIN ANALYZE query and include results in the plan or an appendix.

### 2. Migration file numbering needs verification

The plan suggests `00011_google_id_btree_index.sql`. But what's the current highest migration number? If migrations have advanced beyond 00010, this might conflict or be out of sequence.

**Action needed:** Check `ls apps/api/migrations/` and confirm the next available migration number.

### 3. Missing production data context

The plan mentions "small tables may prefer seq scan" but doesn't discuss:
- Current production table size (approximate row count)
- Expected growth rate
- Whether this query is in a hot path (login flow) that justifies optimization

**Action needed:** If production data is accessible, include rough row counts. If not, note the assumption.

### 4. SQLx offline mode implications

The plan states "SQLx offline mode needs regeneration only if using new queries" but creating an index doesn't add new queries. The migration itself might need `./run db:prepare` if SQLx tracks migrations, but this should be clarified.

**Action needed:** Verify whether `./run db:prepare` is actually needed for index-only migrations.

### 5. No rollback strategy

What happens if the new index causes issues? The plan lacks:
- A rollback migration or procedure
- Verification that `DROP INDEX` is safe

**Suggested addition:**
```sql
-- Rollback: DROP INDEX IF EXISTS idx_domain_end_users_domain_google_id;
```

---

## Suggested Improvements

### 1. Add a "Prerequisite Verification" section

Before the implementation steps, add a concrete checklist:
```
## Prerequisite Verification (Complete Before Implementation)
- [ ] Ran EXPLAIN ANALYZE locally with test data
- [ ] Confirmed migration file number is correct (next available)
- [ ] Reviewed production table size (if accessible)
```

### 2. Consider closing the ticket without code changes

Given that:
- A proper composite unique partial index already exists
- PostgreSQL should use it for `google_id = $2` queries
- The login flow likely doesn't have enough scale to notice a difference

The most likely correct action is **close the ticket with documentation**. The plan should be more definitive about this recommendation rather than hedging with "if needed."

### 3. Rename the planned index more descriptively

`idx_domain_end_users_domain_google_id` is similar to the existing `idx_domain_end_users_google_id`. If you do add an index, make the distinction clearer:
```
idx_domain_end_users_domain_google_id_full  -- (non-partial)
```
or add a comment in the migration explaining the difference.

### 4. Add success criteria

How will you know the task is complete? Add measurable outcomes:
- "EXPLAIN ANALYZE shows Index Scan, not Seq Scan"
- "Query execution time < X ms"

---

## Risks and Concerns

### Risk 1: Unnecessary index bloat

Adding a redundant index when the partial index already works is pure overhead. The storage cost is small, but every INSERT/UPDATE now maintains two similar indexes. Given this is the login path, write performance matters.

**Recommendation:** Default to "no change" unless EXPLAIN ANALYZE shows a clear problem.

### Risk 2: False positive from local testing

A local database with 10 rows will always seq scan. Verifying index usage locally only proves the index *can* be used, not that it *will* be used in production with real data distribution.

**Recommendation:** If possible, test with a data volume closer to production (even synthetic data would help).

### Risk 3: Task originated from a misunderstanding

The ticket says "Created from code review finding #15 Missing index on google_id lookup." But the index exists! The real deliverable might be:
- Documenting that the index exists and is sufficient
- Adding a comment in the Rust code explaining index coverage
- Closing the original code review finding with an explanation

**Recommendation:** Revisit the original code review finding and clarify what was actually missing.

---

## Summary

The plan is well-researched and appropriately cautious. The main gap is **missing verification data**—the EXPLAIN ANALYZE should be run and included before deciding on implementation. The most likely outcome is "no code change needed" since the partial index already covers the query pattern.

**Next steps:**
1. Run EXPLAIN ANALYZE with realistic test data
2. Check migration numbering
3. Make a definitive recommendation (likely: close as "already covered")
4. Update the original code review finding

---

*Feedback ends.*
