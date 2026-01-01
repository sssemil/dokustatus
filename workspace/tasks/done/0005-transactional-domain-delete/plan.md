# Plan v3: Transactional Domain Delete

## Summary

This plan addresses feedback-2 by adding explicit schema verification, clarifying the transaction boundary for future audit/outbox work, and expanding the verification query to cover all domain-related tables.

### Goal
Wrap domain deletion in an explicit transaction to:
1. Provide an atomic boundary for future extensibility (audit logging, outbox events)
2. Satisfy the task requirement for "transactional" deletion
3. Maintain current idempotent behavior (delete of non-existent domain already fails at use-case layer)

## Feedback-2 Analysis

| Feedback Item | Resolution |
|---------------|------------|
| No explicit validation of `ON DELETE CASCADE` | Added schema verification section below with exhaustive list |
| Check and delete not in same transaction | Documented as acceptable; clarified that future audit/outbox work would need `DELETE ... RETURNING` |
| Manual test SQL incomplete | Expanded to include all 11 domain-related tables |
| No note about isolation level or locking | Added brief note: using default `READ COMMITTED`, no explicit locking needed |
| Confirm no SQLx offline change needed | Confirmed: using `sqlx::query()` (unchecked), not `sqlx::query!()` |

## Schema Verification: Tables with `domain_id` Foreign Keys

All tables below have `ON DELETE CASCADE` on their `domain_id` column. Verified from migrations:

| Table | Migration | FK Definition |
|-------|-----------|---------------|
| `domain_end_users` | 00001_init.sql:39 | `domain_id UUID NOT NULL REFERENCES domains(id) ON DELETE CASCADE` |
| `domain_auth_config` | 00001_init.sql:62 | `domain_id UUID NOT NULL UNIQUE REFERENCES domains(id) ON DELETE CASCADE` |
| `domain_auth_magic_link` | 00001_init.sql:75 | `domain_id UUID NOT NULL UNIQUE REFERENCES domains(id) ON DELETE CASCADE` |
| `domain_api_keys` | 00003_api_keys.sql:4 | `domain_id UUID NOT NULL REFERENCES domains(id) ON DELETE CASCADE` |
| `domain_roles` | 00005_domain_roles.sql:4 | `domain_id UUID NOT NULL REFERENCES domains(id) ON DELETE CASCADE` |
| `domain_auth_google_oauth` | 00006_google_oauth.sql:6 | `domain_id UUID NOT NULL UNIQUE REFERENCES domains(id) ON DELETE CASCADE` |
| `domain_billing_stripe_config` | 00007_billing.sql:9 | `domain_id UUID NOT NULL UNIQUE REFERENCES domains(id) ON DELETE CASCADE` |
| `subscription_plans` | 00007_billing.sql:36 | `domain_id UUID NOT NULL REFERENCES domains(id) ON DELETE CASCADE` |
| `user_subscriptions` | 00007_billing.sql:107 | `domain_id UUID NOT NULL REFERENCES domains(id) ON DELETE CASCADE` |
| `billing_payments` | 00009_billing_payments.sql:21 | `domain_id UUID NOT NULL REFERENCES domains(id) ON DELETE CASCADE` |
| `domain_enabled_payment_providers` | 00010_payment_provider.sql:35 | `domain_id UUID NOT NULL REFERENCES domains(id) ON DELETE CASCADE` |

**Cascade chain**: Some tables also cascade further:
- `user_subscriptions` → `subscription_events` (via `subscription_id ON DELETE CASCADE`)
- `domain_end_users` → `billing_payments`, `user_subscriptions` (via `end_user_id ON DELETE CASCADE`)

All cascades originate from `domains`, so a single `DELETE FROM domains WHERE id = $1` removes all related data.

## Call Chain Analysis

```
DELETE /api/domains/{domain_id}
    │
    ▼
apps/api/src/adapters/http/routes/domain.rs:376 delete_domain()
    │ calls
    ▼
apps/api/src/application/use_cases/domain.rs:158 DomainUseCases::delete_domain()
    │ 1. calls get_domain() → returns NotFound if domain doesn't exist
    │ 2. checks owner_end_user_id is Some (blocks system domain deletion)
    │ 3. calls repo.delete()
    ▼
apps/api/src/adapters/persistence/domain.rs:155 impl DomainRepo::delete()
    │ executes DELETE FROM domains WHERE id = $1
    ▼
PostgreSQL cascades to all 11 related tables
```

**Key insight**: The use-case layer already validates existence and ownership. The repo layer performs a "fire and forget" delete that succeeds even if 0 rows are affected. This is the desired idempotent behavior and will be preserved.

## Current vs Proposed Behavior

| Scenario | Current Behavior | Proposed Behavior |
|----------|------------------|-------------------|
| Domain exists | Delete succeeds, cascades, returns 204 | Same (wrapped in transaction) |
| Domain doesn't exist | Use-case returns NotFound (404) | Same (no change) |
| Domain is system domain | Use-case returns InvalidInput (400) | Same (no change) |
| Concurrent deletion | First delete wins, second returns 404 | Same (no change) |
| Delete with many related records | Cascades atomically | Same (already atomic) |

