# Feedback on Plan v2: Add Google ID Index

**Reviewer:** Claude (Opus 4.5)
**Date:** 2026-01-01
**Plan Version:** v2

---

## What's Good About the Plan

### 1. Addresses all v1 feedback systematically

The plan directly responds to each item from feedback-1.md with a clear resolution table. This shows good iteration discipline.

### 2. Clear primary recommendation

Moving "close as already covered" to the executive summary is the right call. The plan now leads with the most likely outcome rather than burying it in scenario analysis.

### 3. Excellent verification procedure

Step 1-3 provide copy-paste SQL commands, expected output interpretation, and a decision matrix based on results. This is executable documentation.

### 4. Proper handling of the "seq scan on small tables" case

The explanation that PostgreSQL's cost-based optimizer prefers seq scan for < ~100 rows is accurate and prevents false alarms during local testing.

### 5. Rollback procedure added

The explicit `DROP INDEX` command for rollback addresses a key gap from v1.

### 6. Success criteria are measurable

"EXPLAIN ANALYZE shows Index Scan" and "< 10ms execution time" are concrete, testable outcomes.

### 7. Migration number verified

Confirming 00011 is the next available prevents numbering conflicts.

### 8. Query location reference is helpful

The table showing where `get_by_domain_and_google_id()` is called (lines 1285 and 1343 in domain_auth.rs) provides context for understanding impact.

---

## What's Missing or Unclear

### 1. EXPLAIN ANALYZE still not run

Despite being the gating step, the prerequisite checklist still shows:
```
- [ ] Run EXPLAIN ANALYZE locally with test data (see Step 1 below)
```

This is the critical verification. The plan should ideally include actual output, or at minimum the plan author should run it before asking for review. Without this, we're still planning based on theory.

**Action needed:** Run the verification procedure before implementation, or add a note that this is intentionally deferred to execution phase.

### 2. Original code review finding #15 not investigated

The checklist includes:
```
- [ ] Revisit original code review finding #15 to clarify what was flagged
```

Understanding the original finding matters because:
- The finding author may have had additional context
- There might be a different query pattern not covered by the existing index
- Closing the finding requires accurate explanation

**Action needed:** Link to or summarize the original finding, or note that it's inaccessible.

### 3. INSERT statement in verification may fail

The test data insertion:
```sql
INSERT INTO domain_end_users (domain_id, email, google_id)
VALUES ('00000000-0000-0000-0000-000000000001', 'test@example.com', 'google_123')
ON CONFLICT DO NOTHING;
```

May fail if:
- The domain_id doesn't exist in the `domains` table (foreign key constraint)
- Required fields like `roles` are NOT NULL without defaults

**Suggested fix:** Either use a known domain_id from seed data, or add a note to check schema requirements first.

### 4. Missing step: verify index exists before testing

The verification procedure assumes the index exists but doesn't include a check. Add:
```sql
\di idx_domain_end_users_google_id
-- or
SELECT indexname FROM pg_indexes WHERE tablename = 'domain_end_users';
```

This confirms the index from migration 00006 was applied correctly.

### 5. "If Index IS NOT Being Used" section is speculative

The contingency plan for adding a new index is well-written, but the comment in the migration says:
```sql
-- Added due to verified performance issue in production (EXPLAIN ANALYZE showed seq scan).
```

This statement would be false if you add the migration preemptively. Either:
- Don't add the migration unless production evidence exists
- Revise the comment to reflect actual motivation

---

## Suggested Improvements

### 1. Add an "Execution Checklist" section

Separate planning from execution with a concrete checklist:
```
## Execution Checklist (for implementer)
- [ ] Start infra: ./run infra && ./run db:migrate && ./run dev:seed
- [ ] Verify index exists: \di idx_domain_end_users_google_id
- [ ] Run EXPLAIN ANALYZE query from Step 1
- [ ] Capture output and paste in ticket history
- [ ] If Index Scan → proceed to close as "already covered"
- [ ] If Seq Scan on large table → investigate further before adding new index
```

### 2. Simplify the decision: close now, reopen if needed

The plan is thorough but could be simpler. Given:
- Index definitely exists (verified in migration 00006)
- PostgreSQL definitely uses partial indexes for equality predicates
- Table size is likely small (new feature, limited production traffic)

The pragmatic path is: **close the ticket with documentation, reopen if a production issue arises.**

### 3. Add a "Post-Close Monitoring" note

If closing without changes:
```
Monitor production for slow queries on the Google OAuth login path.
If P95 latency exceeds 100ms on login, revisit this ticket.
```

This provides a safety net without over-engineering.

---

## Risks and Concerns

### Risk 1: Paralysis by analysis (Low likelihood, Low impact)

The plan is comprehensive but risks delaying a simple decision. The existing index is correct; running EXPLAIN ANALYZE locally will almost certainly confirm it. Don't let verification perfectionism block completion.

**Mitigation:** Set a time-box (30 minutes) for verification. If it works, close. If ambiguous, close with a monitoring note.

### Risk 2: Incomplete closure of finding #15 (Medium likelihood, Low impact)

If the original code review finding isn't properly addressed, it may resurface or cause confusion. Future developers might see "Missing index on google_id" and not know it was resolved.

**Mitigation:** Add a comment to the original finding linking to this task's resolution.

### Risk 3: Test data insertion failure (Medium likelihood, Low impact)

The INSERT in the verification procedure may fail due to FK constraints or NOT NULL columns, causing confusion during execution.

**Mitigation:** Update the verification procedure to use data from `./run dev:seed` or provide a complete INSERT with all required fields.

---

## Summary

Plan v2 is a significant improvement over v1. It's well-structured, addresses previous feedback, and provides clear execution steps. The main remaining gap is that **verification hasn't been done yet**—the EXPLAIN ANALYZE output should be captured before finalizing the decision.

**Recommendation:** Execute the verification procedure now. If it confirms index usage (likely), close the task immediately. The plan is ready for execution.

**Confidence level:** High that this will close as "already covered" with no code changes.

---

*Feedback ends.*
