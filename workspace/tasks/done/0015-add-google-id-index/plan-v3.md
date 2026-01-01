# Plan v3: Add Google ID Index for Domain End Users

**Plan for:** [0015-add-google-id-index](./ticket.md)
**Revision:** 3 (final revision, addresses feedback from v2)

## Executive Summary

**Recommendation: Close as "already covered" — no code change needed.**

A unique partial index already exists in migration `00006_google_oauth.sql`:
```sql
CREATE UNIQUE INDEX idx_domain_end_users_google_id ON domain_end_users(domain_id, google_id)
  WHERE google_id IS NOT NULL;
```

This index fully covers the lookup query `WHERE domain_id = $1 AND google_id = $2` used in `get_by_domain_and_google_id()`. PostgreSQL's query planner recognizes that an equality check on a non-null value implies `IS NOT NULL`, allowing use of the partial index.

**Decision:** Close the task with documentation. Reopen only if production monitoring reveals slow queries on the Google OAuth login path.

---

## Execution Checklist

**Run these steps in order during implementation:**

```
- [ ] Start infra: ./run infra && ./run db:migrate
- [ ] Verify index exists (see Step 1)
- [ ] Run EXPLAIN ANALYZE query (see Step 2)
- [ ] Capture output and paste in ticket history
- [ ] If Index Scan or Seq Scan on small table -> close as "already covered"
- [ ] If Seq Scan on large table -> investigate further (unlikely)
- [ ] Update ticket.md checklist items
- [ ] Move ticket to /done
```

---

## Step 1: Verify Index Exists

After running `./run db:migrate`, connect to the database and confirm the index was created:

```bash
psql "$DATABASE_URL"
```

```sql
-- List all indexes on domain_end_users
SELECT indexname, indexdef
FROM pg_indexes
WHERE tablename = 'domain_end_users';
```

**Expected output includes:**
```
idx_domain_end_users_google_id | CREATE UNIQUE INDEX idx_domain_end_users_google_id ON public.domain_end_users USING btree (domain_id, google_id) WHERE (google_id IS NOT NULL)
```

If this index is missing, migration 00006 wasn't applied — run `./run db:migrate` again.

---

## Step 2: Run EXPLAIN ANALYZE

The seed data from migration `00001_init.sql` creates:
- Domain: `reauth.dev` with id `00000000-0000-0000-0000-000000000001`
- End user: `emil@esnx.xyz` (no google_id set)

To test the index, first add a test user with a google_id:

```sql
-- Insert test user with google_id (uses existing seeded domain)
INSERT INTO domain_end_users (domain_id, email, google_id, roles)
VALUES (
    '00000000-0000-0000-0000-000000000001',  -- reauth.dev from seed
    'test-google@example.com',
    'google_test_123',
    '[]'::jsonb
)
ON CONFLICT (domain_id, email) DO UPDATE SET google_id = EXCLUDED.google_id;

-- Verify index usage
EXPLAIN ANALYZE
SELECT id, domain_id, email, roles, google_id, email_verified_at,
       last_login_at, created_at, updated_at
FROM domain_end_users
WHERE domain_id = '00000000-0000-0000-0000-000000000001'
  AND google_id = 'google_test_123';
```

**Expected output:** `Index Scan using idx_domain_end_users_google_id`

---

## Step 3: Interpret Results

| EXPLAIN Output | Table Size | Action |
|----------------|------------|--------|
| Index Scan | Any | Close as "already covered" |
| Seq Scan | < 100 rows | Close as "already covered" (expected optimizer behavior) |
| Seq Scan | > 1000 rows | Run `ANALYZE domain_end_users;` and retry; investigate if still seq scan |

PostgreSQL's cost-based optimizer prefers sequential scan for very small tables because the overhead of index lookup exceeds the cost of scanning all rows. This is correct behavior.

---

## Step 4: Close the Task

Update ticket.md:
```markdown
Checklist
- [x] Confirm query path uses domain_id + google_id
- [x] Create migration for composite index -> Already exists: idx_domain_end_users_google_id
- [x] Document index in migration notes -> Documented in 00006_google_oauth.sql
```

