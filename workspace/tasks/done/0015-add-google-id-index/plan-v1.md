# Plan v1: Add Google ID Index for Domain End Users

**Plan for:** [0015-add-google-id-index](./ticket.md)

## Summary

The task requests adding a composite index on `(domain_id, google_id)` to speed up lookups. Upon investigation, **a unique partial index already exists** in migration `00006_google_oauth.sql`:

```sql
CREATE UNIQUE INDEX idx_domain_end_users_google_id ON domain_end_users(domain_id, google_id)
  WHERE google_id IS NOT NULL;
```

This index should already serve the lookup query in `get_by_domain_and_google_id()`:
```sql
SELECT ... FROM domain_end_users WHERE domain_id = $1 AND google_id = $2
```

PostgreSQL's query planner recognizes that `google_id = $2` (a specific non-null value) implies `google_id IS NOT NULL`, so it can use the partial index.

**Key Question:** Is the existing index sufficient, or is there a specific scenario where it fails to be used?

## Current State Analysis

### Existing Indexes on `domain_end_users`

| Index Name | Columns | Type | Condition | Migration |
|------------|---------|------|-----------|-----------|
| `idx_domain_end_users_domain_id` | `domain_id` | Regular | None | 00001 |
| `idx_domain_end_users_email` | `email` | Regular | None | 00001 |
| `idx_domain_end_users_google_id` | `domain_id, google_id` | Unique | `WHERE google_id IS NOT NULL` | 00006 |
| (implicit) | `domain_id, email` | Unique constraint | None | 00001 |

### Query Pattern Using Google ID

**File:** `apps/api/src/adapters/persistence/domain_end_user.rs:65-79`

```rust
async fn get_by_domain_and_google_id(
    &self,
    domain_id: Uuid,
    google_id: &str,
) -> AppResult<Option<DomainEndUserProfile>> {
    let row = sqlx::query(
        "SELECT id, domain_id, email, roles, google_id, email_verified_at, last_login_at, is_frozen, is_whitelisted, created_at, updated_at FROM domain_end_users WHERE domain_id = $1 AND google_id = $2",
    )
    .bind(domain_id)
    .bind(google_id)
    .fetch_optional(&self.pool)
    .await
    .map_err(AppError::from)?;
    Ok(row.map(row_to_profile))
}
```

This query is used in:
1. `domain_auth.rs:1285` - Google OAuth login (find existing linked account)
2. `domain_auth.rs:1343` - Link Google account (verify not already linked to another user)

### Why the Existing Partial Index Should Work

When PostgreSQL sees `WHERE domain_id = $1 AND google_id = $2`:
- The `google_id = $2` predicate (equality to a non-null value) is logically equivalent to `google_id = $2 AND google_id IS NOT NULL`
- PostgreSQL recognizes this and can use the partial index `idx_domain_end_users_google_id`

This is documented PostgreSQL behavior: partial indexes are usable when the query conditions imply the index predicate.

## Decision Point

There are two possible scenarios:

### Scenario A: The Existing Index Is Sufficient (Most Likely)

If the existing unique partial index is being used correctly, no additional index is needed. The task may have been created based on a misunderstanding that no index exists.

**Action:** Close the task with documentation explaining the existing index is sufficient.

### Scenario B: The Partial Index Is Not Being Used

If there's evidence (EXPLAIN ANALYZE output) that PostgreSQL is doing a sequential scan instead of using the partial index, we may need to investigate why. Possible reasons:
- Query planner statistics are stale
- Table is small enough that planner prefers seq scan
- Some edge case with partial index usage

**Action:** Add a non-partial index if truly needed after verification.

## Recommended Approach

### Step 1: Verify Index Usage (Manual Investigation)

Before making any changes, verify whether the existing index is being used:

```sql
-- Connect to local database
EXPLAIN ANALYZE
SELECT id, domain_id, email, roles, google_id, email_verified_at, last_login_at, is_frozen, is_whitelisted, created_at, updated_at
FROM domain_end_users
WHERE domain_id = '00000000-0000-0000-0000-000000000001' AND google_id = 'test_google_id';
```