## Implementation

### Minimal Change (Recommended)

**File:** `apps/api/src/adapters/persistence/domain.rs` lines 155-162

Replace:
```rust
async fn delete(&self, domain_id: Uuid) -> AppResult<()> {
    sqlx::query("DELETE FROM domains WHERE id = $1")
        .bind(domain_id)
        .execute(&self.pool)
        .await
        .map_err(AppError::from)?;
    Ok(())
}
```

With:
```rust
async fn delete(&self, domain_id: Uuid) -> AppResult<()> {
    let mut tx = self.pool.begin().await.map_err(AppError::from)?;

    sqlx::query("DELETE FROM domains WHERE id = $1")
        .bind(domain_id)
        .execute(&mut *tx)
        .await
        .map_err(AppError::from)?;

    tx.commit().await.map_err(AppError::from)?;
    Ok(())
}
```

### Why Not Use `DELETE ... RETURNING`?

The v1 plan suggested `DELETE ... RETURNING id` to check if deletion occurred. However:
1. The use-case layer already checks existence before calling delete
2. Changing to return `NotFound` on 0 rows would break the current idempotent semantics
3. Adding `RETURNING` provides no benefit since we don't use the returned value

**Future consideration**: If audit logging or outbox events are added, using `DELETE ... RETURNING *` could make the existence check + delete atomic and provide data for the audit record. This would require changing error semantics (repo returns NotFound on 0 rows). Documented here as the recommended approach for that future work.

### Why Not Add Explicit Deletes?

1. All 11 `domain_id` foreign keys have `ON DELETE CASCADE` (verified above)
2. Explicit deletes are redundant and create maintenance burden
3. PostgreSQL handles cascade ordering internally

### Why Keep the Transaction Wrapper?

While a single `DELETE` statement with CASCADE is already atomic, the transaction wrapper:
1. Satisfies the explicit task requirement for "transactional" deletion
2. Provides a future-proof structure for adding audit logging or outbox events
3. Has negligible performance overhead (single statement within transaction)

### Transaction Isolation & Locking

- Using PostgreSQL default isolation level (`READ COMMITTED`)
- No explicit `SELECT ... FOR UPDATE` needed because:
  - The use-case layer already validates existence/ownership for user feedback
  - The delete is idempotent (0 rows affected is not an error at repo level)
  - Row-level locks would add overhead without benefit for current semantics

## Files to Modify

1. `apps/api/src/adapters/persistence/domain.rs` (lines 155-162) - Wrap delete in transaction

## Verification Steps

### Pre-Implementation
```bash
./run api:build  # Verify current build succeeds
```

### Post-Implementation
```bash
./run api:fmt    # Format code
./run api:lint   # Check lints
./run api:build  # Verify build succeeds
./run api:test   # Run existing tests
```

### Manual Integration Test (Optional)

1. Start local infra: `./run infra`
2. Seed data: `./run dev:seed`
3. Create a test domain via UI or API
4. Add related data (auth config, end users, subscriptions, etc.)
5. Delete the domain
6. Verify all related data is removed using the comprehensive query below

### Comprehensive Verification Query

This query checks all 11 domain-related tables to confirm cascade worked:

```sql
-- Replace <domain_id> with the deleted domain's UUID
SELECT
    (SELECT COUNT(*) FROM domains WHERE id = '<domain_id>') as domains,
    (SELECT COUNT(*) FROM domain_end_users WHERE domain_id = '<domain_id>') as domain_end_users,
    (SELECT COUNT(*) FROM domain_auth_config WHERE domain_id = '<domain_id>') as domain_auth_config,
    (SELECT COUNT(*) FROM domain_auth_magic_link WHERE domain_id = '<domain_id>') as domain_auth_magic_link,
    (SELECT COUNT(*) FROM domain_api_keys WHERE domain_id = '<domain_id>') as domain_api_keys,
    (SELECT COUNT(*) FROM domain_roles WHERE domain_id = '<domain_id>') as domain_roles,
    (SELECT COUNT(*) FROM domain_auth_google_oauth WHERE domain_id = '<domain_id>') as domain_auth_google_oauth,
    (SELECT COUNT(*) FROM domain_billing_stripe_config WHERE domain_id = '<domain_id>') as domain_billing_stripe_config,
    (SELECT COUNT(*) FROM subscription_plans WHERE domain_id = '<domain_id>') as subscription_plans,
    (SELECT COUNT(*) FROM user_subscriptions WHERE domain_id = '<domain_id>') as user_subscriptions,
    (SELECT COUNT(*) FROM billing_payments WHERE domain_id = '<domain_id>') as billing_payments,
    (SELECT COUNT(*) FROM domain_enabled_payment_providers WHERE domain_id = '<domain_id>') as domain_enabled_payment_providers;
-- All counts should be 0
```

