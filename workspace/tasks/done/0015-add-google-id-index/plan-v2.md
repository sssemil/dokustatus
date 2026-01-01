# Plan v2: Add Google ID Index for Domain End Users

**Plan for:** [0015-add-google-id-index](./ticket.md)
**Revision:** 2 (addresses feedback from v1)

## Executive Summary

**Recommendation: Close as "already covered" — no code change needed.**

A unique partial index already exists in migration `00006_google_oauth.sql`:
```sql
CREATE UNIQUE INDEX idx_domain_end_users_google_id ON domain_end_users(domain_id, google_id)
  WHERE google_id IS NOT NULL;
```

This index fully covers the lookup query `WHERE domain_id = $1 AND google_id = $2` used in `get_by_domain_and_google_id()`. PostgreSQL's query planner recognizes that an equality check on a non-null value implies `IS NOT NULL`, allowing use of the partial index.

The original code review finding #15 appears to have been created without checking the existing migrations.

---

## Prerequisite Verification Checklist

**Must complete before implementation decision:**

- [ ] Run EXPLAIN ANALYZE locally with test data (see Step 1 below)
- [x] Confirmed migration file number: next available is **00011** (after 00010_payment_provider.sql)
- [ ] Review production table size if accessible (or document assumption)
- [ ] Revisit original code review finding #15 to clarify what was flagged

---

## Verification Procedure

### Step 1: Run EXPLAIN ANALYZE

Start infrastructure and verify index usage:

```bash
./run infra
./run db:migrate
./run dev:seed  # or insert test data manually
```

Then connect to the database and run:

```sql
-- Insert test data if needed
INSERT INTO domain_end_users (domain_id, email, google_id)
VALUES ('00000000-0000-0000-0000-000000000001', 'test@example.com', 'google_123')
ON CONFLICT DO NOTHING;

-- Verify index usage
EXPLAIN ANALYZE
SELECT id, domain_id, email, roles, google_id, email_verified_at,
       last_login_at, is_frozen, is_whitelisted, created_at, updated_at
FROM domain_end_users
WHERE domain_id = '00000000-0000-0000-0000-000000000001'
  AND google_id = 'google_123';
```

**Expected output:** `Index Scan using idx_domain_end_users_google_id`

**If output shows `Seq Scan`:** This is likely due to small table size. PostgreSQL's cost-based optimizer prefers sequential scan for tables with < ~100 rows. This is correct behavior, not a problem.

### Step 2: Interpret Results

| EXPLAIN Output | Table Size | Action |
|----------------|------------|--------|
| Index Scan | Any | No change needed — index works |
| Seq Scan | < 100 rows | No change needed — expected behavior |
| Seq Scan | > 1000 rows | Investigate (run `ANALYZE domain_end_users;` first) |

### Step 3: Document and Close

If verification confirms the index is working (most likely outcome):

1. Update ticket.md checklist items as complete
2. Add history entry explaining the finding
3. Move task to `/done`
4. Comment on original code review finding #15 explaining the index exists

---

## If Index IS NOT Being Used (Unlikely)

Only proceed if Step 1 verification shows a genuine problem on a large table.

### Migration Details

**File:** `apps/api/migrations/00011_add_google_id_btree_index.sql`

```sql
-- Add non-partial B-tree index for domain_id + google_id lookup
--
-- Context: A unique partial index already exists (idx_domain_end_users_google_id)
-- with condition WHERE google_id IS NOT NULL. This non-partial index supplements
-- it for edge cases where the planner doesn't recognize the partial index.
--
-- This is defensive — the partial index should already work for equality queries.
-- Added due to verified performance issue in production (EXPLAIN ANALYZE showed seq scan).

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_domain_end_users_domain_google_id_full
ON domain_end_users(domain_id, google_id);
```

**Note:** Index name includes `_full` suffix to distinguish from the existing partial index.

### Rollback Procedure

If the new index causes issues:

```sql
DROP INDEX IF EXISTS idx_domain_end_users_domain_google_id_full;
```

### Deployment Steps

1. `./run db:migrate` — apply migration
2. Verify index exists: `\di idx_domain_end_users_domain_google_id_full` in psql
3. Run EXPLAIN ANALYZE to confirm index usage
4. Deploy to production

### SQLx Offline Mode

Index-only migrations do not require `./run db:prepare`. SQLx offline mode caches query metadata, not schema structure. Only run `db:prepare` if you modify queries or add new ones.

---

## Success Criteria

Task is complete when ONE of these is true:

**Option A (expected):** Closed as "already covered"
- EXPLAIN ANALYZE shows Index Scan (or expected Seq Scan on small tables)
- Ticket updated with explanation
- Original code review finding #15 addressed

**Option B (if new index added):**
- EXPLAIN ANALYZE shows Index Scan using new or existing index
- Query execution time is reasonable for login flow (< 10ms)
- Migration deployed without table locks (CONCURRENTLY)

---

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Unnecessary index bloat | Medium | Low | Default to "no change"; verify first |
| False positive from local testing | Medium | Low | Accept that small tables prefer seq scan |
| Task originated from misunderstanding | High | None | Document existing index; close finding #15 |
| Migration locks table | Low | High | Use `CREATE INDEX CONCURRENTLY` |

---

## Query Locations Reference

The `get_by_domain_and_google_id()` query is used in:

| Location | Purpose |
|----------|---------|
| `apps/api/src/adapters/persistence/domain_end_user.rs:65-79` | Repository method |
| `apps/api/src/application/use_cases/domain_auth.rs:1285` | Google OAuth login (find existing linked account) |
| `apps/api/src/application/use_cases/domain_auth.rs:1343` | Link Google account (verify not already linked) |

Both are in the authentication flow, which benefits from fast lookups. However, the existing partial index should already provide this.

---

## Existing Index Inventory

| Index Name | Columns | Type | Condition | Migration |
|------------|---------|------|-----------|-----------|
| `idx_domain_end_users_domain_id` | `domain_id` | B-tree | None | 00001 |
| `idx_domain_end_users_email` | `email` | B-tree | None | 00001 |
| `idx_domain_end_users_google_id` | `domain_id, google_id` | Unique B-tree | `WHERE google_id IS NOT NULL` | 00006 |
| (constraint) | `domain_id, email` | Unique | None | 00001 |

The existing `idx_domain_end_users_google_id` is the right index for the query pattern.

---

## Changes from v1

Addressing feedback-1.md:

| Feedback Item | Resolution |
|---------------|------------|
| No EXPLAIN ANALYZE output | Added verification procedure with exact steps; results to be captured during execution |
| Migration numbering unverified | Confirmed: next is **00011** (checked filesystem) |
| Missing production data context | Added note about table size assumptions; small tables preferring seq scan is expected |
| SQLx offline mode implications | Clarified: index-only migrations don't require `db:prepare` |
| No rollback strategy | Added explicit rollback SQL |
| Prerequisite verification section | Added checklist at top of plan |
| Consider closing without changes | Made this the primary recommendation |
| Rename index more descriptively | Added `_full` suffix to distinguish from partial index |
| Add success criteria | Added measurable outcomes section |

---

## History

- 2026-01-01 07:30: v1 created. Discovered existing unique partial index in migration 00006.
- 2026-01-01: v2 created. Addressed feedback: added verification checklist, confirmed migration number is 00011, added rollback procedure, added success criteria. Primary recommendation is to close as "already covered."