Expected output should show: `Index Scan using idx_domain_end_users_google_id`

If it shows `Seq Scan`, investigate further:
1. Check table row count (small tables may prefer seq scan)
2. Run `ANALYZE domain_end_users` to update statistics
3. Check if `enable_indexscan` is enabled

### Step 2: If Index Is NOT Being Used

If verification shows the partial index is not being used and the table is large enough to benefit from an index:

**Option A: No change needed** - The partial unique index already covers the lookup query. The query planner's decision to use or not use it is based on cost estimation.

**Option B: Add a covering non-partial index** - Only if there's a specific need to force index usage:

```sql
-- Migration: 00011_google_id_btree_index.sql
-- Add B-tree index for google_id lookup performance
-- Note: This supplements the existing unique partial index

CREATE INDEX IF NOT EXISTS idx_domain_end_users_domain_google_id
ON domain_end_users(domain_id, google_id);
```

However, adding a duplicate index (non-partial version of an existing partial index) has downsides:
- Extra storage space
- Extra write overhead on INSERT/UPDATE
- May confuse the planner

### Step 3: Document Decision

Update the task with the investigation results and decision.

## Implementation Plan (If New Index Is Needed)

Only proceed if Step 1 verification confirms the existing index is NOT being used on a sufficiently large table.

### Files to Modify

| File | Changes |
|------|---------|
| `apps/api/migrations/00011_google_id_btree_index.sql` | New migration file |

### Migration Content

```sql
-- Add B-tree index for domain_id + google_id lookup optimization
--
-- Context: A unique partial index already exists (idx_domain_end_users_google_id),
-- but this non-partial index may help query planner in certain scenarios.
--
-- Note: The existing partial unique index still enforces uniqueness for non-null google_id.
-- This index is purely for lookup performance.

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_domain_end_users_domain_google_id
ON domain_end_users(domain_id, google_id);
```

**Note:** Use `CONCURRENTLY` to avoid locking the table during index creation in production.

### Deployment Steps

1. Run migration: `./run db:migrate`
2. Update SQLx offline data: `./run db:prepare`
3. Verify index exists: `\d domain_end_users` in psql
4. Verify index is used: Run EXPLAIN ANALYZE on the lookup query
5. Deploy to production

## Testing Approach

1. **Local verification:**
   - Run `./run infra` to start local database
   - Run `./run db:migrate` to apply migration
   - Run EXPLAIN ANALYZE to verify index usage
   - Run `./run api:build` to verify no build issues

2. **No code changes needed:**
   - This is a database-only change (migration)
   - No Rust code modifications required
   - SQLx offline mode needs regeneration only if using new queries

## Edge Cases

1. **Empty table:** Query planner may prefer seq scan on tiny tables (< 100 rows). This is acceptable behavior.

2. **NULL google_id:** The existing partial unique index excludes NULL values. The new non-partial index would include NULLs, allowing efficient lookup of all users without google_id (if needed in future).

3. **Migration on production:** Use `CONCURRENTLY` to prevent table locks.

4. **Index already exists:** Using `IF NOT EXISTS` makes the migration idempotent.

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| Duplicate index overhead | Verify existing index is insufficient before adding |
| Migration locking table | Use `CREATE INDEX CONCURRENTLY` |
| Unnecessary index | Verify with EXPLAIN ANALYZE first |

## Recommendation

**Before implementing any changes, verify the existing index behavior:**

1. Start local infra: `./run infra`
2. Seed some test data with google_id values
3. Run EXPLAIN ANALYZE on the lookup query
4. If index is used → close task as "no change needed"
5. If index is NOT used → investigate why, then decide on action

The existing unique partial index on `(domain_id, google_id) WHERE google_id IS NOT NULL` should already optimize the lookup query. Adding a duplicate non-partial index is likely unnecessary.

## History

- 2026-01-01: Initial plan (v1) created after codebase analysis. Discovered existing unique partial index in migration 00006. Recommending verification before any changes.