Alternative: Dynamic query to list all tables with `domain_id` (for future-proofing):

```sql
SELECT table_name
FROM information_schema.columns
WHERE column_name = 'domain_id'
  AND table_schema = 'public';
```

### Schema Drift Check

Before implementation, verify no new `domain_id` tables have been added since this plan was written:

```bash
rg "domain_id.*REFERENCES" apps/api/migrations --no-heading
```

Compare output against the 11 tables listed in Schema Verification section. If new tables exist, verify they have `ON DELETE CASCADE` and update the verification query.

## Risk Assessment

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Transaction overhead | Low | Negligible for single statement |
| Breaking existing behavior | Very Low | No semantic changes; use-case layer unchanged |
| Missing new tables in future | Low | Relying on CASCADE; add note to migration template |
| Check/delete race condition | Very Low | See analysis below |
| New table missing CASCADE | Low | Documented schema verification process for PRs with new `domain_id` tables |

### Race Condition Analysis

The existence check (`get_domain()`) happens outside the transaction, leaving a theoretical race where another request could delete the domain between check and delete.

**Explicit acknowledgment**: If the domain is deleted between check and delete, the `DELETE` affects 0 rows and we still return `Ok(())`. This is intentional idempotent behavior—the repo layer does not check `rows_affected()`.

**Assessment**: This is an acceptable tradeoff because:
1. The race window is extremely short (milliseconds)
2. The outcome is benign: delete of 0 rows succeeds silently (idempotent)
3. The use-case layer checks ownership/existence for user feedback purposes
4. Moving the check into the transaction would require changing `DomainRepo` trait and all implementations

**Future path**: If stricter atomicity is needed (e.g., for audit logging), use:
```rust
let deleted = sqlx::query_as::<_, (Uuid,)>("DELETE FROM domains WHERE id = $1 RETURNING id")
    .bind(domain_id)
    .fetch_optional(&mut *tx)
    .await?;

if deleted.is_none() {
    return Err(AppError::NotFound("domain".into()));
}
```
This makes check+delete atomic but changes error semantics. Recommended for audit/outbox work.

### Transaction Boundary Clarification

The current design wraps only the DELETE in a transaction. The existence/ownership check happens before:

```
┌─────────────────────────────────────────────────────────────┐
│  Use-case layer (outside transaction)                       │
│  1. get_domain() → NotFound if missing                      │
│  2. check owner_end_user_id is Some → InvalidInput if null  │
└─────────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────┐
│  Repo adapter (transaction boundary)                         │
│  3. DELETE FROM domains WHERE id = $1                        │
│  4. (Future: INSERT INTO audit_log / outbox)                 │
└─────────────────────────────────────────────────────────────┘
```

**Trade-off**: The transaction boundary is in the repo adapter, not the use-case layer. This works for the current single-DELETE use case. However, if future audit/outbox work requires cross-repo writes (e.g., writing to an audit repo), the transaction would need to be lifted to the use-case layer or a `delete_in_tx` variant added. For now, keeping it in the repo is simpler and matches the current architecture.

For future audit/outbox work, the check should move inside the transaction boundary and use `DELETE ... RETURNING` to atomically verify existence and capture data for the audit record.

## Out of Scope

These items were mentioned in v1 but are explicitly out of scope:
- Audit logging for domain deletion (future task)
- Soft-delete / recovery (future task)
- Deletion webhooks / events (future task)
- Adding `#[instrument]` to persistence layer (not consistent with current patterns)
- Regression test for delete semantics (consider for future hardening)

## Confirmation Checklist

- [x] All `domain_id` FKs have `ON DELETE CASCADE` (verified in Schema Verification section)
- [x] No SQLx offline regeneration needed (using `sqlx::query()`, not `sqlx::query!()`)
- [x] No tracing changes planned (persistence layer has no `#[instrument]` macros)
- [x] Transaction boundary documented for future audit/outbox work
- [x] Verification query covers all 11 domain-related tables

## Estimated Changes

- ~8 lines modified in `apps/api/src/adapters/persistence/domain.rs`
- No new files
- No SQLx offline regeneration needed
- No migration needed

---

## Codex Review (2026-01-01)

**Reviewer**: gpt-5.2-codex

**Feedback received**:
1. Transaction boundary in repo adapter won't help if future audit/outbox needs cross-repo writes → Added trade-off note in Transaction Boundary Clarification section
2. Race condition: DELETE affects 0 rows and returns Ok(()) if domain deleted between check and delete → Added explicit acknowledgment in Race Condition Analysis
3. Schema verification looks thorough; future drift is the only gap → Added Schema Drift Check step with `rg` command
4. Risk assessment accurate if idempotency and race are acceptable → Confirmed acceptable

**Resolutions applied**:
- Added "Trade-off" paragraph explaining when transaction would need to move to use-case layer
- Added "Explicit acknowledgment" paragraph clarifying intentional idempotent behavior
- Added "Schema Drift Check" section with verification command
- Confirmed all feedback addressed; plan is ready for review