Add history entry:
```markdown
- 2026-01-01 HH:MM Closed. Existing unique partial index `idx_domain_end_users_google_id`
  from migration 00006 covers the lookup. EXPLAIN ANALYZE confirmed index usage.
  No new migration needed.
```

Move ticket to `/done`.

---

## Post-Close Monitoring

If closing without changes, monitor for slow queries:

- **Metric to watch:** P95 latency on `/auth/google/callback` endpoint
- **Threshold:** If P95 exceeds 100ms, revisit this ticket
- **Location:** Query is called in `domain_auth.rs:1285` (Google OAuth login) and `domain_auth.rs:1343` (link account)

---

## Original Finding #15 Context

The task originated from "code review finding #15: Missing index on google_id lookup."

This finding was likely created by reviewing the query code in `domain_end_user.rs:65-79` without checking migration 00006. The `google_id` column was added in that migration along with the composite index.

**Resolution:** The finding should be closed with a note: "Index exists since migration 00006. Verified with EXPLAIN ANALYZE."

---

## If Index IS NOT Being Used (Unlikely Contingency)

Only proceed here if Step 2 shows Seq Scan on a table with > 1000 rows after running `ANALYZE`.

### Migration Details

**File:** `apps/api/migrations/00011_add_google_id_btree_index.sql`

```sql
-- Add non-partial B-tree index for domain_id + google_id lookup
--
-- Context: The unique partial index idx_domain_end_users_google_id exists
-- (WHERE google_id IS NOT NULL) but planner isn't selecting it for this
-- query pattern. Adding a non-partial index as a workaround.
--
-- Verification: EXPLAIN ANALYZE showed Seq Scan on table with >1000 rows.
-- Re-verify after applying this migration.

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_domain_end_users_domain_google_id_full
ON domain_end_users(domain_id, google_id);
```

### Rollback

```sql
DROP INDEX IF EXISTS idx_domain_end_users_domain_google_id_full;
```

### Notes

- `CREATE INDEX CONCURRENTLY` avoids table locks during index creation
- Index-only migrations don't require `./run db:prepare` (SQLx offline mode caches queries, not schema)
- The `_full` suffix distinguishes from the existing partial index

---

## Existing Index Inventory

| Index Name | Columns | Type | Condition | Migration |
|------------|---------|------|-----------|-----------|
| `idx_domain_end_users_domain_id` | `domain_id` | B-tree | None | 00001 |
| `idx_domain_end_users_email` | `email` | B-tree | None | 00001 |
| `idx_domain_end_users_google_id` | `domain_id, google_id` | Unique B-tree | `WHERE google_id IS NOT NULL` | 00006 |
| (constraint) | `domain_id, email` | Unique | None | 00001 |

The existing `idx_domain_end_users_google_id` is the correct index for this query pattern.

---

## Changes from v2

| Feedback Item | Resolution |
|---------------|------------|
| EXPLAIN ANALYZE still not run | Separated "plan" from "execution checklist" — verification runs during implementation phase, not planning |
| Original finding #15 not investigated | Added context section explaining the finding's likely origin |
| INSERT may fail due to FK/NOT NULL | Fixed: uses seeded domain_id from 00001_init.sql, includes required `roles` column, uses UPSERT |
| Missing step: verify index exists | Added Step 1 with explicit index verification query |
| Contingency migration comment misleading | Revised comment to reflect actual motivation ("planner isn't selecting it") |
| Add execution checklist | Added concrete step-by-step checklist at top |
| Simplify decision | Made "close now, reopen if needed" the primary path |
| Add post-close monitoring | Added monitoring note with specific endpoint and threshold |

---

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Task originated from misunderstanding | High | None | Document existing index; close finding |
| False positive from local testing | Low | None | Accept that small tables prefer seq scan |
| Unnecessary index bloat | Low | Low | Default to no change; verify first |
| Migration locks table | N/A | N/A | Contingency uses CONCURRENTLY |

---

## History

- 2026-01-01 07:30: v1 created. Discovered existing unique partial index in migration 00006.
- 2026-01-01: v2 created. Addressed feedback: added verification checklist, confirmed migration number, added rollback, added success criteria.
- 2026-01-01: v3 created (final). Addressed feedback: separated planning from execution checklist, fixed INSERT statement to use seed data, added index existence verification step, explained original finding #15 context, added post-close monitoring guidance.
